//! Error types for cryptographic operations

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Authentication failed - data has been tampered with")]
    AuthenticationFailed,

    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[error("Signature generation failed: {0}")]
    SignatureFailed(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Password hashing failed: {0}")]
    HashingFailed(String),

    #[error("Password verification failed")]
    VerificationFailed,

    #[error("Random number generation failed: {0}")]
    RandomGenerationFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Error, Debug)]
pub enum HsmError {
    #[error("HSM connection timeout")]
    ConnectionTimeout,

    #[error("HSM connection failed: {0}")]
    ConnectionFailed(String),

    #[error("HSM operation failed: {0}")]
    OperationFailed(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Invalid key purpose: {0}")]
    InvalidKeyPurpose(String),

    #[error("HSM internal error: {0}")]
    InternalError(String),
}

#[derive(Error, Debug)]
pub enum KmsError {
    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,

    #[error("HSM error: {0}")]
    HsmError(#[from] HsmError),

    #[error("Crypto error: {0}")]
    CryptoError(#[from] CryptoError),

    #[error("Key rotation failed: {0}")]
    RotationFailed(String),

    #[error("Invalid key ID: {0}")]
    InvalidKeyId(String),

    #[error("Operation timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    InternalError(String),
}

// Implement From conversions for better error handling
impl From<aes_gcm::Error> for CryptoError {
    fn from(_err: aes_gcm::Error) -> Self {
        CryptoError::AuthenticationFailed
    }
}

impl From<ed25519_dalek::SignatureError> for CryptoError {
    fn from(_err: ed25519_dalek::SignatureError) -> Self {
        CryptoError::SignatureVerificationFailed
    }
}

impl From<argon2::Error> for CryptoError {
    fn from(err: argon2::Error) -> Self {
        CryptoError::HashingFailed(err.to_string())
    }
}
