//! Error types for Agentic VM

use alloc::string::String;
use core::fmt;

/// Result type alias
pub type Result<T> = core::result::Result<T, AgentVmError>;

/// Main error type for Agentic VM operations
#[derive(Debug, Clone)]
pub enum AgentVmError {
    /// Capability-related errors
    Capability(CapabilityError),
    /// Budget-related errors
    Budget(BudgetError),
    /// Evidence-related errors
    Evidence(EvidenceError),
    /// Scheduling errors
    Scheduling(SchedulingError),
    /// Protocol errors
    Protocol(ProtocolError),
    /// Generic error with message
    Other(String),
}

impl fmt::Display for AgentVmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(e) => write!(f, "capability error: {}", e),
            Self::Budget(e) => write!(f, "budget error: {}", e),
            Self::Evidence(e) => write!(f, "evidence error: {}", e),
            Self::Scheduling(e) => write!(f, "scheduling error: {}", e),
            Self::Protocol(e) => write!(f, "protocol error: {}", e),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AgentVmError {}

/// Capability-specific errors
#[derive(Debug, Clone)]
pub enum CapabilityError {
    /// Capability not found
    NotFound,
    /// Capability has expired
    Expired,
    /// Capability has been revoked
    Revoked,
    /// Quota exhausted
    QuotaExhausted,
    /// Scope violation
    ScopeViolation(String),
    /// Invalid signature
    InvalidSignature,
    /// Cannot delegate (no delegate right)
    NoDelegateRight,
    /// Cannot amplify rights
    AmplificationDenied,
    /// Invalid derivation
    InvalidDerivation(String),
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "capability not found"),
            Self::Expired => write!(f, "capability expired"),
            Self::Revoked => write!(f, "capability revoked"),
            Self::QuotaExhausted => write!(f, "quota exhausted"),
            Self::ScopeViolation(msg) => write!(f, "scope violation: {}", msg),
            Self::InvalidSignature => write!(f, "invalid signature"),
            Self::NoDelegateRight => write!(f, "no delegate right"),
            Self::AmplificationDenied => write!(f, "amplification denied"),
            Self::InvalidDerivation(msg) => write!(f, "invalid derivation: {}", msg),
        }
    }
}

/// Budget-specific errors
#[derive(Debug, Clone)]
pub enum BudgetError {
    /// CPU budget exceeded
    CpuExceeded,
    /// Memory budget exceeded
    MemoryExceeded,
    /// Disk budget exceeded
    DiskExceeded,
    /// Network budget exceeded
    NetworkExceeded,
    /// Generic budget exceeded
    Exceeded(String),
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CpuExceeded => write!(f, "CPU budget exceeded"),
            Self::MemoryExceeded => write!(f, "memory budget exceeded"),
            Self::DiskExceeded => write!(f, "disk budget exceeded"),
            Self::NetworkExceeded => write!(f, "network budget exceeded"),
            Self::Exceeded(msg) => write!(f, "budget exceeded: {}", msg),
        }
    }
}

/// Evidence-specific errors
#[derive(Debug, Clone)]
pub enum EvidenceError {
    /// Merkle proof verification failed
    InvalidMerkleProof,
    /// Signature verification failed
    InvalidSignature,
    /// Bundle format error
    InvalidFormat(String),
    /// Missing required field
    MissingField(String),
    /// Storage error
    Storage(String),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMerkleProof => write!(f, "invalid Merkle proof"),
            Self::InvalidSignature => write!(f, "invalid signature"),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {}", msg),
            Self::MissingField(field) => write!(f, "missing field: {}", field),
            Self::Storage(msg) => write!(f, "storage error: {}", msg),
        }
    }
}

/// Scheduling-specific errors
#[derive(Debug, Clone)]
pub enum SchedulingError {
    /// No feasible nodes for task
    NoFeasibleNodes,
    /// Node not found
    NodeNotFound,
    /// Resource requirements not met
    ResourcesUnavailable,
    /// Preemption failed
    PreemptionFailed,
    /// Timeout waiting for placement
    Timeout,
}

impl fmt::Display for SchedulingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFeasibleNodes => write!(f, "no feasible nodes"),
            Self::NodeNotFound => write!(f, "node not found"),
            Self::ResourcesUnavailable => write!(f, "resources unavailable"),
            Self::PreemptionFailed => write!(f, "preemption failed"),
            Self::Timeout => write!(f, "scheduling timeout"),
        }
    }
}

/// Protocol-specific errors
#[derive(Debug, Clone)]
pub enum ProtocolError {
    /// Invalid magic bytes
    InvalidMagic,
    /// Unsupported version
    UnsupportedVersion(u16),
    /// Buffer too small
    BufferTooSmall,
    /// Invalid message type
    InvalidMessageType(u16),
    /// Checksum mismatch
    ChecksumMismatch,
    /// Parse error
    ParseError(String),
    /// Serialization error
    SerializeError(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "invalid magic bytes"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::InvalidMessageType(t) => write!(f, "invalid message type: {}", t),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::ParseError(msg) => write!(f, "parse error: {}", msg),
            Self::SerializeError(msg) => write!(f, "serialize error: {}", msg),
        }
    }
}

impl From<CapabilityError> for AgentVmError {
    fn from(e: CapabilityError) -> Self {
        Self::Capability(e)
    }
}

impl From<BudgetError> for AgentVmError {
    fn from(e: BudgetError) -> Self {
        Self::Budget(e)
    }
}

impl From<EvidenceError> for AgentVmError {
    fn from(e: EvidenceError) -> Self {
        Self::Evidence(e)
    }
}

impl From<SchedulingError> for AgentVmError {
    fn from(e: SchedulingError) -> Self {
        Self::Scheduling(e)
    }
}

impl From<ProtocolError> for AgentVmError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}
