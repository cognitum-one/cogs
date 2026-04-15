// Unit tests for audit events

use cognitum::audit::{AuditEvent, AuditEventType, AuditOutcome, UserId, ResourceId, SessionId};

#[test]
fn test_phi_access_event_creation() {
    let event = AuditEvent::phi_access(
        UserId::new("user_123"),
        "192.168.1.1".parse().unwrap(),
        SessionId::new("sess_abc"),
        ResourceId::new("patient_456"),
        AuditOutcome::Success,
    );

    assert_eq!(event.event_type, AuditEventType::PhiAccess);
    assert_eq!(event.outcome, AuditOutcome::Success);
    assert_eq!(event.action, "access_phi_record");
    assert!(event.user_id.is_some());
    assert!(event.ip_address.is_some());
    assert!(event.session_id.is_some());
    assert!(event.resource_id.is_some());
}

#[test]
fn test_auth_event_creation() {
    let event = AuditEvent::auth_event(
        Some(UserId::new("user_123")),
        "10.0.0.1".parse().unwrap(),
        "login",
        AuditOutcome::Success,
    );

    assert_eq!(event.event_type, AuditEventType::AuthEvent);
    assert_eq!(event.action, "login");
    assert_eq!(event.outcome, AuditOutcome::Success);
}

#[test]
fn test_api_call_event_creation() {
    let event = AuditEvent::api_call(
        Some(UserId::new("user_123")),
        Some("192.168.1.100".parse().unwrap()),
        Some(SessionId::new("sess_xyz")),
        "/api/v1/simulate",
        AuditOutcome::Success,
    );

    assert_eq!(event.event_type, AuditEventType::ApiCall);
    assert_eq!(event.action, "/api/v1/simulate");
}

#[test]
fn test_config_change_event_creation() {
    let event = AuditEvent::config_change(
        UserId::new("admin_001"),
        "max_simulation_time",
        AuditOutcome::Success,
    );

    assert_eq!(event.event_type, AuditEventType::ConfigChange);
    assert!(event.action.contains("max_simulation_time"));
}

#[test]
fn test_security_event_creation() {
    let event = AuditEvent::security_event(
        Some(UserId::new("user_123")),
        Some("192.168.1.1".parse().unwrap()),
        "brute_force_detected",
        AuditOutcome::Denied,
    );

    assert_eq!(event.event_type, AuditEventType::SecurityEvent);
    assert_eq!(event.outcome, AuditOutcome::Denied);
}

#[test]
fn test_hash_calculation_deterministic() {
    let event = AuditEvent::generic("test_action");
    let hash1 = event.calculate_hash();
    let hash2 = event.calculate_hash();

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // SHA-256 produces 64 hex characters
}

#[test]
fn test_hash_changes_with_chain_hash() {
    let mut event = AuditEvent::generic("test");
    let hash_without_chain = event.calculate_hash();

    event.chain_hash = Some("previous_event_hash".to_string());
    let hash_with_chain = event.calculate_hash();

    assert_ne!(hash_without_chain, hash_with_chain);
}

#[test]
fn test_hash_includes_all_fields() {
    let event1 = AuditEvent::generic("action1");
    let event2 = AuditEvent::generic("action2");

    assert_ne!(event1.calculate_hash(), event2.calculate_hash());
}

#[test]
fn test_event_with_metadata() {
    let metadata = serde_json::json!({
        "reason": "scheduled_maintenance",
        "duration_seconds": 3600
    });

    let event = AuditEvent::config_change(
        UserId::new("admin_001"),
        "system_mode",
        AuditOutcome::Success,
    ).with_metadata(metadata.clone());

    assert_eq!(event.metadata, Some(metadata));
}

#[test]
fn test_event_timestamp_is_recent() {
    use chrono::Utc;

    let event = AuditEvent::generic("test");
    let now = Utc::now();

    let diff = (now - event.timestamp).num_seconds().abs();
    assert!(diff < 2); // Event timestamp should be within 2 seconds of now
}
