//! Core data types for molecular dynamics simulations.
//!
//! This module provides the fundamental data structures used throughout FXNN:
//!
//! - [`Atom`]: Represents a single particle with position, velocity, force, and properties
//! - [`SimulationBox`]: Defines the simulation domain and periodic boundary conditions
//! - [`Topology`]: Describes bonded connectivity for molecules
//! - [`AtomType`]: Identifies atom species for force field parameters
//!
//! # Design Philosophy
//!
//! The types are designed for:
//!
//! - **Performance**: Memory layout optimized for SIMD operations and cache efficiency
//! - **Flexibility**: Support for various simulation setups (periodic/non-periodic, molecular/atomic)
//! - **Interoperability**: Serialization support via serde for checkpointing and I/O
//!
//! # Examples
//!
//! ## Creating Atoms
//!
//! ```rust
//! use fxnn::types::Atom;
//!
//! // Builder pattern for atom creation
//! let argon = Atom::new(0, 0, 39.948)
//!     .with_position(1.0, 2.0, 3.0)
//!     .with_velocity(0.1, -0.1, 0.0);
//!
//! // Calculate kinetic energy: KE = (1/2) * m * v^2
//! let ke = argon.kinetic_energy();
//! ```
//!
//! ## Setting Up a Simulation Box
//!
//! ```rust
//! use fxnn::types::SimulationBox;
//!
//! // Cubic box with periodic boundaries
//! let cubic = SimulationBox::cubic(10.0);
//!
//! // Orthorhombic box
//! let ortho = SimulationBox::orthorhombic(10.0, 12.0, 15.0);
//!
//! // Non-periodic (isolated system)
//! let isolated = SimulationBox::non_periodic(20.0, 20.0, 20.0);
//! ```
//!
//! ## Molecular Topology
//!
//! ```rust
//! use fxnn::types::Topology;
//!
//! // Create topology for a chain of 4 atoms
//! let mut topo = Topology::new(4);
//! topo.add_bond(0, 1, 0);  // Bond between atoms 0-1
//! topo.add_bond(1, 2, 0);  // Bond between atoms 1-2
//! topo.add_bond(2, 3, 0);  // Bond between atoms 2-3
//!
//! // Automatically detect angles from bonds
//! topo.detect_angles();
//! ```

mod atom;
mod simulation_box;
mod topology;

pub use atom::{Atom, AtomType, AtomsSoA};
pub use simulation_box::SimulationBox;
pub use topology::{Topology, Bond, Angle, Dihedral};
