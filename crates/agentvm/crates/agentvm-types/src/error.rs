//! Error types for Agentic VM

use alloc::string::String;
use core::fmt;

/// Result type alias for AgentVM operations
pub type Result<T> = core::result::Result<T, AgentVmError>;

/// Main error type for Agentic VM
#[derive(Debug, Clone)]
pub enum AgentVmError {
    // Capability errors
    /// Capability not found
    CapabilityNotFound,
    /// Capability expired
    CapabilityExpired,
    /// Capability revoked
    CapabilityRevoked,
    /// Capability quota exhausted
    QuotaExhausted,
    /// Scope violation (operation not permitted)
    ScopeViolation(String),
    /// Invalid signature
    InvalidSignature,
    /// Cannot derive capability (would amplify rights)
    DerivationDenied(String),

    // Budget errors
    /// Budget exhausted
    BudgetExhausted,
    /// Insufficient budget for operation
    InsufficientBudget,

    // Execution errors
    /// Execution timeout
    Timeout,
    /// Execution killed (by kill switch)
    Killed,
    /// Execution failed
    ExecutionFailed(String),

    // Evidence errors
    /// Evidence verification failed
    EvidenceVerificationFailed(String),
    /// Merkle proof invalid
    MerkleProofInvalid,
    /// Chain integrity violation
    ChainIntegrityViolation,

    // Scheduling errors
    /// No feasible node for scheduling
    NoFeasibleNode,
    /// Node not found
    NodeNotFound,
    /// Task not found
    TaskNotFound,

    // Protocol errors
    /// Invalid message format
    InvalidMessage(String),
    /// Protocol version mismatch
    ProtocolVersionMismatch,
    /// Checksum mismatch
    ChecksumMismatch,

    // Configuration errors
    /// Invalid configuration
    InvalidConfig(String),
    /// Missing required field
    MissingField(String),

    // I/O errors
    /// I/O error
    Io(String),
    /// Network error
    Network(String),
    /// Filesystem error
    Filesystem(String),

    // Serialization errors
    /// Serialization failed
    Serialization(String),
    /// Deserialization failed
    Deserialization(String),

    // Generic errors
    /// Internal error
    Internal(String),
    /// Not implemented
    NotImplemented,
}

impl fmt::Display for AgentVmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityNotFound => write!(f, "capability not found"),
            Self::CapabilityExpired => write!(f, "capability expired"),
            Self::CapabilityRevoked => write!(f, "capability revoked"),
            Self::QuotaExhausted => write!(f, "quota exhausted"),
            Self::ScopeViolation(msg) => write!(f, "scope violation: {}", msg),
            Self::InvalidSignature => write!(f, "invalid signature"),
            Self::DerivationDenied(msg) => write!(f, "derivation denied: {}", msg),
            Self::BudgetExhausted => write!(f, "budget exhausted"),
            Self::InsufficientBudget => write!(f, "insufficient budget"),
            Self::Timeout => write!(f, "execution timeout"),
            Self::Killed => write!(f, "execution killed"),
            Self::ExecutionFailed(msg) => write!(f, "execution failed: {}", msg),
            Self::EvidenceVerificationFailed(msg) => write!(f, "evidence verification failed: {}", msg),
            Self::MerkleProofInvalid => write!(f, "merkle proof invalid"),
            Self::ChainIntegrityViolation => write!(f, "chain integrity violation"),
            Self::NoFeasibleNode => write!(f, "no feasible node for scheduling"),
            Self::NodeNotFound => write!(f, "node not found"),
            Self::TaskNotFound => write!(f, "task not found"),
            Self::InvalidMessage(msg) => write!(f, "invalid message: {}", msg),
            Self::ProtocolVersionMismatch => write!(f, "protocol version mismatch"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::InvalidConfig(msg) => write!(f, "invalid configuration: {}", msg),
            Self::MissingField(field) => write!(f, "missing required field: {}", field),
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
            Self::Network(msg) => write!(f, "network error: {}", msg),
            Self::Filesystem(msg) => write!(f, "filesystem error: {}", msg),
            Self::Serialization(msg) => write!(f, "serialization error: {}", msg),
            Self::Deserialization(msg) => write!(f, "deserialization error: {}", msg),
            Self::Internal(msg) => write!(f, "internal error: {}", msg),
            Self::NotImplemented => write!(f, "not implemented"),
        }
    }
}

// Implement core::error::Error when std feature is enabled
#[cfg(feature = "std")]
impl std::error::Error for AgentVmError {}
