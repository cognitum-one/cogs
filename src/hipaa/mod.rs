//! HIPAA Compliance Module for Cognitum Chip v1
//!
//! Provides comprehensive HIPAA compliance controls including:
//! - PHI encryption at rest (AES-256-GCM)
//! - Minimum necessary access control
//! - Session management with automatic timeouts
//! - BAA workflow for enterprise customers
//! - Audit logging for all PHI access
//!
//! ## Security Requirements
//!
//! - §164.312(a)(1) Access Control - Role-based access control
//! - §164.312(a)(2)(i) Unique User ID - User identification
//! - §164.312(a)(2)(iii) Auto Logoff - Session timeout
//! - §164.312(b) Audit Controls - Comprehensive logging
//! - §164.312(c)(1) Integrity - Data integrity checks
//! - §164.312(d) Authentication - Multi-factor auth
//! - §164.312(e)(2)(ii) Encryption - AES-256 at rest

pub mod access;
pub mod onboarding;
pub mod session;
pub mod storage;

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// Re-exports
pub use access::{AccessControl, MinimumNecessaryAccess, Role};
pub use onboarding::{BaaWorkflow, CustomerOnboarding};
pub use session::{HipaaSessionManager, SessionConfig};
pub use storage::{EncryptionConfig, HipaaCompliantStorage, KeySource, StorageConfig};

/// HIPAA-specific error types
#[derive(Debug, Error)]
pub enum HipaaError {
    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Session expired or invalid")]
    InvalidSession,

    #[error("BAA (Business Associate Agreement) required for PHI access")]
    BaaRequired,

    #[error("Insufficient permissions for minimum necessary access")]
    InsufficientPermissions,

    #[error("Audit logging failed: {0}")]
    AuditFailed(String),

    #[error("Key management error: {0}")]
    KeyManagement(String),

    #[error("Data integrity verification failed")]
    IntegrityCheckFailed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, HipaaError>;

/// Record identifier for PHI data
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId(String);

impl RecordId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Customer identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomerId(String);

impl CustomerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CustomerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Protected Health Information (PHI) record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhiRecord {
    pub patient_id: String,
    pub dna_sequence: String,
    pub analysis_results: Vec<AnalysisResult>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Analysis result data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub test_name: String,
    pub result_value: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Filtered PHI view based on minimum necessary access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhiView {
    pub patient_id: Option<String>,
    pub dna_sequence: Option<String>,
    pub analysis_results: Option<Vec<AnalysisResult>>,
}

/// User with roles for RBAC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub roles: Vec<String>,
}

impl User {
    pub fn new(id: impl Into<String>, roles: Vec<String>) -> Self {
        Self {
            id: UserId::new(id),
            roles,
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// Customer tier for licensing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    Free,
    Professional,
    Enterprise,
}

/// Customer entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: CustomerId,
    pub tier: Tier,
    pub baa_signed: bool,
    pub baa_signed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_id_creation() {
        let id = RecordId::new("record_123");
        assert_eq!(id.as_str(), "record_123");
    }

    #[test]
    fn test_user_has_role() {
        let user = User::new("user_1", vec!["lab_tech".to_string(), "analyst".to_string()]);
        assert!(user.has_role("lab_tech"));
        assert!(user.has_role("analyst"));
        assert!(!user.has_role("admin"));
    }
}
