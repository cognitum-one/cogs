//! Witness Log System for FXNN Reality Substrate
//!
//! The witness module provides an append-only audit log for tracking all correction
//! events, constraint violations, and state changes in the simulated reality.
//! This is a first-class output that enables debugging, verification, and governance.
//!
//! # Overview
//!
//! Every correction event emits a witness record. This is the difference between
//! a simulator and a reality substrate you can debug and trust.
//!
//! ## Key Components
//!
//! - [`WitnessLog`]: Append-only log for recording events
//! - [`WitnessRecord`]: Individual event record with full state information
//! - [`WitnessEventType`]: Classification of event types
//! - [`StateSnapshot`]: Captured state before/after corrections
//! - [`CorrectionDetails`]: Information about applied corrections
//!
//! # Witness Log Uses
//!
//! - Post-mortem debugging of unexpected behavior
//! - Verification that corrections converged
//! - Audit trail for governance compliance
//! - Training data for learning which states lead to violations
//!
//! # Examples
//!
//! ## Recording an Overlap Correction
//!
//! ```rust,ignore
//! use fxnn::witness::{WitnessLog, WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
//!
//! let mut log = WitnessLog::new();
//!
//! let record = WitnessRecord {
//!     tick: 42,
//!     event_type: WitnessEventType::OverlapCorrection,
//!     entity_ids: vec![1, 2],
//!     constraint_fired: "LennardJones::min_distance".to_string(),
//!     before_state: StateSnapshot::default(),
//!     after_state: StateSnapshot::default(),
//!     correction_applied: CorrectionDetails::default(),
//!     invariant_improved: "no_overlap".to_string(),
//!     delta_magnitude: 0.15,
//! };
//!
//! log.append(record);
//! ```
//!
//! ## Querying Events
//!
//! ```rust,ignore
//! use fxnn::witness::{WitnessLog, WitnessEventType};
//!
//! let log = WitnessLog::new();
//! // ... events are recorded ...
//!
//! // Find all overlap corrections
//! let overlaps = log.query_by_type(WitnessEventType::OverlapCorrection);
//!
//! // Export to JSON for analysis
//! let json = log.export_json().expect("serialization failed");
//! ```
//!
//! # Design Principles
//!
//! 1. **Append-Only**: Records cannot be modified once written (audit trail integrity)
//! 2. **Complete State**: Before/after snapshots enable full reconstruction
//! 3. **Serializable**: Full serde support for persistence and analysis
//! 4. **Queryable**: Efficient filtering by event type, entity, or time range
//!
//! # References
//!
//! - ADR-001: FXNN as Simulated Reality Substrate (Section VII: Proof Strategies)

mod events;
mod log;
mod snapshot;

pub use events::{WitnessEventType, WitnessRecord};
pub use log::WitnessLog;
pub use snapshot::{StateSnapshot, CorrectionDetails, Checkpoint, CheckpointManager};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_witness_log_append_and_query() {
        let mut log = WitnessLog::new();

        // Append some records
        log.append(WitnessRecord {
            tick: 1,
            event_type: WitnessEventType::OverlapCorrection,
            entity_ids: vec![1, 2],
            constraint_fired: "min_distance".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "no_overlap".to_string(),
            delta_magnitude: 0.1,
        });

        log.append(WitnessRecord {
            tick: 2,
            event_type: WitnessEventType::EnergyDriftCorrection,
            entity_ids: vec![],
            constraint_fired: "energy_budget".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "bounded_energy".to_string(),
            delta_magnitude: 0.05,
        });

        assert_eq!(log.len(), 2);

        // Query by type
        let overlaps = log.query_by_type(WitnessEventType::OverlapCorrection);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].tick, 1);

        let energy = log.query_by_type(WitnessEventType::EnergyDriftCorrection);
        assert_eq!(energy.len(), 1);
        assert_eq!(energy[0].tick, 2);
    }

    #[test]
    fn test_witness_log_export_json() {
        let mut log = WitnessLog::new();

        log.append(WitnessRecord {
            tick: 1,
            event_type: WitnessEventType::ForceClipping,
            entity_ids: vec![5],
            constraint_fired: "F_max".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "bounded_force".to_string(),
            delta_magnitude: 100.0,
        });

        let json = log.export_json().expect("serialization should succeed");
        assert!(json.contains("ForceClipping"));
        assert!(json.contains("F_max"));
    }

    #[test]
    fn test_checkpoint_manager() {
        let mut manager = CheckpointManager::new(3);

        // Create and save checkpoints
        manager.save(Checkpoint::new(1, StateSnapshot::default()));
        manager.save(Checkpoint::new(2, StateSnapshot::default()));
        manager.save(Checkpoint::new(3, StateSnapshot::default()));

        assert_eq!(manager.len(), 3);

        // Saving a 4th should evict the oldest
        manager.save(Checkpoint::new(4, StateSnapshot::default()));
        assert_eq!(manager.len(), 3);

        // The oldest (tick 1) should be gone
        assert!(manager.get_by_tick(1).is_none());
        assert!(manager.get_by_tick(2).is_some());
        assert!(manager.get_by_tick(4).is_some());
    }
}
