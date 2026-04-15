//! State snapshots and checkpoint management for the Witness system.
//!
//! This module provides data structures for capturing simulation state
//! before and after corrections, as well as checkpoint management for
//! rollback functionality.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A snapshot of simulation state at a specific point in time.
///
/// State snapshots capture the complete physical state of entities
/// involved in a correction event, enabling full reconstruction
/// of what happened.
///
/// # Fields
///
/// All position, velocity, force, and energy arrays are indexed by
/// entity (atom) order as they appear in the simulation.
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::StateSnapshot;
///
/// // Capture state for 2 atoms
/// let snapshot = StateSnapshot {
///     positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
///     velocities: vec![[0.1, 0.0, 0.0], [-0.1, 0.0, 0.0]],
///     forces: vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]],
///     kinetic_energy: 0.01,
///     potential_energy: -0.5,
///     total_energy: -0.49,
///     temperature: 1.0,
///     momentum: [0.0, 0.0, 0.0],
/// };
///
/// assert_eq!(snapshot.positions.len(), 2);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Positions of entities [x, y, z] in simulation units.
    #[serde(default)]
    pub positions: Vec<[f64; 3]>,

    /// Velocities of entities [vx, vy, vz] in simulation units.
    #[serde(default)]
    pub velocities: Vec<[f64; 3]>,

    /// Forces on entities [fx, fy, fz] in simulation units.
    #[serde(default)]
    pub forces: Vec<[f64; 3]>,

    /// Total kinetic energy of the captured entities.
    #[serde(default)]
    pub kinetic_energy: f64,

    /// Total potential energy of the captured entities.
    #[serde(default)]
    pub potential_energy: f64,

    /// Total energy (kinetic + potential).
    #[serde(default)]
    pub total_energy: f64,

    /// System temperature (if applicable).
    #[serde(default)]
    pub temperature: f64,

    /// Total momentum [px, py, pz] of the captured entities.
    #[serde(default)]
    pub momentum: [f64; 3],
}

impl StateSnapshot {
    /// Create a new empty state snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a snapshot with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `num_entities` - Number of entities to reserve space for
    pub fn with_capacity(num_entities: usize) -> Self {
        Self {
            positions: Vec::with_capacity(num_entities),
            velocities: Vec::with_capacity(num_entities),
            forces: Vec::with_capacity(num_entities),
            ..Default::default()
        }
    }

    /// Add an entity's state to the snapshot.
    ///
    /// # Arguments
    ///
    /// * `position` - Entity position [x, y, z]
    /// * `velocity` - Entity velocity [vx, vy, vz]
    /// * `force` - Force on entity [fx, fy, fz]
    pub fn add_entity(
        &mut self,
        position: [f64; 3],
        velocity: [f64; 3],
        force: [f64; 3],
    ) {
        self.positions.push(position);
        self.velocities.push(velocity);
        self.forces.push(force);
    }

    /// Get the number of entities in this snapshot.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Check if the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Set the energy values.
    ///
    /// # Arguments
    ///
    /// * `kinetic` - Kinetic energy
    /// * `potential` - Potential energy
    pub fn set_energies(&mut self, kinetic: f64, potential: f64) {
        self.kinetic_energy = kinetic;
        self.potential_energy = potential;
        self.total_energy = kinetic + potential;
    }

    /// Calculate the total momentum from velocities and masses.
    ///
    /// # Arguments
    ///
    /// * `masses` - Mass of each entity (must match velocities length)
    pub fn calculate_momentum(&mut self, masses: &[f64]) {
        let mut px = 0.0;
        let mut py = 0.0;
        let mut pz = 0.0;

        for (v, &m) in self.velocities.iter().zip(masses.iter()) {
            px += m * v[0];
            py += m * v[1];
            pz += m * v[2];
        }

        self.momentum = [px, py, pz];
    }

    /// Get the magnitude of the total momentum.
    pub fn momentum_magnitude(&self) -> f64 {
        let [px, py, pz] = self.momentum;
        (px * px + py * py + pz * pz).sqrt()
    }

    /// Compare with another snapshot and return the maximum position difference.
    pub fn max_position_delta(&self, other: &StateSnapshot) -> f64 {
        self.positions
            .iter()
            .zip(other.positions.iter())
            .map(|(a, b)| {
                let dx = a[0] - b[0];
                let dy = a[1] - b[1];
                let dz = a[2] - b[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            })
            .fold(0.0, f64::max)
    }

    /// Compare with another snapshot and return the energy drift.
    pub fn energy_drift(&self, other: &StateSnapshot) -> f64 {
        (self.total_energy - other.total_energy).abs()
    }

    /// Compare with another snapshot and return the relative energy drift.
    pub fn relative_energy_drift(&self, other: &StateSnapshot) -> f64 {
        let avg_energy = (self.total_energy.abs() + other.total_energy.abs()) / 2.0;
        if avg_energy < 1e-10 {
            return 0.0;
        }
        (self.total_energy - other.total_energy).abs() / avg_energy
    }
}

/// Details about a correction that was applied.
///
/// This captures what action was taken to restore a valid state,
/// including the type of correction, parameters used, and outcomes.
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::CorrectionDetails;
///
/// let correction = CorrectionDetails {
///     correction_type: "position_projection".to_string(),
///     method: "Lagrange multiplier".to_string(),
///     parameters: vec![("stiffness".to_string(), 1000.0)],
///     iterations: 3,
///     converged: true,
///     residual: 1e-8,
///     notes: "Separated overlapping atoms".to_string(),
/// };
///
/// assert!(correction.converged);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorrectionDetails {
    /// Type of correction applied (e.g., "position_projection", "velocity_scaling").
    #[serde(default)]
    pub correction_type: String,

    /// Method used for the correction (e.g., "Lagrange multiplier", "SHAKE").
    #[serde(default)]
    pub method: String,

    /// Parameters used in the correction as (name, value) pairs.
    #[serde(default)]
    pub parameters: Vec<(String, f64)>,

    /// Number of iterations required for convergence.
    #[serde(default)]
    pub iterations: u32,

    /// Whether the correction converged successfully.
    #[serde(default)]
    pub converged: bool,

    /// Final residual or error after correction.
    #[serde(default)]
    pub residual: f64,

    /// Additional notes or context about the correction.
    #[serde(default)]
    pub notes: String,
}

impl CorrectionDetails {
    /// Create a new correction details struct.
    pub fn new(correction_type: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            correction_type: correction_type.into(),
            method: method.into(),
            ..Default::default()
        }
    }

    /// Add a parameter to the correction details.
    pub fn with_parameter(mut self, name: impl Into<String>, value: f64) -> Self {
        self.parameters.push((name.into(), value));
        self
    }

    /// Set the iteration and convergence information.
    pub fn with_convergence(mut self, iterations: u32, converged: bool, residual: f64) -> Self {
        self.iterations = iterations;
        self.converged = converged;
        self.residual = residual;
        self
    }

    /// Add a note to the correction details.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes = note.into();
        self
    }

    /// Check if this was a successful correction.
    pub fn is_successful(&self) -> bool {
        self.converged && self.residual < 1e-6
    }
}

/// A simulation checkpoint for rollback functionality.
///
/// Checkpoints store complete state snapshots at specific ticks,
/// enabling recovery to a known-good state after severe violations.
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::{Checkpoint, StateSnapshot};
///
/// let state = StateSnapshot::default();
/// let checkpoint = Checkpoint::new(100, state);
///
/// assert_eq!(checkpoint.tick, 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Simulation tick when this checkpoint was created.
    pub tick: u64,

    /// Complete state snapshot at the checkpoint time.
    pub state: StateSnapshot,

    /// Timestamp when checkpoint was created (wall-clock time).
    #[serde(default)]
    pub created_at: u64,

    /// Optional label or reason for the checkpoint.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl Checkpoint {
    /// Create a new checkpoint.
    ///
    /// # Arguments
    ///
    /// * `tick` - Simulation tick number
    /// * `state` - State snapshot to save
    pub fn new(tick: u64, state: StateSnapshot) -> Self {
        Self {
            tick,
            state,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            label: String::new(),
        }
    }

    /// Create a labeled checkpoint.
    ///
    /// # Arguments
    ///
    /// * `tick` - Simulation tick number
    /// * `state` - State snapshot to save
    /// * `label` - Descriptive label for this checkpoint
    pub fn with_label(tick: u64, state: StateSnapshot, label: impl Into<String>) -> Self {
        let mut cp = Self::new(tick, state);
        cp.label = label.into();
        cp
    }
}

/// Manager for simulation checkpoints with rollback functionality.
///
/// The checkpoint manager maintains a bounded queue of checkpoints,
/// automatically evicting the oldest when capacity is reached.
///
/// # Design
///
/// - **Bounded**: Maximum number of checkpoints prevents unbounded memory growth
/// - **FIFO Eviction**: Oldest checkpoints are removed first
/// - **Query Support**: Find checkpoints by tick or get the latest
///
/// # Examples
///
/// ```rust
/// use fxnn::witness::{CheckpointManager, Checkpoint, StateSnapshot};
///
/// let mut manager = CheckpointManager::new(5);  // Keep at most 5 checkpoints
///
/// // Save checkpoints periodically
/// manager.save(Checkpoint::new(100, StateSnapshot::default()));
/// manager.save(Checkpoint::new(200, StateSnapshot::default()));
///
/// // Get the latest checkpoint for rollback
/// if let Some(cp) = manager.latest() {
///     println!("Rolling back to tick {}", cp.tick);
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointManager {
    /// Queue of checkpoints (newest at back).
    checkpoints: VecDeque<Checkpoint>,

    /// Maximum number of checkpoints to retain.
    max_checkpoints: usize,

    /// Total number of checkpoints ever created.
    total_created: u64,

    /// Number of checkpoints evicted.
    evicted_count: u64,

    /// Number of rollbacks performed.
    rollback_count: u64,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `max_checkpoints` - Maximum number of checkpoints to retain
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            checkpoints: VecDeque::with_capacity(max_checkpoints),
            max_checkpoints,
            total_created: 0,
            evicted_count: 0,
            rollback_count: 0,
        }
    }

    /// Save a checkpoint.
    ///
    /// If the manager is at capacity, the oldest checkpoint is evicted.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - The checkpoint to save
    pub fn save(&mut self, checkpoint: Checkpoint) {
        // Evict oldest if at capacity
        while self.checkpoints.len() >= self.max_checkpoints {
            self.checkpoints.pop_front();
            self.evicted_count += 1;
        }

        self.checkpoints.push_back(checkpoint);
        self.total_created += 1;
    }

    /// Create and save a checkpoint for the current state.
    ///
    /// # Arguments
    ///
    /// * `tick` - Current simulation tick
    /// * `state` - Current state snapshot
    pub fn create_checkpoint(&mut self, tick: u64, state: StateSnapshot) {
        self.save(Checkpoint::new(tick, state));
    }

    /// Get the latest checkpoint.
    ///
    /// # Returns
    ///
    /// Reference to the most recent checkpoint, or `None` if empty.
    pub fn latest(&self) -> Option<&Checkpoint> {
        self.checkpoints.back()
    }

    /// Get the oldest checkpoint.
    pub fn oldest(&self) -> Option<&Checkpoint> {
        self.checkpoints.front()
    }

    /// Find a checkpoint by tick.
    ///
    /// Returns the checkpoint with the exact tick, or `None` if not found.
    pub fn get_by_tick(&self, tick: u64) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|cp| cp.tick == tick)
    }

    /// Find the most recent checkpoint before a given tick.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick to search before
    ///
    /// # Returns
    ///
    /// The most recent checkpoint with tick < the given tick.
    pub fn get_before_tick(&self, tick: u64) -> Option<&Checkpoint> {
        self.checkpoints
            .iter()
            .rev()
            .find(|cp| cp.tick < tick)
    }

    /// Pop and return the latest checkpoint for rollback.
    ///
    /// This removes the checkpoint from the manager and increments
    /// the rollback count.
    ///
    /// # Returns
    ///
    /// The most recent checkpoint, or `None` if empty.
    pub fn pop_for_rollback(&mut self) -> Option<Checkpoint> {
        self.rollback_count += 1;
        self.checkpoints.pop_back()
    }

    /// Get the number of checkpoints currently stored.
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    /// Check if there are no checkpoints.
    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    /// Get the total number of checkpoints ever created.
    pub fn total_created(&self) -> u64 {
        self.total_created
    }

    /// Get the number of checkpoints evicted.
    pub fn evicted_count(&self) -> u64 {
        self.evicted_count
    }

    /// Get the number of rollbacks performed.
    pub fn rollback_count(&self) -> u64 {
        self.rollback_count
    }

    /// Clear all checkpoints.
    pub fn clear(&mut self) {
        self.evicted_count += self.checkpoints.len() as u64;
        self.checkpoints.clear();
    }

    /// Get an iterator over all checkpoints (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &Checkpoint> {
        self.checkpoints.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_snapshot_creation() {
        let mut snapshot = StateSnapshot::with_capacity(2);

        snapshot.add_entity([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        snapshot.add_entity([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);

        assert_eq!(snapshot.len(), 2);
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_state_snapshot_energies() {
        let mut snapshot = StateSnapshot::new();
        snapshot.set_energies(0.5, -1.0);

        assert!((snapshot.kinetic_energy - 0.5).abs() < 1e-10);
        assert!((snapshot.potential_energy - (-1.0)).abs() < 1e-10);
        assert!((snapshot.total_energy - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn test_state_snapshot_momentum() {
        let mut snapshot = StateSnapshot::new();
        snapshot.velocities = vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]];

        let masses = vec![1.0, 2.0];
        snapshot.calculate_momentum(&masses);

        // p = m1*v1 + m2*v2 = 1*1 + 2*(-1) = -1
        assert!((snapshot.momentum[0] - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_state_snapshot_delta() {
        let s1 = StateSnapshot {
            positions: vec![[0.0, 0.0, 0.0]],
            total_energy: -1.0,
            ..Default::default()
        };

        let s2 = StateSnapshot {
            positions: vec![[1.0, 0.0, 0.0]],
            total_energy: -0.9,
            ..Default::default()
        };

        assert!((s1.max_position_delta(&s2) - 1.0).abs() < 1e-10);
        assert!((s1.energy_drift(&s2) - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_correction_details_builder() {
        let correction = CorrectionDetails::new("position_projection", "SHAKE")
            .with_parameter("tolerance", 1e-6)
            .with_parameter("max_iterations", 100.0)
            .with_convergence(5, true, 1e-8)
            .with_note("Resolved overlap between atoms 1 and 2");

        assert_eq!(correction.correction_type, "position_projection");
        assert_eq!(correction.method, "SHAKE");
        assert_eq!(correction.parameters.len(), 2);
        assert_eq!(correction.iterations, 5);
        assert!(correction.converged);
        assert!(correction.is_successful());
    }

    #[test]
    fn test_checkpoint_creation() {
        let state = StateSnapshot::default();
        let checkpoint = Checkpoint::new(100, state);

        assert_eq!(checkpoint.tick, 100);
        assert!(checkpoint.label.is_empty());

        let labeled = Checkpoint::with_label(200, StateSnapshot::default(), "before_learning");
        assert_eq!(labeled.label, "before_learning");
    }

    #[test]
    fn test_checkpoint_manager_capacity() {
        let mut manager = CheckpointManager::new(3);

        manager.save(Checkpoint::new(1, StateSnapshot::default()));
        manager.save(Checkpoint::new(2, StateSnapshot::default()));
        manager.save(Checkpoint::new(3, StateSnapshot::default()));

        assert_eq!(manager.len(), 3);
        assert_eq!(manager.evicted_count(), 0);

        // Adding a 4th should evict the oldest
        manager.save(Checkpoint::new(4, StateSnapshot::default()));

        assert_eq!(manager.len(), 3);
        assert_eq!(manager.evicted_count(), 1);
        assert_eq!(manager.oldest().unwrap().tick, 2);
    }

    #[test]
    fn test_checkpoint_manager_queries() {
        let mut manager = CheckpointManager::new(5);

        manager.save(Checkpoint::new(100, StateSnapshot::default()));
        manager.save(Checkpoint::new(200, StateSnapshot::default()));
        manager.save(Checkpoint::new(300, StateSnapshot::default()));

        assert_eq!(manager.latest().unwrap().tick, 300);
        assert_eq!(manager.oldest().unwrap().tick, 100);
        assert_eq!(manager.get_by_tick(200).unwrap().tick, 200);
        assert!(manager.get_by_tick(150).is_none());
        assert_eq!(manager.get_before_tick(250).unwrap().tick, 200);
    }

    #[test]
    fn test_checkpoint_manager_rollback() {
        let mut manager = CheckpointManager::new(5);

        manager.save(Checkpoint::new(100, StateSnapshot::default()));
        manager.save(Checkpoint::new(200, StateSnapshot::default()));

        assert_eq!(manager.rollback_count(), 0);

        let cp = manager.pop_for_rollback().unwrap();
        assert_eq!(cp.tick, 200);
        assert_eq!(manager.rollback_count(), 1);
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_state_snapshot_serialization() {
        let snapshot = StateSnapshot {
            positions: vec![[1.0, 2.0, 3.0]],
            velocities: vec![[0.1, 0.2, 0.3]],
            forces: vec![[0.0, 0.0, -1.0]],
            kinetic_energy: 0.5,
            potential_energy: -1.0,
            total_energy: -0.5,
            temperature: 1.0,
            momentum: [0.0, 0.0, 0.0],
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: StateSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.positions, snapshot.positions);
        assert!((restored.total_energy - snapshot.total_energy).abs() < 1e-10);
    }
}
