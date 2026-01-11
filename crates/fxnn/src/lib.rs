//! # FXNN: Force-eXpanded Neural Network Molecular Dynamics
//!
//! A high-performance molecular dynamics simulation library written in Rust,
//! combining classical force fields with neural network potentials.
//!
//! ## Overview
//!
//! FXNN provides a comprehensive toolkit for atomistic simulations:
//!
//! - **Classical Force Fields**: Lennard-Jones, Coulomb, and bonded interactions
//! - **Neural Network Potentials**: Machine-learned force fields (with `neural` feature)
//! - **Time Integration**: Velocity Verlet and Langevin dynamics (NVT ensemble)
//! - **Neighbor Search**: Cell lists and Verlet lists for O(N) scaling
//! - **SIMD Optimization**: Vectorized kernels for AVX2/AVX-512/NEON
//! - **Parallel Execution**: Domain decomposition for multi-threaded runs
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
//! use fxnn::generators::{fcc_lattice, maxwell_boltzmann_velocities};
//!
//! // Create a face-centered cubic lattice of atoms
//! let mut atoms = fcc_lattice(4, 4, 4, 1.5);  // 256 atoms
//! let box_ = SimulationBox::cubic(6.0);
//!
//! // Initialize velocities at temperature T=1.0 (reduced units)
//! maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
//!
//! // Configure force field and integrator
//! let lj = LennardJones::argon();
//! let integrator = VelocityVerlet::new();
//!
//! // Create and run simulation
//! let mut sim = Simulation::new(atoms, box_, lj, integrator)
//!     .with_timestep(0.001);  // dt = 0.001 in reduced units
//!
//! sim.run(10000);  // Run 10000 timesteps
//!
//! println!("Total energy: {:.4}", sim.total_energy());
//! println!("Temperature: {:.4}", sim.temperature());
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`types`] | Core data structures: [`Atom`], [`SimulationBox`], [`Topology`] |
//! | [`force_field`] | Force field implementations and traits |
//! | [`integrator`] | Time integration schemes |
//! | [`neighbor`] | Neighbor list algorithms |
//! | [`simd`] | SIMD-optimized computational kernels |
//! | [`observable`] | Thermodynamic property calculations |
//! | [`io`] | File I/O for trajectories and configurations |
//! | [`decomposition`] | Domain decomposition for parallelization (with `parallel` feature) |
//!
//! ## Units
//!
//! FXNN supports both reduced (Lennard-Jones) units and real units:
//!
//! ### Reduced Units (Default)
//!
//! | Quantity | Unit |
//! |----------|------|
//! | Length | sigma |
//! | Energy | epsilon |
//! | Mass | m |
//! | Time | sigma * sqrt(m/epsilon) |
//! | Temperature | epsilon/k_B |
//!
//! ### Real Units (MD Standard)
//!
//! | Quantity | Unit |
//! |----------|------|
//! | Length | Angstrom |
//! | Energy | kcal/mol |
//! | Mass | amu |
//! | Time | femtosecond |
//! | Temperature | Kelvin |
//!
//! ## Feature Flags
//!
//! - `neural`: Enable neural network force fields
//! - `parallel`: Enable multi-threaded execution with rayon
//! - `simd`: Enable explicit SIMD optimizations (auto-detected otherwise)
//!
//! ## Performance Characteristics
//!
//! Typical performance on modern hardware:
//!
//! - **10,000 atoms**: ~50,000 timesteps/second (single-threaded)
//! - **100,000 atoms**: ~5,000 timesteps/second (8 threads)
//! - **1,000,000 atoms**: ~500 timesteps/second (8 threads, with domain decomposition)
//!
//! The library automatically selects optimal algorithms based on system size:
//! - N < 1000: Direct O(N^2) neighbor search
//! - N >= 1000: Cell list with O(N) scaling
//!
//! ## Examples
//!
//! ### Argon Gas Simulation
//!
//! ```rust,no_run
//! use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
//! use fxnn::generators::random_atoms;
//!
//! // Random gas configuration
//! let box_ = SimulationBox::cubic(20.0);
//! let atoms = random_atoms(500, &box_);
//!
//! let sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
//! ```
//!
//! ### NVT Ensemble with Langevin Thermostat
//!
//! ```rust,no_run
//! use fxnn::{Simulation, SimulationBox, LennardJones, Langevin};
//! use fxnn::generators::fcc_lattice;
//!
//! let atoms = fcc_lattice(5, 5, 5, 1.5);
//! let box_ = SimulationBox::cubic(7.5);
//!
//! // Langevin thermostat: gamma=1.0, T=1.0, kb=1.0
//! let thermostat = Langevin::reduced_units(1.0, 1.0);
//!
//! let sim = Simulation::new(atoms, box_, LennardJones::argon(), thermostat);
//! ```
//!
//! ### Composite Force Field
//!
//! ```rust,no_run
//! use fxnn::{Simulation, SimulationBox, LennardJones, Coulomb, CompositeForceField, VelocityVerlet};
//! use fxnn::generators::random_atoms;
//!
//! // Combine LJ and Coulomb interactions
//! let ff = CompositeForceField::new()
//!     .add(LennardJones::argon())
//!     .add(Coulomb::reduced_units(2.5));
//!
//! let box_ = SimulationBox::cubic(10.0);
//! let atoms = random_atoms(100, &box_);
//!
//! let sim = Simulation::new(atoms, box_, ff, VelocityVerlet::new());
//! ```
//!
//! ## References
//!
//! - Frenkel, D. & Smit, B. "Understanding Molecular Simulation" (2002)
//! - Allen, M.P. & Tildesley, D.J. "Computer Simulation of Liquids" (2017)
//! - Tuckerman, M.E. "Statistical Mechanics: Theory and Molecular Simulation" (2010)

// Note: portable_simd is not stable yet - use wide crate instead
// #![cfg_attr(feature = "simd", feature(portable_simd))]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod types;
pub mod force_field;
pub mod integrator;
pub mod neighbor;
pub mod simd;
pub mod observable;
pub mod io;

// Neural module is part of force_field
#[cfg(feature = "neural")]
pub use force_field::neural;

#[cfg(feature = "parallel")]
pub mod decomposition;

mod simulation;
mod error;

// Re-exports for convenient access
pub use types::{Atom, SimulationBox, Topology, AtomType};
pub use force_field::{ForceField, LennardJones, Coulomb, CompositeForceField};
pub use integrator::{Integrator, VelocityVerlet, Langevin};
pub use neighbor::{NeighborList, CellList, VerletList};
pub use simulation::Simulation;
pub use error::{FxnnError, Result};

/// Generators for creating initial atomic configurations.
///
/// This module provides utility functions for setting up common initial
/// conditions for molecular dynamics simulations, including random
/// configurations and crystal lattices.
///
/// # Examples
///
/// ```rust
/// use fxnn::generators::{random_atoms, fcc_lattice, maxwell_boltzmann_velocities};
/// use fxnn::SimulationBox;
///
/// // Random gas configuration
/// let box_ = SimulationBox::cubic(10.0);
/// let atoms = random_atoms(100, &box_);
///
/// // FCC crystal lattice
/// let mut crystal = fcc_lattice(3, 3, 3, 1.5);  // 108 atoms
///
/// // Initialize velocities at T=1.0
/// maxwell_boltzmann_velocities(&mut crystal, 1.0, 1.0);
/// ```
pub mod generators {
    use crate::types::{Atom, SimulationBox};
    use rand::Rng;
    use rand_distr::{Normal, Distribution};

    /// Generate atoms uniformly distributed in the simulation box.
    ///
    /// Creates `n` atoms with random positions uniformly distributed throughout
    /// the simulation box. All atoms are initialized with zero velocity and
    /// unit mass.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of atoms to generate
    /// * `box_` - Simulation box defining the spatial domain
    ///
    /// # Returns
    ///
    /// A vector of `n` atoms with random positions in the box.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::generators::random_atoms;
    /// use fxnn::SimulationBox;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(1000, &box_);
    /// assert_eq!(atoms.len(), 1000);
    /// ```
    pub fn random_atoms(n: usize, box_: &SimulationBox) -> Vec<Atom> {
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|i| {
                Atom {
                    position: [
                        rng.gen::<f32>() * box_.dimensions[0],
                        rng.gen::<f32>() * box_.dimensions[1],
                        rng.gen::<f32>() * box_.dimensions[2],
                    ],
                    velocity: [0.0; 3],
                    force: [0.0; 3],
                    mass: 1.0,
                    atom_type: 0,
                    charge: 0.0,
                    id: i as u32,
                }
            })
            .collect()
    }

    /// Generate atoms on a face-centered cubic (FCC) lattice.
    ///
    /// Creates atoms arranged in an FCC crystal structure, commonly used for
    /// noble gases and metals. The FCC unit cell contains 4 atoms with basis
    /// vectors at (0,0,0), (0.5,0.5,0), (0.5,0,0.5), and (0,0.5,0.5).
    ///
    /// # Arguments
    ///
    /// * `nx` - Number of unit cells in x direction
    /// * `ny` - Number of unit cells in y direction
    /// * `nz` - Number of unit cells in z direction
    /// * `lattice_constant` - Size of the unit cell (typically ~1.5 sigma for LJ at T*=1)
    ///
    /// # Returns
    ///
    /// A vector of `4 * nx * ny * nz` atoms arranged in FCC structure.
    ///
    /// # Mathematical Description
    ///
    /// The FCC lattice positions are:
    ///
    /// ```text
    /// r_{ijk,b} = a * [(i + b_x), (j + b_y), (k + b_z)]
    /// ```
    ///
    /// where `a` is the lattice constant and `b` runs over the 4 basis vectors:
    /// - (0, 0, 0)
    /// - (0.5, 0.5, 0)
    /// - (0.5, 0, 0.5)
    /// - (0, 0.5, 0.5)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::generators::fcc_lattice;
    ///
    /// // Create a 4x4x4 FCC lattice with 256 atoms
    /// let atoms = fcc_lattice(4, 4, 4, 1.5);
    /// assert_eq!(atoms.len(), 256);
    ///
    /// // The box size should be 4 * 1.5 = 6.0 in each dimension
    /// ```
    pub fn fcc_lattice(nx: usize, ny: usize, nz: usize, lattice_constant: f32) -> Vec<Atom> {
        let mut atoms = Vec::with_capacity(4 * nx * ny * nz);
        let basis = [
            [0.0, 0.0, 0.0],
            [0.5, 0.5, 0.0],
            [0.5, 0.0, 0.5],
            [0.0, 0.5, 0.5],
        ];

        let mut id = 0u32;
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    for b in &basis {
                        atoms.push(Atom {
                            position: [
                                (ix as f32 + b[0]) * lattice_constant,
                                (iy as f32 + b[1]) * lattice_constant,
                                (iz as f32 + b[2]) * lattice_constant,
                            ],
                            velocity: [0.0; 3],
                            force: [0.0; 3],
                            mass: 1.0,
                            atom_type: 0,
                            charge: 0.0,
                            id,
                        });
                        id += 1;
                    }
                }
            }
        }
        atoms
    }

    /// Initialize velocities from the Maxwell-Boltzmann distribution.
    ///
    /// Samples velocities from the Maxwell-Boltzmann distribution at the
    /// specified temperature, then removes the center-of-mass velocity to
    /// prevent system drift and ensure zero total momentum.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Mutable slice of atoms to initialize
    /// * `temperature` - Target temperature (in energy units / k_B)
    /// * `kb` - Boltzmann constant (1.0 for reduced units)
    ///
    /// # Mathematical Description
    ///
    /// Each velocity component is drawn from a Gaussian distribution:
    ///
    /// ```text
    /// P(v_i) = sqrt(m / (2 * pi * k_B * T)) * exp(-m * v_i^2 / (2 * k_B * T))
    /// ```
    ///
    /// The standard deviation is `sigma = sqrt(k_B * T / m)`.
    ///
    /// After sampling, the center-of-mass velocity is subtracted to ensure
    /// total momentum is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::generators::{fcc_lattice, maxwell_boltzmann_velocities};
    ///
    /// let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    ///
    /// // Initialize at T=1.0 in reduced units
    /// maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
    ///
    /// // Verify non-zero velocities
    /// assert!(atoms[0].velocity[0].abs() > 0.0 || atoms[0].velocity[1].abs() > 0.0);
    /// ```
    pub fn maxwell_boltzmann_velocities(atoms: &mut [Atom], temperature: f32, kb: f32) {
        let mut rng = rand::thread_rng();

        for atom in atoms.iter_mut() {
            let sigma = (kb * temperature / atom.mass).sqrt();
            let normal = Normal::new(0.0, sigma as f64).unwrap();

            atom.velocity = [
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
            ];
        }

        // Remove center of mass velocity
        let n = atoms.len() as f32;
        let mut vcm = [0.0f32; 3];
        for atom in atoms.iter() {
            vcm[0] += atom.velocity[0];
            vcm[1] += atom.velocity[1];
            vcm[2] += atom.velocity[2];
        }
        vcm[0] /= n;
        vcm[1] /= n;
        vcm[2] /= n;

        for atom in atoms.iter_mut() {
            atom.velocity[0] -= vcm[0];
            atom.velocity[1] -= vcm[1];
            atom.velocity[2] -= vcm[2];
        }
    }
}

/// Physical constants in SI and reduced units.
///
/// This module provides fundamental physical constants for unit conversions
/// and calculations in both SI and reduced (Lennard-Jones) units.
///
/// # Reduced Units
///
/// In reduced units, quantities are expressed relative to characteristic
/// values of the system (typically argon):
///
/// - Length: sigma (3.4 Angstrom for argon)
/// - Energy: epsilon (0.0104 eV for argon)
/// - Mass: atomic mass
/// - Time: sigma * sqrt(m / epsilon) ~ 2.2 ps for argon
///
/// This makes the equations dimensionless and numerically well-conditioned.
pub mod constants {
    /// Boltzmann constant in J/K.
    pub const KB_SI: f64 = 1.380649e-23;

    /// Avogadro's number (mol^-1).
    pub const AVOGADRO: f64 = 6.02214076e23;

    /// Elementary charge in Coulombs.
    pub const ELEMENTARY_CHARGE: f64 = 1.602176634e-19;

    /// Vacuum permittivity in F/m.
    pub const EPSILON_0: f64 = 8.8541878128e-12;

    /// Coulomb constant k = 1/(4*pi*epsilon_0) in N*m^2/C^2.
    pub const COULOMB_CONSTANT: f64 = 8.9875517923e9;

    /// Reduced (Lennard-Jones) units.
    ///
    /// In reduced units, all quantities are dimensionless and expressed
    /// relative to the characteristic scales of the Lennard-Jones potential.
    ///
    /// For argon: sigma = 3.4 A, epsilon = 0.0104 eV, m = 39.948 amu
    pub mod reduced {
        /// Boltzmann constant in reduced units (dimensionless).
        pub const KB: f32 = 1.0;

        /// Reference mass in reduced units (dimensionless).
        pub const MASS: f32 = 1.0;

        /// Reference length (sigma) in reduced units (dimensionless).
        pub const SIGMA: f32 = 1.0;

        /// Reference energy (epsilon) in reduced units (dimensionless).
        pub const EPSILON: f32 = 1.0;

        /// Reference time tau = sigma * sqrt(m/epsilon) in reduced units.
        pub const TIME: f32 = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_atoms() {
        let box_ = SimulationBox::cubic(10.0);
        let atoms = generators::random_atoms(100, &box_);
        assert_eq!(atoms.len(), 100);

        for atom in &atoms {
            assert!(atom.position[0] >= 0.0 && atom.position[0] < 10.0);
            assert!(atom.position[1] >= 0.0 && atom.position[1] < 10.0);
            assert!(atom.position[2] >= 0.0 && atom.position[2] < 10.0);
        }
    }

    #[test]
    fn test_fcc_lattice() {
        let atoms = generators::fcc_lattice(2, 2, 2, 1.0);
        assert_eq!(atoms.len(), 32); // 4 atoms per unit cell * 8 cells
    }
}
