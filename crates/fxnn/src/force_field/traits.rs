//! Force field trait definitions

use crate::types::{Atom, SimulationBox};
use crate::neighbor::NeighborList;

/// Trait for force field implementations
///
/// A force field computes the potential energy and forces between atoms.
pub trait ForceField: Send + Sync {
    /// Compute forces on all atoms
    ///
    /// Forces are accumulated (added to existing forces).
    /// Call `atom.zero_force()` before computing forces if starting fresh.
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    );

    /// Compute potential energy of the system
    fn potential_energy(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) -> f64;

    /// Get the cutoff radius for this force field
    fn cutoff(&self) -> f32;

    /// Get the name of this force field
    fn name(&self) -> &str;

    /// Check if this force field requires a neighbor list
    fn requires_neighbor_list(&self) -> bool {
        true
    }
}

/// Parameters for a pair interaction
#[derive(Debug, Clone, Copy)]
pub struct PairParameters {
    /// Epsilon (energy well depth)
    pub epsilon: f32,
    /// Sigma (distance at zero potential)
    pub sigma: f32,
    /// Cutoff distance
    pub cutoff: f32,
    /// Potential shift at cutoff
    pub shift: f32,
}

impl PairParameters {
    /// Create new pair parameters
    pub fn new(epsilon: f32, sigma: f32, cutoff: f32) -> Self {
        // Calculate shift for smooth cutoff
        let r6 = (sigma / cutoff).powi(6);
        let r12 = r6 * r6;
        let shift = 4.0 * epsilon * (r12 - r6);

        Self {
            epsilon,
            sigma,
            cutoff,
            shift,
        }
    }

    /// Apply Lorentz-Berthelot mixing rules
    pub fn mix(params1: &PairParameters, params2: &PairParameters) -> Self {
        let sigma = (params1.sigma + params2.sigma) * 0.5;
        let epsilon = (params1.epsilon * params2.epsilon).sqrt();
        let cutoff = params1.cutoff.max(params2.cutoff);
        Self::new(epsilon, sigma, cutoff)
    }
}
