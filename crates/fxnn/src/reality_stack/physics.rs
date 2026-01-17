//! # Layer 1: PHYSICS
//!
//! The physics layer provides the ground truth simulation with conservation law validation.
//! This layer wraps the existing FXNN molecular dynamics core and adds:
//!
//! - Conservation law verification (energy, momentum, angular momentum)
//! - Physical constraint enforcement
//! - Action application interface for agents
//! - World state queries for perception
//!
//! ## Conservation Laws
//!
//! The physics layer validates three fundamental conservation laws:
//!
//! 1. **Energy Conservation**: Total energy (KE + PE) remains constant in NVE
//! 2. **Linear Momentum Conservation**: Total momentum remains zero (no external forces)
//! 3. **Angular Momentum Conservation**: Total angular momentum remains constant
//!
//! ## Design Philosophy
//!
//! The physics layer is the "ground truth" - it cannot be violated by higher layers.
//! All agent actions must ultimately be physically realizable within this layer.

use crate::error::{FxnnError, Result};
use crate::types::{Atom, SimulationBox};
use crate::force_field::ForceField;
use crate::integrator::Integrator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Core Types
// ============================================================================

/// Result of advancing the physics simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsResult {
    /// Timestep that was completed
    pub step: u64,
    /// Total energy after step
    pub total_energy: f64,
    /// Kinetic energy after step
    pub kinetic_energy: f64,
    /// Potential energy after step
    pub potential_energy: f64,
    /// Temperature after step
    pub temperature: f32,
    /// Conservation law status
    pub conservation: ConservationReport,
}

/// World state accessible to perception layer
#[derive(Debug, Clone)]
pub struct WorldState {
    /// All atoms in the simulation (read-only view)
    pub atoms: Vec<Atom>,
    /// Simulation box
    pub box_: SimulationBox,
    /// Current simulation time
    pub time: f64,
    /// Current step number
    pub step: u64,
}

/// Error type for physics layer
#[derive(Debug, Clone, thiserror::Error)]
pub enum PhysicsError {
    /// Conservation law violated
    #[error("Conservation law violated: {law:?}, drift = {drift}")]
    ConservationViolation {
        law: ConservationLaw,
        drift: f64,
    },

    /// Invalid action
    #[error("Invalid action: {0}")]
    InvalidAction(String),

    /// Numerical instability
    #[error("Numerical instability: {0}")]
    NumericalInstability(String),

    /// Force field error
    #[error("Force field error: {0}")]
    ForceFieldError(String),
}

// ============================================================================
// Conservation Laws
// ============================================================================

/// Types of conservation laws
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConservationLaw {
    /// Total energy conservation (NVE ensemble)
    Energy,
    /// Total linear momentum conservation
    LinearMomentum,
    /// Total angular momentum conservation
    AngularMomentum,
    /// Total mass conservation
    Mass,
    /// Total charge conservation
    Charge,
}

/// Report on conservation law status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationReport {
    /// Initial values (at simulation start)
    pub initial: ConservationValues,
    /// Current values
    pub current: ConservationValues,
    /// Relative drift for each quantity
    pub drift: HashMap<ConservationLaw, f64>,
    /// Whether all laws are within tolerance
    pub all_valid: bool,
}

/// Conserved quantity values
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConservationValues {
    /// Total energy
    pub energy: f64,
    /// Total linear momentum [px, py, pz]
    pub linear_momentum: [f64; 3],
    /// Total angular momentum [Lx, Ly, Lz]
    pub angular_momentum: [f64; 3],
    /// Total mass
    pub mass: f64,
    /// Total charge
    pub charge: f64,
}

impl ConservationValues {
    /// Compute conservation values from atoms
    pub fn from_atoms(atoms: &[Atom], box_: &SimulationBox) -> Self {
        let mut values = Self::default();

        for atom in atoms {
            // Mass
            values.mass += atom.mass as f64;

            // Charge
            values.charge += atom.charge as f64;

            // Linear momentum: p = m * v
            values.linear_momentum[0] += (atom.mass * atom.velocity[0]) as f64;
            values.linear_momentum[1] += (atom.mass * atom.velocity[1]) as f64;
            values.linear_momentum[2] += (atom.mass * atom.velocity[2]) as f64;

            // Angular momentum: L = r x p
            let rx = atom.position[0] as f64;
            let ry = atom.position[1] as f64;
            let rz = atom.position[2] as f64;
            let px = (atom.mass * atom.velocity[0]) as f64;
            let py = (atom.mass * atom.velocity[1]) as f64;
            let pz = (atom.mass * atom.velocity[2]) as f64;

            values.angular_momentum[0] += ry * pz - rz * py;
            values.angular_momentum[1] += rz * px - rx * pz;
            values.angular_momentum[2] += rx * py - ry * px;
        }

        values
    }

    /// Compute relative drift from initial values
    pub fn drift_from(&self, initial: &ConservationValues) -> HashMap<ConservationLaw, f64> {
        let mut drift = HashMap::new();

        // Energy drift (requires separate calculation)
        // drift.insert(ConservationLaw::Energy, ...);

        // Momentum drift (magnitude)
        let dp = [
            self.linear_momentum[0] - initial.linear_momentum[0],
            self.linear_momentum[1] - initial.linear_momentum[1],
            self.linear_momentum[2] - initial.linear_momentum[2],
        ];
        let p_mag = (initial.linear_momentum[0].powi(2)
            + initial.linear_momentum[1].powi(2)
            + initial.linear_momentum[2].powi(2))
        .sqrt()
        .max(1e-10);
        let dp_mag = (dp[0].powi(2) + dp[1].powi(2) + dp[2].powi(2)).sqrt();
        drift.insert(ConservationLaw::LinearMomentum, dp_mag / p_mag);

        // Angular momentum drift
        let dL = [
            self.angular_momentum[0] - initial.angular_momentum[0],
            self.angular_momentum[1] - initial.angular_momentum[1],
            self.angular_momentum[2] - initial.angular_momentum[2],
        ];
        let L_mag = (initial.angular_momentum[0].powi(2)
            + initial.angular_momentum[1].powi(2)
            + initial.angular_momentum[2].powi(2))
        .sqrt()
        .max(1e-10);
        let dL_mag = (dL[0].powi(2) + dL[1].powi(2) + dL[2].powi(2)).sqrt();
        drift.insert(ConservationLaw::AngularMomentum, dL_mag / L_mag);

        // Mass drift
        drift.insert(
            ConservationLaw::Mass,
            (self.mass - initial.mass).abs() / initial.mass.max(1e-10),
        );

        // Charge drift
        drift.insert(
            ConservationLaw::Charge,
            (self.charge - initial.charge).abs() / initial.charge.abs().max(1e-10),
        );

        drift
    }
}

// ============================================================================
// Conservation Validator
// ============================================================================

/// Validator for conservation laws
#[derive(Debug, Clone)]
pub struct ConservationValidator {
    /// Tolerance for energy drift (relative)
    pub energy_tolerance: f64,
    /// Tolerance for momentum drift (relative)
    pub momentum_tolerance: f64,
    /// Tolerance for angular momentum drift (relative)
    pub angular_momentum_tolerance: f64,
    /// Which laws to enforce
    pub enforced_laws: Vec<ConservationLaw>,
    /// Initial values at simulation start
    initial_values: Option<ConservationValues>,
    /// Initial energy
    initial_energy: Option<f64>,
}

impl Default for ConservationValidator {
    fn default() -> Self {
        Self {
            energy_tolerance: 1e-4,           // 0.01% energy drift allowed
            momentum_tolerance: 1e-10,        // Very tight for momentum
            angular_momentum_tolerance: 1e-6, // Looser for angular momentum
            enforced_laws: vec![
                ConservationLaw::Energy,
                ConservationLaw::LinearMomentum,
            ],
            initial_values: None,
            initial_energy: None,
        }
    }
}

impl ConservationValidator {
    /// Create a new validator with default tolerances
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a strict validator with tight tolerances
    pub fn strict() -> Self {
        Self {
            energy_tolerance: 1e-6,
            momentum_tolerance: 1e-12,
            angular_momentum_tolerance: 1e-8,
            enforced_laws: vec![
                ConservationLaw::Energy,
                ConservationLaw::LinearMomentum,
                ConservationLaw::AngularMomentum,
                ConservationLaw::Mass,
            ],
            initial_values: None,
            initial_energy: None,
        }
    }

    /// Create a lenient validator for thermostated simulations
    pub fn lenient() -> Self {
        Self {
            energy_tolerance: 0.1,            // 10% drift allowed (thermostat)
            momentum_tolerance: 1e-6,
            angular_momentum_tolerance: 1e-3,
            enforced_laws: vec![
                ConservationLaw::LinearMomentum,
                ConservationLaw::Mass,
            ],
            initial_values: None,
            initial_energy: None,
        }
    }

    /// Set energy tolerance
    pub fn with_energy_tolerance(mut self, tolerance: f64) -> Self {
        self.energy_tolerance = tolerance;
        self
    }

    /// Set momentum tolerance
    pub fn with_momentum_tolerance(mut self, tolerance: f64) -> Self {
        self.momentum_tolerance = tolerance;
        self
    }

    /// Initialize with starting values
    pub fn initialize(&mut self, atoms: &[Atom], box_: &SimulationBox, energy: f64) {
        self.initial_values = Some(ConservationValues::from_atoms(atoms, box_));
        self.initial_energy = Some(energy);
    }

    /// Validate current state against initial values
    pub fn validate(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        current_energy: f64,
    ) -> Result<ConservationReport> {
        let initial = self.initial_values.as_ref()
            .ok_or_else(|| FxnnError::invalid_parameter("Validator not initialized"))?;
        let initial_energy = self.initial_energy
            .ok_or_else(|| FxnnError::invalid_parameter("Validator not initialized"))?;

        let current = ConservationValues::from_atoms(atoms, box_);
        let mut drift = current.drift_from(initial);

        // Add energy drift
        let energy_drift = (current_energy - initial_energy).abs() / initial_energy.abs().max(1e-10);
        drift.insert(ConservationLaw::Energy, energy_drift);

        // Check all enforced laws
        let mut all_valid = true;
        for law in &self.enforced_laws {
            let law_drift = drift.get(law).copied().unwrap_or(0.0);
            let tolerance = match law {
                ConservationLaw::Energy => self.energy_tolerance,
                ConservationLaw::LinearMomentum => self.momentum_tolerance,
                ConservationLaw::AngularMomentum => self.angular_momentum_tolerance,
                ConservationLaw::Mass => 1e-15, // Should be exactly conserved
                ConservationLaw::Charge => 1e-15, // Should be exactly conserved
            };

            if law_drift > tolerance {
                all_valid = false;
            }
        }

        Ok(ConservationReport {
            initial: initial.clone(),
            current,
            drift,
            all_valid,
        })
    }
}

// ============================================================================
// Physics Engine
// ============================================================================

/// Main physics engine wrapping FXNN simulation
pub struct PhysicsEngine<F: ForceField, I: Integrator> {
    /// The underlying simulation
    simulation: crate::simulation::Simulation<F, I>,
    /// Conservation validator
    validator: ConservationValidator,
    /// Whether validation is enabled
    validation_enabled: bool,
    /// Applied actions this step
    pending_actions: Vec<super::agency::ValidatedAction>,
}

impl<F: ForceField, I: Integrator> PhysicsEngine<F, I> {
    /// Create a new physics engine from a simulation
    pub fn new(simulation: crate::simulation::Simulation<F, I>) -> Self {
        Self {
            simulation,
            validator: ConservationValidator::default(),
            validation_enabled: true,
            pending_actions: Vec::new(),
        }
    }

    /// Set the conservation validator
    pub fn with_validator(mut self, validator: ConservationValidator) -> Self {
        self.validator = validator;
        self
    }

    /// Enable or disable validation
    pub fn with_validation(mut self, enabled: bool) -> Self {
        self.validation_enabled = enabled;
        self
    }

    /// Initialize the validator with current state
    pub fn initialize_validation(&mut self) {
        let atoms = self.simulation.atoms();
        let box_ = self.simulation.box_();
        let energy = self.simulation.total_energy();
        self.validator.initialize(atoms, box_, energy);
    }

    /// Get world state for perception
    pub fn world_state(&self) -> WorldState {
        WorldState {
            atoms: self.simulation.atoms().to_vec(),
            box_: *self.simulation.box_(),
            time: self.simulation.time(),
            step: self.simulation.step() as u64,
        }
    }

    /// Advance simulation by one step
    pub fn advance(&mut self) -> Result<PhysicsResult> {
        // Apply any pending actions first
        let actions: Vec<_> = self.pending_actions.drain(..).collect();
        for action in actions {
            self.apply_action_internal(&action)?;
        }

        // Advance simulation
        self.simulation.step_forward();

        // Compute observables
        let ke = self.simulation.kinetic_energy();
        let pe = self.simulation.potential_energy();
        let total_e = ke + pe;
        let temp = self.simulation.temperature();

        // Validate conservation if enabled
        let conservation = if self.validation_enabled {
            self.validator.validate(
                self.simulation.atoms(),
                self.simulation.box_(),
                total_e,
            )?
        } else {
            ConservationReport {
                initial: ConservationValues::default(),
                current: ConservationValues::default(),
                drift: HashMap::new(),
                all_valid: true,
            }
        };

        Ok(PhysicsResult {
            step: self.simulation.step() as u64,
            total_energy: total_e,
            kinetic_energy: ke,
            potential_energy: pe,
            temperature: temp,
            conservation,
        })
    }

    /// Apply a validated action to the simulation
    pub fn apply_action(&mut self, action: super::agency::ValidatedAction) {
        self.pending_actions.push(action);
    }

    /// Internal action application
    fn apply_action_internal(&mut self, action: &super::agency::ValidatedAction) -> Result<()> {
        use super::agency::ActionKind;

        match &action.kind {
            ActionKind::ApplyForce { atom_id, force } => {
                let atoms = self.simulation.atoms_mut();
                if let Some(atom) = atoms.iter_mut().find(|a| a.id == *atom_id) {
                    atom.add_force(force[0], force[1], force[2]);
                }
            }
            ActionKind::SetVelocity { atom_id, velocity } => {
                let atoms = self.simulation.atoms_mut();
                if let Some(atom) = atoms.iter_mut().find(|a| a.id == *atom_id) {
                    atom.velocity = *velocity;
                }
            }
            ActionKind::MoveAtom { atom_id, displacement } => {
                let atoms = self.simulation.atoms_mut();
                if let Some(atom) = atoms.iter_mut().find(|a| a.id == *atom_id) {
                    atom.position[0] += displacement[0];
                    atom.position[1] += displacement[1];
                    atom.position[2] += displacement[2];
                }
            }
            ActionKind::Noop => {}
        }

        Ok(())
    }
}

// ============================================================================
// Physical Constraints
// ============================================================================

/// Physical constraint that can be applied to the simulation
pub trait PhysicalConstraint: Send + Sync {
    /// Apply constraint to atoms (modify positions/velocities)
    fn apply(&self, atoms: &mut [Atom], box_: &SimulationBox);

    /// Get the name of this constraint
    fn name(&self) -> &str;

    /// Check if constraint is satisfied
    fn is_satisfied(&self, atoms: &[Atom], box_: &SimulationBox) -> bool;
}

/// SHAKE constraint for rigid bonds
pub struct ShakeConstraint {
    /// Bond constraints: (atom_i, atom_j, target_distance)
    bonds: Vec<(u32, u32, f32)>,
    /// Tolerance for constraint satisfaction
    tolerance: f32,
    /// Maximum iterations
    max_iterations: usize,
}

impl ShakeConstraint {
    /// Create a new SHAKE constraint
    pub fn new(tolerance: f32) -> Self {
        Self {
            bonds: Vec::new(),
            tolerance,
            max_iterations: 100,
        }
    }

    /// Add a bond constraint
    pub fn add_bond(&mut self, atom_i: u32, atom_j: u32, distance: f32) {
        self.bonds.push((atom_i, atom_j, distance));
    }
}

impl PhysicalConstraint for ShakeConstraint {
    fn apply(&self, atoms: &mut [Atom], box_: &SimulationBox) {
        // SHAKE algorithm implementation
        for _ in 0..self.max_iterations {
            let mut max_error = 0.0f32;

            for &(i, j, d0) in &self.bonds {
                let atom_i = &atoms[i as usize];
                let atom_j = &atoms[j as usize];

                // Current distance
                let dr = box_.minimum_image(
                    atom_i.position[0] - atom_j.position[0],
                    atom_i.position[1] - atom_j.position[1],
                    atom_i.position[2] - atom_j.position[2],
                );
                let d2 = dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2];
                let d = d2.sqrt();

                let error = (d - d0).abs();
                max_error = max_error.max(error);

                if error > self.tolerance {
                    // Apply correction
                    let lambda = (d2 - d0 * d0) / (2.0 * d2 * (1.0 / atoms[i as usize].mass + 1.0 / atoms[j as usize].mass));

                    let correction = [
                        lambda * dr[0] / d,
                        lambda * dr[1] / d,
                        lambda * dr[2] / d,
                    ];

                    let mi = atoms[i as usize].mass;
                    let mj = atoms[j as usize].mass;

                    atoms[i as usize].position[0] -= correction[0] / mi;
                    atoms[i as usize].position[1] -= correction[1] / mi;
                    atoms[i as usize].position[2] -= correction[2] / mi;

                    atoms[j as usize].position[0] += correction[0] / mj;
                    atoms[j as usize].position[1] += correction[1] / mj;
                    atoms[j as usize].position[2] += correction[2] / mj;
                }
            }

            if max_error < self.tolerance {
                break;
            }
        }
    }

    fn name(&self) -> &str {
        "SHAKE"
    }

    fn is_satisfied(&self, atoms: &[Atom], box_: &SimulationBox) -> bool {
        for &(i, j, d0) in &self.bonds {
            let dr = box_.minimum_image(
                atoms[i as usize].position[0] - atoms[j as usize].position[0],
                atoms[i as usize].position[1] - atoms[j as usize].position[1],
                atoms[i as usize].position[2] - atoms[j as usize].position[2],
            );
            let d = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
            if (d - d0).abs() > self.tolerance {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservation_values() {
        let atoms = vec![
            Atom::new(0, 0, 1.0)
                .with_position(0.0, 0.0, 0.0)
                .with_velocity(1.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0)
                .with_position(1.0, 0.0, 0.0)
                .with_velocity(-1.0, 0.0, 0.0),
        ];
        let box_ = SimulationBox::cubic(10.0);

        let values = ConservationValues::from_atoms(&atoms, &box_);

        // Total mass should be 2.0
        assert!((values.mass - 2.0).abs() < 1e-10);

        // Total momentum should be zero (equal and opposite)
        assert!(values.linear_momentum[0].abs() < 1e-10);
    }

    #[test]
    fn test_validator_initialization() {
        let mut validator = ConservationValidator::default();
        let atoms = vec![Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0)];
        let box_ = SimulationBox::cubic(10.0);

        validator.initialize(&atoms, &box_, 0.5);

        assert!(validator.initial_values.is_some());
        assert!(validator.initial_energy.is_some());
    }
}
