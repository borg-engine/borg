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

pub type TicketStore = Arc<TokioMutex<HashMap<String, Instant>>>;

const TICKET_TTL: Duration = Duration::from_secs(30);

pub fn new_ticket_store() -> TicketStore {
    Arc::new(TokioMutex::new(HashMap::new()))
}

pub fn generate_token() -> String {
    rand::thread_rng()
        .gen::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// Paths exempt from bearer auth entirely.
fn is_exempt(path: &str) -> bool {
    path == "/api/health" || path == "/api/auth/token" || !path.starts_with("/api/")
}

// SSE endpoints that accept a one-time ticket in place of a Bearer header.
fn is_sse_path(path: &str) -> bool {
    path == "/api/logs"
        || path == "/api/chat/events"
        || path.ends_with("/stream")
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

    // Check Authorization: Bearer header
    let header_token = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if header_token == Some(state.api_token.as_str()) {
        return next.run(request).await;
    }

    // For SSE endpoints: accept a short-lived one-time ticket from ?ticket=
    if is_sse_path(path) {
        let ticket = request.uri().query().and_then(|q| {
            q.split('&').find_map(|kv| {
                let mut parts = kv.splitn(2, '=');
                let k = parts.next()?;
                let v = parts.next()?;
                if k == "ticket" { Some(v.to_string()) } else { None }
            })
        });
        if let Some(ref t) = ticket {
            let mut store = state.sse_tickets.lock().await;
            if let Some(&issued_at) = store.get(t.as_str()) {
                if issued_at.elapsed() < TICKET_TTL {
                    store.remove(t.as_str());
                    return next.run(request).await;
                }
            }
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
pub async fn get_token(
    State(state): State<Arc<AppState>>,
) -> Response {
    Json(json!({"token": state.api_token})).into_response()
}

// POST /api/auth/sse-ticket — exchange a Bearer token for a short-lived
// one-time ticket that can be passed as ?ticket= on SSE endpoints.
// EventSource cannot set custom headers, so this avoids logging the long-lived
// token in URLs.
pub async fn issue_sse_ticket(
    State(state): State<Arc<AppState>>,
) -> Response {
    let ticket = generate_token();
    state.sse_tickets.lock().await.insert(ticket.clone(), Instant::now());
    Json(json!({"ticket": ticket})).into_response()
}

// Remove expired tickets. Call periodically from a background task.
pub async fn purge_expired_tickets(store: &TicketStore) {
    let mut map = store.lock().await;
    map.retain(|_, issued_at| issued_at.elapsed() < TICKET_TTL * 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_64_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_is_random() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn ticket_accepted_within_ttl() {
        let store = new_ticket_store();
        let ticket = generate_token();
        store.lock().await.insert(ticket.clone(), Instant::now());
        // Should still be valid immediately
        let mut map = store.lock().await;
        let issued = map.get(&ticket).copied().expect("ticket must exist");
        assert!(issued.elapsed() < TICKET_TTL);
        map.remove(&ticket);
        assert!(map.get(&ticket).is_none(), "ticket consumed");
    }

    #[tokio::test]
    async fn expired_ticket_rejected() {
        let store = new_ticket_store();
        let ticket = generate_token();
        // Backdate by inserting a timestamp that is already past the TTL
        let past = Instant::now() - (TICKET_TTL + Duration::from_secs(1));
        store.lock().await.insert(ticket.clone(), past);
        let map = store.lock().await;
        let issued = map.get(&ticket).copied().expect("ticket must exist");
        assert!(issued.elapsed() >= TICKET_TTL, "ticket should be expired");
    }

    #[tokio::test]
    async fn purge_removes_expired() {
        let store = new_ticket_store();
        let old = generate_token();
        let fresh = generate_token();
        let past = Instant::now() - (TICKET_TTL * 3);
        store.lock().await.insert(old.clone(), past);
        store.lock().await.insert(fresh.clone(), Instant::now());
        purge_expired_tickets(&store).await;
        let map = store.lock().await;
        assert!(map.get(&old).is_none(), "expired ticket purged");
        assert!(map.get(&fresh).is_some(), "fresh ticket kept");
    }

    #[test]
    fn is_sse_path_matches_known_endpoints() {
        assert!(is_sse_path("/api/logs"));
        assert!(is_sse_path("/api/chat/events"));
        assert!(is_sse_path("/api/tasks/42/stream"));
        assert!(!is_sse_path("/api/tasks"));
        assert!(!is_sse_path("/api/status"));
    }
}
