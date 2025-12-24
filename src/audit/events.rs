// Audit event types and structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::{UserId, ResourceId, SessionId};

/// Types of audit events tracked in the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// PHI (Protected Health Information) data access for HIPAA compliance
    PhiAccess,

    /// Authentication events (login, logout, token refresh)
    AuthEvent,

    /// API endpoint access
    ApiCall,

    /// System configuration changes
    ConfigChange,

    /// Security-related events (failed auth, suspicious activity)
    SecurityEvent,
}

/// Outcome of an audit event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
}

/// Core audit event structure with all required fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event timestamp
    pub timestamp: DateTime<Utc>,

    /// User who initiated the event
    pub user_id: Option<UserId>,

    /// IP address of the request
    pub ip_address: Option<IpAddr>,

    /// Session identifier
    pub session_id: Option<SessionId>,

    /// Type of event
    pub event_type: AuditEventType,

    /// Resource being accessed/modified
    pub resource_id: Option<ResourceId>,

    /// Action performed
    pub action: String,

    /// Outcome of the action
    pub outcome: AuditOutcome,

    /// Chain hash - SHA-256 of previous event for tamper evidence
    pub chain_hash: Option<String>,

    /// Additional event-specific metadata
    pub metadata: Option<serde_json::Value>,
}

impl AuditEvent {
    /// Create a new audit event with current timestamp
    pub fn new(
        user_id: Option<UserId>,
        ip_address: Option<IpAddr>,
        session_id: Option<SessionId>,
        event_type: AuditEventType,
        resource_id: Option<ResourceId>,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            user_id,
            ip_address,
            session_id,
            event_type,
            resource_id,
            action: action.into(),
            outcome,
            chain_hash: None,
            metadata: None,
        }
    }

    /// Create a PHI access event
    pub fn phi_access(
        user_id: UserId,
        ip_address: IpAddr,
        session_id: SessionId,
        resource_id: ResourceId,
        outcome: AuditOutcome,
    ) -> Self {
        Self::new(
            Some(user_id),
            Some(ip_address),
            Some(session_id),
            AuditEventType::PhiAccess,
            Some(resource_id),
            "access_phi_record",
            outcome,
        )
    }

    /// Create an authentication event
    pub fn auth_event(
        user_id: Option<UserId>,
        ip_address: IpAddr,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self::new(
            user_id,
            Some(ip_address),
            None,
            AuditEventType::AuthEvent,
            None,
            action,
            outcome,
        )
    }

    /// Create an API call event
    pub fn api_call(
        user_id: Option<UserId>,
        ip_address: Option<IpAddr>,
        session_id: Option<SessionId>,
        endpoint: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        let endpoint = endpoint.into();
        Self::new(
            user_id,
            ip_address,
            session_id,
            AuditEventType::ApiCall,
            Some(ResourceId::new(endpoint.clone())),
            endpoint,
            outcome,
        )
    }

    /// Create a configuration change event
    pub fn config_change(
        user_id: UserId,
        config_key: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        let key = config_key.into();
        Self::new(
            Some(user_id),
            None,
            None,
            AuditEventType::ConfigChange,
            Some(ResourceId::new(key.clone())),
            format!("update_config_{}", key),
            outcome,
        )
    }

    /// Create a security event
    pub fn security_event(
        user_id: Option<UserId>,
        ip_address: Option<IpAddr>,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self::new(
            user_id,
            ip_address,
            None,
            AuditEventType::SecurityEvent,
            None,
            action,
            outcome,
        )
    }

    /// Generic event creation
    pub fn generic(action: impl Into<String>) -> Self {
        Self::new(
            None,
            None,
            None,
            AuditEventType::ApiCall,
            None,
            action,
            AuditOutcome::Success,
        )
    }

    /// Set the chain hash for tamper evidence
    pub fn with_chain_hash(mut self, hash: String) -> Self {
        self.chain_hash = Some(hash);
        self
    }

    /// Set additional metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Calculate the hash of this event for chaining
    pub fn calculate_hash(&self) -> String {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();

        // Hash all immutable fields
        hasher.update(self.timestamp.to_rfc3339().as_bytes());

        if let Some(ref user_id) = self.user_id {
            hasher.update(user_id.as_str().as_bytes());
        }

        if let Some(ref ip) = self.ip_address {
            hasher.update(ip.to_string().as_bytes());
        }

        if let Some(ref session_id) = self.session_id {
            hasher.update(session_id.as_str().as_bytes());
        }

        hasher.update(format!("{:?}", self.event_type).as_bytes());

        if let Some(ref resource_id) = self.resource_id {
            hasher.update(resource_id.as_str().as_bytes());
        }

        hasher.update(self.action.as_bytes());
        hasher.update(format!("{:?}", self.outcome).as_bytes());

        if let Some(ref prev_hash) = self.chain_hash {
            hasher.update(prev_hash.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::phi_access(
            UserId::new("user_123"),
            "192.168.1.1".parse().unwrap(),
            SessionId::new("sess_abc"),
            ResourceId::new("patient_456"),
            AuditOutcome::Success,
        );

        assert_eq!(event.event_type, AuditEventType::PhiAccess);
        assert_eq!(event.outcome, AuditOutcome::Success);
        assert!(event.user_id.is_some());
        assert!(event.ip_address.is_some());
    }

    #[test]
    fn test_hash_calculation() {
        let event = AuditEvent::generic("test_action");
        let hash1 = event.calculate_hash();
        let hash2 = event.calculate_hash();

        // Hash should be deterministic
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn test_chain_hash_affects_calculation() {
        let mut event1 = AuditEvent::generic("test");
        let hash1 = event1.calculate_hash();

        event1.chain_hash = Some("previous_hash".to_string());
        let hash2 = event1.calculate_hash();

        // Adding chain hash should change the calculated hash
        assert_ne!(hash1, hash2);
    }
}
