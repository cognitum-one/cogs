// Audit logger implementation with tamper-evident chaining

use std::sync::{Arc, Mutex};
use super::{
    AuditEvent, AuditOutcome, AuditError, Result,
    AuditStore, AuditId, UserId, ResourceId, RequestContext,
};

/// Audit logger with chain hash linking for tamper evidence
pub struct AuditLogger {
    store: Box<dyn AuditStore>,
    last_hash: Arc<Mutex<Option<String>>>,
    enable_chaining: bool,
}

impl AuditLogger {
    /// Create a new audit logger without chaining
    pub fn new(store: Box<dyn AuditStore>) -> Self {
        Self {
            store,
            last_hash: Arc::new(Mutex::new(None)),
            enable_chaining: false,
        }
    }

    /// Create a new audit logger with chain hash linking
    pub fn with_chaining(store: Box<dyn AuditStore>) -> Self {
        Self {
            store,
            last_hash: Arc::new(Mutex::new(None)),
            enable_chaining: true,
        }
    }

    /// Log a generic audit event
    pub async fn log_event(&self, mut event: AuditEvent) -> Result<AuditId> {
        // Add chain hash if chaining is enabled
        if self.enable_chaining {
            let prev_hash = self.last_hash.lock()
                .map_err(|e| AuditError::StoreError(format!("Lock error: {}", e)))?
                .clone();

            if let Some(hash) = prev_hash {
                event = event.with_chain_hash(hash);
            }
        }

        // Store the event
        let id = self.store.append(event.clone())
            .map_err(|e| AuditError::StoreError(format!("Failed to append: {:?}", e)))?;

        // Update last hash for next event
        if self.enable_chaining {
            let new_hash = event.calculate_hash();
            *self.last_hash.lock()
                .map_err(|e| AuditError::StoreError(format!("Lock error: {}", e)))? = Some(new_hash);
        }

        Ok(id)
    }

    /// Log PHI access for HIPAA compliance
    pub async fn log_phi_access(
        &self,
        ctx: &RequestContext,
        resource_id: &ResourceId,
    ) -> Result<AuditId> {
        let event = AuditEvent::phi_access(
            ctx.user_id.clone().ok_or_else(||
                AuditError::InvalidFilter("User ID required for PHI access".to_string()))?,
            ctx.ip_address.ok_or_else(||
                AuditError::InvalidFilter("IP address required for PHI access".to_string()))?,
            ctx.session_id.clone().ok_or_else(||
                AuditError::InvalidFilter("Session ID required for PHI access".to_string()))?,
            resource_id.clone(),
            AuditOutcome::Success,
        );

        self.log_event(event).await
    }

    /// Log authentication event
    pub async fn log_auth_event(
        &self,
        user_id: Option<UserId>,
        ip_address: std::net::IpAddr,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Result<AuditId> {
        let event = AuditEvent::auth_event(user_id, ip_address, action, outcome);
        self.log_event(event).await
    }

    /// Log API call
    pub async fn log_api_call(
        &self,
        ctx: &RequestContext,
        endpoint: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Result<AuditId> {
        let event = AuditEvent::api_call(
            ctx.user_id.clone(),
            ctx.ip_address,
            ctx.session_id.clone(),
            endpoint,
            outcome,
        );
        self.log_event(event).await
    }

    /// Log configuration change
    pub async fn log_config_change(
        &self,
        user_id: UserId,
        config_key: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Result<AuditId> {
        let event = AuditEvent::config_change(user_id, config_key, outcome);
        self.log_event(event).await
    }

    /// Log security event
    pub async fn log_security_event(
        &self,
        user_id: Option<UserId>,
        ip_address: Option<std::net::IpAddr>,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Result<AuditId> {
        let event = AuditEvent::security_event(user_id, ip_address, action, outcome);
        self.log_event(event).await
    }

    /// Check if user has raw key count (for testing)
    pub fn raw_key_count(&self) -> usize {
        0 // No raw keys stored in audit logger
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::store::MockAuditStore;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_log_event_without_chaining() {
        let mut mock_store = MockAuditStore::new();

        mock_store
            .expect_append()
            .withf(|event| event.chain_hash.is_none())
            .times(1)
            .returning(|_| Ok(AuditId::new()));

        let logger = AuditLogger::new(Box::new(mock_store));
        let event = AuditEvent::generic("test_action");

        let result = logger.log_event(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_event_with_chaining() {
        let mut mock_store = MockAuditStore::new();

        // First event has no chain hash
        mock_store
            .expect_append()
            .withf(|event| event.chain_hash.is_none())
            .times(1)
            .returning(|_| Ok(AuditId::new()));

        // Second event has chain hash
        mock_store
            .expect_append()
            .withf(|event| event.chain_hash.is_some())
            .times(1)
            .returning(|_| Ok(AuditId::new()));

        let logger = AuditLogger::with_chaining(Box::new(mock_store));

        let event1 = AuditEvent::generic("event_1");
        logger.log_event(event1).await.unwrap();

        let event2 = AuditEvent::generic("event_2");
        logger.log_event(event2).await.unwrap();
    }

    #[tokio::test]
    async fn test_log_phi_access() {
        let mut mock_store = MockAuditStore::new();

        mock_store
            .expect_append()
            .withf(|event| {
                event.event_type == AuditEventType::PhiAccess &&
                event.user_id.is_some() &&
                event.ip_address.is_some() &&
                event.session_id.is_some()
            })
            .times(1)
            .returning(|_| Ok(AuditId::new()));

        let logger = AuditLogger::new(Box::new(mock_store));

        let ctx = RequestContext {
            user_id: Some(UserId::new("user_123")),
            ip_address: Some("192.168.1.1".parse().unwrap()),
            session_id: Some(super::super::SessionId::new("sess_abc")),
        };

        let result = logger.log_phi_access(&ctx, &ResourceId::new("patient_456")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_auth_event() {
        let mut mock_store = MockAuditStore::new();

        mock_store
            .expect_append()
            .withf(|event| event.event_type == AuditEventType::AuthEvent)
            .times(1)
            .returning(|_| Ok(AuditId::new()));

        let logger = AuditLogger::new(Box::new(mock_store));

        let result = logger.log_auth_event(
            Some(UserId::new("user_123")),
            "192.168.1.1".parse().unwrap(),
            "login",
            AuditOutcome::Success,
        ).await;

        assert!(result.is_ok());
    }
}
