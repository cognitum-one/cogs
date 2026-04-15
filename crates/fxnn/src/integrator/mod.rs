//! Time integration schemes for molecular dynamics simulations.
//!
//! This module provides numerical integrators that advance the equations
//! of motion in time. The integrator determines how positions and velocities
//! are updated based on the forces acting on atoms.
//!
//! # Overview
//!
//! FXNN provides two main integrators:
//!
//! - [`VelocityVerlet`]: Symplectic integrator for NVE (microcanonical) simulations
//! - [`Langevin`]: Stochastic integrator for NVT (canonical) simulations with thermostat
//!
//! # The Integrator Trait
//!
//! All integrators implement the [`Integrator`] trait:
//!
//! ```rust,ignore
//! pub trait Integrator: Send + Sync {
//!     fn step<F>(&self, atoms: &mut [Atom], box_: &SimulationBox, dt: f32, compute_forces: F)
//!     where
//!         F: FnMut(&mut [Atom]);
//!
//!     fn name(&self) -> &str;
//!     fn is_symplectic(&self) -> bool;
//! }
//! ```
//!
//! The `step` method takes a force computation closure, allowing the integrator
//! to request force updates at the appropriate points in its algorithm.
//!
//! # Examples
//!
//! ## NVE simulation with Velocity Verlet
//!
//! ```rust
//! use fxnn::integrator::VelocityVerlet;
//! use fxnn::Integrator;
//!
//! // Create integrator (default: wrap positions into box)
//! let integrator = VelocityVerlet::new();
//!
//! // For isolated systems, disable position wrapping
//! let isolated = VelocityVerlet::new().with_wrap_positions(false);
//!
//! // Check properties
//! assert!(integrator.is_symplectic());  // Conserves energy
//! assert_eq!(integrator.name(), "Velocity Verlet");
//! ```
//!
//! ## NVT simulation with Langevin thermostat
//!
//! ```rust
//! use fxnn::integrator::Langevin;
//!
//! // Langevin dynamics at T=1.0 with friction gamma=1.0
//! let thermostat = Langevin::reduced_units(1.0, 1.0);
//!
//! // With custom seed for reproducibility
//! let reproducible = Langevin::reduced_units(1.0, 1.0).with_seed(12345);
//!
//! // Real units (SI)
//! let real = Langevin::new(
//!     1.0,     // gamma in ps^-1
//!     300.0,   // temperature in K
//!     0.00831, // k_B in kJ/(mol*K)
//! );
//! ```
//!
//! # Mathematical Background
//!
//! ## Velocity Verlet Algorithm
//!
//! The velocity Verlet algorithm is a second-order symplectic integrator.
//! For each timestep dt:
//!
//! ```text
//! 1. v(t + dt/2) = v(t) + (dt/2) * F(t) / m
//! 2. r(t + dt) = r(t) + dt * v(t + dt/2)
//! 3. Compute F(t + dt) from r(t + dt)
//! 4. v(t + dt) = v(t + dt/2) + (dt/2) * F(t + dt) / m
//! ```
//!
//! Properties:
//! - **Time-reversible**: The algorithm is symmetric in time
//! - **Symplectic**: Preserves phase space volume (energy conservation)
//! - **Second-order accurate**: Error ~ O(dt^2)
//!
//! ## Langevin Dynamics (BAOAB)
//!
//! Langevin dynamics adds friction and random forces to model a heat bath:
//!
//! ```text
//! m * dv/dt = F - gamma*m*v + sqrt(2*gamma*k_B*T*m) * eta(t)
//! ```
//!
//! Where:
//! - `gamma` is the friction coefficient (1/ps)
//! - `T` is the target temperature
//! - `eta(t)` is Gaussian white noise
//!
//! FXNN uses the BAOAB splitting scheme:
//! - **B**: Half velocity kick from forces
//! - **A**: Half position drift
//! - **O**: Ornstein-Uhlenbeck velocity update (thermostat)
//! - **A**: Half position drift
//! - **B**: Half velocity kick from new forces
//!
//! # Choosing an Integrator
//!
//! | Integrator | Ensemble | Energy | Temperature | Use Case |
//! |------------|----------|--------|-------------|----------|
//! | VelocityVerlet | NVE | Conserved | Fluctuates | Energy conservation tests, equilibrium properties |
//! | Langevin | NVT | Fluctuates | Controlled | Canonical sampling, temperature control |
//!
//! # Timestep Selection
//!
//! The timestep `dt` must be small enough to accurately integrate the fastest
//! motions in the system. Typical values:
//!
//! - **Reduced units**: dt = 0.001 - 0.005 tau
//! - **Real units**: dt = 0.5 - 2.0 fs (with constraints on H atoms)
//!
//! A rule of thumb: the fastest oscillation period should be resolved by
//! at least 10-20 timesteps.

mod traits;
mod velocity_verlet;
mod langevin;

pub use traits::Integrator;
pub use velocity_verlet::VelocityVerlet;
pub use langevin::Langevin;
