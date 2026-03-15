use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use super::internal;
use crate::AppState;

#[derive(Deserialize)]
pub(crate) struct RegisterPushBody {
    pub token: String,
    pub platform: String,
}

#[derive(Deserialize)]
pub(crate) struct UnregisterPushBody {
    pub token: String,
}

pub(crate) async fn register_push_token(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
    Json(body): Json<RegisterPushBody>,
) -> Result<Json<Value>, StatusCode> {
    if body.token.is_empty() || body.platform.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !matches!(body.platform.as_str(), "ios" | "android") {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .db
        .register_push_token(user.id, &body.token, &body.platform)
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn unregister_push_token(
    State(state): State<Arc<AppState>>,
    axum::Extension(_user): axum::Extension<crate::auth::AuthUser>,
    Json(body): Json<UnregisterPushBody>,
) -> Result<Json<Value>, StatusCode> {
    if body.token.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .db
        .unregister_push_token(&body.token)
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn send_push_to_user(
    db: &borg_core::db::Db,
    user_id: i64,
    title: &str,
    body: &str,
    _data: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let tokens = db.get_push_tokens_for_user(user_id)?;
    for (token, platform) in tokens {
        // TODO: integrate with Expo Push API or APNs/FCM
        tracing::info!(
            user_id,
            platform = platform.as_str(),
            token = token.as_str(),
            "would send push: {title} - {body}"
        );
    }
    Ok(())
}
