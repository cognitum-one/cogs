//! Event types for the Witness Log system.
//!
//! This module defines the core event structures as specified in ADR-001.
//! Each witness record captures a complete snapshot of a correction event,
//! including before/after states and the applied correction.

use serde::{Deserialize, Serialize};
use super::snapshot::{StateSnapshot, CorrectionDetails};

/// Classification of witness events.
///
/// Each event type represents a different kind of correction or violation
/// that the reality substrate detected and responded to.
///
/// # Event Types (ADR-001 Compliant)
///
/// | Type | Description | Typical Cause |
/// |------|-------------|---------------|
/// | `OverlapCorrection` | Two bodies were overlapping | Atoms closer than sigma_min |
/// | `EnergyDriftCorrection` | Energy exceeded budget | Numerical integration drift |
/// | `ConstraintViolation` | Joint limit or wall penetration | Physical constraint breach |
/// | `ForceClipping` | Force exceeded F_max | Extreme proximity or bad config |
/// | `ActionRejected` | Governance denied action | Unauthorized agent action |
/// | `RollbackTriggered` | State restored from checkpoint | Budget violation recovery |
/// | `MomentumDriftCorrection` | Momentum exceeded budget | Conservation law violation |
/// | `NumericalInstability` | NaN or Inf detected | Simulation divergence |
/// | `GradientClipping` | Learning gradient was clipped | Policy update too large |
/// | `RewardClipping` | Reward signal was clipped | Extreme reward value |
/// | `MemoryRateLimitExceeded` | Memory write rate exceeded | Learning rate limit hit |
/// | `ComputeBudgetExceeded` | Compute budget exhausted | Agent compute limit hit |
/// | `BandwidthLimitExceeded` | Observation bandwidth exhausted | Perception rate limit |
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::WitnessEventType;
///
/// let event = WitnessEventType::OverlapCorrection;
/// assert!(matches!(event, WitnessEventType::OverlapCorrection));
///
/// // Events are serializable
/// let json = serde_json::to_string(&event).unwrap();
/// assert_eq!(json, "\"OverlapCorrection\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WitnessEventType {
    // =========================================================================
    // Core Physics Events (ADR-001 Part II-A)
    // =========================================================================

    /// Two bodies were overlapping (distance < sigma_min).
    ///
    /// This indicates the Lennard-Jones repulsion or similar short-range
    /// force pushed the atoms apart to restore a valid configuration.
    OverlapCorrection,

    /// Energy exceeded the configured budget.
    ///
    /// The system applied energy correction (e.g., velocity scaling or
    /// thermostat coupling) to bring energy back within bounds.
    EnergyDriftCorrection,

    /// Momentum exceeded the configured budget.
    ///
    /// The system detected momentum drift beyond the allowed threshold
    /// (<10^-6 per 10,000 steps as per ADR-001).
    MomentumDriftCorrection,

    /// A physical constraint was violated.
    ///
    /// This includes joint limits, rigid body constraints, wall penetrations,
    /// and other kinematic constraints that were projected back to validity.
    ConstraintViolation,

    /// Force magnitude exceeded the maximum allowed value.
    ///
    /// The force was clipped to F_max to prevent numerical instability.
    /// This often indicates particles are too close together.
    ForceClipping,

    /// Numerical instability detected (NaN or Inf).
    ///
    /// This is a critical event requiring immediate state rollback.
    /// ADR-001: Emergency state rollback on NaN/Inf.
    NumericalInstability,

    // =========================================================================
    // Governance Events (ADR-001 Part II-B)
    // =========================================================================

    /// An agent action was rejected by the governance layer.
    ///
    /// The action was not in the agent's allowed set, exceeded budget,
    /// or violated authority boundaries.
    ActionRejected,

    /// State was restored from a checkpoint.
    ///
    /// A severe violation (e.g., 10x baseline drift after learning)
    /// triggered rollback to the last known good state.
    RollbackTriggered,

    // =========================================================================
    // Learning Safety Events (ADR-001 Part II-B Learning Bounds)
    // =========================================================================

    /// Learning gradient was clipped to maximum norm.
    ///
    /// ADR-001: Max policy update magnitude enforcement.
    GradientClipping,

    /// Reward signal was clipped to bounded range.
    ///
    /// ADR-001: Reward signal bound |R| < R_max.
    RewardClipping,

    /// Memory modification rate limit was exceeded.
    ///
    /// The agent attempted more memory writes than allowed per tick.
    MemoryRateLimitExceeded,

    // =========================================================================
    // Resource Budget Events (ADR-001 Part II-B Agent Budgets)
    // =========================================================================

    /// Agent compute budget was exhausted.
    ///
    /// The agent's FLOPs allocation for this tick was exceeded.
    ComputeBudgetExceeded,

    /// Observation bandwidth limit was exceeded.
    ///
    /// The agent's observation byte budget was exhausted.
    BandwidthLimitExceeded,
}

impl WitnessEventType {
    /// Returns a human-readable description of this event type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::WitnessEventType;
    ///
    /// let desc = WitnessEventType::OverlapCorrection.description();
    /// assert_eq!(desc, "Two bodies were overlapping");
    /// ```
    pub fn description(&self) -> &'static str {
        match self {
            Self::OverlapCorrection => "Two bodies were overlapping",
            Self::EnergyDriftCorrection => "Energy exceeded budget",
            Self::MomentumDriftCorrection => "Momentum exceeded budget",
            Self::ConstraintViolation => "Physical constraint violated",
            Self::ForceClipping => "Force exceeded maximum",
            Self::NumericalInstability => "NaN or Inf detected",
            Self::ActionRejected => "Agent action denied by governance",
            Self::RollbackTriggered => "State restored from checkpoint",
            Self::GradientClipping => "Learning gradient clipped",
            Self::RewardClipping => "Reward signal clipped",
            Self::MemoryRateLimitExceeded => "Memory write rate exceeded",
            Self::ComputeBudgetExceeded => "Compute budget exhausted",
            Self::BandwidthLimitExceeded => "Observation bandwidth exceeded",
        }
    }

    /// Returns the severity level of this event type.
    ///
    /// Severity levels:
    /// - 1: Informational (normal operation)
    /// - 2: Warning (potential issue)
    /// - 3: Error (significant violation)
    /// - 4: Critical (system recovery required)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::witness::WitnessEventType;
    ///
    /// assert_eq!(WitnessEventType::ForceClipping.severity(), 2);
    /// assert_eq!(WitnessEventType::RollbackTriggered.severity(), 4);
    /// ```
    pub fn severity(&self) -> u8 {
        match self {
            // Warning level (2) - common corrections
            Self::ForceClipping => 2,
            Self::OverlapCorrection => 2,
            Self::EnergyDriftCorrection => 2,
            Self::MomentumDriftCorrection => 2,
            Self::GradientClipping => 2,
            Self::RewardClipping => 2,
            Self::BandwidthLimitExceeded => 2,

            // Error level (3) - significant violations
            Self::ConstraintViolation => 3,
            Self::ActionRejected => 3,
            Self::MemoryRateLimitExceeded => 3,
            Self::ComputeBudgetExceeded => 3,

            // Critical level (4) - system recovery required
            Self::RollbackTriggered => 4,
            Self::NumericalInstability => 4,
        }
    }

    /// Returns true if this event type represents a critical issue.
    ///
    /// Critical events may require immediate attention or indicate
    /// a fundamental problem with the simulation configuration.
    pub fn is_critical(&self) -> bool {
        self.severity() >= 4
    }
}

impl std::fmt::Display for WitnessEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A complete witness record capturing a single correction event.
///
/// This is the core data structure of the witness log system. Each record
/// contains all information needed to understand what happened, why it
/// happened, and what was done about it.
///
/// # Fields
///
/// | Field | Description |
/// |-------|-------------|
/// | `tick` | Simulation timestep when event occurred |
/// | `event_type` | Classification of the event |
/// | `entity_ids` | IDs of entities involved |
/// | `constraint_fired` | Name of the constraint that triggered |
/// | `before_state` | State snapshot before correction |
/// | `after_state` | State snapshot after correction |
/// | `correction_applied` | Details of the correction |
/// | `invariant_improved` | Name of the invariant restored |
/// | `delta_magnitude` | Magnitude of the change applied |
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::{WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
///
/// let record = WitnessRecord {
///     tick: 42,
///     event_type: WitnessEventType::OverlapCorrection,
///     entity_ids: vec![1, 2],
///     constraint_fired: "LennardJones::sigma_min".to_string(),
///     before_state: StateSnapshot::default(),
///     after_state: StateSnapshot::default(),
///     correction_applied: CorrectionDetails::default(),
///     invariant_improved: "no_overlap".to_string(),
///     delta_magnitude: 0.15,
/// };
///
/// assert_eq!(record.tick, 42);
/// assert_eq!(record.entity_ids.len(), 2);
/// ```
///
/// # Serialization
///
/// Records are fully serializable to JSON for persistence and analysis:
///
/// ```rust
/// use fxnn::witness::{WitnessRecord, WitnessEventType, StateSnapshot, CorrectionDetails};
///
/// let record = WitnessRecord {
///     tick: 1,
///     event_type: WitnessEventType::ForceClipping,
///     entity_ids: vec![5],
///     constraint_fired: "F_max".to_string(),
///     before_state: StateSnapshot::default(),
///     after_state: StateSnapshot::default(),
///     correction_applied: CorrectionDetails::default(),
///     invariant_improved: "bounded_force".to_string(),
///     delta_magnitude: 100.0,
/// };
///
/// let json = serde_json::to_string(&record).unwrap();
/// let restored: WitnessRecord = serde_json::from_str(&json).unwrap();
/// assert_eq!(restored.tick, 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Simulation timestep when this event occurred.
    pub tick: u64,

    /// Classification of the event type.
    pub event_type: WitnessEventType,

    /// IDs of entities (atoms, agents) involved in this event.
    ///
    /// For pairwise interactions (e.g., overlap), contains both entity IDs.
    /// For global events (e.g., energy correction), may be empty.
    pub entity_ids: Vec<u64>,

    /// Name of the constraint or rule that fired.
    ///
    /// Examples: "LennardJones::sigma_min", "energy_budget", "F_max"
    pub constraint_fired: String,

    /// State snapshot captured before the correction was applied.
    pub before_state: StateSnapshot,

    /// State snapshot captured after the correction was applied.
    pub after_state: StateSnapshot,

    /// Details about the correction that was applied.
    pub correction_applied: CorrectionDetails,

    /// Name of the invariant that was improved or restored.
    ///
    /// Examples: "no_overlap", "bounded_energy", "momentum_conservation"
    pub invariant_improved: String,

    /// Magnitude of the change applied during correction.
    ///
    /// For position corrections, this is the distance moved.
    /// For energy corrections, this is the energy change.
    /// For force clipping, this is the force reduction.
    pub delta_magnitude: f64,
}

impl WitnessRecord {
    /// Create a new witness record with the given parameters.
    ///
    /// This is a convenience constructor that takes all required fields.
    /// For more complex construction, build the struct directly.
    ///
    /// # Arguments
    ///
    /// * `tick` - Simulation timestep
    /// * `event_type` - Type of event
    /// * `entity_ids` - IDs of involved entities
    /// * `constraint_fired` - Name of triggered constraint
    ///
    /// # Returns
    ///
    /// A new `WitnessRecord` with default state snapshots and correction details.
    pub fn new(
        tick: u64,
        event_type: WitnessEventType,
        entity_ids: Vec<u64>,
        constraint_fired: impl Into<String>,
    ) -> Self {
        Self {
            tick,
            event_type,
            entity_ids,
            constraint_fired: constraint_fired.into(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: String::new(),
            delta_magnitude: 0.0,
        }
    }

    /// Set the before and after state snapshots.
    pub fn with_states(mut self, before: StateSnapshot, after: StateSnapshot) -> Self {
        self.before_state = before;
        self.after_state = after;
        self
    }

    /// Set the correction details.
    pub fn with_correction(mut self, correction: CorrectionDetails) -> Self {
        self.correction_applied = correction;
        self
    }

    /// Set the invariant that was improved.
    pub fn with_invariant(mut self, invariant: impl Into<String>) -> Self {
        self.invariant_improved = invariant.into();
        self
    }

    /// Set the delta magnitude.
    pub fn with_delta(mut self, delta: f64) -> Self {
        self.delta_magnitude = delta;
        self
    }

    /// Check if this record involves a specific entity.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - The entity ID to check for
    ///
    /// # Returns
    ///
    /// `true` if the entity is involved in this event.
    pub fn involves_entity(&self, entity_id: u64) -> bool {
        self.entity_ids.contains(&entity_id)
    }

    /// Check if this is a critical event.
    ///
    /// Delegates to the event type's criticality check.
    pub fn is_critical(&self) -> bool {
        self.event_type.is_critical()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_serialization() {
        let event = WitnessEventType::OverlapCorrection;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"OverlapCorrection\"");

        let restored: WitnessEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, event);
    }

    #[test]
    fn test_event_type_severity() {
        assert_eq!(WitnessEventType::ForceClipping.severity(), 2);
        assert_eq!(WitnessEventType::ConstraintViolation.severity(), 3);
        assert_eq!(WitnessEventType::RollbackTriggered.severity(), 4);
    }

    #[test]
    fn test_witness_record_builder() {
        let record = WitnessRecord::new(
            100,
            WitnessEventType::EnergyDriftCorrection,
            vec![],
            "energy_budget",
        )
        .with_invariant("bounded_energy")
        .with_delta(0.01);

        assert_eq!(record.tick, 100);
        assert_eq!(record.invariant_improved, "bounded_energy");
        assert!((record.delta_magnitude - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_witness_record_involves_entity() {
        let record = WitnessRecord::new(
            1,
            WitnessEventType::OverlapCorrection,
            vec![5, 10],
            "min_distance",
        );

        assert!(record.involves_entity(5));
        assert!(record.involves_entity(10));
        assert!(!record.involves_entity(15));
    }

    #[test]
    fn test_witness_record_serialization() {
        let record = WitnessRecord {
            tick: 42,
            event_type: WitnessEventType::OverlapCorrection,
            entity_ids: vec![1, 2],
            constraint_fired: "test_constraint".to_string(),
            before_state: StateSnapshot::default(),
            after_state: StateSnapshot::default(),
            correction_applied: CorrectionDetails::default(),
            invariant_improved: "test_invariant".to_string(),
            delta_magnitude: 0.5,
        };

        let json = serde_json::to_string(&record).unwrap();
        let restored: WitnessRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.tick, 42);
        assert_eq!(restored.entity_ids, vec![1, 2]);
        assert_eq!(restored.constraint_fired, "test_constraint");
    }
}
