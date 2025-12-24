//! Error types for Cognitum Ruvector SDK

use thiserror::Error;

/// Result type alias for SDK operations
pub type Result<T> = std::result::Result<T, RuvectorError>;

/// Comprehensive error types for Ruvector SDK
#[derive(Debug, Error)]
pub enum RuvectorError {
    /// Vector indexing errors
    #[error("Index error: {0}")]
    Index(#[from] cognitum::ruvector::IndexError),

    /// Neural routing errors
    #[error("Router error: {0}")]
    Router(#[from] cognitum::ruvector::RouterError),

    /// Configuration validation errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid dimension error
    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    /// Invalid embedding ID
    #[error("Invalid embedding ID: {0}")]
    InvalidId(u64),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Operation timeout
    #[error("Operation timeout after {0}ms")]
    Timeout(u64),

    /// Concurrent access error
    #[error("Concurrent access error: {0}")]
    Concurrency(String),

    /// Model not trained
    #[error("Model not trained: {0}")]
    ModelNotTrained(String),

    /// Invalid model state
    #[error("Invalid model state: {0}")]
    InvalidModelState(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Client not initialized
    #[error("Client not initialized: {0}")]
    NotInitialized(String),

    /// Operation not supported
    #[error("Operation not supported: {0}")]
    NotSupported(String),
}

impl RuvectorError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RuvectorError::Timeout(_) | RuvectorError::Concurrency(_)
        )
    }

    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            RuvectorError::Timeout(_)
                | RuvectorError::Concurrency(_)
                | RuvectorError::Io(_)
        )
    }

    /// Get error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            RuvectorError::Index(_) => "index",
            RuvectorError::Router(_) => "router",
            RuvectorError::Config(_) => "config",
            RuvectorError::InvalidDimension { .. } => "validation",
            RuvectorError::InvalidId(_) => "validation",
            RuvectorError::NotFound(_) => "not_found",
            RuvectorError::Timeout(_) => "timeout",
            RuvectorError::Concurrency(_) => "concurrency",
            RuvectorError::ModelNotTrained(_) => "model",
            RuvectorError::InvalidModelState(_) => "model",
            RuvectorError::Io(_) => "io",
            RuvectorError::Serialization(_) => "serialization",
            RuvectorError::Internal(_) => "internal",
            RuvectorError::NotInitialized(_) => "initialization",
            RuvectorError::NotSupported(_) => "not_supported",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        let timeout = RuvectorError::Timeout(1000);
        assert!(timeout.is_retryable());
        assert!(timeout.is_transient());

        let config = RuvectorError::Config("invalid".to_string());
        assert!(!config.is_retryable());
        assert!(!config.is_transient());
    }

    #[test]
    fn test_error_category() {
        let timeout = RuvectorError::Timeout(1000);
        assert_eq!(timeout.category(), "timeout");

        let config = RuvectorError::Config("test".to_string());
        assert_eq!(config.category(), "config");
    }

    #[test]
    fn test_error_display() {
        let err = RuvectorError::InvalidDimension {
            expected: 256,
            actual: 128,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("256"));
        assert!(msg.contains("128"));
    }
}
