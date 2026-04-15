//! Error types for the capability proxy.

use crate::types::ValidationResult;
use thiserror::Error;

/// Top-level proxy error
#[derive(Debug, Error)]
pub enum ProxyError {
    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// Evidence logging error
    #[error("evidence error: {0}")]
    Evidence(String),

    /// Capability error
    #[error("capability error: {0}")]
    Capability(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

/// Error during capability invocation
#[derive(Debug, Error)]
pub enum InvokeError {
    /// Capability not found
    #[error("capability not found")]
    CapabilityNotFound,

    /// Capability validation failed
    #[error("validation failed: {0}")]
    ValidationFailed(ValidationResult),

    /// Operation not supported
    #[error("unsupported operation")]
    UnsupportedOperation,

    /// Execution failed
    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    /// Budget deduction failed
    #[error("budget error: {0}")]
    BudgetError(#[from] BudgetError),

    /// Evidence logging failed
    #[error("evidence error: {0}")]
    EvidenceError(String),

    /// Timeout
    #[error("operation timed out")]
    Timeout,

    /// Operation was cancelled
    #[error("operation cancelled")]
    Cancelled,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Error during capability grant
#[derive(Debug, Error)]
pub enum GrantError {
    /// Invalid grant parameters
    #[error("invalid grant: {0}")]
    InvalidGrant(String),

    /// Capsule not registered
    #[error("capsule not found")]
    CapsuleNotFound,

    /// Grant exceeds policy limits
    #[error("grant exceeds policy: {0}")]
    PolicyViolation(String),

    /// Signing failed
    #[error("signing failed: {0}")]
    SigningFailed(String),

    /// Evidence logging failed
    #[error("evidence error: {0}")]
    EvidenceError(String),
}

/// Error during capability revocation
#[derive(Debug, Error)]
pub enum RevokeError {
    /// Capability not found
    #[error("capability not found")]
    NotFound,

    /// Already revoked
    #[error("already revoked")]
    AlreadyRevoked,

    /// Evidence logging failed
    #[error("evidence error: {0}")]
    EvidenceError(String),
}

/// Error during budget operations
#[derive(Debug, Error)]
pub enum BudgetError {
    /// Capability not found
    #[error("capability not found")]
    CapabilityNotFound,

    /// Quota would be exceeded
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    /// Invalid deduction amount
    #[error("invalid deduction: {0}")]
    InvalidDeduction(String),
}

/// Error during wire protocol parsing
#[derive(Debug, Error)]
pub enum ParseError {
    /// Buffer too small
    #[error("buffer too small: need {needed}, have {have}")]
    BufferTooSmall {
        /// Bytes needed
        needed: usize,
        /// Bytes available
        have: usize,
    },

    /// Invalid magic number
    #[error("invalid magic: expected 0x{expected:08x}, got 0x{got:08x}")]
    InvalidMagic {
        /// Expected magic
        expected: u32,
        /// Actual magic
        got: u32,
    },

    /// Unsupported version
    #[error("unsupported version: {0}")]
    UnsupportedVersion(u16),

    /// Invalid message type
    #[error("invalid message type: 0x{0:04x}")]
    InvalidMessageType(u16),

    /// Checksum mismatch
    #[error("checksum mismatch: expected 0x{expected:08x}, got 0x{got:08x}")]
    ChecksumMismatch {
        /// Expected checksum
        expected: u32,
        /// Actual checksum
        got: u32,
    },

    /// Payload too large
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(u32),

    /// Invalid payload
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}

/// Error during vsock operations
#[derive(Debug, Error)]
pub enum VsockError {
    /// Connection failed
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// Listener error
    #[error("listener error: {0}")]
    ListenerError(String),

    /// Parse error
    #[error("parse error: {0}")]
    ParseError(#[from] ParseError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Connection closed
    #[error("connection closed")]
    ConnectionClosed,

    /// Timeout
    #[error("timeout")]
    Timeout,
}

/// Error from executor operations
#[derive(Debug, Error)]
pub enum ExecutorError {
    /// Operation not supported
    #[error("operation not supported: {0}")]
    NotSupported(String),

    /// Permission denied
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Resource not found
    #[error("not found: {0}")]
    NotFound(String),

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(String),

    /// Rate limited
    #[error("rate limited")]
    RateLimited,

    /// Timeout
    #[error("timeout")]
    Timeout,

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for ExecutorError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ExecutorError::Timeout
        } else if err.is_connect() {
            ExecutorError::Network(format!("connection failed: {}", err))
        } else {
            ExecutorError::Http(err.to_string())
        }
    }
}
