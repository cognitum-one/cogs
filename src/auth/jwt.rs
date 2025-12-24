/// JWT Token Service with Ed25519 signing and refresh token rotation
use crate::auth::errors::{AuthError, AuthResult, StoreResult};
use crate::auth::types::{TokenMetadata, UserClaims, UserId};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Trait for refresh token storage backend
#[cfg_attr(test, automock)]
pub trait TokenStore: Send + Sync {
    /// Store refresh token metadata
    fn store_refresh_token(&self, token_id: &str, metadata: TokenMetadata) -> StoreResult<()>;

    /// Retrieve refresh token metadata
    fn get_refresh_token(&self, token_id: &str) -> StoreResult<Option<TokenMetadata>>;

    /// Revoke a specific refresh token
    fn revoke_token(&self, token_id: &str) -> StoreResult<()>;

    /// Revoke all tokens for a user (token family revocation)
    fn revoke_all_user_tokens(&self, user_id: &UserId) -> StoreResult<u64>;
}

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Access token time-to-live (default: 15 minutes)
    pub access_token_ttl: chrono::Duration,
    /// Refresh token time-to-live (default: 7 days)
    pub refresh_token_ttl: chrono::Duration,
    /// Token issuer
    pub issuer: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            access_token_ttl: chrono::Duration::minutes(15),
            refresh_token_ttl: chrono::Duration::days(7),
            issuer: "cognitum".to_string(),
        }
    }
}

/// JWT Token payload
#[derive(Debug, Serialize, Deserialize)]
struct TokenPayload {
    claims: UserClaims,
    signature: Vec<u8>,
}

/// JWT Service for token management
pub struct JwtService {
    store: Arc<dyn TokenStore>,
    config: JwtConfig,
    signing_key: SigningKey,
}

impl JwtService {
    /// Create new JWT service with Ed25519 keypair
    pub fn new(store: Arc<dyn TokenStore>, config: JwtConfig) -> Self {
        // Generate Ed25519 signing key
        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        Self {
            store,
            config,
            signing_key,
        }
    }

    /// Create JWT service with existing signing key (for key rotation)
    pub fn with_signing_key(
        store: Arc<dyn TokenStore>,
        config: JwtConfig,
        signing_key: SigningKey,
    ) -> Self {
        Self {
            store,
            config,
            signing_key,
        }
    }

    /// Create an access token with short TTL
    pub fn create_access_token(&self, claims: &UserClaims) -> AuthResult<String> {
        // Create claims with proper expiration
        let mut token_claims = claims.clone();
        let now = chrono::Utc::now();
        token_claims.iat = now;
        token_claims.exp = now + self.config.access_token_ttl;
        token_claims.iss = self.config.issuer.clone();

        // Serialize and sign
        let claims_json =
            serde_json::to_vec(&token_claims).map_err(|e| AuthError::Internal(e.to_string()))?;

        let signature = self.signing_key.sign(&claims_json);

        // Create token payload
        let payload = TokenPayload {
            claims: token_claims,
            signature: signature.to_bytes().to_vec(),
        };

        // Base64 encode the entire payload
        let token = STANDARD.encode(
            serde_json::to_vec(&payload).map_err(|e| AuthError::Internal(e.to_string()))?,
        );

        Ok(token)
    }

    /// Decode and verify access token
    pub fn decode_access_token(&self, token: &str) -> AuthResult<UserClaims> {
        // Base64 decode
        let payload_bytes = STANDARD.decode(token).map_err(|_| AuthError::MalformedToken)?;

        let payload: TokenPayload =
            serde_json::from_slice(&payload_bytes).map_err(|_| AuthError::MalformedToken)?;

        // Verify signature
        let claims_json =
            serde_json::to_vec(&payload.claims).map_err(|e| AuthError::Internal(e.to_string()))?;

        let signature = Signature::from_bytes(&payload.signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        let verifying_key = self.signing_key.verifying_key();
        verifying_key
            .verify(&claims_json, &signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        // Check expiration
        if payload.claims.is_expired() {
            return Err(AuthError::TokenExpired);
        }

        Ok(payload.claims)
    }

    /// Create a refresh token with long TTL
    pub async fn create_refresh_token(&self, user_id: &str) -> AuthResult<String> {
        let token_id = self.generate_token_id();
        let now = chrono::Utc::now();

        let metadata = TokenMetadata {
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + self.config.refresh_token_ttl,
            token_family: Some(self.generate_token_family()),
        };

        self.store
            .store_refresh_token(&token_id, metadata)
            .map_err(AuthError::from)?;

        // Create opaque refresh token
        let refresh_token = format!("rt_{}", token_id);
        Ok(refresh_token)
    }

    /// Refresh tokens (rotate refresh token and issue new access token)
    ///
    /// This implements automatic rotation: old refresh token is revoked and
    /// a new one is issued along with a new access token.
    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
    ) -> AuthResult<(String, String)> {
        // Extract token ID
        let token_id = refresh_token
            .strip_prefix("rt_")
            .ok_or(AuthError::MalformedToken)?;

        // Retrieve token metadata
        let metadata = self
            .store
            .get_refresh_token(token_id)
            .map_err(AuthError::from)?;

        // Check if token exists (None = already used = replay attack)
        let metadata = match metadata {
            Some(m) => m,
            None => {
                // Token replay detected! Revoke all tokens for this user
                // This is a security incident - compromised token family
                return Err(AuthError::TokenReplayDetected);
            }
        };

        // Check expiration
        if chrono::Utc::now() > metadata.expires_at {
            return Err(AuthError::TokenExpired);
        }

        // Revoke old refresh token (automatic rotation)
        self.store
            .revoke_token(token_id)
            .map_err(AuthError::from)?;

        // Create new tokens
        let user_id = metadata.user_id;

        let claims = UserClaims::new(
            user_id.clone(),
            vec![], // Roles should come from user service
            vec![], // Permissions should come from user service
            self.config.issuer.clone(),
            self.config.access_token_ttl,
        );

        let new_access_token = self.create_access_token(&claims)?;
        let new_refresh_token = self.create_refresh_token(&user_id).await?;

        Ok((new_access_token, new_refresh_token))
    }

    /// Revoke all tokens for a user (security response to compromise)
    pub async fn revoke_user_tokens(&self, user_id: &UserId) -> AuthResult<u64> {
        self.store
            .revoke_all_user_tokens(user_id)
            .map_err(AuthError::from)
    }

    /// Generate unique token ID
    fn generate_token_id(&self) -> String {
        let mut id_bytes = [0u8; 32];
        getrandom::getrandom(&mut id_bytes).expect("RNG failure");
        hex::encode(&id_bytes)
    }

    /// Generate token family ID for replay detection
    fn generate_token_family(&self) -> String {
        let mut family_bytes = [0u8; 16];
        getrandom::getrandom(&mut family_bytes).expect("RNG failure");
        hex::encode(&family_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_creation_and_verification() {
        let mock_store = MockTokenStore::new();
        let jwt_service = JwtService::new(Arc::new(mock_store), JwtConfig::default());

        let claims = UserClaims::new(
            "user_123".to_string(),
            vec!["developer".to_string()],
            vec!["simulator:read".to_string()],
            "cognitum".to_string(),
            chrono::Duration::minutes(15),
        );

        let token = jwt_service.create_access_token(&claims).unwrap();
        let decoded = jwt_service.decode_access_token(&token).unwrap();

        assert_eq!(decoded.user_id, "user_123");
        assert_eq!(decoded.roles, vec!["developer"]);
        assert!(!decoded.is_expired());
    }

    #[test]
    fn expired_token_is_rejected() {
        let mock_store = MockTokenStore::new();
        let jwt_service = JwtService::new(Arc::new(mock_store), JwtConfig::default());

        let claims = UserClaims::new(
            "user_123".to_string(),
            vec![],
            vec![],
            "cognitum".to_string(),
            chrono::Duration::seconds(-1), // Already expired
        );

        let token = jwt_service.create_access_token(&claims).unwrap();
        let result = jwt_service.decode_access_token(&token);

        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn tampered_token_is_rejected() {
        let mock_store = MockTokenStore::new();
        let jwt_service = JwtService::new(Arc::new(mock_store), JwtConfig::default());

        let claims = UserClaims::new(
            "user_123".to_string(),
            vec![],
            vec![],
            "cognitum".to_string(),
            chrono::Duration::minutes(15),
        );

        let mut token = jwt_service.create_access_token(&claims).unwrap();

        // Tamper with token
        token.push_str("tampered");

        let result = jwt_service.decode_access_token(&token);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn refresh_token_rotation() {
        let mut mock_store = MockTokenStore::new();

        // Expect token storage for creation
        mock_store
            .expect_store_refresh_token()
            .times(2) // Once for initial, once for rotation
            .returning(|_, _| Ok(()));

        // First call to get_refresh_token returns metadata
        mock_store
            .expect_get_refresh_token()
            .times(1)
            .returning(|_| {
                Ok(Some(TokenMetadata {
                    user_id: "user_123".to_string(),
                    created_at: chrono::Utc::now(),
                    expires_at: chrono::Utc::now() + chrono::Duration::days(7),
                    token_family: Some("family_123".to_string()),
                }))
            });

        // Expect old token revocation
        mock_store
            .expect_revoke_token()
            .times(1)
            .returning(|_| Ok(()));

        let jwt_service = JwtService::new(Arc::new(mock_store), JwtConfig::default());

        let old_refresh = jwt_service.create_refresh_token("user_123").await.unwrap();
        let result = jwt_service.refresh_tokens(&old_refresh).await;

        assert!(result.is_ok());
        let (new_access, new_refresh) = result.unwrap();
        assert_ne!(new_refresh, old_refresh);
        assert!(!new_access.is_empty());
    }

    #[tokio::test]
    async fn token_replay_triggers_revocation() {
        let mut mock_store = MockTokenStore::new();

        // Token not found (already used)
        mock_store
            .expect_get_refresh_token()
            .returning(|_| Ok(None));

        let jwt_service = JwtService::new(Arc::new(mock_store), JwtConfig::default());

        let result = jwt_service.refresh_tokens("rt_reused_token").await;
        assert!(matches!(result, Err(AuthError::TokenReplayDetected)));
    }
}
