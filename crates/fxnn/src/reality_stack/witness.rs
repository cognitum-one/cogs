//! # Witness Module
//!
//! The witness module provides logging, snapshotting, and event tracking
//! for the Reality Stack. This enables:
//!
//! - **Reproducibility**: Replay simulations from recorded events
//! - **Debugging**: Trace agent decisions and outcomes
//! - **Analysis**: Post-hoc analysis of emergent behaviors
//! - **Auditing**: Verify that governance policies were followed

use super::agency::{AgentId, ValidatedAction};
use super::physics::PhysicsResult;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

// ============================================================================
// Events
// ============================================================================

/// Kind of event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    /// Simulation step started
    StepStart { step: u64 },

    /// Simulation step ended
    StepEnd { step: u64, duration_us: u64 },

    /// Physics advanced
    Physics(PhysicsEventData),

    /// Agent action executed
    Action {
        agent_id: AgentId,
        action: super::agency::ActionKind,
        success: bool,
    },

    /// Observation generated
    Observation {
        agent_id: AgentId,
        n_values: usize,
    },

    /// Memory updated
    MemoryUpdate {
        agent_id: AgentId,
        entries_added: usize,
    },

    /// Goal achieved
    GoalAchieved {
        agent_id: AgentId,
        goal_name: String,
    },

    /// Conservation law violation
    ConservationViolation {
        law: String,
        drift: f64,
    },

    /// Budget warning
    BudgetWarning {
        agent_id: AgentId,
        resource: String,
        usage: f32,
    },

    /// Custom event
    Custom {
        name: String,
        data: String,
    },
}

/// Physics event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsEventData {
    /// Step number
    pub step: u64,
    /// Total energy
    pub energy: f64,
    /// Temperature
    pub temperature: f32,
    /// Conservation valid
    pub conservation_valid: bool,
}

/// A recorded event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event ID
    pub id: u64,
    /// Simulation step when event occurred
    pub step: u64,
    /// Timestamp (relative to simulation start)
    pub timestamp_us: u64,
    /// Event kind
    pub kind: EventKind,
}

impl Event {
    /// Create a new event
    fn new(id: u64, step: u64, timestamp_us: u64, kind: EventKind) -> Self {
        Self {
            id,
            step,
            timestamp_us,
            kind,
        }
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// Snapshot of simulation state
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Step when snapshot was taken
    pub step: u64,
    /// Time when snapshot was taken
    pub timestamp: Instant,
    /// Stack ID
    pub stack_id: super::StackId,
}

impl Snapshot {
    /// Create a new snapshot
    pub fn new(step: u64, stack_id: super::StackId) -> Self {
        Self {
            step,
            timestamp: Instant::now(),
            stack_id,
        }
    }

    /// Get age in milliseconds
    pub fn age_ms(&self) -> u128 {
        self.timestamp.elapsed().as_millis()
    }
}

/// Full state snapshot including atoms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSnapshot {
    /// Step number
    pub step: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Atom positions
    pub positions: Vec<[f32; 3]>,
    /// Atom velocities
    pub velocities: Vec<[f32; 3]>,
    /// Agent states
    pub agent_states: std::collections::HashMap<AgentId, Vec<f32>>,
    /// Total energy
    pub total_energy: f64,
}

// ============================================================================
// Witness Log
// ============================================================================

/// Main witness log for recording events
#[derive(Debug)]
pub struct WitnessLog {
    /// All recorded events
    events: VecDeque<Event>,
    /// Maximum events to keep in memory
    max_events: usize,
    /// Event counter
    counter: u64,
    /// Simulation start time
    start_time: Instant,
    /// Current step being recorded
    current_step: u64,
    /// Step start time
    step_start: Option<Instant>,
    /// Snapshots
    snapshots: VecDeque<FullSnapshot>,
    /// Maximum snapshots to keep
    max_snapshots: usize,
    /// Snapshot interval
    snapshot_interval: u64,
    /// Event filters
    filters: Vec<EventFilter>,
}

/// Filter for events
#[derive(Debug, Clone)]
pub enum EventFilter {
    /// Only physics events
    PhysicsOnly,
    /// Only action events
    ActionsOnly,
    /// Only for specific agent
    AgentOnly(AgentId),
    /// Only errors/warnings
    ErrorsOnly,
    /// Custom filter function
    Custom(String),
}

impl WitnessLog {
    /// Create a new witness log
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            max_events: 100000,
            counter: 0,
            start_time: Instant::now(),
            current_step: 0,
            step_start: None,
            snapshots: VecDeque::new(),
            max_snapshots: 100,
            snapshot_interval: 100,
            filters: Vec::new(),
        }
    }

    /// Set maximum events
    pub fn with_max_events(mut self, max: usize) -> Self {
        self.max_events = max;
        self
    }

    /// Set snapshot interval
    pub fn with_snapshot_interval(mut self, interval: u64) -> Self {
        self.snapshot_interval = interval;
        self
    }

    /// Add event filter
    pub fn with_filter(mut self, filter: EventFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Begin recording a step
    pub fn begin_step(&mut self, step: u64) {
        self.current_step = step;
        self.step_start = Some(Instant::now());
        self.record_event(EventKind::StepStart { step });
    }

    /// End recording a step
    pub fn end_step(&mut self) {
        let duration = self.step_start
            .map(|s| s.elapsed().as_micros() as u64)
            .unwrap_or(0);

        self.record_event(EventKind::StepEnd {
            step: self.current_step,
            duration_us: duration,
        });

        self.step_start = None;
    }

    /// Record physics result
    pub fn record_physics(&mut self, result: PhysicsResult) {
        self.record_event(EventKind::Physics(PhysicsEventData {
            step: result.step,
            energy: result.total_energy,
            temperature: result.temperature,
            conservation_valid: result.conservation.all_valid,
        }));

        // Record conservation violations
        if !result.conservation.all_valid {
            for (law, &drift) in &result.conservation.drift {
                self.record_event(EventKind::ConservationViolation {
                    law: format!("{:?}", law),
                    drift,
                });
            }
        }
    }

    /// Record validated actions
    pub fn record_actions(&mut self, actions: &[ValidatedAction]) {
        for action in actions {
            self.record_event(EventKind::Action {
                agent_id: action.agent_id,
                action: action.kind.clone(),
                success: true,
            });
        }
    }

    /// Record custom event
    pub fn record_custom(&mut self, name: &str, data: &str) {
        self.record_event(EventKind::Custom {
            name: name.to_string(),
            data: data.to_string(),
        });
    }

    /// Record an event
    fn record_event(&mut self, kind: EventKind) {
        // Check filters
        if !self.filters.is_empty() && !self.should_record(&kind) {
            return;
        }

        let event = Event::new(
            self.counter,
            self.current_step,
            self.start_time.elapsed().as_micros() as u64,
            kind,
        );

        self.counter += 1;

        // Maintain max events
        if self.events.len() >= self.max_events {
            self.events.pop_front();
        }

        self.events.push_back(event);
    }

    /// Check if event should be recorded based on filters
    fn should_record(&self, kind: &EventKind) -> bool {
        for filter in &self.filters {
            match filter {
                EventFilter::PhysicsOnly => {
                    if !matches!(kind, EventKind::Physics(_)) {
                        return false;
                    }
                }
                EventFilter::ActionsOnly => {
                    if !matches!(kind, EventKind::Action { .. }) {
                        return false;
                    }
                }
                EventFilter::AgentOnly(agent_id) => {
                    match kind {
                        EventKind::Action { agent_id: a, .. } |
                        EventKind::Observation { agent_id: a, .. } |
                        EventKind::MemoryUpdate { agent_id: a, .. } |
                        EventKind::GoalAchieved { agent_id: a, .. } |
                        EventKind::BudgetWarning { agent_id: a, .. } => {
                            if a != agent_id {
                                return false;
                            }
                        }
                        _ => {}
                    }
                }
                EventFilter::ErrorsOnly => {
                    if !matches!(kind, EventKind::ConservationViolation { .. } | EventKind::BudgetWarning { .. }) {
                        return false;
                    }
                }
                EventFilter::Custom(_) => {
                    // Custom filters would need external logic
                }
            }
        }
        true
    }

    /// Take a full snapshot
    pub fn take_snapshot(&mut self, positions: Vec<[f32; 3]>, velocities: Vec<[f32; 3]>, energy: f64) {
        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.pop_front();
        }

        self.snapshots.push_back(FullSnapshot {
            step: self.current_step,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            positions,
            velocities,
            agent_states: std::collections::HashMap::new(),
            total_energy: energy,
        });
    }

    /// Get events in time range
    pub fn events_in_range(&self, start_step: u64, end_step: u64) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.step >= start_step && e.step <= end_step)
            .collect()
    }

    /// Get events of specific kind
    pub fn events_of_kind(&self, kind_pattern: &str) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| format!("{:?}", e.kind).contains(kind_pattern))
            .collect()
    }

    /// Get recent events
    pub fn recent_events(&self, n: usize) -> Vec<&Event> {
        self.events.iter().rev().take(n).collect()
    }

    /// Get all snapshots
    pub fn snapshots(&self) -> &VecDeque<FullSnapshot> {
        &self.snapshots
    }

    /// Get snapshot at or before step
    pub fn snapshot_at(&self, step: u64) -> Option<&FullSnapshot> {
        self.snapshots
            .iter()
            .rev()
            .find(|s| s.step <= step)
    }

    /// Get event count
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get total events recorded
    pub fn total_events(&self) -> u64 {
        self.counter
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
        self.snapshots.clear();
    }

    /// Export events to JSON
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let events: Vec<_> = self.events.iter().collect();
        serde_json::to_string_pretty(&events)
    }

    /// Get statistics
    pub fn statistics(&self) -> WitnessStatistics {
        let mut stats = WitnessStatistics::default();

        for event in &self.events {
            match &event.kind {
                EventKind::Physics(_) => stats.physics_events += 1,
                EventKind::Action { .. } => stats.action_events += 1,
                EventKind::Observation { .. } => stats.observation_events += 1,
                EventKind::ConservationViolation { .. } => stats.violations += 1,
                EventKind::BudgetWarning { .. } => stats.warnings += 1,
                _ => stats.other_events += 1,
            }
        }

        stats.total_events = self.events.len();
        stats.total_snapshots = self.snapshots.len();
        stats
    }
}

impl Default for WitnessLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the witness log
#[derive(Debug, Clone, Default)]
pub struct WitnessStatistics {
    /// Total events
    pub total_events: usize,
    /// Physics events
    pub physics_events: usize,
    /// Action events
    pub action_events: usize,
    /// Observation events
    pub observation_events: usize,
    /// Violations recorded
    pub violations: usize,
    /// Warnings recorded
    pub warnings: usize,
    /// Other events
    pub other_events: usize,
    /// Total snapshots
    pub total_snapshots: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_witness_log() {
        let mut log = WitnessLog::new();

        log.begin_step(0);
        log.record_custom("test", "data");
        log.end_step();

        assert!(log.event_count() >= 3);
    }

    #[test]
    fn test_event_filtering() {
        let mut log = WitnessLog::new()
            .with_filter(EventFilter::PhysicsOnly);

        log.begin_step(0);
        log.record_custom("test", "data");

        // Custom event should be filtered out
        let stats = log.statistics();
        assert_eq!(stats.physics_events, 0);
    }

    #[test]
    fn test_snapshots() {
        let mut log = WitnessLog::new();

        log.current_step = 10;
        log.take_snapshot(
            vec![[0.0, 0.0, 0.0]],
            vec![[1.0, 0.0, 0.0]],
            -100.0,
        );

        assert_eq!(log.snapshots().len(), 1);
        assert!(log.snapshot_at(10).is_some());
    }
}
