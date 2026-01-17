//! Force field implementations for molecular dynamics simulations.
//!
//! This module provides various force field implementations that compute
//! interatomic potentials and forces. Force fields are the physics engine
//! of molecular dynamics, defining how atoms interact.
//!
//! # Overview
//!
//! FXNN supports several types of interactions:
//!
//! - **Non-bonded interactions**:
//!   - [`LennardJones`]: Van der Waals interactions (12-6 potential)
//!   - [`Coulomb`]: Electrostatic interactions
//!
//! - **Bonded interactions** (via [`BondedForces`]):
//!   - [`HarmonicBond`]: Covalent bond stretching
//!   - [`HarmonicAngle`]: Bond angle bending
//!   - [`PeriodicDihedral`]: Torsional angles
//!
//! - **Composite force fields**:
//!   - [`CompositeForceField`]: Combine multiple force fields
//!
//! - **Machine learning potentials** (with `neural` feature):
//!   - `NeuralForceField`: Neural network-based potentials
//!   - `SchNetModel`: SchNet continuous-filter convolutions
//!
//! # The ForceField Trait
//!
//! All force fields implement the [`ForceField`] trait:
//!
//! ```rust,ignore
//! pub trait ForceField: Send + Sync {
//!     fn compute_forces(&self, atoms: &mut [Atom], box_: &SimulationBox, neighbor_list: Option<&NeighborList>);
//!     fn potential_energy(&self, atoms: &[Atom], box_: &SimulationBox, neighbor_list: Option<&NeighborList>) -> f64;
//!     fn cutoff(&self) -> f32;
//!     fn name(&self) -> &str;
//! }
//! ```
//!
//! # Examples
//!
//! ## Lennard-Jones for noble gases
//!
//! ```rust
//! use fxnn::force_field::LennardJones;
//!
//! // Argon in reduced units (epsilon=1, sigma=1)
//! let lj_reduced = LennardJones::argon();
//!
//! // Custom parameters for different species
//! let mut lj = LennardJones::new(2, 2.5);  // 2 atom types, cutoff=2.5
//! lj.set_parameters(0, 0, 1.0, 1.0);  // Type 0-0 interaction
//! lj.set_parameters(0, 1, 0.8, 1.1);  // Type 0-1 (mixed)
//! lj.set_parameters(1, 1, 0.6, 1.2);  // Type 1-1 interaction
//! ```
//!
//! ## Combining force fields
//!
//! ```rust
//! use fxnn::force_field::{LennardJones, Coulomb, CompositeForceField};
//!
//! // LJ + Coulomb for ionic systems
//! let lj = LennardJones::argon();
//! let coulomb = Coulomb::new(2.5);  // cutoff = 2.5
//!
//! let composite = CompositeForceField::new()
//!     .add(lj)
//!     .add(coulomb);
//! ```
//!
//! ## Bonded interactions for molecules
//!
//! ```rust
//! use fxnn::force_field::BondedForces;
//! use fxnn::types::Topology;
//!
//! // Create topology for a water molecule
//! let mut topology = Topology::new(3);
//! topology.add_bond(0, 1, 0);  // O-H bond (type 0)
//! topology.add_bond(0, 2, 0);  // O-H bond (type 0)
//! topology.add_angle(1, 0, 2, 0);  // H-O-H angle (type 0)
//!
//! // Create bonded force field
//! let mut bonded = BondedForces::new(topology);
//!
//! // Add parameters for bond type 0: k=1000 kJ/(mol*nm^2), r0=0.1 nm
//! bonded.bonds.add_type(1000.0, 0.1);
//!
//! // Add parameters for angle type 0: k=100 kJ/(mol*rad^2), theta0=1.91 rad
//! bonded.angles.add_type(100.0, 1.91);
//! ```
//!
//! # Mathematical Background
//!
//! ## Lennard-Jones Potential
//!
//! The 12-6 Lennard-Jones potential models van der Waals interactions:
//!
//! ```text
//! V(r) = 4*epsilon * [(sigma/r)^12 - (sigma/r)^6]
//! ```
//!
//! Where:
//! - `epsilon` is the depth of the potential well
//! - `sigma` is the distance at which V(r) = 0
//! - The r^-12 term represents Pauli repulsion
//! - The r^-6 term represents London dispersion attraction
//!
//! ## Coulomb Potential
//!
//! Electrostatic interactions between charged particles:
//!
//! ```text
//! V(r) = (1/4*pi*epsilon_0) * (q_i * q_j) / r
//! ```
//!
//! ## Harmonic Bond Potential
//!
//! Covalent bond stretching:
//!
//! ```text
//! V(r) = (1/2) * k * (r - r0)^2
//! ```
//!
//! # Performance
//!
//! Force calculation is the computational bottleneck in MD simulations.
//! FXNN optimizes this through:
//!
//! - **Neighbor lists**: O(N) instead of O(N^2) scaling
//! - **SIMD vectorization**: Process 4-8 pairs simultaneously
//! - **Cache optimization**: Memory access patterns tuned for modern CPUs
//! - **Newton's third law**: Compute each pair only once

mod traits;
mod lennard_jones;
mod coulomb;
mod bonded;
mod composite;

#[cfg(feature = "neural")]
pub mod neural;

pub use traits::{ForceField, PairParameters};
pub use lennard_jones::LennardJones;
pub use coulomb::{Coulomb, CoulombMethod};
pub use bonded::{HarmonicBond, HarmonicAngle, PeriodicDihedral, BondedForces};
pub use composite::CompositeForceField;

#[cfg(feature = "neural")]
pub use neural::{
    NeuralForceField, SchNetModel, RadialBasisFunctions,
    DenseLayer, CFConvLayer, InteractionBlock,
    sona::{LearningStats, SonaAdapter},
};
