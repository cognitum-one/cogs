// Unit tests for audit query functionality

use cognitum::audit::{
    AuditEvent, AuditEventType, AuditOutcome,
    UserId, ResourceId, User,
    store::InMemoryAuditStore,
    query::{AuditFilter, AuditQuery},
    AuditError,
};
use chrono::Utc;

#[test]
fn test_filter_by_user_id() {
    let filter = AuditFilter::new()
        .with_user_id(UserId::new("user_123"));

    let mut matching = AuditEvent::generic("test");
    matching.user_id = Some(UserId::new("user_123"));

    let mut non_matching = AuditEvent::generic("test");
    non_matching.user_id = Some(UserId::new("user_456"));

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn test_filter_by_event_type() {
    let filter = AuditFilter::new()
        .with_event_type(AuditEventType::PhiAccess);

    let mut matching = AuditEvent::generic("test");
    matching.event_type = AuditEventType::PhiAccess;

    let mut non_matching = AuditEvent::generic("test");
    non_matching.event_type = AuditEventType::AuthEvent;

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn test_filter_by_outcome() {
    let filter = AuditFilter::new()
        .with_outcome(AuditOutcome::Failure);

    let mut matching = AuditEvent::generic("test");
    matching.outcome = AuditOutcome::Failure;

    let mut non_matching = AuditEvent::generic("test");
    non_matching.outcome = AuditOutcome::Success;

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn test_filter_by_ip_address() {
    let ip = "192.168.1.1".parse().unwrap();
    let filter = AuditFilter::new().with_ip_address(ip);

    let mut matching = AuditEvent::generic("test");
    matching.ip_address = Some(ip);

    let mut non_matching = AuditEvent::generic("test");
    non_matching.ip_address = Some("10.0.0.1".parse().unwrap());

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn test_filter_by_time_range() {
    use chrono::Duration;

    let now = Utc::now();
    let start = now - Duration::hours(1);
    let end = now + Duration::hours(1);

    let filter = AuditFilter::new().with_time_range(start, end);

    let mut matching = AuditEvent::generic("test");
    matching.timestamp = now;

    let mut non_matching = AuditEvent::generic("test");
    non_matching.timestamp = now - Duration::hours(2);

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn test_filter_multiple_criteria() {
    let filter = AuditFilter::new()
        .with_user_id(UserId::new("user_123"))
        .with_event_type(AuditEventType::PhiAccess)
        .with_outcome(AuditOutcome::Success);

    let mut matching = AuditEvent::generic("test");
    matching.user_id = Some(UserId::new("user_123"));
    matching.event_type = AuditEventType::PhiAccess;
    matching.outcome = AuditOutcome::Success;

    let mut partial_match = AuditEvent::generic("test");
    partial_match.user_id = Some(UserId::new("user_123"));
    partial_match.event_type = AuditEventType::PhiAccess;
    partial_match.outcome = AuditOutcome::Failure; // Wrong outcome

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&partial_match));
}

#[tokio::test]
async fn test_query_requires_security_admin_role() {
    let store = InMemoryAuditStore::new();
    store.append(AuditEvent::generic("test")).unwrap();

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
    let events = result.unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn test_query_denies_free_tier_user() {
    let store = InMemoryAuditStore::new();
    let query = AuditQuery::new(&store);

    let free_user = User {
        id: UserId::new("user_123"),
        roles: vec!["free".to_string()],
    };

    let filter = AuditFilter::default();
    let result = query.query_as(&free_user, filter);

    assert!(matches!(result, Err(AuditError::Unauthorized)));
}

#[tokio::test]
async fn test_get_phi_access_history() {
    let store = InMemoryAuditStore::new();

    // Add PHI access events
    let mut phi_event1 = AuditEvent::generic("phi_access_1");
    phi_event1.event_type = AuditEventType::PhiAccess;
    phi_event1.user_id = Some(UserId::new("user_123"));
    store.append(phi_event1).unwrap();

    let mut phi_event2 = AuditEvent::generic("phi_access_2");
    phi_event2.event_type = AuditEventType::PhiAccess;
    phi_event2.user_id = Some(UserId::new("user_123"));
    store.append(phi_event2).unwrap();

    // Add non-PHI event
    let mut other_event = AuditEvent::generic("api_call");
    other_event.event_type = AuditEventType::ApiCall;
    other_event.user_id = Some(UserId::new("user_123"));
    store.append(other_event).unwrap();

    let query = AuditQuery::new(&store);

    let admin_user = User {
        id: UserId::new("admin_001"),
        roles: vec!["security_admin".to_string()],
    };

    let result = query.get_phi_access_history(&admin_user, &UserId::new("user_123"));
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.event_type == AuditEventType::PhiAccess));
}

#[tokio::test]
async fn test_get_failed_auth_attempts() {
    let store = InMemoryAuditStore::new();

    let ip = "192.168.1.100".parse().unwrap();

    // Add failed auth attempts
    let mut failed1 = AuditEvent::generic("login_failed");
    failed1.event_type = AuditEventType::AuthEvent;
    failed1.outcome = AuditOutcome::Failure;
    failed1.ip_address = Some(ip);
    store.append(failed1).unwrap();

    let mut failed2 = AuditEvent::generic("login_failed");
    failed2.event_type = AuditEventType::AuthEvent;
    failed2.outcome = AuditOutcome::Failure;
    failed2.ip_address = Some(ip);
    store.append(failed2).unwrap();

    // Add successful auth
    let mut success = AuditEvent::generic("login_success");
    success.event_type = AuditEventType::AuthEvent;
    success.outcome = AuditOutcome::Success;
    success.ip_address = Some(ip);
    store.append(success).unwrap();

    let query = AuditQuery::new(&store);

    let admin_user = User {
        id: UserId::new("admin_001"),
        roles: vec!["security_admin".to_string()],
    };

    let result = query.get_failed_auth_attempts(&admin_user, ip);
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.outcome == AuditOutcome::Failure));
}

#[tokio::test]
async fn test_get_recent_security_events() {
    let store = InMemoryAuditStore::new();

    // Add security events
    for i in 0..5 {
        let mut event = AuditEvent::generic(format!("security_event_{}", i));
        event.event_type = AuditEventType::SecurityEvent;
        store.append(event).unwrap();
    }

    let query = AuditQuery::new(&store);

    let admin_user = User {
        id: UserId::new("admin_001"),
        roles: vec!["security_admin".to_string()],
    };

    let result = query.get_recent_security_events(&admin_user, 3);
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 3);
}
