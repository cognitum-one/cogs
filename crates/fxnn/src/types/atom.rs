//! Atom representation for molecular dynamics simulations.
//!
//! This module provides the [`Atom`] struct representing a single particle
//! in the simulation, along with [`AtomType`] for species identification and
//! [`AtomsSoA`] for SIMD-optimized Structure-of-Arrays storage.
//!
//! # Overview
//!
//! Each atom stores:
//! - **Position** `[x, y, z]` in nanometers (nm)
//! - **Velocity** `[vx, vy, vz]` in nm/ps
//! - **Force** `[fx, fy, fz]` in kJ/(mol*nm)
//! - **Mass** in atomic mass units (amu)
//! - **Charge** in elementary charge units (e)
//! - **Atom type** identifier for force field parameters
//!
//! # Memory Layout
//!
//! The `Atom` struct uses `#[repr(C)]` for predictable memory layout and
//! is designed for cache-efficient access patterns. For SIMD-intensive
//! workloads, convert to [`AtomsSoA`] which provides better vectorization.
//!
//! # Examples
//!
//! ## Creating atoms with the builder pattern
//!
//! ```rust
//! use fxnn::types::Atom;
//!
//! // Create an argon atom at a specific position
//! let argon = Atom::new(0, 0, 39.948)  // id, type, mass
//!     .with_position(1.0, 2.0, 3.0)
//!     .with_velocity(0.1, -0.1, 0.0);
//!
//! // Create a charged particle
//! let sodium = Atom::new(1, 1, 22.99)
//!     .with_position(0.0, 0.0, 0.0)
//!     .with_charge(1.0);  // Na+ ion
//! ```
//!
//! ## Calculating properties
//!
//! ```rust
//! use fxnn::types::Atom;
//!
//! let atom = Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0);
//!
//! // Kinetic energy: KE = (1/2) * m * v^2
//! let ke = atom.kinetic_energy();  // 0.5 in reduced units
//!
//! // Speed: |v| = sqrt(vx^2 + vy^2 + vz^2)
//! let speed = atom.speed();  // 1.0
//! ```
//!
//! ## Structure-of-Arrays for SIMD
//!
//! ```rust
//! use fxnn::types::{Atom, AtomsSoA};
//!
//! let atoms = vec![
//!     Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
//!     Atom::new(1, 0, 1.0).with_position(1.0, 1.0, 1.0),
//! ];
//!
//! // Convert to SoA for SIMD operations
//! let mut soa = AtomsSoA::from_atoms(&atoms);
//!
//! // Zero forces (vectorizable operation)
//! soa.zero_forces();
//!
//! // Convert back to AoS
//! let atoms_back = soa.to_atoms();
//! ```

use serde::{Deserialize, Serialize};

/// Atom type identifier for force field parameter lookup.
///
/// Atoms with the same type share force field parameters (epsilon, sigma, etc.).
/// This is typically used to distinguish different chemical species (e.g., C, N, O)
/// or different parameterizations of the same element.
///
/// # Examples
///
/// ```rust
/// use fxnn::types::AtomType;
///
/// let carbon = AtomType::new(0);
/// let nitrogen = AtomType::new(1);
///
/// assert_eq!(carbon.id(), 0);
/// assert_ne!(carbon, nitrogen);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AtomType(pub u16);

impl AtomType {
    /// Create a new atom type with the given identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this atom type (0-65535)
    ///
    /// # Returns
    ///
    /// A new `AtomType` instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::AtomType;
    ///
    /// let argon_type = AtomType::new(0);
    /// ```
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Get the type identifier.
    ///
    /// # Returns
    ///
    /// The numeric identifier for this atom type.
    pub const fn id(&self) -> u16 {
        self.0
    }
}

impl Default for AtomType {
    fn default() -> Self {
        Self(0)
    }
}

/// Represents a single atom in the molecular dynamics simulation.
///
/// The `Atom` struct stores all per-particle data needed for MD simulations:
/// position, velocity, force, mass, charge, and type. It uses `#[repr(C)]`
/// for a predictable memory layout compatible with FFI and certain SIMD patterns.
///
/// # Units
///
/// By default, FXNN uses reduced (dimensionless) units where:
/// - Length: sigma (typical value: 0.34 nm for argon)
/// - Energy: epsilon (typical value: 1.0 kJ/mol for argon)
/// - Mass: atomic mass (1.0 = one atom mass)
/// - Time: tau = sigma * sqrt(m/epsilon)
///
/// Real units can also be used:
/// - Position: nanometers (nm)
/// - Velocity: nm/ps
/// - Force: kJ/(mol*nm)
/// - Mass: amu (g/mol)
/// - Charge: elementary charges (e)
///
/// # Examples
///
/// ```rust
/// use fxnn::types::Atom;
///
/// // Create atom with builder pattern
/// let atom = Atom::new(0, 0, 39.948)
///     .with_position(1.0, 2.0, 3.0)
///     .with_velocity(0.1, 0.0, 0.0)
///     .with_charge(0.0);
///
/// // Access properties
/// assert_eq!(atom.id, 0);
/// assert!((atom.mass - 39.948).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C)]
pub struct Atom {
    /// Position in 3D space `[x, y, z]` in nm (or reduced units).
    pub position: [f32; 3],

    /// Velocity `[vx, vy, vz]` in nm/ps (or reduced units).
    pub velocity: [f32; 3],

    /// Force `[fx, fy, fz]` in kJ/(mol*nm) (or reduced units).
    pub force: [f32; 3],

    /// Atomic mass in amu (or 1.0 in reduced units).
    pub mass: f32,

    /// Atom type identifier for force field parameter lookup.
    pub atom_type: u16,

    /// Partial charge in elementary charge units (e).
    pub charge: f32,

    /// Unique atom identifier within the simulation.
    pub id: u32,
}

impl Atom {
    /// Create a new atom at the origin with zero velocity.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this atom
    /// * `atom_type` - Type identifier for force field parameters
    /// * `mass` - Atomic mass in amu (or reduced units)
    ///
    /// # Returns
    ///
    /// A new `Atom` at position (0, 0, 0) with zero velocity and force.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// // Create an argon atom (mass 39.948 amu)
    /// let argon = Atom::new(0, 0, 39.948);
    ///
    /// // In reduced units (mass = 1.0)
    /// let reduced = Atom::new(0, 0, 1.0);
    /// ```
    pub fn new(id: u32, atom_type: u16, mass: f32) -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            force: [0.0; 3],
            mass,
            atom_type,
            charge: 0.0,
            id,
        }
    }

    /// Set the position using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    /// * `z` - Z coordinate
    ///
    /// # Returns
    ///
    /// Self with updated position.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let atom = Atom::new(0, 0, 1.0)
    ///     .with_position(1.0, 2.0, 3.0);
    ///
    /// assert_eq!(atom.position, [1.0, 2.0, 3.0]);
    /// ```
    pub fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    /// Set the velocity using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `vx` - X velocity component
    /// * `vy` - Y velocity component
    /// * `vz` - Z velocity component
    ///
    /// # Returns
    ///
    /// Self with updated velocity.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let atom = Atom::new(0, 0, 1.0)
    ///     .with_velocity(0.1, -0.1, 0.0);
    ///
    /// assert_eq!(atom.velocity, [0.1, -0.1, 0.0]);
    /// ```
    pub fn with_velocity(mut self, vx: f32, vy: f32, vz: f32) -> Self {
        self.velocity = [vx, vy, vz];
        self
    }

    /// Set the charge using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `charge` - Partial charge in elementary charge units
    ///
    /// # Returns
    ///
    /// Self with updated charge.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// // Sodium ion with +1 charge
    /// let sodium = Atom::new(0, 1, 22.99).with_charge(1.0);
    ///
    /// // Chloride ion with -1 charge
    /// let chloride = Atom::new(1, 2, 35.45).with_charge(-1.0);
    /// ```
    pub fn with_charge(mut self, charge: f32) -> Self {
        self.charge = charge;
        self
    }

    /// Calculate the kinetic energy of this atom.
    ///
    /// Computes KE = (1/2) * m * |v|^2 where |v|^2 = vx^2 + vy^2 + vz^2.
    ///
    /// # Returns
    ///
    /// Kinetic energy in simulation units (kJ/mol or reduced).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// // Unit mass moving at unit velocity
    /// let atom = Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0);
    /// let ke = atom.kinetic_energy();
    ///
    /// // KE = 0.5 * 1.0 * 1.0^2 = 0.5
    /// assert!((ke - 0.5).abs() < 1e-6);
    /// ```
    #[inline(always)]
    pub fn kinetic_energy(&self) -> f32 {
        let v2 = self.velocity[0] * self.velocity[0]
            + self.velocity[1] * self.velocity[1]
            + self.velocity[2] * self.velocity[2];
        0.5 * self.mass * v2
    }

    /// Calculate the speed (magnitude of velocity).
    ///
    /// Computes |v| = sqrt(vx^2 + vy^2 + vz^2).
    ///
    /// # Returns
    ///
    /// Speed in simulation units (nm/ps or reduced).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let atom = Atom::new(0, 0, 1.0).with_velocity(3.0, 4.0, 0.0);
    /// let speed = atom.speed();
    ///
    /// // |v| = sqrt(9 + 16) = 5
    /// assert!((speed - 5.0).abs() < 1e-6);
    /// ```
    #[inline(always)]
    pub fn speed(&self) -> f32 {
        let v2 = self.velocity[0] * self.velocity[0]
            + self.velocity[1] * self.velocity[1]
            + self.velocity[2] * self.velocity[2];
        v2.sqrt()
    }

    /// Zero the force on this atom.
    ///
    /// This is called at the beginning of each force computation cycle
    /// before accumulating new forces. This is a hot path called every timestep.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let mut atom = Atom::new(0, 0, 1.0);
    /// atom.force = [1.0, 2.0, 3.0];
    ///
    /// atom.zero_force();
    ///
    /// assert_eq!(atom.force, [0.0, 0.0, 0.0]);
    /// ```
    #[inline(always)]
    pub fn zero_force(&mut self) {
        self.force = [0.0; 3];
    }

    /// Add force components to this atom.
    ///
    /// Forces are accumulated during force calculation. This is a hot path
    /// in the innermost loop of force computation.
    ///
    /// # Arguments
    ///
    /// * `fx` - Force X component to add
    /// * `fy` - Force Y component to add
    /// * `fz` - Force Z component to add
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let mut atom = Atom::new(0, 0, 1.0);
    /// atom.add_force(1.0, 0.0, 0.0);
    /// atom.add_force(0.0, 2.0, 0.0);
    ///
    /// assert_eq!(atom.force, [1.0, 2.0, 0.0]);
    /// ```
    #[inline(always)]
    pub fn add_force(&mut self, fx: f32, fy: f32, fz: f32) {
        self.force[0] += fx;
        self.force[1] += fy;
        self.force[2] += fz;
    }

    /// Calculate the squared distance to another atom (without PBC).
    ///
    /// For periodic boundary conditions, use [`SimulationBox::distance_squared`]
    /// instead. This method is useful for non-periodic systems or when PBC
    /// has already been applied.
    ///
    /// # Arguments
    ///
    /// * `other` - The other atom
    ///
    /// # Returns
    ///
    /// Squared distance |r_i - r_j|^2.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let atom1 = Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0);
    /// let atom2 = Atom::new(1, 0, 1.0).with_position(3.0, 4.0, 0.0);
    ///
    /// let d2 = atom1.distance_squared(&atom2);
    /// assert!((d2 - 25.0).abs() < 1e-6);  // 3^2 + 4^2 = 25
    /// ```
    ///
    /// [`SimulationBox::distance_squared`]: crate::types::SimulationBox::distance_squared
    #[inline(always)]
    pub fn distance_squared(&self, other: &Atom) -> f32 {
        let dx = self.position[0] - other.position[0];
        let dy = self.position[1] - other.position[1];
        let dz = self.position[2] - other.position[2];
        dx * dx + dy * dy + dz * dz
    }

    /// Calculate the distance to another atom (without PBC).
    ///
    /// For periodic boundary conditions, use [`SimulationBox::distance`]
    /// instead.
    ///
    /// # Arguments
    ///
    /// * `other` - The other atom
    ///
    /// # Returns
    ///
    /// Euclidean distance |r_i - r_j|.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::Atom;
    ///
    /// let atom1 = Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0);
    /// let atom2 = Atom::new(1, 0, 1.0).with_position(3.0, 4.0, 0.0);
    ///
    /// let d = atom1.distance(&atom2);
    /// assert!((d - 5.0).abs() < 1e-6);  // sqrt(25) = 5
    /// ```
    ///
    /// [`SimulationBox::distance`]: crate::types::SimulationBox::distance
    #[inline(always)]
    pub fn distance(&self, other: &Atom) -> f32 {
        self.distance_squared(other).sqrt()
    }
}

impl Default for Atom {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            force: [0.0; 3],
            mass: 1.0,
            atom_type: 0,
            charge: 0.0,
            id: 0,
        }
    }
}

/// Structure-of-Arrays (SoA) representation for SIMD-optimized access.
///
/// While [`Atom`] uses Array-of-Structures (AoS) layout where each atom's
/// data is contiguous, `AtomsSoA` stores each property in a separate array.
/// This layout enables efficient SIMD vectorization where the same operation
/// is applied to many atoms simultaneously.
///
/// # Memory Layout Comparison
///
/// **Array of Structures (AoS)** - `Vec<Atom>`:
/// ```text
/// [x0 y0 z0 vx0 vy0 vz0 ...] [x1 y1 z1 vx1 vy1 vz1 ...] ...
/// ```
///
/// **Structure of Arrays (SoA)** - `AtomsSoA`:
/// ```text
/// x:  [x0  x1  x2  x3  ...]
/// y:  [y0  y1  y2  y3  ...]
/// z:  [z0  z1  z2  z3  ...]
/// vx: [vx0 vx1 vx2 vx3 ...]
/// ...
/// ```
///
/// SoA enables loading 4 or 8 x-coordinates at once with SIMD instructions,
/// performing vectorized operations, and storing results back efficiently.
///
/// # When to Use
///
/// - **Use SoA** for batch operations on large systems (1000+ atoms)
/// - **Use AoS** for random access, small systems, or single-atom operations
///
/// # Examples
///
/// ```rust
/// use fxnn::types::{Atom, AtomsSoA};
///
/// // Create atoms in AoS format
/// let atoms = vec![
///     Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
///     Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0),
///     Atom::new(2, 0, 1.0).with_position(2.0, 0.0, 0.0),
///     Atom::new(3, 0, 1.0).with_position(3.0, 0.0, 0.0),
/// ];
///
/// // Convert to SoA for SIMD operations
/// let soa = AtomsSoA::from_atoms(&atoms);
/// assert_eq!(soa.len(), 4);
/// assert_eq!(soa.x, vec![0.0, 1.0, 2.0, 3.0]);
///
/// // Convert back to AoS
/// let atoms_back = soa.to_atoms();
/// assert_eq!(atoms_back.len(), 4);
/// ```
#[derive(Debug, Clone)]
pub struct AtomsSoA {
    /// X positions of all atoms.
    pub x: Vec<f32>,
    /// Y positions of all atoms.
    pub y: Vec<f32>,
    /// Z positions of all atoms.
    pub z: Vec<f32>,
    /// X velocities of all atoms.
    pub vx: Vec<f32>,
    /// Y velocities of all atoms.
    pub vy: Vec<f32>,
    /// Z velocities of all atoms.
    pub vz: Vec<f32>,
    /// X forces on all atoms.
    pub fx: Vec<f32>,
    /// Y forces on all atoms.
    pub fy: Vec<f32>,
    /// Z forces on all atoms.
    pub fz: Vec<f32>,
    /// Masses of all atoms.
    pub mass: Vec<f32>,
    /// Charges of all atoms.
    pub charge: Vec<f32>,
    /// Atom types for all atoms.
    pub atom_type: Vec<u16>,
}

impl AtomsSoA {
    /// Create SoA representation from a slice of atoms.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Slice of atoms in AoS format
    ///
    /// # Returns
    ///
    /// New `AtomsSoA` with data transposed from AoS to SoA layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::{Atom, AtomsSoA};
    ///
    /// let atoms = vec![
    ///     Atom::new(0, 0, 1.0).with_position(1.0, 2.0, 3.0),
    ///     Atom::new(1, 0, 2.0).with_position(4.0, 5.0, 6.0),
    /// ];
    ///
    /// let soa = AtomsSoA::from_atoms(&atoms);
    ///
    /// assert_eq!(soa.x, vec![1.0, 4.0]);
    /// assert_eq!(soa.mass, vec![1.0, 2.0]);
    /// ```
    pub fn from_atoms(atoms: &[Atom]) -> Self {
        let n = atoms.len();
        let mut soa = Self {
            x: Vec::with_capacity(n),
            y: Vec::with_capacity(n),
            z: Vec::with_capacity(n),
            vx: Vec::with_capacity(n),
            vy: Vec::with_capacity(n),
            vz: Vec::with_capacity(n),
            fx: Vec::with_capacity(n),
            fy: Vec::with_capacity(n),
            fz: Vec::with_capacity(n),
            mass: Vec::with_capacity(n),
            charge: Vec::with_capacity(n),
            atom_type: Vec::with_capacity(n),
        };

        for atom in atoms {
            soa.x.push(atom.position[0]);
            soa.y.push(atom.position[1]);
            soa.z.push(atom.position[2]);
            soa.vx.push(atom.velocity[0]);
            soa.vy.push(atom.velocity[1]);
            soa.vz.push(atom.velocity[2]);
            soa.fx.push(atom.force[0]);
            soa.fy.push(atom.force[1]);
            soa.fz.push(atom.force[2]);
            soa.mass.push(atom.mass);
            soa.charge.push(atom.charge);
            soa.atom_type.push(atom.atom_type);
        }

        soa
    }

    /// Convert back to array of atoms (AoS format).
    ///
    /// # Returns
    ///
    /// Vector of atoms with data transposed from SoA back to AoS layout.
    /// Atom IDs are assigned sequentially starting from 0.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::{Atom, AtomsSoA};
    ///
    /// let original = vec![
    ///     Atom::new(0, 0, 1.0).with_position(1.0, 0.0, 0.0),
    ///     Atom::new(1, 0, 1.0).with_position(2.0, 0.0, 0.0),
    /// ];
    ///
    /// let soa = AtomsSoA::from_atoms(&original);
    /// let restored = soa.to_atoms();
    ///
    /// assert_eq!(restored[0].position[0], 1.0);
    /// assert_eq!(restored[1].position[0], 2.0);
    /// ```
    pub fn to_atoms(&self) -> Vec<Atom> {
        let n = self.x.len();
        (0..n)
            .map(|i| Atom {
                position: [self.x[i], self.y[i], self.z[i]],
                velocity: [self.vx[i], self.vy[i], self.vz[i]],
                force: [self.fx[i], self.fy[i], self.fz[i]],
                mass: self.mass[i],
                charge: self.charge[i],
                atom_type: self.atom_type[i],
                id: i as u32,
            })
            .collect()
    }

    /// Get the number of atoms.
    ///
    /// # Returns
    ///
    /// Number of atoms in the SoA.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::{Atom, AtomsSoA};
    ///
    /// let atoms = vec![Atom::new(0, 0, 1.0); 100];
    /// let soa = AtomsSoA::from_atoms(&atoms);
    ///
    /// assert_eq!(soa.len(), 100);
    /// ```
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Check if the SoA is empty.
    ///
    /// # Returns
    ///
    /// `true` if no atoms are stored, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Zero all forces.
    ///
    /// This is called at the beginning of each force computation cycle.
    /// Uses `fill()` for efficient bulk memory operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::types::{Atom, AtomsSoA};
    ///
    /// let atoms = vec![Atom::new(0, 0, 1.0); 4];
    /// let mut soa = AtomsSoA::from_atoms(&atoms);
    ///
    /// // Simulate some force accumulation
    /// soa.fx[0] = 1.0;
    /// soa.fy[1] = 2.0;
    ///
    /// // Zero all forces
    /// soa.zero_forces();
    ///
    /// assert!(soa.fx.iter().all(|&f| f == 0.0));
    /// assert!(soa.fy.iter().all(|&f| f == 0.0));
    /// assert!(soa.fz.iter().all(|&f| f == 0.0));
    /// ```
    pub fn zero_forces(&mut self) {
        self.fx.fill(0.0);
        self.fy.fill(0.0);
        self.fz.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_creation() {
        let atom = Atom::new(0, 1, 39.948)
            .with_position(1.0, 2.0, 3.0)
            .with_velocity(0.1, 0.2, 0.3);

        assert_eq!(atom.id, 0);
        assert_eq!(atom.atom_type, 1);
        assert!((atom.mass - 39.948).abs() < 1e-6);
        assert_eq!(atom.position, [1.0, 2.0, 3.0]);
        assert_eq!(atom.velocity, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_kinetic_energy() {
        let atom = Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0);
        assert!((atom.kinetic_energy() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_soa_conversion() {
        let atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 1.0, 1.0),
        ];

        let soa = AtomsSoA::from_atoms(&atoms);
        assert_eq!(soa.len(), 2);
        assert_eq!(soa.x[0], 0.0);
        assert_eq!(soa.x[1], 1.0);

        let atoms_back = soa.to_atoms();
        assert_eq!(atoms_back.len(), 2);
    }
}
