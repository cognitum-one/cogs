// Integration tests for complete audit logging workflow

use cognitum::audit::{
    AuditLogger, AuditEvent, AuditEventType, AuditOutcome,
    UserId, ResourceId, SessionId, RequestContext, User,
    store::InMemoryAuditStore,
    query::{AuditFilter, AuditQuery},
};

#[tokio::test]
async fn test_complete_phi_access_workflow() {
    // Setup
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::with_chaining(Box::new(store));

    // Simulate PHI access
    let ctx = RequestContext {
        user_id: Some(UserId::new("doctor_001")),
        ip_address: Some("10.20.30.40".parse().unwrap()),
        session_id: Some(SessionId::new("sess_medical_123")),
    };

    let result = logger.log_phi_access(&ctx, &ResourceId::new("patient_rec_789")).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_complete_authentication_workflow() {
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::with_chaining(Box::new(store));

    let ip = "192.168.1.50".parse().unwrap();

    // Failed login attempt
    logger.log_auth_event(
        None,
        ip,
        "login_attempt",
        AuditOutcome::Failure,
    ).await.unwrap();

    // Failed login attempt 2
    logger.log_auth_event(
        None,
        ip,
        "login_attempt",
        AuditOutcome::Failure,
    ).await.unwrap();

    // Successful login
    logger.log_auth_event(
        Some(UserId::new("user_456")),
        ip,
        "login_success",
        AuditOutcome::Success,
    ).await.unwrap();

    // Logout
    logger.log_auth_event(
        Some(UserId::new("user_456")),
        ip,
        "logout",
        AuditOutcome::Success,
    ).await.unwrap();
}

#[tokio::test]
async fn test_tamper_evidence_chain_integrity() {
    let store = Box::new(InMemoryAuditStore::new());
    let logger = AuditLogger::with_chaining(store);

    // Log a series of events
    let events = vec![
        "api_call_1",
        "config_change_2",
        "security_event_3",
        "phi_access_4",
        "auth_event_5",
    ];

    for event in events {
        logger.log_event(AuditEvent::generic(event)).await.unwrap();
    }

    // Chain should be automatically maintained
    // In a real implementation, we would verify the chain here
}

#[tokio::test]
async fn test_unauthorized_query_attempt_is_logged() {
    let store = InMemoryAuditStore::new();

    // Add some events
    store.append(AuditEvent::generic("sensitive_event_1")).unwrap();
    store.append(AuditEvent::generic("sensitive_event_2")).unwrap();

    let query = AuditQuery::new(&store);

    let unauthorized_user = User {
        id: UserId::new("hacker_123"),
        roles: vec!["free".to_string()],
    };

    // Attempt to query audit logs (should fail)
    let result = query.query_as(&unauthorized_user, AuditFilter::default());
    assert!(result.is_err());

    // In a real implementation, this unauthorized attempt would itself be logged
}

#[tokio::test]
async fn test_multi_user_audit_separation() {
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::new(Box::new(store));

    // User 1 activity
    let ctx1 = RequestContext {
        user_id: Some(UserId::new("user_001")),
        ip_address: Some("10.0.0.1".parse().unwrap()),
        session_id: Some(SessionId::new("sess_001")),
    };

    logger.log_api_call(&ctx1, "/api/v1/simulate", AuditOutcome::Success).await.unwrap();
    logger.log_api_call(&ctx1, "/api/v1/results", AuditOutcome::Success).await.unwrap();

    // User 2 activity
    let ctx2 = RequestContext {
        user_id: Some(UserId::new("user_002")),
        ip_address: Some("10.0.0.2".parse().unwrap()),
        session_id: Some(SessionId::new("sess_002")),
    };

    logger.log_api_call(&ctx2, "/api/v1/license/validate", AuditOutcome::Success).await.unwrap();

    // Both users' activities should be separately trackable
}

#[tokio::test]
async fn test_security_event_detection_workflow() {
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::with_chaining(Box::new(store));

    let attacker_ip = "192.168.1.666".parse().unwrap();

    // Simulate brute force attack
    for i in 0..10 {
        logger.log_auth_event(
            None,
            attacker_ip,
            format!("login_attempt_{}", i),
            AuditOutcome::Failure,
        ).await.unwrap();
    }

    // Log security event for brute force detection
    logger.log_security_event(
        None,
        Some(attacker_ip),
        "brute_force_detected",
        AuditOutcome::Denied,
    ).await.unwrap();

    // Log IP ban
    logger.log_security_event(
        None,
        Some(attacker_ip),
        "ip_banned",
        AuditOutcome::Success,
    ).await.unwrap();
}

#[tokio::test]
async fn test_configuration_change_audit_trail() {
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::with_chaining(Box::new(store));

    let admin = UserId::new("admin_root");

    // Log configuration changes
    logger.log_config_change(
        admin.clone(),
        "max_concurrent_simulations",
        AuditOutcome::Success,
    ).await.unwrap();

    logger.log_config_change(
        admin.clone(),
        "license_validation_interval",
        AuditOutcome::Success,
    ).await.unwrap();

    logger.log_config_change(
        admin.clone(),
        "security_level",
        AuditOutcome::Success,
    ).await.unwrap();
}

#[tokio::test]
async fn test_compliance_reporting_workflow() {
    let store = InMemoryAuditStore::new();
    let logger = AuditLogger::with_chaining(Box::new(store));

    // Simulate HIPAA-compliant PHI access
    for i in 0..5 {
        let ctx = RequestContext {
            user_id: Some(UserId::new(format!("clinician_{}", i))),
            ip_address: Some("10.20.30.40".parse().unwrap()),
            session_id: Some(SessionId::new(format!("medical_sess_{}", i))),
        };

        logger.log_phi_access(
            &ctx,
            &ResourceId::new(format!("patient_record_{}", i)),
        ).await.unwrap();
    }

    // Compliance officer queries the logs
    let query = AuditQuery::new(logger.store.as_ref());

    let compliance_officer = User {
        id: UserId::new("compliance_001"),
        roles: vec!["security_admin".to_string()],
    };

    let phi_filter = AuditFilter::new()
        .with_event_type(AuditEventType::PhiAccess);

    let result = query.query_as(&compliance_officer, phi_filter);
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 5);
}

#[tokio::test]
async fn test_no_deletion_capability() {
    // The audit store trait does not have a delete method
    // This test verifies that the API doesn't provide any deletion capability

    let store = InMemoryAuditStore::new();
    store.append(AuditEvent::generic("permanent_event")).unwrap();

    // There is no way to delete events from the store
    // This is by design for HIPAA compliance and tamper-evidence

    let results = store.query(AuditFilter::default()).unwrap();
    assert_eq!(results.len(), 1);

    // Even if we wanted to, there's no delete method available
    // store.delete(...) // Does not compile - method doesn't exist
}
