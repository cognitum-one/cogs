/// Authentication type definitions for Cognitum chip v1 commercialization
use serde::{Deserialize, Serialize};
use std::fmt;

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// API key scope/permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyScope {
    /// Read-only access
    ReadOnly,
    /// Read and write access
    ReadWrite,
    /// Full admin access
    Admin,
}

impl fmt::Display for KeyScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyScope::ReadOnly => write!(f, "read_only"),
            KeyScope::ReadWrite => write!(f, "read_write"),
            KeyScope::Admin => write!(f, "admin"),
        }
    }
}

/// Metadata for API keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub key_id: String,
    pub user_id: UserId,
    pub scope: KeyScope,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked: bool,
    pub revoked_reason: Option<String>,
}

/// Metadata for JWT refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub user_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub token_family: Option<String>,
}

/// JWT claims for access tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaims {
    pub user_id: String,
    pub roles: Vec<String>,
    pub scope: Vec<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub exp: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub iat: chrono::DateTime<chrono::Utc>,
    pub iss: String,
}

impl UserClaims {
    pub fn new(
        user_id: String,
        roles: Vec<String>,
        scope: Vec<String>,
        issuer: String,
        ttl: chrono::Duration,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            user_id,
            roles,
            scope,
            exp: now + ttl,
            iat: now,
            iss: issuer,
        }
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.exp
    }
}

/// Resource identifier for access control
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(String);

impl ResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_creation_and_display() {
        let id = UserId::new("user_123");
        assert_eq!(id.as_str(), "user_123");
        assert_eq!(id.to_string(), "user_123");
    }

    #[test]
    fn key_scope_display() {
        assert_eq!(KeyScope::ReadOnly.to_string(), "read_only");
        assert_eq!(KeyScope::ReadWrite.to_string(), "read_write");
        assert_eq!(KeyScope::Admin.to_string(), "admin");
    }

    #[test]
    fn user_claims_expiration() {
        let claims = UserClaims::new(
            "user_123".to_string(),
            vec!["developer".to_string()],
            vec!["simulator:read".to_string()],
            "cognitum".to_string(),
            chrono::Duration::seconds(-1), // Already expired
        );
        assert!(claims.is_expired());

        let valid_claims = UserClaims::new(
            "user_123".to_string(),
            vec!["developer".to_string()],
            vec!["simulator:read".to_string()],
            "cognitum".to_string(),
            chrono::Duration::minutes(15),
        );
        assert!(!valid_claims.is_expired());
    }
}
