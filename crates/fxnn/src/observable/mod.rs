//! Observable calculators for molecular dynamics simulations.
//!
//! This module provides functions to calculate thermodynamic and structural
//! observables from the instantaneous state of the simulation.
//!
//! # Overview
//!
//! Observables are quantities computed from atom positions, velocities,
//! and forces. They provide physical insight into the system's behavior:
//!
//! - **Thermodynamic**: Temperature, kinetic energy, pressure
//! - **Structural**: Center of mass, density profiles
//! - **Dynamic**: Diffusion coefficients, velocity autocorrelation
//!
//! # Examples
//!
//! ## Basic observables
//!
//! ```rust
//! use fxnn::observable;
//! use fxnn::types::{Atom, SimulationBox};
//!
//! let atoms = vec![
//!     Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0),
//!     Atom::new(1, 0, 1.0).with_velocity(-1.0, 0.0, 0.0),
//! ];
//!
//! // Kinetic energy: sum of (1/2) * m * v^2
//! let ke = observable::kinetic_energy(&atoms);
//! assert!((ke - 1.0).abs() < 1e-6);  // 2 * 0.5 * 1.0 * 1.0
//!
//! // Temperature from equipartition theorem
//! let temp = observable::temperature(&atoms, 1.0);  // kb = 1.0
//! ```
//!
//! ## Center of mass velocity
//!
//! ```rust
//! use fxnn::observable;
//! use fxnn::types::Atom;
//!
//! let mut atoms = vec![
//!     Atom::new(0, 0, 1.0).with_velocity(2.0, 0.0, 0.0),
//!     Atom::new(1, 0, 1.0).with_velocity(0.0, 0.0, 0.0),
//! ];
//!
//! // COM velocity is (2+0)/2 = 1.0 in x
//! let vcm = observable::center_of_mass_velocity(&atoms);
//! assert!((vcm[0] - 1.0).abs() < 1e-6);
//!
//! // Remove COM velocity to prevent drift
//! observable::remove_com_velocity(&mut atoms);
//!
//! // Now COM velocity is zero
//! let vcm_after = observable::center_of_mass_velocity(&atoms);
//! assert!(vcm_after[0].abs() < 1e-6);
//! ```
//!
//! # Mathematical Background
//!
//! ## Kinetic Energy
//!
//! Total kinetic energy:
//! ```text
//! KE = sum_i (1/2) * m_i * |v_i|^2
//! ```
//!
//! ## Temperature (Equipartition Theorem)
//!
//! Temperature is related to kinetic energy via equipartition:
//! ```text
//! T = 2 * KE / (N_dof * k_B)
//! ```
//!
//! Where N_dof is the number of degrees of freedom:
//! ```text
//! N_dof = 3*N - 3  (excluding center-of-mass motion)
//! ```
//!
//! ## Pressure (Virial Theorem)
//!
//! Instantaneous pressure from the virial:
//! ```text
//! P = (N * k_B * T + W) / V
//! ```
//!
//! Where the virial W is:
//! ```text
//! W = (1/3) * sum_i r_i . F_i
//! ```

use crate::types::{Atom, SimulationBox};

/// Calculate total kinetic energy of the system.
///
/// Computes the sum of kinetic energies for all atoms:
/// ```text
/// KE = sum_i (1/2) * m_i * (vx_i^2 + vy_i^2 + vz_i^2)
/// ```
///
/// # Arguments
///
/// * `atoms` - Slice of atoms with velocities
///
/// # Returns
///
/// Total kinetic energy in simulation units (kJ/mol or reduced).
///
/// # Examples
///
/// ```rust
/// use fxnn::observable::kinetic_energy;
/// use fxnn::types::Atom;
///
/// let atoms = vec![
///     Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0),
///     Atom::new(1, 0, 2.0).with_velocity(0.0, 1.0, 0.0),
/// ];
///
/// // KE = 0.5*1*1 + 0.5*2*1 = 0.5 + 1.0 = 1.5
/// let ke = kinetic_energy(&atoms);
/// assert!((ke - 1.5).abs() < 1e-6);
/// ```
pub fn kinetic_energy(atoms: &[Atom]) -> f64 {
    atoms.iter().map(|a| a.kinetic_energy() as f64).sum()
}

/// Calculate instantaneous temperature from kinetic energy.
///
/// Uses the equipartition theorem:
/// ```text
/// T = 2 * KE / (N_dof * k_B)
/// ```
///
/// Where N_dof = 3*N - 3 accounts for removing center-of-mass motion.
///
/// # Arguments
///
/// * `atoms` - Slice of atoms with velocities
/// * `kb` - Boltzmann constant (1.0 in reduced units, 0.00831 kJ/(mol*K) in real units)
///
/// # Returns
///
/// Temperature in simulation units (K or reduced).
///
/// # Examples
///
/// ```rust
/// use fxnn::observable::temperature;
/// use fxnn::types::Atom;
///
/// // Two atoms with opposite velocities (no COM motion)
/// let atoms = vec![
///     Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0),
///     Atom::new(1, 0, 1.0).with_velocity(-1.0, 0.0, 0.0),
/// ];
///
/// // KE = 1.0, N_dof = 3*2 - 3 = 3
/// // T = 2 * 1.0 / (3 * 1.0) = 2/3
/// let temp = temperature(&atoms, 1.0);
/// assert!((temp - 2.0/3.0).abs() < 1e-5);
/// ```
///
/// # Note
///
/// For systems with constraints (e.g., rigid bonds), the degrees of
/// freedom should be adjusted accordingly. This function assumes no
/// constraints.
pub fn temperature(atoms: &[Atom], kb: f32) -> f32 {
    let n = atoms.len() as f32;
    let dof = 3.0 * n - 3.0;
    if dof <= 0.0 { return 0.0; }
    let ke = kinetic_energy(atoms) as f32;
    2.0 * ke / (dof * kb)
}

/// Calculate center of mass velocity.
///
/// The center of mass velocity is the mass-weighted average:
/// ```text
/// v_cm = sum_i (m_i * v_i) / sum_i m_i
/// ```
///
/// # Arguments
///
/// * `atoms` - Slice of atoms with velocities
///
/// # Returns
///
/// Center of mass velocity as `[vx, vy, vz]`.
///
/// # Examples
///
/// ```rust
/// use fxnn::observable::center_of_mass_velocity;
/// use fxnn::types::Atom;
///
/// let atoms = vec![
///     Atom::new(0, 0, 1.0).with_velocity(2.0, 0.0, 0.0),
///     Atom::new(1, 0, 3.0).with_velocity(0.0, 0.0, 0.0),
/// ];
///
/// // v_cm_x = (1*2 + 3*0) / (1+3) = 0.5
/// let vcm = center_of_mass_velocity(&atoms);
/// assert!((vcm[0] - 0.5).abs() < 1e-6);
/// ```
pub fn center_of_mass_velocity(atoms: &[Atom]) -> [f32; 3] {
    let mut vcm = [0.0f32; 3];
    let mut total_mass = 0.0f32;
    for atom in atoms {
        vcm[0] += atom.mass * atom.velocity[0];
        vcm[1] += atom.mass * atom.velocity[1];
        vcm[2] += atom.mass * atom.velocity[2];
        total_mass += atom.mass;
    }
    if total_mass > 0.0 {
        vcm[0] /= total_mass;
        vcm[1] /= total_mass;
        vcm[2] /= total_mass;
    }
    vcm
}

/// Remove center of mass velocity from the system.
///
/// Subtracts the center of mass velocity from each atom, ensuring
/// the system has no net linear momentum. This is important for:
///
/// - Preventing system drift in periodic boundaries
/// - Correctly computing temperature (which excludes COM motion)
/// - Maintaining energy conservation in NVE simulations
///
/// # Arguments
///
/// * `atoms` - Mutable slice of atoms
///
/// # Examples
///
/// ```rust
/// use fxnn::observable::{remove_com_velocity, center_of_mass_velocity};
/// use fxnn::types::Atom;
///
/// let mut atoms = vec![
///     Atom::new(0, 0, 1.0).with_velocity(3.0, 2.0, 1.0),
///     Atom::new(1, 0, 1.0).with_velocity(1.0, 0.0, -1.0),
/// ];
///
/// remove_com_velocity(&mut atoms);
///
/// // COM velocity is now zero
/// let vcm = center_of_mass_velocity(&atoms);
/// assert!(vcm[0].abs() < 1e-6);
/// assert!(vcm[1].abs() < 1e-6);
/// assert!(vcm[2].abs() < 1e-6);
/// ```
pub fn remove_com_velocity(atoms: &mut [Atom]) {
    let vcm = center_of_mass_velocity(atoms);
    for atom in atoms {
        atom.velocity[0] -= vcm[0];
        atom.velocity[1] -= vcm[1];
        atom.velocity[2] -= vcm[2];
    }
}

/// Calculate instantaneous pressure using the virial theorem.
///
/// The pressure is computed from the kinetic contribution and the virial:
/// ```text
/// P = (N * k_B * T + W/3) / V
/// ```
///
/// Where the virial is:
/// ```text
/// W = sum_i r_i . F_i
/// ```
///
/// # Arguments
///
/// * `atoms` - Slice of atoms with positions and forces
/// * `box_` - Simulation box for volume calculation
/// * `kb` - Boltzmann constant
///
/// # Returns
///
/// Pressure in simulation units.
///
/// # Examples
///
/// ```rust
/// use fxnn::observable::pressure;
/// use fxnn::types::{Atom, SimulationBox};
///
/// let box_ = SimulationBox::cubic(10.0);
/// let atoms = vec![
///     Atom::new(0, 0, 1.0)
///         .with_position(1.0, 0.0, 0.0)
///         .with_velocity(1.0, 0.0, 0.0),
/// ];
///
/// let p = pressure(&atoms, &box_, 1.0);
/// // Pressure depends on temperature and virial
/// ```
///
/// # Note
///
/// For accurate pressure calculation in periodic systems, the virial
/// should be computed during force calculation using the minimum image
/// convention. This function computes a simple estimate.
pub fn pressure(atoms: &[Atom], box_: &SimulationBox, kb: f32) -> f64 {
    let n = atoms.len() as f64;
    let t = temperature(atoms, kb) as f64;
    let v = box_.volume() as f64;
    let mut virial = 0.0f64;
    for atom in atoms {
        virial += (atom.position[0] * atom.force[0] + atom.position[1] * atom.force[1] + atom.position[2] * atom.force[2]) as f64;
    }
    (n * kb as f64 * t + virial / 3.0) / v
}
