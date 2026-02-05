//! Capability validation

use alloc::string::String;
use alloc::vec::Vec;
use agentvm_types::{Capability, CapabilityScope, CapabilityType};

/// Result of capability validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResult {
    /// Capability is valid for the operation
    Valid,
    /// Capability has expired
    Expired,
    /// Capability has been revoked
    Revoked,
    /// Quota has been exhausted
    QuotaExhausted,
    /// Operation violates capability scope
    ScopeViolation,
    /// Signature verification failed
    InvalidSignature,
}

impl ValidationResult {
    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Convert to Result type
    pub fn to_result(self) -> Result<(), ValidationError> {
        match self {
            Self::Valid => Ok(()),
            Self::Expired => Err(ValidationError::Expired),
            Self::Revoked => Err(ValidationError::Revoked),
            Self::QuotaExhausted => Err(ValidationError::QuotaExhausted),
            Self::ScopeViolation => Err(ValidationError::ScopeViolation),
            Self::InvalidSignature => Err(ValidationError::InvalidSignature),
        }
    }
}

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    Expired,
    Revoked,
    QuotaExhausted,
    ScopeViolation,
    InvalidSignature,
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Expired => write!(f, "capability expired"),
            Self::Revoked => write!(f, "capability revoked"),
            Self::QuotaExhausted => write!(f, "quota exhausted"),
            Self::ScopeViolation => write!(f, "scope violation"),
            Self::InvalidSignature => write!(f, "invalid signature"),
        }
    }
}

/// Operation to be performed with a capability
#[derive(Debug, Clone)]
pub enum Operation {
    /// HTTP request
    HttpRequest {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    },

    /// TCP connection
    TcpConnect { host: String, port: u16 },

    /// DNS resolution
    DnsResolve { name: String },

    /// File read
    FileRead { path: String, offset: u64, len: u64 },

    /// File write
    FileWrite {
        path: String,
        offset: u64,
        data: Vec<u8>,
    },

    /// File delete
    FileDelete { path: String },

    /// Directory listing
    DirectoryList { path: String },

    /// Process spawn
    ProcessSpawn {
        executable: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },

    /// Process signal
    ProcessSignal { pid: u32, signal: i32 },

    /// Secret read
    SecretRead { name: String },
}

impl Operation {
    /// Get the target resource for scope checking
    pub fn target(&self) -> &str {
        match self {
            Self::HttpRequest { url, .. } => url,
            Self::TcpConnect { host, .. } => host,
            Self::DnsResolve { name } => name,
            Self::FileRead { path, .. } => path,
            Self::FileWrite { path, .. } => path,
            Self::FileDelete { path } => path,
            Self::DirectoryList { path } => path,
            Self::ProcessSpawn { executable, .. } => executable,
            Self::ProcessSignal { .. } => "",
            Self::SecretRead { name } => name,
        }
    }

    /// Get the required capability type for this operation
    pub fn required_capability_type(&self) -> CapabilityType {
        match self {
            Self::HttpRequest { .. } => CapabilityType::NetworkHttp,
            Self::TcpConnect { .. } => CapabilityType::NetworkTcp,
            Self::DnsResolve { .. } => CapabilityType::NetworkDns,
            Self::FileRead { .. } => CapabilityType::FileRead,
            Self::FileWrite { .. } => CapabilityType::FileWrite,
            Self::FileDelete { .. } => CapabilityType::FileDelete,
            Self::DirectoryList { .. } => CapabilityType::DirectoryList,
            Self::ProcessSpawn { .. } => CapabilityType::ProcessSpawn,
            Self::ProcessSignal { .. } => CapabilityType::ProcessSignal,
            Self::SecretRead { .. } => CapabilityType::SecretRead,
        }
    }

    /// Estimate bytes this operation will transfer
    pub fn estimated_bytes(&self) -> u64 {
        match self {
            Self::HttpRequest { body, .. } => body.as_ref().map(|b| b.len()).unwrap_or(0) as u64,
            Self::FileRead { len, .. } => *len,
            Self::FileWrite { data, .. } => data.len() as u64,
            _ => 0,
        }
    }
}

/// Validate that a capability can perform an operation
pub fn validate_operation(
    cap: &Capability,
    operation: &Operation,
    current_time: u64,
) -> ValidationResult {
    // Check capability type matches operation
    if cap.cap_type != operation.required_capability_type() {
        return ValidationResult::ScopeViolation;
    }

    // Delegate to main validation
    crate::validate_capability(cap, operation, current_time)
}

/// Batch validate multiple operations
pub fn validate_operations(
    cap: &Capability,
    operations: &[Operation],
    current_time: u64,
) -> Vec<ValidationResult> {
    operations
        .iter()
        .map(|op| validate_operation(cap, op, current_time))
        .collect()
}
