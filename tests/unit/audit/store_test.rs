// Unit tests for audit store

use cognitum::audit::{
    AuditEvent, AuditEventType, AuditOutcome,
    store::{InMemoryAuditStore, AuditStore, AuditId},
    query::AuditFilter,
};

#[test]
fn test_in_memory_store_append() {
    let store = InMemoryAuditStore::new();
    let event = AuditEvent::generic("test_event");

    let result = store.append(event);
    assert!(result.is_ok());

    let id = result.unwrap();
    assert!(!id.as_str().is_empty());
}

#[test]
fn test_in_memory_store_append_multiple() {
    let store = InMemoryAuditStore::new();

    for i in 0..10 {
        let event = AuditEvent::generic(format!("event_{}", i));
        let result = store.append(event);
        assert!(result.is_ok());
    }
}

#[test]
fn test_in_memory_store_query_all() {
    let store = InMemoryAuditStore::new();

    store.append(AuditEvent::generic("event1")).unwrap();
    store.append(AuditEvent::generic("event2")).unwrap();
    store.append(AuditEvent::generic("event3")).unwrap();

    let filter = AuditFilter::default();
    let results = store.query(filter).unwrap();

    assert_eq!(results.len(), 3);
}

#[test]
fn test_in_memory_store_query_filtered() {
    let store = InMemoryAuditStore::new();

    let mut phi_event = AuditEvent::generic("phi_access");
    phi_event.event_type = AuditEventType::PhiAccess;
    store.append(phi_event).unwrap();

    let mut auth_event = AuditEvent::generic("login");
    auth_event.event_type = AuditEventType::AuthEvent;
    store.append(auth_event).unwrap();

    let filter = AuditFilter::new()
        .with_event_type(AuditEventType::PhiAccess);

    let results = store.query(filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].event_type, AuditEventType::PhiAccess);
}

#[test]
fn test_verify_integrity_with_valid_chain() {
    let store = InMemoryAuditStore::new();

    // Create first event
    let event1 = AuditEvent::generic("event1");
    let id1 = store.append(event1.clone()).unwrap();

    // Create second event with correct chain hash
    let hash1 = event1.calculate_hash();
    let event2 = AuditEvent::generic("event2").with_chain_hash(hash1);
    let id2 = store.append(event2.clone()).unwrap();

    // Create third event with correct chain hash
    let hash2 = event2.calculate_hash();
    let event3 = AuditEvent::generic("event3").with_chain_hash(hash2);
    let id3 = store.append(event3).unwrap();

    // Verify integrity from id1 to id2
    let result = store.verify_integrity(id1.clone(), id2.clone());
    assert!(result.is_ok());
    assert!(result.unwrap());

    // Verify integrity from id2 to id3
    let result = store.verify_integrity(id2, id3);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_verify_integrity_detects_tampering() {
    let store = InMemoryAuditStore::new();

    // Create first event
    let event1 = AuditEvent::generic("event1");
    let id1 = store.append(event1).unwrap();

    // Create second event with WRONG chain hash (simulating tampering)
    let event2 = AuditEvent::generic("event2")
        .with_chain_hash("tampered_hash_12345".to_string());
    let id2 = store.append(event2).unwrap();

    // Verify integrity should detect the tampering
    let result = store.verify_integrity(id1, id2);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false (integrity violated)
}

#[test]
fn test_verify_integrity_with_long_chain() {
    let store = InMemoryAuditStore::new();

    let mut ids = Vec::new();
    let mut last_hash: Option<String> = None;

    // Create a chain of 5 events
    for i in 0..5 {
        let mut event = AuditEvent::generic(format!("event_{}", i));

        if let Some(hash) = last_hash {
            event = event.with_chain_hash(hash);
        }

        last_hash = Some(event.calculate_hash());
        let id = store.append(event).unwrap();
        ids.push(id);
    }

    // Verify integrity across the entire chain
    let result = store.verify_integrity(ids[0].clone(), ids[4].clone());
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_verify_integrity_invalid_range() {
    let store = InMemoryAuditStore::new();

    let id1 = store.append(AuditEvent::generic("event1")).unwrap();
    let id2 = store.append(AuditEvent::generic("event2")).unwrap();

    // Try to verify with reversed range (should fail)
    let result = store.verify_integrity(id2, id1);
    assert!(result.is_err());
}

#[test]
fn test_verify_integrity_nonexistent_ids() {
    let store = InMemoryAuditStore::new();

    let fake_id1 = AuditId::new();
    let fake_id2 = AuditId::new();

    let result = store.verify_integrity(fake_id1, fake_id2);
    assert!(result.is_err());
}

#[test]
fn test_concurrent_appends() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(InMemoryAuditStore::new());
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let event = AuditEvent::generic(format!("concurrent_event_{}", i));
            store_clone.append(event)
        });
        handles.push(handle);
    }

    for handle in handles {
        assert!(handle.join().unwrap().is_ok());
    }

    // Verify all events were stored
    let filter = AuditFilter::default();
    let results = store.query(filter).unwrap();
    assert_eq!(results.len(), 10);
}
