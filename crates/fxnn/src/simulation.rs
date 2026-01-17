//! Main simulation engine for molecular dynamics.
//!
//! This module provides the [`Simulation`] struct, which orchestrates all
//! components of a molecular dynamics simulation: atoms, force fields,
//! integrators, and neighbor lists.
//!
//! # Overview
//!
//! The simulation engine handles:
//!
//! - Time stepping via configurable integrators (Verlet, Langevin)
//! - Force calculation through pluggable force fields
//! - Neighbor list management with automatic rebuilding
//! - Observable calculation (energy, temperature)
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
//! use fxnn::generators::fcc_lattice;
//!
//! // Setup system
//! let atoms = fcc_lattice(4, 4, 4, 1.5);
//! let box_ = SimulationBox::cubic(6.0);
//! let lj = LennardJones::argon();
//! let integrator = VelocityVerlet::new();
//!
//! // Create simulation
//! let mut sim = Simulation::new(atoms, box_, lj, integrator)
//!     .with_timestep(0.001);
//!
//! // Run and analyze
//! println!("Initial energy: {}", sim.total_energy());
//! sim.run(1000);
//! println!("Final energy: {}", sim.total_energy());
//! ```
//!
//! # Algorithm
//!
//! Each simulation step proceeds as follows:
//!
//! 1. Check if neighbor list needs rebuilding (based on atom displacements)
//! 2. Rebuild neighbor list if necessary
//! 3. Call integrator to advance positions and velocities
//! 4. The integrator internally calls force computation as needed
//! 5. Update simulation time and step counter

use crate::types::{Atom, SimulationBox};
use crate::force_field::ForceField;
use crate::integrator::Integrator;
use crate::neighbor::{NeighborList, VerletList, NeighborSearch};
use crate::observable;

/// Main molecular dynamics simulation container.
///
/// The `Simulation` struct brings together all components needed to run a
/// molecular dynamics simulation. It is generic over the force field `F`
/// and integrator `I`, allowing flexible combination of different methods.
///
/// # Type Parameters
///
/// * `F` - Force field type implementing [`ForceField`]
/// * `I` - Integrator type implementing [`Integrator`]
///
/// # Examples
///
/// ```rust,no_run
/// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
/// use fxnn::generators::random_atoms;
///
/// let box_ = SimulationBox::cubic(10.0);
/// let atoms = random_atoms(100, &box_);
///
/// let mut sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new())
///     .with_timestep(0.001);
///
/// // Run equilibration
/// sim.run(1000);
///
/// // Access properties
/// println!("Step: {}", sim.step());
/// println!("Time: {}", sim.time());
/// println!("Temperature: {}", sim.temperature());
/// ```
pub struct Simulation<F: ForceField, I: Integrator> {
    /// The atoms in the simulation.
    atoms: Vec<Atom>,
    /// The simulation box defining domain and boundaries.
    box_: SimulationBox,
    /// The force field for computing interactions.
    force_field: F,
    /// The time integration scheme.
    integrator: I,
    /// Verlet neighbor list for efficient force calculation.
    neighbor_list: VerletList,
    /// Current simulation step number.
    step: usize,
    /// Current simulation time.
    time: f64,
    /// Timestep for integration.
    dt: f32,
    /// Boltzmann constant (1.0 for reduced units).
    kb: f32,
}

impl<F: ForceField, I: Integrator> Simulation<F, I> {
    /// Create a new molecular dynamics simulation.
    ///
    /// Initializes the simulation with the given atoms, box, force field,
    /// and integrator. The neighbor list is automatically built and initial
    /// forces are computed.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Initial atomic configuration
    /// * `box_` - Simulation box with periodic boundary conditions
    /// * `force_field` - Force field for computing interactions
    /// * `integrator` - Time integration scheme
    ///
    /// # Returns
    ///
    /// A new `Simulation` instance ready for time stepping.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    ///
    /// let sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    /// ```
    pub fn new(atoms: Vec<Atom>, box_: SimulationBox, force_field: F, integrator: I) -> Self {
        let n_atoms = atoms.len();
        let cutoff = force_field.cutoff();
        let skin = 0.5;
        let mut sim = Self {
            atoms, box_, force_field, integrator,
            neighbor_list: VerletList::new(n_atoms, cutoff, skin),
            step: 0, time: 0.0, dt: 0.001, kb: 1.0,
        };
        sim.rebuild_neighbor_list();
        sim.compute_forces();
        sim
    }

    /// Set the integration timestep (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `dt` - Timestep size (typical: 0.001-0.005 in reduced units)
    ///
    /// # Returns
    ///
    /// Self with the updated timestep.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    ///
    /// let sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new())
    ///     .with_timestep(0.002);
    /// ```
    pub fn with_timestep(mut self, dt: f32) -> Self {
        self.dt = dt;
        self
    }

    /// Get a reference to the atoms.
    ///
    /// # Returns
    ///
    /// Slice of all atoms in the simulation.
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    /// Get a mutable reference to the atoms.
    ///
    /// # Returns
    ///
    /// Mutable slice of all atoms in the simulation.
    ///
    /// # Warning
    ///
    /// Modifying atom positions directly may invalidate the neighbor list.
    /// Call `step_forward()` or manually rebuild the neighbor list after
    /// significant position changes.
    pub fn atoms_mut(&mut self) -> &mut [Atom] {
        &mut self.atoms
    }

    /// Get a reference to the simulation box.
    ///
    /// # Returns
    ///
    /// Reference to the [`SimulationBox`].
    pub fn box_(&self) -> &SimulationBox {
        &self.box_
    }

    /// Get the current step number.
    ///
    /// # Returns
    ///
    /// Number of simulation steps completed.
    pub fn step(&self) -> usize {
        self.step
    }

    /// Get the current simulation time.
    ///
    /// # Returns
    ///
    /// Accumulated simulation time (step * dt).
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Rebuild the neighbor list from current positions.
    fn rebuild_neighbor_list(&mut self) {
        let cutoff = self.force_field.cutoff();
        self.neighbor_list.build(&self.atoms, &self.box_, cutoff);
    }

    /// Compute forces on all atoms.
    fn compute_forces(&mut self) {
        for atom in &mut self.atoms {
            atom.zero_force();
        }
        let nl = self.neighbor_list.neighbor_list();
        self.force_field.compute_forces(&mut self.atoms, &self.box_, Some(nl));
    }

    /// Perform one integration step.
    ///
    /// Advances the simulation by one timestep:
    /// 1. Checks and rebuilds neighbor list if needed
    /// 2. Integrates equations of motion
    /// 3. Updates time and step counter
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    /// let mut sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    ///
    /// // Single step
    /// sim.step_forward();
    /// assert_eq!(sim.step(), 1);
    /// ```
    pub fn step_forward(&mut self) {
        // Rebuild neighbor list if atoms have moved too far
        if self.neighbor_list.needs_rebuild(&self.atoms, &self.box_) {
            self.rebuild_neighbor_list();
        }

        let box_ = self.box_;
        let force_field = &self.force_field;
        let neighbor_list = &self.neighbor_list;

        // Perform integration step with force callback
        self.integrator.step(&mut self.atoms, &box_, self.dt, |atoms| {
            force_field.compute_forces(atoms, &box_, Some(neighbor_list.neighbor_list()));
        });

        self.step += 1;
        self.time += self.dt as f64;
    }

    /// Run multiple integration steps.
    ///
    /// Convenience method to advance the simulation by `n_steps` timesteps.
    ///
    /// # Arguments
    ///
    /// * `n_steps` - Number of steps to run
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    /// let mut sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    ///
    /// // Run 10000 steps
    /// sim.run(10000);
    /// assert_eq!(sim.step(), 10000);
    /// ```
    pub fn run(&mut self, n_steps: usize) {
        for _ in 0..n_steps {
            self.step_forward();
        }
    }

    /// Calculate the total kinetic energy of the system.
    ///
    /// Computes KE = sum_i (1/2) * m_i * |v_i|^2
    ///
    /// # Returns
    ///
    /// Total kinetic energy in simulation units.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::{random_atoms, maxwell_boltzmann_velocities};
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let mut atoms = random_atoms(100, &box_);
    /// maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
    ///
    /// let sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    /// println!("Kinetic energy: {}", sim.kinetic_energy());
    /// ```
    pub fn kinetic_energy(&self) -> f64 {
        observable::kinetic_energy(&self.atoms)
    }

    /// Calculate the total potential energy of the system.
    ///
    /// Computes the potential energy from all interactions in the force field.
    ///
    /// # Returns
    ///
    /// Total potential energy in simulation units.
    pub fn potential_energy(&self) -> f64 {
        let nl = self.neighbor_list.neighbor_list();
        self.force_field.potential_energy(&self.atoms, &self.box_, Some(nl))
    }

    /// Calculate the total energy of the system.
    ///
    /// Returns kinetic + potential energy. For conservative (NVE) simulations,
    /// this should be approximately constant.
    ///
    /// # Returns
    ///
    /// Total energy (kinetic + potential) in simulation units.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    /// let mut sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    ///
    /// let e0 = sim.total_energy();
    /// sim.run(1000);
    /// let e1 = sim.total_energy();
    ///
    /// // Energy should be conserved for Verlet integration
    /// println!("Energy drift: {:.6}%", (e1 - e0) / e0.abs() * 100.0);
    /// ```
    pub fn total_energy(&self) -> f64 {
        self.kinetic_energy() + self.potential_energy()
    }

    /// Calculate the instantaneous temperature.
    ///
    /// Computes temperature from kinetic energy via equipartition:
    /// T = 2 * KE / (N_dof * k_B)
    ///
    /// where N_dof = 3*N - 3 (excluding center-of-mass motion).
    ///
    /// # Returns
    ///
    /// Temperature in simulation units.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::{random_atoms, maxwell_boltzmann_velocities};
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let mut atoms = random_atoms(100, &box_);
    /// maxwell_boltzmann_velocities(&mut atoms, 1.5, 1.0);  // T = 1.5
    ///
    /// let sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    /// println!("Temperature: {:.2}", sim.temperature());  // Should be ~1.5
    /// ```
    pub fn temperature(&self) -> f32 {
        observable::temperature(&self.atoms, self.kb)
    }

    /// Get the number of atoms in the simulation.
    ///
    /// # Returns
    ///
    /// Number of atoms.
    pub fn n_atoms(&self) -> usize {
        self.atoms.len()
    }

    /// Set atom velocities to achieve a target temperature.
    ///
    /// Samples new velocities from the Maxwell-Boltzmann distribution
    /// at the specified temperature, then removes center-of-mass velocity.
    ///
    /// # Arguments
    ///
    /// * `target_temp` - Target temperature in simulation units
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
    /// use fxnn::generators::random_atoms;
    ///
    /// let box_ = SimulationBox::cubic(10.0);
    /// let atoms = random_atoms(100, &box_);
    /// let mut sim = Simulation::new(atoms, box_, LennardJones::argon(), VelocityVerlet::new());
    ///
    /// // Heat the system to T = 2.0
    /// sim.set_temperature(2.0);
    /// println!("New temperature: {:.2}", sim.temperature());
    /// ```
    pub fn set_temperature(&mut self, target_temp: f32) {
        crate::generators::maxwell_boltzmann_velocities(&mut self.atoms, target_temp, self.kb);
        observable::remove_com_velocity(&mut self.atoms);
    }
}
