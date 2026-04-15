//! Audit logging for FXNN governance layer.
//!
//! This module provides append-only audit logging and witness records for
//! debugging, compliance, and verification of the simulated reality.
//!
//! # Overview
//!
//! The audit system maintains an immutable record of all significant events:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      AUDIT LOG STRUCTURE                        │
//! ├────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   AuditEntry          WitnessRecord                            │
//! │   ├── timestamp       ├── tick                                 │
//! │   ├── agent_id        ├── event_type                           │
//! │   ├── action          ├── entity_ids                           │
//! │   ├── result          ├── constraint_fired                     │
//! │   └── details         ├── before_state                         │
//! │                       ├── after_state                          │
//! │                       ├── correction_applied                   │
//! │                       └── delta_magnitude                      │
//! │                                                                 │
//! │   Purpose:                                                      │
//! │   • Post-mortem debugging                                       │
//! │   • Verification of corrections                                 │
//! │   • Governance compliance audit                                 │
//! │   • Training data for violation prediction                      │
//! │                                                                 │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use fxnn::governance::{AuditLog, AuditEntry, AuditResult, WitnessRecord, WitnessEventType};
//!
//! let mut log = AuditLog::new();
//!
//! // Log an authorized action
//! log.append(AuditEntry::action_authorized(1, fxnn::governance::ActionKind::Move, 10.0));
//!
//! // Query by agent
//! let agent_entries = log.query_by_agent(1);
//! assert!(!agent_entries.is_empty());
//! ```

use std::collections::VecDeque;
use std::time::{Instant, SystemTime};
use super::{AgentId, ActionKind, BudgetViolation, GovernanceError};
use serde::{Serialize, Deserialize};

/// Result of an audited action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    /// Action was authorized and executed
    Authorized,
    /// Action was denied
    Denied,
    /// Action failed during execution
    Failed,
    /// Action was modified (e.g., clamped)
    Modified,
    /// Budget violation occurred
    BudgetViolation,
    /// Memory access attempted
    MemoryAccess,
    /// Rollback was triggered
    Rollback,
}

/// A single entry in the audit log
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: u64,
    /// When the entry was created
    pub timestamp: Instant,
    /// Wall-clock time for external logging
    pub wall_time: SystemTime,
    /// Agent that triggered the event (if applicable)
    pub agent_id: Option<AgentId>,
    /// Type of action or event
    pub action: String,
    /// Result of the action
    pub result: AuditResult,
    /// Additional details
    pub details: String,
    /// Associated tick number
    pub tick: Option<u64>,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(action: impl Into<String>, result: AuditResult) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        Self {
            id: COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            timestamp: Instant::now(),
            wall_time: SystemTime::now(),
            agent_id: None,
            action: action.into(),
            result,
            details: String::new(),
            tick: None,
        }
    }

    /// Set the agent ID
    pub fn with_agent(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    /// Set the details
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = details.into();
        self
    }

    /// Set the tick
    pub fn with_tick(mut self, tick: u64) -> Self {
        self.tick = Some(tick);
        self
    }

    /// Create an entry for an authorized action
    pub fn action_authorized(agent_id: AgentId, action: ActionKind, energy_cost: f64) -> Self {
        Self::new(format!("{:?}", action), AuditResult::Authorized)
            .with_agent(agent_id)
            .with_details(format!("energy_cost={}", energy_cost))
    }

    /// Create an entry for a denied action
    pub fn action_denied(agent_id: AgentId, action: ActionKind, reason: &str) -> Self {
        Self::new(format!("{:?}", action), AuditResult::Denied)
            .with_agent(agent_id)
            .with_details(reason.to_string())
    }

    /// Create an entry for a memory write
    pub fn memory_write(agent_id: AgentId, region_id: u64) -> Self {
        Self::new("MemoryWrite", AuditResult::MemoryAccess)
            .with_agent(agent_id)
            .with_details(format!("region_id={}", region_id))
    }

    /// Create an entry for a budget violation
    pub fn budget_violation(violation: BudgetViolation) -> Self {
        Self::new("BudgetViolation", AuditResult::BudgetViolation)
            .with_details(format!("{:?}", violation))
    }

    /// Create an entry for a rollback
    pub fn rollback(reason: &str, from_tick: u64, to_tick: u64) -> Self {
        Self::new("Rollback", AuditResult::Rollback)
            .with_details(format!("reason={}, from={}, to={}", reason, from_tick, to_tick))
    }
}

/// Types of witness events (from ADR-001)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WitnessEventType {
    /// Two bodies were overlapping and were corrected
    OverlapCorrection,
    /// Energy exceeded budget and was corrected
    EnergyDriftCorrection,
    /// Constraint (joint limit, wall) was violated
    ConstraintViolation,
    /// Force exceeded maximum and was clipped
    ForceClipping,
    /// Governance denied an action
    ActionRejected,
    /// State was restored from checkpoint
    RollbackTriggered,
    /// Momentum drift exceeded threshold
    MomentumDriftCorrection,
    /// Numerical error (NaN/Inf) detected
    NumericalError,
    /// Budget limit reached
    BudgetExceeded,
    /// Memory access denied
    MemoryAccessDenied,
}

/// State snapshot for before/after comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Entity positions (serialized)
    pub positions: Vec<[f64; 3]>,
    /// Entity velocities (serialized)
    pub velocities: Vec<[f64; 3]>,
    /// Total energy
    pub total_energy: f64,
    /// Total momentum magnitude
    pub total_momentum: f64,
}

impl StateSnapshot {
    /// Create an empty snapshot
    pub fn empty() -> Self {
        Self {
            positions: Vec::new(),
            velocities: Vec::new(),
            total_energy: 0.0,
            total_momentum: 0.0,
        }
    }

    /// Create a snapshot with given energy and momentum
    pub fn with_energetics(total_energy: f64, total_momentum: f64) -> Self {
        Self {
            positions: Vec::new(),
            velocities: Vec::new(),
            total_energy,
            total_momentum,
        }
    }
}

/// Details of a correction that was applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionDetails {
    /// Type of correction
    pub correction_type: String,
    /// Entities affected
    pub affected_entities: Vec<u64>,
    /// Amount of correction applied
    pub magnitude: f64,
    /// Method used for correction
    pub method: String,
}

impl CorrectionDetails {
    /// Create a new correction details
    pub fn new(correction_type: impl Into<String>) -> Self {
        Self {
            correction_type: correction_type.into(),
            affected_entities: Vec::new(),
            magnitude: 0.0,
            method: String::new(),
        }
    }

    /// Add affected entities
    pub fn with_entities(mut self, entities: Vec<u64>) -> Self {
        self.affected_entities = entities;
        self
    }

    /// Set the magnitude
    pub fn with_magnitude(mut self, magnitude: f64) -> Self {
        self.magnitude = magnitude;
        self
    }

    /// Set the method
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = method.into();
        self
    }
}

/// A witness record for correction events (from ADR-001)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Simulation tick when event occurred
    pub tick: u64,
    /// Type of event
    pub event_type: WitnessEventType,
    /// Entities involved
    pub entity_ids: Vec<u64>,
    /// Constraint that fired (if applicable)
    pub constraint_fired: Option<String>,
    /// State before correction
    pub before_state: StateSnapshot,
    /// State after correction
    pub after_state: StateSnapshot,
    /// Details of the correction applied
    pub correction_applied: Option<CorrectionDetails>,
    /// Which invariant was improved
    pub invariant_improved: Option<String>,
    /// Magnitude of the correction (for metrics)
    pub delta_magnitude: f64,
    /// Wall-clock time
    pub timestamp: SystemTime,
}

impl WitnessRecord {
    /// Create a new witness record
    pub fn new(tick: u64, event_type: WitnessEventType) -> Self {
        Self {
            tick,
            event_type,
            entity_ids: Vec::new(),
            constraint_fired: None,
            before_state: StateSnapshot::empty(),
            after_state: StateSnapshot::empty(),
            correction_applied: None,
            invariant_improved: None,
            delta_magnitude: 0.0,
            timestamp: SystemTime::now(),
        }
    }

    /// Add entity IDs
    pub fn with_entities(mut self, ids: Vec<u64>) -> Self {
        self.entity_ids = ids;
        self
    }

    /// Set the constraint that fired
    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraint_fired = Some(constraint.into());
        self
    }

    /// Set before/after states
    pub fn with_states(mut self, before: StateSnapshot, after: StateSnapshot) -> Self {
        self.before_state = before;
        self.after_state = after;
        self
    }

    /// Set correction details
    pub fn with_correction(mut self, correction: CorrectionDetails) -> Self {
        self.delta_magnitude = correction.magnitude;
        self.correction_applied = Some(correction);
        self
    }

    /// Set the invariant that was improved
    pub fn with_invariant(mut self, invariant: impl Into<String>) -> Self {
        self.invariant_improved = Some(invariant.into());
        self
    }

    /// Create a witness from a governance error
    pub fn from_governance_error(error: &GovernanceError) -> Self {
        let event_type = match error {
            GovernanceError::UnauthorizedAction { .. } => WitnessEventType::ActionRejected,
            GovernanceError::BudgetExceeded { .. } => WitnessEventType::BudgetExceeded,
            GovernanceError::MemoryAccessDenied { .. } => WitnessEventType::MemoryAccessDenied,
            GovernanceError::InvalidCapability { .. } => WitnessEventType::ActionRejected,
            GovernanceError::RealityBudgetViolation { .. } => WitnessEventType::BudgetExceeded,
            GovernanceError::ActionBoundsExceeded { .. } => WitnessEventType::ForceClipping,
        };

        Self::new(0, event_type)
            .with_constraint(error.to_string())
    }

    /// Create a witness for an overlap correction
    pub fn overlap_correction(tick: u64, entity_a: u64, entity_b: u64, correction_magnitude: f64) -> Self {
        Self::new(tick, WitnessEventType::OverlapCorrection)
            .with_entities(vec![entity_a, entity_b])
            .with_constraint("minimum_separation")
            .with_correction(
                CorrectionDetails::new("overlap")
                    .with_entities(vec![entity_a, entity_b])
                    .with_magnitude(correction_magnitude)
                    .with_method("force_separation")
            )
            .with_invariant("no_overlap")
    }

    /// Create a witness for energy drift correction
    pub fn energy_drift_correction(tick: u64, before_energy: f64, after_energy: f64) -> Self {
        Self::new(tick, WitnessEventType::EnergyDriftCorrection)
            .with_states(
                StateSnapshot::with_energetics(before_energy, 0.0),
                StateSnapshot::with_energetics(after_energy, 0.0),
            )
            .with_correction(
                CorrectionDetails::new("energy_rescaling")
                    .with_magnitude((before_energy - after_energy).abs())
                    .with_method("velocity_rescaling")
            )
            .with_invariant("bounded_energy")
    }

    /// Create a witness for a rollback
    pub fn rollback(tick: u64, reason: &str, from_tick: u64, to_tick: u64) -> Self {
        Self::new(tick, WitnessEventType::RollbackTriggered)
            .with_constraint(format!("rollback: {} -> {}", from_tick, to_tick))
            .with_correction(
                CorrectionDetails::new("state_rollback")
                    .with_method(reason.to_string())
            )
    }
}

/// Append-only audit log
#[derive(Debug)]
pub struct AuditLog {
    /// Regular audit entries
    entries: VecDeque<AuditEntry>,
    /// Witness records for correction events
    witnesses: VecDeque<WitnessRecord>,
    /// Maximum number of entries to retain
    max_entries: usize,
    /// Maximum number of witnesses to retain
    max_witnesses: usize,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    /// Create a new audit log
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            witnesses: VecDeque::new(),
            max_entries: 100_000,
            max_witnesses: 10_000,
        }
    }

    /// Create with custom capacity limits
    pub fn with_capacity(max_entries: usize, max_witnesses: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries.min(10_000)),
            witnesses: VecDeque::with_capacity(max_witnesses.min(1_000)),
            max_entries,
            max_witnesses,
        }
    }

    /// Append an audit entry
    pub fn append(&mut self, entry: AuditEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Append a witness record
    pub fn append_witness(&mut self, witness: WitnessRecord) {
        if self.witnesses.len() >= self.max_witnesses {
            self.witnesses.pop_front();
        }
        self.witnesses.push_back(witness);
    }

    /// Query entries by agent ID
    pub fn query_by_agent(&self, agent_id: AgentId) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.agent_id == Some(agent_id))
            .collect()
    }

    /// Query entries by result type
    pub fn query_by_result(&self, result: &AuditResult) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| &e.result == result)
            .collect()
    }

    /// Query entries within a tick range
    pub fn query_by_tick_range(&self, start: u64, end: u64) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(tick) = e.tick {
                    tick >= start && tick <= end
                } else {
                    false
                }
            })
            .collect()
    }

    /// Query witnesses by event type
    pub fn query_witnesses_by_type(&self, event_type: &WitnessEventType) -> Vec<&WitnessRecord> {
        self.witnesses
            .iter()
            .filter(|w| &w.event_type == event_type)
            .collect()
    }

    /// Query witnesses within a tick range
    pub fn query_witnesses_by_tick(&self, start: u64, end: u64) -> Vec<&WitnessRecord> {
        self.witnesses
            .iter()
            .filter(|w| w.tick >= start && w.tick <= end)
            .collect()
    }

    /// Get recent witnesses (last N)
    pub fn recent_witnesses(&self, count: usize) -> Vec<&WitnessRecord> {
        self.witnesses.iter().rev().take(count).collect()
    }

    /// Get all entries (for export)
    pub fn all_entries(&self) -> impl Iterator<Item = &AuditEntry> {
        self.entries.iter()
    }

    /// Get all witnesses (for export)
    pub fn all_witnesses(&self) -> impl Iterator<Item = &WitnessRecord> {
        self.witnesses.iter()
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get witness count
    pub fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    /// Clear all entries (use with caution)
    pub fn clear(&mut self) {
        self.entries.clear();
        self.witnesses.clear();
    }

    /// Get summary statistics
    pub fn summary(&self) -> AuditSummary {
        let mut authorized = 0;
        let mut denied = 0;
        let mut violations = 0;
        let mut rollbacks = 0;

        for entry in &self.entries {
            match entry.result {
                AuditResult::Authorized => authorized += 1,
                AuditResult::Denied => denied += 1,
                AuditResult::BudgetViolation => violations += 1,
                AuditResult::Rollback => rollbacks += 1,
                _ => {}
            }
        }

        AuditSummary {
            total_entries: self.entries.len(),
            total_witnesses: self.witnesses.len(),
            authorized_actions: authorized,
            denied_actions: denied,
            budget_violations: violations,
            rollbacks,
        }
    }
}

/// Summary statistics for the audit log
#[derive(Debug, Clone)]
pub struct AuditSummary {
    /// Total number of entries
    pub total_entries: usize,
    /// Total number of witness records
    pub total_witnesses: usize,
    /// Number of authorized actions
    pub authorized_actions: usize,
    /// Number of denied actions
    pub denied_actions: usize,
    /// Number of budget violations
    pub budget_violations: usize,
    /// Number of rollbacks
    pub rollbacks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::action_authorized(1, ActionKind::Move, 10.0);
        assert_eq!(entry.agent_id, Some(1));
        assert_eq!(entry.result, AuditResult::Authorized);
    }

    #[test]
    fn test_audit_log_append() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::action_authorized(1, ActionKind::Move, 10.0));
        log.append(AuditEntry::action_denied(2, ActionKind::Admin, "not authorized"));

        assert_eq!(log.entry_count(), 2);
    }

    #[test]
    fn test_audit_log_query() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::action_authorized(1, ActionKind::Move, 10.0));
        log.append(AuditEntry::action_authorized(1, ActionKind::Observe, 0.0));
        log.append(AuditEntry::action_denied(2, ActionKind::Admin, "not authorized"));

        let agent1_entries = log.query_by_agent(1);
        assert_eq!(agent1_entries.len(), 2);

        let denied_entries = log.query_by_result(&AuditResult::Denied);
        assert_eq!(denied_entries.len(), 1);
    }

    #[test]
    fn test_witness_record() {
        let witness = WitnessRecord::overlap_correction(100, 1, 2, 0.5);

        assert_eq!(witness.tick, 100);
        assert_eq!(witness.event_type, WitnessEventType::OverlapCorrection);
        assert_eq!(witness.entity_ids, vec![1, 2]);
        assert!(witness.correction_applied.is_some());
    }

    #[test]
    fn test_audit_log_capacity() {
        let mut log = AuditLog::with_capacity(10, 5);

        for i in 0..20 {
            log.append(AuditEntry::action_authorized(i as u64, ActionKind::Move, 0.0));
        }

        // Should only keep last 10
        assert_eq!(log.entry_count(), 10);
    }

    #[test]
    fn test_audit_summary() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::new("test", AuditResult::Authorized));
        log.append(AuditEntry::new("test", AuditResult::Authorized));
        log.append(AuditEntry::new("test", AuditResult::Denied));
        log.append(AuditEntry::new("test", AuditResult::BudgetViolation));

        let summary = log.summary();
        assert_eq!(summary.total_entries, 4);
        assert_eq!(summary.authorized_actions, 2);
        assert_eq!(summary.denied_actions, 1);
        assert_eq!(summary.budget_violations, 1);
    }

    #[test]
    fn test_witness_from_error() {
        let error = GovernanceError::UnauthorizedAction {
            agent_id: 1,
            role: 0,
            action: ActionKind::Admin,
        };

        let witness = WitnessRecord::from_governance_error(&error);
        assert_eq!(witness.event_type, WitnessEventType::ActionRejected);
    }
}
