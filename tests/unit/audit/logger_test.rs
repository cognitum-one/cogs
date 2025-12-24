// Unit tests for audit logger

use cognitum::audit::{
    AuditLogger, AuditEvent, AuditEventType, AuditOutcome,
    UserId, ResourceId, SessionId, RequestContext,
    store::{MockAuditStore, AuditId, StoreError},
};
use mockall::predicate::*;

#[tokio::test]
async fn test_logger_appends_event_without_chaining() {
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
async fn test_logger_chains_events() {
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

    // Third event also has chain hash
    mock_store
        .expect_append()
        .withf(|event| event.chain_hash.is_some())
        .times(1)
        .returning(|_| Ok(AuditId::new()));

    let logger = AuditLogger::with_chaining(Box::new(mock_store));

    logger.log_event(AuditEvent::generic("event_1")).await.unwrap();
    logger.log_event(AuditEvent::generic("event_2")).await.unwrap();
    logger.log_event(AuditEvent::generic("event_3")).await.unwrap();
}

#[tokio::test]
async fn test_log_phi_access_requires_all_fields() {
    let mut mock_store = MockAuditStore::new();

    mock_store
        .expect_append()
        .withf(|event| {
            event.event_type == AuditEventType::PhiAccess &&
            event.user_id.is_some() &&
            event.ip_address.is_some() &&
            event.session_id.is_some() &&
            event.resource_id.is_some()
        })
        .times(1)
        .returning(|_| Ok(AuditId::new()));

    let logger = AuditLogger::new(Box::new(mock_store));

    let ctx = RequestContext {
        user_id: Some(UserId::new("user_123")),
        ip_address: Some("192.168.1.1".parse().unwrap()),
        session_id: Some(SessionId::new("sess_abc")),
    };

    let result = logger.log_phi_access(&ctx, &ResourceId::new("patient_record_456")).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_log_phi_access_fails_without_user_id() {
    let mock_store = MockAuditStore::new();
    let logger = AuditLogger::new(Box::new(mock_store));

    let ctx = RequestContext {
        user_id: None, // Missing user ID
        ip_address: Some("192.168.1.1".parse().unwrap()),
        session_id: Some(SessionId::new("sess_abc")),
    };

    let result = logger.log_phi_access(&ctx, &ResourceId::new("patient_456")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_log_auth_event() {
    let mut mock_store = MockAuditStore::new();

    mock_store
        .expect_append()
        .withf(|event| {
            event.event_type == AuditEventType::AuthEvent &&
            event.action == "login" &&
            event.outcome == AuditOutcome::Success
        })
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

#[tokio::test]
async fn test_log_api_call() {
    let mut mock_store = MockAuditStore::new();

    mock_store
        .expect_append()
        .withf(|event| {
            event.event_type == AuditEventType::ApiCall &&
            event.action == "/api/v1/simulate"
        })
        .times(1)
        .returning(|_| Ok(AuditId::new()));

    let logger = AuditLogger::new(Box::new(mock_store));

    let ctx = RequestContext {
        user_id: Some(UserId::new("user_123")),
        ip_address: Some("10.0.0.1".parse().unwrap()),
        session_id: Some(SessionId::new("sess_xyz")),
    };

    let result = logger.log_api_call(&ctx, "/api/v1/simulate", AuditOutcome::Success).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_log_config_change() {
    let mut mock_store = MockAuditStore::new();

    mock_store
        .expect_append()
        .withf(|event| {
            event.event_type == AuditEventType::ConfigChange &&
            event.resource_id.is_some()
        })
        .times(1)
        .returning(|_| Ok(AuditId::new()));

    let logger = AuditLogger::new(Box::new(mock_store));

    let result = logger.log_config_change(
        UserId::new("admin_001"),
        "max_memory_size",
        AuditOutcome::Success,
    ).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_log_security_event() {
    let mut mock_store = MockAuditStore::new();

    mock_store
        .expect_append()
        .withf(|event| {
            event.event_type == AuditEventType::SecurityEvent &&
            event.action == "rate_limit_exceeded" &&
            event.outcome == AuditOutcome::Denied
        })
        .times(1)
        .returning(|_| Ok(AuditId::new()));

    let logger = AuditLogger::new(Box::new(mock_store));

    let result = logger.log_security_event(
        Some(UserId::new("user_456")),
        Some("192.168.1.100".parse().unwrap()),
        "rate_limit_exceeded",
        AuditOutcome::Denied,
    ).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_raw_key_count_always_zero() {
    let mock_store = MockAuditStore::new();
    let logger = AuditLogger::new(Box::new(mock_store));

    // Audit logger should never store raw keys in memory
    assert_eq!(logger.raw_key_count(), 0);
}
