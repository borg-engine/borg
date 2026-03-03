use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use rand::Rng;
use serde_json::json;

use crate::AppState;

pub fn generate_token() -> String {
    rand::thread_rng()
        .gen::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// Paths exempt from bearer auth entirely.
pub(crate) fn is_exempt(path: &str) -> bool {
    path == "/api/health" || path == "/api/auth/token" || !path.starts_with("/api/")
}

fn verify_token(headers: &axum::http::HeaderMap, expected: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        == Some(expected)
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    if is_exempt(path) {
        return next.run(request).await;
    }

    if verify_token(request.headers(), &state.api_token) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response()
    }
}

// GET /api/auth/token — restricted to loopback connections only.
// Remote callers (e.g. internet-facing proxies) must obtain the token
// out-of-band from {data_dir}/.api-token on the server filesystem.
pub async fn get_token(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    if !addr.ip().is_loopback() {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    Json(json!({"token": state.api_token})).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_exempt_health_and_token() {
        assert!(is_exempt("/api/health"));
        assert!(is_exempt("/api/auth/token"));
    }

    #[test]
    fn test_is_exempt_non_api_paths() {
        assert!(is_exempt("/"));
        assert!(is_exempt("/index.html"));
        assert!(is_exempt("/static/app.js"));
    }

    #[test]
    fn test_is_exempt_api_paths_require_auth() {
        assert!(!is_exempt("/api/tasks"));
        assert!(!is_exempt("/api/settings"));
        assert!(!is_exempt("/api/projects"));
        assert!(!is_exempt("/api/status"));
    }

    #[test]
    fn test_loopback_ipv4() {
        let loopback: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert!(loopback.is_loopback());
        let other: std::net::IpAddr = "127.0.0.2".parse().unwrap();
        assert!(other.is_loopback());
    }

    #[test]
    fn test_loopback_ipv6() {
        let loopback: std::net::IpAddr = "::1".parse().unwrap();
        assert!(loopback.is_loopback());
    }

    #[test]
    fn test_non_loopback_rejected() {
        let ips = ["192.168.1.1", "10.0.0.1", "172.16.0.1", "8.8.8.8", "2001:db8::1"];
        for ip in ips {
            let addr: std::net::IpAddr = ip.parse().unwrap();
            assert!(!addr.is_loopback(), "{ip} should not be loopback");
        }
    }
}
