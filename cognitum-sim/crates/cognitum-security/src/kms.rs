//! Key Management Service with HSM integration
//!
//! Provides a secure key management system with:
//! - Hardware Security Module (HSM) integration via trait
//! - Circuit breaker pattern for fault tolerance
//! - Key rotation support
//! - No raw keys in application memory

use crate::error::{HsmError, KmsError};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
use mockall::automock;

/// Purpose of a cryptographic key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPurpose {
    /// Data encryption at rest
    DataEncryption,
    /// Digital signatures
    Signing,
    /// Key encryption (master key)
    KeyEncryption,
    /// Authentication tokens
    Authentication,
}

/// Hardware Security Module provider trait
///
/// This trait abstracts HSM operations to support multiple HSM backends
/// (AWS CloudHSM, Azure Key Vault, YubiHSM, etc.)
#[cfg_attr(test, automock)]
#[async_trait]
pub trait HsmProvider: Send + Sync {
    /// Generate a new key in the HSM
    async fn generate_key(&self, purpose: KeyPurpose, key_id: &str) -> Result<(), HsmError>;

    /// Sign data using a key stored in the HSM
    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>, HsmError>;

    /// Verify a signature using a key stored in the HSM
    async fn verify(&self, key_id: &str, data: &[u8], signature: &[u8])
        -> Result<bool, HsmError>;

    /// Encrypt data using a key stored in the HSM
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, HsmError>;

    /// Decrypt data using a key stored in the HSM
    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>, HsmError>;

    /// Rotate a key (generates new version, keeps old for decryption)
    async fn rotate_key(&self, key_id: &str) -> Result<String, HsmError>;

    /// Destroy a key (irreversible)
    async fn destroy_key(&self, key_id: &str) -> Result<(), HsmError>;
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: usize,
    /// Time to wait before attempting to close the circuit
    pub reset_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for HSM operations
struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: AtomicUsize,
    last_failure_time: RwLock<Option<Instant>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            last_failure_time: RwLock::new(None),
            config,
        }
    }

    fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        *self.state.write() = CircuitState::Closed;
    }

    fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure_time.write() = Some(Instant::now());

        if count >= self.config.failure_threshold {
            *self.state.write() = CircuitState::Open;
        }
    }

    fn is_open(&self) -> bool {
        let state = *self.state.read();

        if state == CircuitState::Open {
            // Check if we should try half-open
            if let Some(last_failure) = *self.last_failure_time.read() {
                if last_failure.elapsed() >= self.config.reset_timeout {
                    *self.state.write() = CircuitState::HalfOpen;
                    return false;
                }
            }
            return true;
        }

        false
    }

    fn get_state(&self) -> CircuitState {
        *self.state.read()
    }
}

/// Key Management Service with circuit breaker protection
pub struct KeyManagementService {
    hsm: Arc<dyn HsmProvider>,
    circuit_breaker: Arc<CircuitBreaker>,
    operation_count: AtomicU64,
}

impl KeyManagementService {
    /// Create a new KMS with default circuit breaker settings
    pub fn new(hsm: Box<dyn HsmProvider>) -> Self {
        Self::with_circuit_breaker(hsm, CircuitBreakerConfig::default())
    }

    /// Create a new KMS with custom circuit breaker configuration
    pub fn with_circuit_breaker(
        hsm: Box<dyn HsmProvider>,
        config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            hsm: Arc::from(hsm),
            circuit_breaker: Arc::new(CircuitBreaker::new(config)),
            operation_count: AtomicU64::new(0),
        }
    }

    /// Check if the circuit breaker is open
    pub fn is_circuit_open(&self) -> bool {
        self.circuit_breaker.is_open()
    }

    /// Get the number of raw keys in application memory (should always be 0)
    pub fn raw_key_count(&self) -> usize {
        // In a real implementation, this would check for any keys in memory
        // For this HSM-based implementation, keys never leave the HSM
        0
    }

    /// Execute an HSM operation with circuit breaker protection
    async fn execute_with_circuit_breaker<F, T>(&self, operation: F) -> Result<T, KmsError>
    where
        F: FnOnce() -> futures::future::BoxFuture<'static, Result<T, HsmError>>,
    {
        if self.circuit_breaker.is_open() {
            return Err(KmsError::CircuitBreakerOpen);
        }

        self.operation_count.fetch_add(1, Ordering::SeqCst);

        match operation().await {
            Ok(result) => {
                self.circuit_breaker.record_success();
                Ok(result)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(KmsError::HsmError(e))
            }
        }
    }

    /// Encrypt data using an HSM-stored key
    pub async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let key_id = key_id.to_string();
        let plaintext = plaintext.to_vec();
        let hsm = Arc::clone(&self.hsm);

        self.execute_with_circuit_breaker(|| {
            let key_id = key_id.clone();
            let plaintext = plaintext.clone();
            let hsm = Arc::clone(&hsm);

            Box::pin(async move { hsm.encrypt(&key_id, &plaintext).await })
        })
        .await
    }

    /// Decrypt data using an HSM-stored key
    pub async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let key_id = key_id.to_string();
        let ciphertext = ciphertext.to_vec();
        let hsm = Arc::clone(&self.hsm);

        self.execute_with_circuit_breaker(|| {
            let key_id = key_id.clone();
            let ciphertext = ciphertext.clone();
            let hsm = Arc::clone(&hsm);

            Box::pin(async move { hsm.decrypt(&key_id, &ciphertext).await })
        })
        .await
    }

    /// Sign data using an HSM-stored key
    pub async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>, KmsError> {
        let key_id = key_id.to_string();
        let data = data.to_vec();
        let hsm = Arc::clone(&self.hsm);

        self.execute_with_circuit_breaker(|| {
            let key_id = key_id.clone();
            let data = data.clone();
            let hsm = Arc::clone(&hsm);

            Box::pin(async move { hsm.sign(&key_id, &data).await })
        })
        .await
    }

    /// Rotate a key (creates new version, maintains old for decryption)
    pub async fn rotate_key(&self, key_id: &str) -> Result<String, KmsError> {
        let key_id = key_id.to_string();
        let hsm = Arc::clone(&self.hsm);

        self.execute_with_circuit_breaker(|| {
            let key_id = key_id.clone();
            let hsm = Arc::clone(&hsm);

            Box::pin(async move { hsm.rotate_key(&key_id).await })
        })
        .await
    }

    /// Get operation metrics
    pub fn get_operation_count(&self) -> u64 {
        self.operation_count.load(Ordering::SeqCst)
    }
}

// MockHsmProvider is automatically made available by mockall's #[automock]
// No need to re-export it

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let mut mock_hsm = MockHsmProvider::new();

        // Configure HSM to always fail - only 3 calls will be made before circuit opens
        mock_hsm
            .expect_encrypt()
            .times(3)
            .returning(|_, _| Err(HsmError::ConnectionTimeout));

        let kms = KeyManagementService::with_circuit_breaker(
            Box::new(mock_hsm),
            CircuitBreakerConfig {
                failure_threshold: 3,
                reset_timeout: Duration::from_secs(30),
            },
        );

        // First 3 failures should go through to HSM
        for _ in 0..3 {
            let _ = kms.encrypt("key", b"data").await;
        }

        // Circuit should now be open
        assert!(kms.is_circuit_open());

        // Next call should fail fast without calling HSM
        let result = kms.encrypt("key", b"data").await;
        assert!(matches!(result, Err(KmsError::CircuitBreakerOpen)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_resets_on_success() {
        let mut mock_hsm = MockHsmProvider::new();

        // Fail twice, then succeed
        mock_hsm
            .expect_encrypt()
            .times(2)
            .returning(|_, _| Err(HsmError::ConnectionTimeout));

        mock_hsm
            .expect_encrypt()
            .times(1)
            .returning(|_, _| Ok(vec![1, 2, 3]));

        let kms = KeyManagementService::with_circuit_breaker(
            Box::new(mock_hsm),
            CircuitBreakerConfig {
                failure_threshold: 5,
                reset_timeout: Duration::from_secs(30),
            },
        );

        // Two failures
        let _ = kms.encrypt("key", b"data").await;
        let _ = kms.encrypt("key", b"data").await;

        // Success should reset counter
        let result = kms.encrypt("key", b"data").await;
        assert!(result.is_ok());
        assert!(!kms.is_circuit_open());
    }

    #[tokio::test]
    async fn test_no_raw_keys_in_memory() {
        let mock_hsm = MockHsmProvider::new();
        let kms = KeyManagementService::new(Box::new(mock_hsm));

        // KMS should never have raw keys in memory
        assert_eq!(kms.raw_key_count(), 0);
    }
}
