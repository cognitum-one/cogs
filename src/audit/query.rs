// Audit query and filtering capabilities

use chrono::{DateTime, Utc};
use std::net::IpAddr;

use super::{
    AuditEvent, AuditEventType, AuditOutcome, AuditError, Result,
    AuditStore, UserId, ResourceId, User,
};

/// Filter criteria for querying audit events
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub user_id: Option<UserId>,
    pub event_type: Option<AuditEventType>,
    pub resource_id: Option<ResourceId>,
    pub outcome: Option<AuditOutcome>,
    pub ip_address: Option<IpAddr>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

impl AuditFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_event_type(mut self, event_type: AuditEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    pub fn with_resource_id(mut self, resource_id: ResourceId) -> Self {
        self.resource_id = Some(resource_id);
        self
    }

    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn with_ip_address(mut self, ip: IpAddr) -> Self {
        self.ip_address = Some(ip);
        self
    }

    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if an event matches this filter
    pub fn matches(&self, event: &AuditEvent) -> bool {
        if let Some(ref user_id) = self.user_id {
            if event.user_id.as_ref() != Some(user_id) {
                return false;
            }
        }

        if let Some(ref event_type) = self.event_type {
            if &event.event_type != event_type {
                return false;
            }
        }

        if let Some(ref resource_id) = self.resource_id {
            if event.resource_id.as_ref() != Some(resource_id) {
                return false;
            }
        }

        if let Some(ref outcome) = self.outcome {
            if &event.outcome != outcome {
                return false;
            }
        }

        if let Some(ref ip) = self.ip_address {
            if event.ip_address.as_ref() != Some(ip) {
                return false;
            }
        }

        if let Some(start) = self.start_time {
            if event.timestamp < start {
                return false;
            }
        }

        if let Some(end) = self.end_time {
            if event.timestamp > end {
                return false;
            }
        }

        true
    }
}

/// Query interface for audit logs with authorization
pub struct AuditQuery<'a> {
    store: &'a dyn AuditStore,
}

impl<'a> AuditQuery<'a> {
    pub fn new(store: &'a dyn AuditStore) -> Self {
        Self { store }
    }

    /// Query audit logs with user authorization check
    pub fn query_as(&self, user: &User, filter: AuditFilter) -> Result<Vec<AuditEvent>> {
        // Only security_admin role can query audit logs
        if !user.is_security_admin() {
            // Log the unauthorized access attempt
            return Err(AuditError::Unauthorized);
        }

        self.store.query(filter)
            .map_err(|e| AuditError::StoreError(format!("{:?}", e)))
    }

    /// Query without authorization (internal use only)
    pub fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>> {
        self.store.query(filter)
            .map_err(|e| AuditError::StoreError(format!("{:?}", e)))
    }

    /// Get all PHI access events for a specific user
    pub fn get_phi_access_history(
        &self,
        requesting_user: &User,
        target_user_id: &UserId,
    ) -> Result<Vec<AuditEvent>> {
        let filter = AuditFilter::new()
            .with_user_id(target_user_id.clone())
            .with_event_type(AuditEventType::PhiAccess);

        self.query_as(requesting_user, filter)
    }

    /// Get all failed authentication attempts from an IP
    pub fn get_failed_auth_attempts(
        &self,
        requesting_user: &User,
        ip: IpAddr,
    ) -> Result<Vec<AuditEvent>> {
        let filter = AuditFilter::new()
            .with_ip_address(ip)
            .with_event_type(AuditEventType::AuthEvent)
            .with_outcome(AuditOutcome::Failure);

        self.query_as(requesting_user, filter)
    }

    /// Get recent security events
    pub fn get_recent_security_events(
        &self,
        requesting_user: &User,
        limit: usize,
    ) -> Result<Vec<AuditEvent>> {
        let filter = AuditFilter::new()
            .with_event_type(AuditEventType::SecurityEvent)
            .with_limit(limit);

        self.query_as(requesting_user, filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::store::InMemoryAuditStore;

    #[test]
    fn test_filter_matches_user_id() {
        let filter = AuditFilter::new()
            .with_user_id(UserId::new("user_123"));

        let matching_event = AuditEvent::generic("test")
            .with_chain_hash("hash".to_string());
        let mut matching_event_clone = matching_event.clone();
        matching_event_clone.user_id = Some(UserId::new("user_123"));

        let non_matching = AuditEvent::generic("test2")
            .with_chain_hash("hash2".to_string());
        let mut non_matching_clone = non_matching.clone();
        non_matching_clone.user_id = Some(UserId::new("user_456"));

        assert!(filter.matches(&matching_event_clone));
        assert!(!filter.matches(&non_matching_clone));
    }

    #[test]
    fn test_filter_matches_event_type() {
        let filter = AuditFilter::new()
            .with_event_type(AuditEventType::PhiAccess);

        let mut matching = AuditEvent::generic("test");
        matching.event_type = AuditEventType::PhiAccess;

        let mut non_matching = AuditEvent::generic("test");
        non_matching.event_type = AuditEventType::AuthEvent;

        assert!(filter.matches(&matching));
        assert!(!filter.matches(&non_matching));
    }

    #[tokio::test]
    async fn test_query_requires_security_admin() {
        let store = InMemoryAuditStore::new();
        let query = AuditQuery::new(&store);

        let regular_user = User {
            id: UserId::new("user_123"),
            roles: vec!["developer".to_string()],
        };

        let filter = AuditFilter::default();
        let result = query.query_as(&regular_user, filter);

        assert!(matches!(result, Err(AuditError::Unauthorized)));
    }

    #[tokio::test]
    async fn test_query_allows_security_admin() {
        let store = InMemoryAuditStore::new();

        // Add some events
        store.append(AuditEvent::generic("event1")).unwrap();
        store.append(AuditEvent::generic("event2")).unwrap();

        let query = AuditQuery::new(&store);

        let admin_user = User {
            id: UserId::new("admin_001"),
            roles: vec!["security_admin".to_string()],
        };

        let filter = AuditFilter::default();
        let result = query.query_as(&admin_user, filter);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_get_phi_access_history() {
        let store = InMemoryAuditStore::new();

        // Add PHI access events
        let mut phi_event = AuditEvent::generic("phi_access");
        phi_event.event_type = AuditEventType::PhiAccess;
        phi_event.user_id = Some(UserId::new("user_123"));
        store.append(phi_event).unwrap();

        let query = AuditQuery::new(&store);

        let admin_user = User {
            id: UserId::new("admin_001"),
            roles: vec!["security_admin".to_string()],
        };

        let result = query.get_phi_access_history(&admin_user, &UserId::new("user_123"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
