// Audit logging module for Cognitum chip v1 commercialization
// Implements HIPAA-compliant tamper-evident audit logging

pub mod events;
pub mod logger;
pub mod store;
pub mod query;

pub use events::{AuditEvent, AuditEventType, AuditOutcome};
pub use logger::AuditLogger;
pub use store::{AuditStore, AuditId};
pub use query::{AuditFilter, AuditQuery};

// Re-export common types
pub use chrono::{DateTime, Utc};
pub use std::net::IpAddr;

/// User ID type for audit events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resource ID type for audit events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(String);

impl ResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Session ID type for audit events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Request context for audit logging
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: Option<UserId>,
    pub ip_address: Option<IpAddr>,
    pub session_id: Option<SessionId>,
}

/// User role enumeration for RBAC
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    SecurityAdmin,
    Developer,
    LabTech,
    Free,
}

impl UserRole {
    pub fn can_query_audit_logs(&self) -> bool {
        matches!(self, UserRole::SecurityAdmin)
    }
}

/// User type for authorization checks
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub roles: Vec<String>,
}

impl User {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn is_security_admin(&self) -> bool {
        self.has_role("security_admin")
    }
}

/// Common error types for audit module
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Unauthorized access to audit logs")]
    Unauthorized,

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("Invalid audit filter: {0}")]
    InvalidFilter(String),

    #[error("Chain integrity violation detected")]
    ChainIntegrityViolation,

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, AuditError>;
