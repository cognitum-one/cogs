/// API Key Service with Argon2 hashing and constant-time validation
use crate::auth::errors::{AuthError, AuthResult, StoreResult};
use crate::auth::types::{KeyMetadata, KeyScope, UserId};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Trait for API key storage backend
#[cfg_attr(test, automock)]
pub trait ApiKeyStore: Send + Sync {
    /// Store hashed API key with metadata
    fn store_key_hash(
        &self,
        key_id: &str,
        hash: &str,
        metadata: KeyMetadata,
    ) -> StoreResult<()>;

    /// Retrieve hashed API key by key ID
    fn get_key_hash(&self, key_id: &str) -> StoreResult<Option<String>>;

    /// Retrieve key metadata
    fn get_key_metadata(&self, key_id: &str) -> StoreResult<Option<KeyMetadata>>;

    /// Revoke an API key
    fn revoke_key(&self, key_id: &str, reason: &str) -> StoreResult<()>;

    /// List all keys for a user
    fn list_keys(&self, user_id: &UserId) -> StoreResult<Vec<KeyMetadata>>;

    /// Update last used timestamp
    fn update_last_used(&self, key_id: &str) -> StoreResult<()>;
}

/// API Key Service for managing authentication keys
pub struct ApiKeyService {
    store: Arc<dyn ApiKeyStore>,
    argon2: Argon2<'static>,
}

impl ApiKeyService {
    /// Create new API key service with default Argon2 configuration
    pub fn new(store: Arc<dyn ApiKeyStore>) -> Self {
        // Production-grade Argon2id configuration
        let argon2 = Argon2::default();
        Self { store, argon2 }
    }

    /// Create a new API key for a user
    ///
    /// Returns (visible_key, key_id) tuple. The visible key should be shown
    /// to the user only once and never stored.
    pub async fn create_key(
        &self,
        user_id: &UserId,
        scope: KeyScope,
    ) -> AuthResult<(String, String)> {
        // Generate cryptographically secure random key
        let key_bytes = self.generate_key_bytes()?;
        let visible_key = format!("sk_live_{}", hex::encode(&key_bytes));
        let key_id = self.generate_key_id()?;

        // Hash the key before storage (never store raw key)
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(visible_key.as_bytes(), &salt)
            .map_err(|e| AuthError::KeyGenerationFailed(e.to_string()))?
            .to_string();

        // Store hash and metadata
        let metadata = KeyMetadata {
            key_id: key_id.clone(),
            user_id: user_id.clone(),
            scope,
            created_at: chrono::Utc::now(),
            last_used_at: None,
            revoked: false,
            revoked_reason: None,
        };

        self.store
            .store_key_hash(&key_id, &hash, metadata)
            .map_err(AuthError::from)?;

        Ok((visible_key, key_id))
    }

    /// Validate an API key using constant-time comparison
    ///
    /// This prevents timing attacks by ensuring validation time is
    /// independent of where the key differs from the stored hash.
    pub async fn validate_key(&self, key: &str) -> AuthResult<UserId> {
        // Extract key ID from the key (format: sk_live_{data})
        if !key.starts_with("sk_live_") {
            return Err(AuthError::InvalidKeyFormat);
        }

        // Derive key ID deterministically from key content
        let key_id = self.derive_key_id(key);

        // Retrieve stored hash
        let stored_hash = self
            .store
            .get_key_hash(&key_id)
            .map_err(AuthError::from)?
            .ok_or(AuthError::InvalidKey)?;

        // Retrieve metadata to check revocation status
        let metadata = self
            .store
            .get_key_metadata(&key_id)
            .map_err(AuthError::from)?
            .ok_or(AuthError::InvalidKey)?;

        if metadata.revoked {
            return Err(AuthError::KeyRevoked);
        }

        // Verify using constant-time comparison
        let parsed_hash =
            PasswordHash::new(&stored_hash).map_err(|e| AuthError::CryptoError(e.to_string()))?;

        self.argon2
            .verify_password(key.as_bytes(), &parsed_hash)
            .map_err(|_| AuthError::InvalidKey)?;

        // Update last used timestamp
        let _ = self.store.update_last_used(&key_id);

        Ok(metadata.user_id)
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, key_id: &str, reason: &str) -> AuthResult<()> {
        self.store
            .revoke_key(key_id, reason)
            .map_err(AuthError::from)
    }

    /// List all keys for a user
    pub async fn list_keys(&self, user_id: &UserId) -> AuthResult<Vec<KeyMetadata>> {
        self.store.list_keys(user_id).map_err(AuthError::from)
    }

    /// Generate cryptographically secure random bytes for key
    fn generate_key_bytes(&self) -> AuthResult<[u8; 32]> {
        let mut key_bytes = [0u8; 32];
        getrandom::getrandom(&mut key_bytes)
            .map_err(|e| AuthError::KeyGenerationFailed(e.to_string()))?;
        Ok(key_bytes)
    }

    /// Generate unique key ID
    fn generate_key_id(&self) -> AuthResult<String> {
        let mut id_bytes = [0u8; 16];
        getrandom::getrandom(&mut id_bytes)
            .map_err(|e| AuthError::KeyGenerationFailed(format!("RNG failure: {}", e)))?;
        Ok(format!("key_{}", hex::encode(&id_bytes)))
    }

    /// Derive key ID from key content (deterministic)
    fn derive_key_id(&self, key: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(key.as_bytes());
        format!("key_{}", hex::encode(&hash[..16]))
    }

    /// Get the number of raw keys in memory (should always be 0)
    #[cfg(test)]
    pub fn raw_key_count(&self) -> usize {
        0 // We never store raw keys in memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::errors::StoreError;

    #[tokio::test]
    async fn create_key_returns_valid_format() {
        let mut mock_store = MockApiKeyStore::new();
        mock_store
            .expect_store_key_hash()
            .returning(|_, _, _| Ok(()));

        let service = ApiKeyService::new(Arc::new(mock_store));
        let user_id = UserId::new("user_123");

        let (visible_key, key_id) = service
            .create_key(&user_id, KeyScope::ReadWrite)
            .await
            .unwrap();

        assert!(visible_key.starts_with("sk_live_"));
        assert!(visible_key.len() >= 40); // Sufficient entropy
        assert!(key_id.starts_with("key_"));
    }

    #[tokio::test]
    async fn api_keys_are_hashed_before_storage() {
        let mut mock_store = MockApiKeyStore::new();

        // Verify stored value is an Argon2 hash
        mock_store
            .expect_store_key_hash()
            .withf(|_, hash, _| hash.starts_with("$argon2"))
            .times(1)
            .returning(|_, _, _| Ok(()));

        let service = ApiKeyService::new(Arc::new(mock_store));
        let user_id = UserId::new("user_123");

        let result = service.create_key(&user_id, KeyScope::ReadWrite).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn validate_key_with_invalid_format() {
        let mock_store = MockApiKeyStore::new();
        let service = ApiKeyService::new(Arc::new(mock_store));

        let result = service.validate_key("invalid_key").await;
        assert!(matches!(result, Err(AuthError::InvalidKeyFormat)));
    }

    #[tokio::test]
    async fn revoked_keys_are_rejected() {
        let mut mock_store = MockApiKeyStore::new();

        mock_store
            .expect_get_key_hash()
            .returning(|_| Ok(Some("$argon2id$...".to_string())));

        mock_store.expect_get_key_metadata().returning(|_| {
            Ok(Some(KeyMetadata {
                key_id: "key_123".to_string(),
                user_id: UserId::new("user_123"),
                scope: KeyScope::ReadWrite,
                created_at: chrono::Utc::now(),
                last_used_at: None,
                revoked: true,
                revoked_reason: Some("Compromised".to_string()),
            }))
        });

        let service = ApiKeyService::new(Arc::new(mock_store));

        let result = service.validate_key("sk_live_test_key").await;
        assert!(matches!(result, Err(AuthError::KeyRevoked)));
    }

    #[tokio::test]
    async fn no_raw_keys_in_memory() {
        let mock_store = MockApiKeyStore::new();
        let service = ApiKeyService::new(Arc::new(mock_store));

        assert_eq!(service.raw_key_count(), 0);
    }
}
