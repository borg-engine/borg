use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use serde_json::json;

use crate::{auth, AppState};

fn should_log_request(
    method: &axum::http::Method,
    route: &str,
    status: u16,
    duration_ms: u128,
) -> bool {
    if matches!(
        route,
        "/api/health" | "/api/logs" | "/api/chat/events" | "/api/tasks/:id/stream"
    ) {
        return false;
    }
    if status >= 400 || duration_ms >= 1_000 {
        return true;
    }
    if !matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        return true;
    }
    matches!(
        route,
        "/api/search"
            | "/api/borgsearch/query"
            | "/api/projects/:id/files/:file_id/content"
            | "/api/knowledge/:id/content"
    )
}

pub(crate) async fn request_telemetry_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| path.clone());
    let auth_user = auth::resolve_auth_user_from_headers(
        request.headers(),
        &state.jwt_secret,
        &state.api_token,
        state.config.disable_auth,
        &state.config.auth_mode,
        &state.config.cloudflare_access_email_header,
    );

    let response = next.run(request).await;
    let status = response.status().as_u16();
    let duration_ms = started.elapsed().as_millis();

    if should_log_request(&method, &matched_path, status, duration_ms) {
        let event = json!({
            "method": method.to_string(),
            "route": matched_path,
            "path": path,
            "status": status,
            "duration_ms": duration_ms,
            "user_id": auth_user.as_ref().map(|u| u.id),
            "username": auth_user.as_ref().map(|u| u.username.clone()),
            "is_admin": auth_user.as_ref().map(|u| u.is_admin),
        });
        if status >= 500 {
            tracing::warn!(target: "instrumentation.http", message = "request completed", metadata = %event);
        } else {
            tracing::info!(target: "instrumentation.http", message = "request completed", metadata = %event);
        }
    }

    response
}
