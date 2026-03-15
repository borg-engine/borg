use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use borg_core::db::Db;
use serde::Deserialize;
use serde_json::{json, Value};

use super::internal;
use crate::AppState;

const MS365_SCOPES: &str = "offline_access User.Read Mail.ReadWrite Mail.Send Calendars.ReadWrite Contacts.Read Files.ReadWrite.All Sites.Read.All Team.ReadBasic.All Channel.ReadBasic.All Chat.Read";

#[derive(Deserialize)]
pub(crate) struct Ms365CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((combined >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((combined >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((combined >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(combined & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn ms365_callback_url(config: &borg_core::config::Config) -> String {
    let base = config.get_base_url();
    format!("{base}/api/user/microsoft/callback")
}

pub(crate) async fn ms365_auth_init(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
) -> Result<axum::response::Response, StatusCode> {
    let public_url = state
        .db
        .get_config("public_url")
        .map_err(internal)?
        .unwrap_or_default();
    if public_url.trim().is_empty() {
        return Ok(
            axum::response::Redirect::temporary("/#/?ms365_error=missing_public_url")
                .into_response(),
        );
    }

    let client_id = state
        .db
        .get_config("ms_client_id")
        .map_err(internal)?
        .unwrap_or_default();
    if client_id.trim().is_empty() {
        return Ok(
            axum::response::Redirect::temporary("/#/?ms365_error=missing_credentials")
                .into_response(),
        );
    }

    let state_json = json!({ "user_id": user.id }).to_string();
    let encoded_state = base64_encode(state_json.as_bytes());
    let redirect_uri = ms365_callback_url(&state.config);

    let auth_url = format!(
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?\
         client_id={client_id}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &state={encoded_state}\
         &prompt=consent",
        super::cloud::percent_encode(&redirect_uri),
        super::cloud::percent_encode(MS365_SCOPES),
    );

    Ok(axum::response::Redirect::temporary(&auth_url).into_response())
}

pub(crate) async fn ms365_auth_callback(
    State(state): State<Arc<AppState>>,
    Query(q): Query<Ms365CallbackQuery>,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(err) = q.error {
        tracing::warn!("ms365 OAuth error: {err}");
        return Ok(
            axum::response::Redirect::temporary("/#/?ms365_error=access_denied").into_response(),
        );
    }

    let code = q.code.ok_or(StatusCode::BAD_REQUEST)?;
    let state_raw = q.state.ok_or(StatusCode::BAD_REQUEST)?;
    let state_bytes =
        super::utils::base64_decode(&state_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let state_val: Value =
        serde_json::from_slice(&state_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let user_id = state_val["user_id"]
        .as_i64()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let client_id = state
        .db
        .get_config("ms_client_id")
        .map_err(internal)?
        .ok_or(StatusCode::BAD_REQUEST)?;
    let client_secret = state
        .db
        .get_config("ms_client_secret")
        .map_err(internal)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let redirect_uri = ms365_callback_url(&state.config);
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
    ];
    let resp = client
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await
        .map_err(internal)?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!("ms365 token exchange failed: {body}");
        return Ok(
            axum::response::Redirect::temporary("/#/?ms365_error=token_exchange").into_response(),
        );
    }

    let token_json: Value = resp.json().await.map_err(internal)?;
    let access_token = token_json["access_token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let refresh_token = token_json["refresh_token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let expires_in = token_json["expires_in"].as_i64().unwrap_or(3600);
    let expiry = (chrono::Utc::now() + chrono::Duration::seconds(expires_in)).to_rfc3339();

    let account_email = fetch_ms365_account_email(&client, &access_token).await;

    state
        .db
        .set_user_setting(
            user_id,
            "ms365_access_token",
            &Db::encrypt_secret(&access_token),
        )
        .map_err(internal)?;
    state
        .db
        .set_user_setting(
            user_id,
            "ms365_refresh_token",
            &Db::encrypt_secret(&refresh_token),
        )
        .map_err(internal)?;
    state
        .db
        .set_user_setting(user_id, "ms365_token_expiry", &expiry)
        .map_err(internal)?;
    state
        .db
        .set_user_setting(user_id, "ms365_account_email", &account_email)
        .map_err(internal)?;

    Ok(axum::response::Redirect::temporary("/#/?ms365_connected=true").into_response())
}

async fn fetch_ms365_account_email(client: &reqwest::Client, access_token: &str) -> String {
    let resp = client
        .get("https://graph.microsoft.com/v1.0/me")
        .bearer_auth(access_token)
        .send()
        .await;
    if let Ok(r) = resp {
        if let Ok(v) = r.json::<Value>().await {
            return v["mail"]
                .as_str()
                .or_else(|| v["userPrincipalName"].as_str())
                .unwrap_or("")
                .to_string();
        }
    }
    String::new()
}

pub(crate) async fn ms365_status(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let has_token = state
        .db
        .get_user_setting(user.id, "ms365_access_token")
        .map_err(internal)?
        .is_some();
    let account_email = state
        .db
        .get_user_setting(user.id, "ms365_account_email")
        .map_err(internal)?
        .unwrap_or_default();
    Ok(Json(json!({
        "connected": has_token,
        "account_email": account_email,
    })))
}

pub(crate) async fn ms365_disconnect(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    for key in &[
        "ms365_access_token",
        "ms365_refresh_token",
        "ms365_token_expiry",
        "ms365_account_email",
    ] {
        state
            .db
            .delete_user_setting(user.id, key)
            .map_err(internal)?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn refresh_ms365_token(db: &Db, user_id: i64) -> Option<String> {
    let encrypted_access = db.get_user_setting(user_id, "ms365_access_token").ok()??;
    let encrypted_refresh = db.get_user_setting(user_id, "ms365_refresh_token").ok()??;
    let expiry_str = db
        .get_user_setting(user_id, "ms365_token_expiry")
        .ok()
        .flatten()
        .unwrap_or_default();

    let expires_soon = chrono::DateTime::parse_from_rfc3339(&expiry_str)
        .map(|exp| exp.signed_duration_since(chrono::Utc::now()).num_seconds() < 300)
        .unwrap_or(true);

    if !expires_soon {
        return Some(Db::decrypt_secret(&encrypted_access));
    }

    let refresh_token = Db::decrypt_secret(&encrypted_refresh);
    if refresh_token.is_empty() {
        return Some(Db::decrypt_secret(&encrypted_access));
    }

    let client_id = db.get_config("ms_client_id").ok()?.unwrap_or_default();
    let client_secret = db.get_config("ms_client_secret").ok()?.unwrap_or_default();

    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("scope", MS365_SCOPES),
    ];

    let resp = client
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await
        .ok()?;

    let v: Value = resp.json().await.ok()?;
    let new_access = v["access_token"].as_str().unwrap_or("").to_string();
    if new_access.is_empty() {
        return Some(Db::decrypt_secret(&encrypted_access));
    }

    let new_refresh = v["refresh_token"]
        .as_str()
        .unwrap_or(&refresh_token)
        .to_string();
    let expires_in = v["expires_in"].as_i64().unwrap_or(3600);
    let new_expiry = (chrono::Utc::now() + chrono::Duration::seconds(expires_in)).to_rfc3339();

    let _ = db.set_user_setting(
        user_id,
        "ms365_access_token",
        &Db::encrypt_secret(&new_access),
    );
    let _ = db.set_user_setting(
        user_id,
        "ms365_refresh_token",
        &Db::encrypt_secret(&new_refresh),
    );
    let _ = db.set_user_setting(user_id, "ms365_token_expiry", &new_expiry);

    Some(new_access)
}
