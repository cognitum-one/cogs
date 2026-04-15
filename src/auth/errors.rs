/// Error types for authentication and authorization
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid API key format")]
    InvalidKeyFormat,

    #[error("API key has been revoked")]
    KeyRevoked,

    #[error("API key not found or invalid")]
    InvalidKey,

    #[error("Token has expired")]
    TokenExpired,

    #[error("Invalid token signature")]
    InvalidSignature,

    #[error("Token replay attack detected")]
    TokenReplayDetected,

    #[error("Malformed token")]
    MalformedToken,

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Hash verification failed")]
    HashVerificationFailed,

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Token creation failed: {0}")]
    TokenCreationFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for authentication operations
pub type AuthResult<T> = Result<T, AuthError>;

/// Store errors for persistence layer
#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Item not found")]
    NotFound,

    #[error("Database connection error: {0}")]
    ConnectionError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Internal store error: {0}")]
    Internal(String),
}

/// Result type alias for store operations
pub type StoreResult<T> = Result<T, StoreError>;

impl From<StoreError> for AuthError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound => AuthError::InvalidKey,
            StoreError::ConnectionError(msg) => AuthError::DatabaseError(msg),
            StoreError::SerializationError(msg) => AuthError::DatabaseError(msg),
            StoreError::Internal(msg) => AuthError::Internal(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_error_display() {
        let err = AuthError::KeyRevoked;
        assert_eq!(err.to_string(), "API key has been revoked");

        let err = AuthError::TokenExpired;
        assert_eq!(err.to_string(), "Token has expired");
    }

    #[test]
    fn store_error_to_auth_error() {
        let store_err = StoreError::NotFound;
        let auth_err: AuthError = store_err.into();
        assert!(matches!(auth_err, AuthError::InvalidKey));

        let store_err = StoreError::ConnectionError("Connection timeout".to_string());
        let auth_err: AuthError = store_err.into();
        assert!(matches!(auth_err, AuthError::DatabaseError(_)));
    }
}
