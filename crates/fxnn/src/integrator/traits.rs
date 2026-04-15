//! Integrator trait definitions

use crate::types::{Atom, SimulationBox};

/// Trait for time integration schemes
pub trait Integrator: Send + Sync {
    /// Perform one integration step
    ///
    /// # Arguments
    /// * `atoms` - Mutable slice of atoms to update
    /// * `box_` - Simulation box for boundary conditions
    /// * `dt` - Timestep
    /// * `compute_forces` - Closure that computes forces on atoms
    fn step<F>(&self, atoms: &mut [Atom], box_: &SimulationBox, dt: f32, compute_forces: F)
    where
        F: FnMut(&mut [Atom]);

    /// Get the name of this integrator
    fn name(&self) -> &str;

    /// Check if this integrator preserves energy (is symplectic)
    fn is_symplectic(&self) -> bool {
        false
    }
}
