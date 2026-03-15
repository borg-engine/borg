use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentJwtClaims {
    pub sub: i64,
    pub username: String,
    pub workspace_id: i64,
    pub is_admin: bool,
    pub agent: bool,
    pub exp: usize,
}

/// Generate a short-lived JWT for an agent process, scoped to the creating user's identity.
/// The agent will have the same permissions as the user -- same workspace access, same role.
/// NOT admin unless the creating user is admin.
pub fn generate_agent_token(
    jwt_secret: &str,
    user_id: i64,
    username: &str,
    workspace_id: i64,
    is_admin: bool,
    ttl_secs: u64,
) -> String {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(ttl_secs as i64))
        .unwrap_or_else(chrono::Utc::now)
        .timestamp() as usize;
    let claims = AgentJwtClaims {
        sub: user_id,
        username: username.to_string(),
        workspace_id,
        is_admin,
        agent: true,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .unwrap_or_else(|e| {
        tracing::error!("agent JWT encode failed: {e}");
        String::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{decode, DecodingKey, Validation};

    #[test]
    fn agent_token_roundtrip() {
        let secret = "test_agent_secret";
        let token = generate_agent_token(secret, 42, "testuser", 7, false, 3600);
        assert!(!token.is_empty());

        let decoded = decode::<AgentJwtClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .expect("should decode");
        assert_eq!(decoded.claims.sub, 42);
        assert_eq!(decoded.claims.username, "testuser");
        assert_eq!(decoded.claims.workspace_id, 7);
        assert!(!decoded.claims.is_admin);
        assert!(decoded.claims.agent);
    }

    #[test]
    fn agent_token_respects_admin_flag() {
        let secret = "test_secret";
        let token = generate_agent_token(secret, 1, "admin", 1, true, 3600);
        let decoded = decode::<AgentJwtClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .expect("should decode");
        assert!(decoded.claims.is_admin);
        assert!(decoded.claims.agent);
    }

    #[test]
    fn agent_token_wrong_secret_fails() {
        let token = generate_agent_token("secret1", 1, "u", 1, false, 3600);
        let result = decode::<AgentJwtClaims>(
            &token,
            &DecodingKey::from_secret("secret2".as_bytes()),
            &Validation::default(),
        );
        assert!(result.is_err());
    }

    /// Verify that agent tokens can be decoded by the server's existing JwtClaims
    /// struct (which lacks workspace_id/agent fields). Serde ignores unknown fields
    /// by default, so the extra claims are silently dropped.
    #[test]
    fn agent_token_compatible_with_base_claims() {
        #[derive(Debug, Deserialize)]
        struct BaseClaims {
            sub: i64,
            username: String,
            is_admin: bool,
            exp: usize,
        }

        let secret = "compat_test";
        let token = generate_agent_token(secret, 99, "alice", 5, false, 3600);
        let decoded = decode::<BaseClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .expect("base claims should decode from agent token");
        assert_eq!(decoded.claims.sub, 99);
        assert_eq!(decoded.claims.username, "alice");
        assert!(!decoded.claims.is_admin);
        assert!(decoded.claims.exp > 0);
    }
}
