use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use rand::Rng;
use serde_json::json;
use tokio::sync::Mutex as TokioMutex;

use crate::AppState;

pub type SseTickets = Arc<TokioMutex<HashMap<String, Instant>>>;

const TICKET_TTL: Duration = Duration::from_secs(60);

pub fn generate_token() -> String {
    rand::thread_rng()
        .gen::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn generate_ticket() -> String {
    rand::thread_rng()
        .gen::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// Paths exempt from auth entirely.
fn is_exempt(path: &str) -> bool {
    path == "/api/health" || path == "/api/auth/token" || !path.starts_with("/api/")
}

// Validate and consume a one-time SSE ticket. Returns true if the ticket was
// present and not expired; always removes it regardless.
fn validate_ticket(tickets: &mut HashMap<String, Instant>, ticket: &str) -> bool {
    match tickets.remove(ticket) {
        Some(expires_at) => expires_at > Instant::now(),
        None => false,
    }
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

    // Check Authorization: Bearer header first
    let header_token = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(token) = header_token {
        if token == state.api_token.as_str() {
            return next.run(request).await;
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }

    // For SSE endpoints: accept a one-time ?ticket= query param
    let ticket = request.uri().query().and_then(|q| {
        q.split('&').find_map(|kv| {
            let mut parts = kv.splitn(2, '=');
            let k = parts.next()?;
            let v = parts.next()?;
            if k == "ticket" { Some(v.to_string()) } else { None }
        })
    });

    if let Some(ticket) = ticket {
        let mut tickets = state.sse_tickets.lock().await;
        if validate_ticket(&mut tickets, &ticket) {
            return next.run(request).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "unauthorized"})),
    )
        .into_response()
}

// GET /api/auth/token — returns the token to any caller that can reach the
// dashboard. The token protects against rogue local processes (e.g. a
// compromised container), not against someone who already has HTTP access to
// the dashboard. If the dashboard page loads, the caller is authorized.
pub async fn get_token(State(state): State<Arc<AppState>>) -> Response {
    Json(json!({"token": state.api_token})).into_response()
}

// POST /api/auth/sse-ticket — exchange a Bearer token for a short-lived
// one-time ticket that can be passed as ?ticket= on an SSE URL.
// Prevents the long-lived API token from appearing in HTTP access logs.
pub async fn post_sse_ticket(State(state): State<Arc<AppState>>) -> Response {
    let ticket = generate_ticket();
    let expires_at = Instant::now() + TICKET_TTL;
    state.sse_tickets.lock().await.insert(ticket.clone(), expires_at);
    Json(json!({"ticket": ticket})).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ticket_accepted_and_consumed() {
        let mut tickets = HashMap::new();
        let ticket = "abc123".to_string();
        tickets.insert(ticket.clone(), Instant::now() + Duration::from_secs(60));

        assert!(validate_ticket(&mut tickets, &ticket));
        // Consumed — second use rejected
        assert!(!validate_ticket(&mut tickets, &ticket));
    }

    #[test]
    fn expired_ticket_rejected() {
        let mut tickets = HashMap::new();
        let ticket = "expired".to_string();
        tickets.insert(ticket.clone(), Instant::now() - Duration::from_secs(1));

        assert!(!validate_ticket(&mut tickets, &ticket));
    }

    #[test]
    fn unknown_ticket_rejected() {
        let mut tickets = HashMap::<String, Instant>::new();
        assert!(!validate_ticket(&mut tickets, "nonexistent"));
    }

    #[test]
    fn generate_ticket_is_32_hex_chars() {
        let t = generate_ticket();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn is_exempt_paths() {
        assert!(is_exempt("/api/health"));
        assert!(is_exempt("/api/auth/token"));
        assert!(is_exempt("/static/app.js"));
        assert!(!is_exempt("/api/tasks"));
        assert!(!is_exempt("/api/auth/sse-ticket"));
    }
}
