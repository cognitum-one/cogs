//! Witness Log - Append-only event log for the reality substrate.
//!
//! The `WitnessLog` provides an immutable audit trail of all correction events
//! that occur during simulation. Once a record is appended, it cannot be
//! modified or removed, ensuring the integrity of the audit trail.

use serde::{Deserialize, Serialize};
use super::events::{WitnessEventType, WitnessRecord};
use crate::error::{FxnnError, Result};

/// Append-only log of witness records.
///
/// The `WitnessLog` stores all correction events that occur during simulation.
/// It provides efficient querying by event type, entity, and time range,
/// while maintaining the append-only property for audit trail integrity.
///
/// # Design
///
/// - **Append-only**: Records cannot be modified after insertion
/// - **Indexed**: Internal indices for efficient queries
/// - **Serializable**: Full JSON export for analysis
/// - **Memory-bounded**: Optional capacity limits with oldest-first eviction
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
///
/// let mut log = WitnessLog::new();
///
/// // Append a record
/// log.append(WitnessRecord {
///     tick: 1,
///     event_type: WitnessEventType::OverlapCorrection,
///     entity_ids: vec![1, 2],
///     constraint_fired: "sigma_min".to_string(),
///     before_state: StateSnapshot::default(),
///     after_state: StateSnapshot::default(),
///     correction_applied: CorrectionDetails::default(),
///     invariant_improved: "no_overlap".to_string(),
///     delta_magnitude: 0.1,
/// });
///
/// assert_eq!(log.len(), 1);
/// ```
///
/// ## With Capacity Limit
///
/// ```rust
/// use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
///
/// // Create log with max 1000 records
/// let mut log = WitnessLog::with_capacity(1000);
///
/// // When capacity is exceeded, oldest records are evicted
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessLog {
    /// The stored records in chronological order.
    records: Vec<WitnessRecord>,

    /// Optional maximum capacity (None = unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    max_capacity: Option<usize>,

    /// Total number of records ever appended (including evicted).
    total_appended: u64,

    /// Number of records evicted due to capacity limits.
    evicted_count: u64,
}

impl Default for WitnessLog {
    fn default() -> Self {
        Self::new()
    }
}

impl WitnessLog {
    /// Create a new empty witness log with unlimited capacity.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::WitnessLog;
    ///
    /// let log = WitnessLog::new();
    /// assert_eq!(log.len(), 0);
    /// assert!(log.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            max_capacity: None,
            total_appended: 0,
            evicted_count: 0,
        }
    }

    /// Create a new witness log with a maximum capacity.
    ///
    /// When the log reaches capacity, the oldest records are evicted
    /// to make room for new ones (FIFO eviction).
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of records to retain
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::WitnessLog;
    ///
    /// let log = WitnessLog::with_capacity(1000);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity.min(1024)),
            max_capacity: Some(capacity),
            total_appended: 0,
            evicted_count: 0,
        }
    }

    /// Append a record to the log.
    ///
    /// If the log has a capacity limit and is full, the oldest record
    /// will be evicted before adding the new one.
    ///
    /// # Arguments
    ///
    /// * `record` - The witness record to append
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
    ///
    /// let mut log = WitnessLog::new();
    ///
    /// log.append(WitnessRecord {
    ///     tick: 1,
    ///     event_type: WitnessEventType::ForceClipping,
    ///     entity_ids: vec![5],
    ///     constraint_fired: "F_max".to_string(),
    ///     before_state: StateSnapshot::default(),
    ///     after_state: StateSnapshot::default(),
    ///     correction_applied: CorrectionDetails::default(),
    ///     invariant_improved: "bounded_force".to_string(),
    ///     delta_magnitude: 50.0,
    /// });
    ///
    /// assert_eq!(log.len(), 1);
    /// ```
    pub fn append(&mut self, record: WitnessRecord) {
        // Evict oldest if at capacity
        if let Some(cap) = self.max_capacity {
            while self.records.len() >= cap {
                self.records.remove(0);
                self.evicted_count += 1;
            }
        }

        self.records.push(record);
        self.total_appended += 1;
    }

    /// Get the number of records currently in the log.
    ///
    /// This may be less than `total_appended()` if records have been evicted.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get the total number of records ever appended.
    ///
    /// This includes records that have been evicted due to capacity limits.
    pub fn total_appended(&self) -> u64 {
        self.total_appended
    }

    /// Get the number of records that have been evicted.
    pub fn evicted_count(&self) -> u64 {
        self.evicted_count
    }

    /// Get all records in the log.
    ///
    /// Returns a slice of all currently stored records in chronological order.
    pub fn records(&self) -> &[WitnessRecord] {
        &self.records
    }

    /// Query records by event type.
    ///
    /// Returns all records matching the specified event type.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The event type to filter by
    ///
    /// # Returns
    ///
    /// A vector of references to matching records.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
    ///
    /// let mut log = WitnessLog::new();
    ///
    /// // Add various events
    /// log.append(WitnessRecord::new(1, WitnessEventType::OverlapCorrection, vec![1, 2], "test"));
    /// log.append(WitnessRecord::new(2, WitnessEventType::ForceClipping, vec![3], "test"));
    /// log.append(WitnessRecord::new(3, WitnessEventType::OverlapCorrection, vec![4, 5], "test"));
    ///
    /// let overlaps = log.query_by_type(WitnessEventType::OverlapCorrection);
    /// assert_eq!(overlaps.len(), 2);
    /// ```
    pub fn query_by_type(&self, event_type: WitnessEventType) -> Vec<&WitnessRecord> {
        self.records
            .iter()
            .filter(|r| r.event_type == event_type)
            .collect()
    }

    /// Query records by entity ID.
    ///
    /// Returns all records involving the specified entity.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - The entity ID to filter by
    ///
    /// # Returns
    ///
    /// A vector of references to matching records.
    pub fn query_by_entity(&self, entity_id: u64) -> Vec<&WitnessRecord> {
        self.records
            .iter()
            .filter(|r| r.involves_entity(entity_id))
            .collect()
    }

    /// Query records within a tick range.
    ///
    /// Returns all records with tick values in the range `[start, end]` (inclusive).
    ///
    /// # Arguments
    ///
    /// * `start_tick` - Start of the range (inclusive)
    /// * `end_tick` - End of the range (inclusive)
    ///
    /// # Returns
    ///
    /// A vector of references to matching records.
    pub fn query_by_tick_range(&self, start_tick: u64, end_tick: u64) -> Vec<&WitnessRecord> {
        self.records
            .iter()
            .filter(|r| r.tick >= start_tick && r.tick <= end_tick)
            .collect()
    }

    /// Query critical events only.
    ///
    /// Returns all records where `is_critical()` returns true.
    pub fn query_critical(&self) -> Vec<&WitnessRecord> {
        self.records.iter().filter(|r| r.is_critical()).collect()
    }

    /// Get the most recent record, if any.
    pub fn last(&self) -> Option<&WitnessRecord> {
        self.records.last()
    }

    /// Get the record at a specific index.
    pub fn get(&self, index: usize) -> Option<&WitnessRecord> {
        self.records.get(index)
    }

    /// Export the log to JSON format.
    ///
    /// Returns the complete log serialized as a JSON string with
    /// pretty-printing for human readability.
    ///
    /// # Returns
    ///
    /// `Ok(String)` containing the JSON representation, or an error
    /// if serialization fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
    ///
    /// let mut log = WitnessLog::new();
    /// log.append(WitnessRecord::new(1, WitnessEventType::ForceClipping, vec![1], "test"));
    ///
    /// let json = log.export_json().expect("serialization should succeed");
    /// assert!(json.contains("ForceClipping"));
    /// ```
    pub fn export_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self)
            .map_err(|e| FxnnError::Serialization(e))
    }

    /// Export the log to compact JSON format.
    ///
    /// Like `export_json()` but without pretty-printing, for smaller file sizes.
    pub fn export_json_compact(&self) -> Result<String> {
        serde_json::to_string(&self)
            .map_err(|e| FxnnError::Serialization(e))
    }

    /// Export only the records (without metadata) to JSON.
    ///
    /// This exports just the records array, not the full log structure.
    pub fn export_records_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.records)
            .map_err(|e| FxnnError::Serialization(e))
    }

    /// Import a log from JSON.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON string to parse
    ///
    /// # Returns
    ///
    /// `Ok(WitnessLog)` if parsing succeeds, or an error if the JSON is invalid.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| FxnnError::Serialization(e))
    }

    /// Clear all records from the log.
    ///
    /// Note: This updates `evicted_count` to reflect the cleared records.
    pub fn clear(&mut self) {
        self.evicted_count += self.records.len() as u64;
        self.records.clear();
    }

    /// Get statistics about the log.
    ///
    /// Returns a summary of the log contents including counts by event type.
    pub fn statistics(&self) -> WitnessLogStatistics {
        let mut stats = WitnessLogStatistics {
            total_records: self.records.len(),
            total_appended: self.total_appended,
            evicted_count: self.evicted_count,
            // Core physics events
            overlap_corrections: 0,
            energy_drift_corrections: 0,
            momentum_drift_corrections: 0,
            constraint_violations: 0,
            force_clippings: 0,
            numerical_instabilities: 0,
            // Governance events
            action_rejections: 0,
            rollbacks: 0,
            // Learning safety events
            gradient_clippings: 0,
            reward_clippings: 0,
            memory_rate_violations: 0,
            // Resource budget events
            compute_budget_violations: 0,
            bandwidth_violations: 0,
            // Critical events
            critical_events: 0,
        };

        for record in &self.records {
            match record.event_type {
                WitnessEventType::OverlapCorrection => stats.overlap_corrections += 1,
                WitnessEventType::EnergyDriftCorrection => stats.energy_drift_corrections += 1,
                WitnessEventType::MomentumDriftCorrection => stats.momentum_drift_corrections += 1,
                WitnessEventType::ConstraintViolation => stats.constraint_violations += 1,
                WitnessEventType::ForceClipping => stats.force_clippings += 1,
                WitnessEventType::NumericalInstability => stats.numerical_instabilities += 1,
                WitnessEventType::ActionRejected => stats.action_rejections += 1,
                WitnessEventType::RollbackTriggered => stats.rollbacks += 1,
                WitnessEventType::GradientClipping => stats.gradient_clippings += 1,
                WitnessEventType::RewardClipping => stats.reward_clippings += 1,
                WitnessEventType::MemoryRateLimitExceeded => stats.memory_rate_violations += 1,
                WitnessEventType::ComputeBudgetExceeded => stats.compute_budget_violations += 1,
                WitnessEventType::BandwidthLimitExceeded => stats.bandwidth_violations += 1,
            }
            if record.is_critical() {
                stats.critical_events += 1;
            }
        }

        stats
    }
}

/// Statistics about a witness log.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessLogStatistics {
    /// Number of records currently in the log.
    pub total_records: usize,
    /// Total number of records ever appended.
    pub total_appended: u64,
    /// Number of records evicted due to capacity limits.
    pub evicted_count: u64,

    // Core physics events
    /// Count of overlap correction events.
    pub overlap_corrections: usize,
    /// Count of energy drift correction events.
    pub energy_drift_corrections: usize,
    /// Count of momentum drift correction events.
    pub momentum_drift_corrections: usize,
    /// Count of constraint violation events.
    pub constraint_violations: usize,
    /// Count of force clipping events.
    pub force_clippings: usize,
    /// Count of numerical instability events.
    pub numerical_instabilities: usize,

    // Governance events
    /// Count of action rejection events.
    pub action_rejections: usize,
    /// Count of rollback events.
    pub rollbacks: usize,

    // Learning safety events
    /// Count of gradient clipping events.
    pub gradient_clippings: usize,
    /// Count of reward clipping events.
    pub reward_clippings: usize,
    /// Count of memory rate limit violations.
    pub memory_rate_violations: usize,

    // Resource budget events
    /// Count of compute budget violations.
    pub compute_budget_violations: usize,
    /// Count of bandwidth limit violations.
    pub bandwidth_violations: usize,

    /// Count of critical events.
    pub critical_events: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::snapshot::{StateSnapshot, CorrectionDetails};

    fn make_record(tick: u64, event_type: WitnessEventType) -> WitnessRecord {
        WitnessRecord {
            tick,
            event_type,
            entity_ids: vec![],
            constraint_fired: "test".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "test".to_string(),
            delta_magnitude: 0.0,
        }
    }

    #[test]
    fn test_log_append_and_query() {
        let mut log = WitnessLog::new();

        log.append(make_record(1, WitnessEventType::OverlapCorrection));
        log.append(make_record(2, WitnessEventType::ForceClipping));
        log.append(make_record(3, WitnessEventType::OverlapCorrection));

        assert_eq!(log.len(), 3);
        assert_eq!(log.total_appended(), 3);

        let overlaps = log.query_by_type(WitnessEventType::OverlapCorrection);
        assert_eq!(overlaps.len(), 2);

        let force = log.query_by_type(WitnessEventType::ForceClipping);
        assert_eq!(force.len(), 1);
    }

    #[test]
    fn test_log_capacity_eviction() {
        let mut log = WitnessLog::with_capacity(3);

        log.append(make_record(1, WitnessEventType::OverlapCorrection));
        log.append(make_record(2, WitnessEventType::OverlapCorrection));
        log.append(make_record(3, WitnessEventType::OverlapCorrection));

        assert_eq!(log.len(), 3);
        assert_eq!(log.evicted_count(), 0);

        // This should evict the oldest
        log.append(make_record(4, WitnessEventType::OverlapCorrection));

        assert_eq!(log.len(), 3);
        assert_eq!(log.evicted_count(), 1);
        assert_eq!(log.total_appended(), 4);

        // Oldest should be tick 2 now
        assert_eq!(log.get(0).unwrap().tick, 2);
    }

    #[test]
    fn test_log_tick_range_query() {
        let mut log = WitnessLog::new();

        for i in 1..=10 {
            log.append(make_record(i, WitnessEventType::ForceClipping));
        }

        let range = log.query_by_tick_range(3, 7);
        assert_eq!(range.len(), 5);
        assert!(range.iter().all(|r| r.tick >= 3 && r.tick <= 7));
    }

    #[test]
    fn test_log_entity_query() {
        let mut log = WitnessLog::new();

        log.append(WitnessRecord {
            tick: 1,
            event_type: WitnessEventType::OverlapCorrection,
            entity_ids: vec![1, 2],
            constraint_fired: "test".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "test".to_string(),
            delta_magnitude: 0.0,
        });

        log.append(WitnessRecord {
            tick: 2,
            event_type: WitnessEventType::OverlapCorrection,
            entity_ids: vec![2, 3],
            constraint_fired: "test".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "test".to_string(),
            delta_magnitude: 0.0,
        });

        let entity_1 = log.query_by_entity(1);
        assert_eq!(entity_1.len(), 1);

        let entity_2 = log.query_by_entity(2);
        assert_eq!(entity_2.len(), 2);

        let entity_3 = log.query_by_entity(3);
        assert_eq!(entity_3.len(), 1);
    }

    #[test]
    fn test_log_json_roundtrip() {
        let mut log = WitnessLog::new();
        log.append(make_record(1, WitnessEventType::RollbackTriggered));
        log.append(make_record(2, WitnessEventType::ForceClipping));

        let json = log.export_json().unwrap();
        let restored = WitnessLog::from_json(&json).unwrap();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored.get(0).unwrap().tick, 1);
        assert_eq!(restored.get(1).unwrap().tick, 2);
    }

    #[test]
    fn test_log_statistics() {
        let mut log = WitnessLog::new();

        log.append(make_record(1, WitnessEventType::OverlapCorrection));
        log.append(make_record(2, WitnessEventType::OverlapCorrection));
        log.append(make_record(3, WitnessEventType::ForceClipping));
        log.append(make_record(4, WitnessEventType::RollbackTriggered));

        let stats = log.statistics();

        assert_eq!(stats.total_records, 4);
        assert_eq!(stats.overlap_corrections, 2);
        assert_eq!(stats.force_clippings, 1);
        assert_eq!(stats.rollbacks, 1);
        assert_eq!(stats.critical_events, 1); // RollbackTriggered is critical
    }
}
