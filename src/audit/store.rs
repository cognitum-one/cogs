// Audit store trait and implementations

use mockall::automock;
use uuid::Uuid;

use super::{AuditEvent, query::AuditFilter};

/// Unique identifier for audit events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditId(String);

impl AuditId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AuditId {
    fn default() -> Self {
        Self::new()
    }
}

/// Error types for audit store operations
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Event not found")]
    NotFound,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Invalid filter: {0}")]
    InvalidFilter(String),
}

/// Trait for audit event storage with tamper-evident capabilities
#[automock]
pub trait AuditStore: Send + Sync {
    /// Append a new audit event to the immutable log
    /// Returns the unique ID of the stored event
    fn append(&self, event: AuditEvent) -> Result<AuditId, StoreError>;

    /// Query audit events based on filter criteria
    /// Returns matching events in chronological order
    fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>, StoreError>;

    /// Verify the integrity of the audit chain between two event IDs
    /// Returns true if the chain is intact, false if tampering is detected
    fn verify_integrity(&self, from: AuditId, to: AuditId) -> Result<bool, StoreError>;
}

/// In-memory implementation for testing and development
pub struct InMemoryAuditStore {
    events: std::sync::Mutex<Vec<(AuditId, AuditEvent)>>,
}

impl InMemoryAuditStore {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for InMemoryAuditStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditStore for InMemoryAuditStore {
    fn append(&self, event: AuditEvent) -> Result<AuditId, StoreError> {
        let id = AuditId::new();
        let mut events = self.events.lock()
            .map_err(|e| StoreError::DatabaseError(format!("Lock error: {}", e)))?;

        events.push((id.clone(), event));
        Ok(id)
    }

    fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>, StoreError> {
        let events = self.events.lock()
            .map_err(|e| StoreError::DatabaseError(format!("Lock error: {}", e)))?;

        let filtered: Vec<AuditEvent> = events.iter()
            .map(|(_, event)| event.clone())
            .filter(|event| filter.matches(event))
            .collect();

        Ok(filtered)
    }

    fn verify_integrity(&self, from: AuditId, to: AuditId) -> Result<bool, StoreError> {
        let events = self.events.lock()
            .map_err(|e| StoreError::DatabaseError(format!("Lock error: {}", e)))?;

        // Find the range of events
        let start_idx = events.iter()
            .position(|(id, _)| id == &from)
            .ok_or(StoreError::NotFound)?;

        let end_idx = events.iter()
            .position(|(id, _)| id == &to)
            .ok_or(StoreError::NotFound)?;

        if start_idx >= end_idx {
            return Err(StoreError::InvalidFilter("Invalid range".to_string()));
        }

        // Verify chain integrity
        for i in start_idx..end_idx {
            let current_event = &events[i].1;
            let next_event = &events[i + 1].1;

            // Calculate hash of current event
            let current_hash = current_event.calculate_hash();

            // Check if next event's chain_hash matches current hash
            if let Some(ref chain_hash) = next_event.chain_hash {
                if chain_hash != &current_hash {
                    return Ok(false); // Integrity violation
                }
            }
        }

        Ok(true) // Chain is intact
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditEvent, AuditOutcome};

    #[test]
    fn test_in_memory_store_append() {
        let store = InMemoryAuditStore::new();
        let event = AuditEvent::generic("test");

        let result = store.append(event);
        assert!(result.is_ok());
    }

    #[test]
    fn test_in_memory_store_query() {
        let store = InMemoryAuditStore::new();

        let event1 = AuditEvent::generic("event1");
        let event2 = AuditEvent::generic("event2");

        store.append(event1).unwrap();
        store.append(event2).unwrap();

        let filter = AuditFilter::default();
        let results = store.query(filter).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_verify_integrity_success() {
        let store = InMemoryAuditStore::new();

        // Create chained events
        let event1 = AuditEvent::generic("event1");
        let id1 = store.append(event1.clone()).unwrap();

        let hash1 = event1.calculate_hash();
        let event2 = AuditEvent::generic("event2").with_chain_hash(hash1);
        let id2 = store.append(event2).unwrap();

        let result = store.verify_integrity(id1, id2);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_integrity_tampering() {
        let store = InMemoryAuditStore::new();

        // Create events with wrong chain hash
        let event1 = AuditEvent::generic("event1");
        let id1 = store.append(event1).unwrap();

        let event2 = AuditEvent::generic("event2")
            .with_chain_hash("wrong_hash".to_string());
        let id2 = store.append(event2).unwrap();

        let result = store.verify_integrity(id1, id2);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should detect tampering
    }
}
