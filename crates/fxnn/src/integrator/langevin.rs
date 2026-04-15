//! Langevin dynamics integrator (thermostat)
//!
//! Implements stochastic dynamics with friction and random forces:
//! m * dv/dt = F - γ*v + √(2*γ*kT*m) * η(t)
//!
//! Uses the BAOAB splitting scheme for accurate sampling.

use crate::types::{Atom, SimulationBox};
use super::traits::Integrator;
use rand::Rng;
use rand_distr::{Normal, Distribution};

/// Langevin dynamics integrator with velocity rescaling
#[derive(Debug, Clone)]
pub struct Langevin {
    /// Friction coefficient (1/ps)
    gamma: f32,
    /// Target temperature (K or reduced units)
    temperature: f32,
    /// Boltzmann constant (kJ/(mol·K) or 1.0 in reduced units)
    kb: f32,
    /// Random number generator seed
    seed: u64,
    /// Whether to wrap positions into the box
    wrap_positions: bool,
}

impl Langevin {
    /// Create a new Langevin integrator
    ///
    /// # Arguments
    /// * `gamma` - Friction coefficient (1/ps)
    /// * `temperature` - Target temperature
    /// * `kb` - Boltzmann constant
    pub fn new(gamma: f32, temperature: f32, kb: f32) -> Self {
        Self {
            gamma,
            temperature,
            kb,
            seed: 42,
            wrap_positions: true,
        }
    }

    /// Create in reduced units (kb = 1)
    pub fn reduced_units(gamma: f32, temperature: f32) -> Self {
        Self::new(gamma, temperature, 1.0)
    }

    /// Set random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Set whether to wrap positions
    pub fn with_wrap_positions(mut self, wrap: bool) -> Self {
        self.wrap_positions = wrap;
        self
    }

    /// Get the target temperature
    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    /// Set the target temperature
    pub fn set_temperature(&mut self, temperature: f32) {
        self.temperature = temperature;
    }
}

impl Integrator for Langevin {
    fn step<F>(&self, atoms: &mut [Atom], box_: &SimulationBox, dt: f32, mut compute_forces: F)
    where
        F: FnMut(&mut [Atom]),
    {
        let mut rng = rand::thread_rng();
        let dt_half = dt * 0.5;

        // BAOAB splitting scheme:
        // B: velocity update (kick)
        // A: position update (drift)
        // O: Ornstein-Uhlenbeck process (thermostat)

        // Precompute thermostat factors
        let c1 = (-self.gamma * dt).exp();
        let c2 = (1.0 - c1 * c1).sqrt();

        // B step: half kick
        for atom in atoms.iter_mut() {
            let inv_mass = 1.0 / atom.mass;
            atom.velocity[0] += dt_half * atom.force[0] * inv_mass;
            atom.velocity[1] += dt_half * atom.force[1] * inv_mass;
            atom.velocity[2] += dt_half * atom.force[2] * inv_mass;
        }

        // A step: half drift
        for atom in atoms.iter_mut() {
            atom.position[0] += dt_half * atom.velocity[0];
            atom.position[1] += dt_half * atom.velocity[1];
            atom.position[2] += dt_half * atom.velocity[2];
        }

        // O step: Ornstein-Uhlenbeck (thermostat)
        for atom in atoms.iter_mut() {
            let sigma = (self.kb * self.temperature / atom.mass).sqrt();
            let normal = Normal::new(0.0, sigma as f64).unwrap();

            atom.velocity[0] = c1 * atom.velocity[0] + c2 * normal.sample(&mut rng) as f32;
            atom.velocity[1] = c1 * atom.velocity[1] + c2 * normal.sample(&mut rng) as f32;
            atom.velocity[2] = c1 * atom.velocity[2] + c2 * normal.sample(&mut rng) as f32;
        }

        // A step: half drift
        for atom in atoms.iter_mut() {
            atom.position[0] += dt_half * atom.velocity[0];
            atom.position[1] += dt_half * atom.velocity[1];
            atom.position[2] += dt_half * atom.velocity[2];
        }

        // Wrap positions if requested
        if self.wrap_positions {
            for atom in atoms.iter_mut() {
                let wrapped = box_.wrap_position(
                    atom.position[0],
                    atom.position[1],
                    atom.position[2],
                );
                atom.position = wrapped;
            }
        }

        // Compute new forces
        for atom in atoms.iter_mut() {
            atom.zero_force();
        }
        compute_forces(atoms);

        // B step: half kick
        for atom in atoms.iter_mut() {
            let inv_mass = 1.0 / atom.mass;
            atom.velocity[0] += dt_half * atom.force[0] * inv_mass;
            atom.velocity[1] += dt_half * atom.force[1] * inv_mass;
            atom.velocity[2] += dt_half * atom.force[2] * inv_mass;
        }
    }

    fn name(&self) -> &str {
        "Langevin (BAOAB)"
    }

    fn is_symplectic(&self) -> bool {
        false // Stochastic, not symplectic
    }
}

/// Calculate instantaneous temperature from kinetic energy
pub fn calculate_temperature(atoms: &[Atom], kb: f32) -> f32 {
    let n = atoms.len() as f32;
    let dof = 3.0 * n - 3.0; // Degrees of freedom (subtract COM motion)

    if dof <= 0.0 {
        return 0.0;
    }

    let kinetic_energy: f32 = atoms.iter().map(|a| a.kinetic_energy()).sum();
    2.0 * kinetic_energy / (dof * kb)
}

/// Velocity rescaling thermostat (simple)
pub fn rescale_velocities(atoms: &mut [Atom], target_temp: f32, kb: f32) {
    let current_temp = calculate_temperature(atoms, kb);

    if current_temp < 1e-10 {
        return;
    }

    let scale = (target_temp / current_temp).sqrt();

    for atom in atoms.iter_mut() {
        atom.velocity[0] *= scale;
        atom.velocity[1] *= scale;
        atom.velocity[2] *= scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_calculation() {
        let atoms = vec![
            Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_velocity(-1.0, 0.0, 0.0),
        ];

        // KE = 2 * 0.5 * 1.0 * 1.0 = 1.0
        // T = 2 * KE / (dof * kb) = 2 * 1.0 / (3 * 1.0) = 2/3
        let temp = calculate_temperature(&atoms, 1.0);
        assert!((temp - 2.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_velocity_rescaling() {
        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_velocity(1.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_velocity(-1.0, 0.0, 0.0),
        ];

        let target_temp = 1.0;
        rescale_velocities(&mut atoms, target_temp, 1.0);

        let final_temp = calculate_temperature(&atoms, 1.0);
        assert!((final_temp - target_temp).abs() < 1e-5);
    }
}
