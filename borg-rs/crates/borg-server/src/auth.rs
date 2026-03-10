use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

const MAX_LOGIN_ATTEMPTS: u32 = 5;
const LOGIN_WINDOW_SECS: u64 = 300;
pub const WORKSPACE_HEADER: &str = "x-workspace-id";
pub const DEFAULT_CLOUDFLARE_ACCESS_EMAIL_HEADER: &str = "cf-access-authenticated-user-email";

// ── Token generation ─────────────────────────────────────────────────────

pub fn generate_token() -> String {
    rand::thread_rng()
        .gen::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ── JWT ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: i64, // user id
    pub username: String,
    pub is_admin: bool,
    pub exp: usize, // expiry (unix timestamp)
}

pub fn create_jwt(user_id: i64, username: &str, is_admin: bool, secret: &str) -> String {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(30))
        .unwrap_or_else(chrono::Utc::now)
        .timestamp() as usize;
    let claims = JwtClaims {
        sub: user_id,
        username: username.to_string(),
        is_admin,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap_or_else(|e| {
        tracing::error!("JWT encode failed: {e}");
        String::new()
    })
}

pub fn verify_jwt(token: &str, secret: &str) -> Option<JwtClaims> {
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}

// ── Password hashing ─────────────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ── Auth user (extracted from request) ───────────────────────────────────

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub default_workspace_id: i64,
}

impl AuthUser {
    pub fn system_admin() -> Self {
        Self {
            id: 0,
            username: "admin".to_string(),
            is_admin: true,
            default_workspace_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub role: String,
    pub is_default: bool,
}

fn auth_mode_is_cloudflare_access(mode: &str) -> bool {
    mode.eq_ignore_ascii_case("cloudflare_access")
}

fn extract_email_header(headers: &HeaderMap, header_name: &str) -> Option<String> {
    let header_name = if header_name.trim().is_empty() {
        DEFAULT_CLOUDFLARE_ACCESS_EMAIL_HEADER
    } else {
        header_name
    };
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// Extract bearer token from Authorization header
fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

pub fn resolve_auth_user_from_headers(
    headers: &HeaderMap,
    secret: &str,
    api_token: &str,
    auth_disabled: bool,
    auth_mode: &str,
    cloudflare_access_email_header: &str,
) -> Option<AuthUser> {
    if auth_disabled {
        return Some(AuthUser::system_admin());
    }
    if auth_mode_is_cloudflare_access(auth_mode) {
        if let Some(token) = extract_bearer(headers) {
            if token == api_token {
                return Some(AuthUser::system_admin());
            }
        }
        if let Some(email) = extract_email_header(headers, cloudflare_access_email_header) {
            return Some(AuthUser {
                id: 0,
                username: email,
                is_admin: false,
                default_workspace_id: 0,
            });
        }
        return None;
    }
    let token = extract_bearer(headers)?;
    if let Some(claims) = verify_jwt(token, secret) {
        return Some(AuthUser {
            id: claims.sub,
            username: claims.username,
            is_admin: claims.is_admin,
            default_workspace_id: 0,
        });
    }
    if token == api_token {
        return Some(AuthUser::system_admin());
    }
    None
}

fn cloudflare_email_is_admin(config: &borg_core::config::Config, email: &str) -> bool {
    config
        .cloudflare_admin_emails
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(email))
}

fn provision_cloudflare_user(state: &AppState, email: &str) -> Result<AuthUser, Response> {
    let desired_admin = cloudflare_email_is_admin(&state.config, email);
    let existing = state.db.get_user_by_username(email).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("user lookup failed: {e}")})),
        )
            .into_response()
    })?;

    let user_id = if let Some((id, _, _, _, is_admin)) = existing {
        if desired_admin && !is_admin {
            state.db.set_user_admin(id, true).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("failed to promote admin: {e}")})),
                )
                    .into_response()
            })?;
        }
        id
    } else {
        let password_hash = hash_password(&generate_token()).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("password setup failed: {e}")})),
            )
                .into_response()
        })?;
        state
            .db
            .create_user(email, email, &password_hash, desired_admin)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("user create failed: {e}")})),
                )
                    .into_response()
            })?
    };

    let is_admin = state
        .db
        .get_user_by_id(user_id)
        .ok()
        .flatten()
        .map(|(_, _, _, is_admin)| is_admin)
        .unwrap_or(desired_admin);

    if is_admin {
        if let Err(e) = state.db.ensure_admin_workspace_memberships(user_id) {
            tracing::warn!(
                user_id,
                email,
                "failed to sync admin workspace memberships: {e}"
            );
        }
        if let Err(e) = state.db.set_preferred_admin_workspace(user_id) {
            tracing::warn!(
                user_id,
                email,
                "failed to set preferred admin workspace: {e}"
            );
        }
    }

    let default_workspace_id = state
        .db
        .get_user_default_workspace_id(user_id)
        .ok()
        .flatten()
        .unwrap_or(0);
    Ok(AuthUser {
        id: user_id,
        username: email.to_string(),
        is_admin,
        default_workspace_id,
    })
}

// Paths exempt from bearer auth entirely.
fn is_exempt(path: &str) -> bool {
    path == "/api/health"
        || path == "/api/auth/login"
        || path == "/api/auth/setup"
        || path == "/api/auth/status"
        || !path.starts_with("/api/")
}

// ── Middleware ────────────────────────────────────────────────────────────

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    if is_exempt(&path) {
        return next.run(request).await;
    }

    if state.config.disable_auth {
        request.extensions_mut().insert(AuthUser::system_admin());
        return next.run(request).await;
    }

    if auth_mode_is_cloudflare_access(&state.config.auth_mode) {
        if let Some(token) = extract_bearer(request.headers()) {
            if token == state.api_token {
                request.extensions_mut().insert(AuthUser::system_admin());
                return next.run(request).await;
            }
        }
        let Some(email) = extract_email_header(
            request.headers(),
            &state.config.cloudflare_access_email_header,
        ) else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing Cloudflare Access identity"})),
            )
                .into_response();
        };
        match provision_cloudflare_user(state.as_ref(), &email) {
            Ok(user) => {
                request.extensions_mut().insert(user);
                return next.run(request).await;
            },
            Err(response) => return response,
        }
    }

    // Try JWT first
    if let Some(token) = extract_bearer(request.headers()) {
        if let Some(claims) = verify_jwt(token, &state.jwt_secret) {
            let default_workspace_id = state
                .db
                .get_user_default_workspace_id(claims.sub)
                .ok()
                .flatten()
                .unwrap_or(0);
            request.extensions_mut().insert(AuthUser {
                id: claims.sub,
                username: claims.username,
                is_admin: claims.is_admin,
                default_workspace_id,
            });
            return next.run(request).await;
        }

        // Fall back to shared API token (sidecar, CLI, etc.) — treated as admin
        if token == state.api_token {
            request.extensions_mut().insert(AuthUser::system_admin());
            return next.run(request).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "unauthorized"})),
    )
        .into_response()
}

fn requested_workspace_id(headers: &HeaderMap) -> Option<i64> {
    headers
        .get(WORKSPACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|id| *id > 0)
}

pub async fn workspace_middleware(
    State(state): State<Arc<AppState>>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if is_exempt(&path) {
        return next.run(request).await;
    }

    let Some(user) = request.extensions().get::<AuthUser>().cloned() else {
        return next.run(request).await;
    };

    let requested_id = requested_workspace_id(request.headers());
    let workspace = if user.id == 0 {
        let candidate = if let Some(id) = requested_id {
            state.db.get_workspace(id).ok().flatten()
        } else {
            state.db.get_system_workspace().ok().flatten()
        };
        match candidate {
            Some(workspace) => WorkspaceContext {
                id: workspace.id,
                name: workspace.name,
                kind: workspace.kind,
                role: "admin".to_string(),
                is_default: requested_id.is_none(),
            },
            None => return next.run(request).await,
        }
    } else {
        let workspace_id = requested_id.unwrap_or(user.default_workspace_id);
        if workspace_id <= 0 {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "no workspace available"})),
            )
                .into_response();
        }
        let membership = match state
            .db
            .get_user_workspace_membership(user.id, workspace_id)
        {
            Ok(Some(m)) => m,
            Ok(None) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "workspace access denied"})),
                )
                    .into_response();
            },
            Err(e) => {
                tracing::error!("workspace resolution failed: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "workspace resolution failed"})),
                )
                    .into_response();
            },
        };
        WorkspaceContext {
            id: membership.workspace_id,
            name: membership.name,
            kind: membership.kind,
            role: membership.role,
            is_default: membership.is_default,
        }
    };

    request.extensions_mut().insert(workspace);
    next.run(request).await
}

// ── Handlers ─────────────────────────────────────────────────────────────

// GET /api/auth/token — returns shared token for backward compat
pub async fn get_token(State(state): State<Arc<AppState>>) -> Response {
    if !state.config.disable_auth {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "shared token disabled"})),
        )
            .into_response();
    }
    Json(json!({"token": state.api_token})).into_response()
}

// GET /api/auth/status — whether setup is needed, and user count
pub async fn auth_status(State(state): State<Arc<AppState>>) -> Response {
    if state.config.disable_auth {
        return Json(json!({
            "needs_setup": false,
            "user_count": 1,
            "auth_disabled": true,
            "auth_mode": "disabled",
        }))
        .into_response();
    }
    if auth_mode_is_cloudflare_access(&state.config.auth_mode) {
        let user_count = state.db.count_users().unwrap_or(0);
        return Json(json!({
            "needs_setup": false,
            "user_count": user_count,
            "auth_disabled": false,
            "auth_mode": "cloudflare_access",
        }))
        .into_response();
    }
    let user_count = state.db.count_users().unwrap_or(0);
    Json(json!({
        "needs_setup": user_count == 0,
        "user_count": user_count,
        "auth_disabled": false,
        "auth_mode": "local",
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct SetupBody {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

// POST /api/auth/setup — create first admin user (only when no users exist)
static SETUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub async fn setup(State(state): State<Arc<AppState>>, Json(body): Json<SetupBody>) -> Response {
    if auth_mode_is_cloudflare_access(&state.config.auth_mode) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "setup disabled when AUTH_MODE=cloudflare_access"})),
        )
            .into_response();
    }
    let _guard = match SETUP_LOCK.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": "setup already in progress"})),
            )
                .into_response();
        },
    };

    let user_count = match state.db.count_users() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    if user_count > 0 {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "setup already completed"})),
        )
            .into_response();
    }

    if body.username.trim().is_empty() || body.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "username required, password min 8 chars"})),
        )
            .into_response();
    }

    let password_hash = match hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    let display = body.display_name.as_deref().unwrap_or(&body.username);
    match state
        .db
        .create_user(&body.username, display, &password_hash, true)
    {
        Ok(id) => {
            let default_workspace_id = state
                .db
                .get_user_default_workspace_id(id)
                .ok()
                .flatten()
                .unwrap_or(0);
            let token = create_jwt(id, &body.username, true, &state.jwt_secret);
            Json(json!({
                "token": token,
                "user": { "id": id, "username": body.username, "display_name": display, "is_admin": true, "default_workspace_id": default_workspace_id }
            }))
            .into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

// POST /api/auth/login
pub async fn login(State(state): State<Arc<AppState>>, Json(body): Json<LoginBody>) -> Response {
    if auth_mode_is_cloudflare_access(&state.config.auth_mode) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "login disabled when AUTH_MODE=cloudflare_access"})),
        )
            .into_response();
    }
    // Rate limiting
    {
        let mut attempts = state
            .login_attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        // Clean stale entries
        attempts.retain(|_, (_, t)| now.duration_since(*t).as_secs() < LOGIN_WINDOW_SECS);
        if let Some((count, _)) = attempts.get(&body.username) {
            if *count >= MAX_LOGIN_ATTEMPTS {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": "too many login attempts, try again later"})),
                )
                    .into_response();
            }
        }
    }

    let user = match state.db.get_user_by_username(&body.username) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid credentials"})),
            )
                .into_response();
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    let (id, username, display_name, password_hash, is_admin) = user;

    if !verify_password(&body.password, &password_hash) {
        let mut attempts = state
            .login_attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let entry = attempts
            .entry(body.username.clone())
            .or_insert((0, std::time::Instant::now()));
        entry.0 += 1;
        entry.1 = std::time::Instant::now();
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid credentials"})),
        )
            .into_response();
    }

    // Clear rate limit on success
    {
        let mut attempts = state
            .login_attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        attempts.remove(&body.username);
    }

    let token = create_jwt(id, &username, is_admin, &state.jwt_secret);
    let default_workspace_id = state
        .db
        .get_user_default_workspace_id(id)
        .ok()
        .flatten()
        .unwrap_or(0);
    Json(json!({
        "token": token,
        "user": { "id": id, "username": username, "display_name": display_name, "is_admin": is_admin, "default_workspace_id": default_workspace_id }
    }))
    .into_response()
}

// GET /api/auth/me — return current user info
pub async fn get_me(request: axum::extract::Request) -> Response {
    let user = request.extensions().get::<AuthUser>().cloned();
    let workspace = request.extensions().get::<WorkspaceContext>().cloned();
    match user {
        Some(u) => Json(json!({
            "id": u.id,
            "username": u.username,
            "display_name": u.username,
            "is_admin": u.is_admin,
            "default_workspace_id": u.default_workspace_id,
            "workspace": workspace.as_ref().map(|w| json!({
                "id": w.id,
                "name": w.name,
                "kind": w.kind,
                "role": w.role,
                "is_default": w.is_default,
            })),
        }))
        .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_exempt_health() {
        assert!(is_exempt("/api/health"));
    }

    #[test]
    fn auth_token_requires_auth() {
        assert!(!is_exempt("/api/auth/token"));
    }

    #[test]
    fn is_exempt_auth_login() {
        assert!(is_exempt("/api/auth/login"));
    }

    #[test]
    fn is_exempt_auth_setup() {
        assert!(is_exempt("/api/auth/setup"));
    }

    #[test]
    fn is_exempt_static_assets() {
        assert!(is_exempt("/"));
        assert!(is_exempt("/index.html"));
        assert!(is_exempt("/static/main.js"));
    }

    #[test]
    fn not_exempt_api_paths() {
        assert!(!is_exempt("/api/tasks"));
        assert!(!is_exempt("/api/logs"));
        assert!(!is_exempt("/api/tasks/1/stream"));
        assert!(!is_exempt("/api/chat/events"));
    }

    #[test]
    fn generate_token_is_64_hex_chars() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_tokens_are_unique() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn jwt_roundtrip() {
        let secret = "test_secret_key";
        let token = create_jwt(42, "testuser", true, secret);
        let claims = verify_jwt(&token, secret).expect("should verify");
        assert_eq!(claims.sub, 42);
        assert_eq!(claims.username, "testuser");
        assert!(claims.is_admin);
    }

    #[test]
    fn jwt_wrong_secret_fails() {
        let token = create_jwt(1, "u", false, "secret1");
        assert!(verify_jwt(&token, "secret2").is_none());
    }

    #[test]
    fn cloudflare_access_header_extracts_email() {
        let mut headers = HeaderMap::new();
        headers.insert(
            DEFAULT_CLOUDFLARE_ACCESS_EMAIL_HEADER,
            "user@example.com".parse().unwrap(),
        );
        let user = resolve_auth_user_from_headers(
            &headers,
            "secret",
            "api-token",
            false,
            "cloudflare_access",
            DEFAULT_CLOUDFLARE_ACCESS_EMAIL_HEADER,
        )
        .expect("cloudflare user should resolve");
        assert_eq!(user.username, "user@example.com");
    }

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("mypassword").expect("should hash");
        assert!(verify_password("mypassword", &hash));
        assert!(!verify_password("wrongpassword", &hash));
    }
}
