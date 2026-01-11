//! Velocity Verlet integrator
//!
//! The velocity Verlet algorithm is a symplectic integrator that conserves
//! energy well over long simulations.
//!
//! Algorithm:
//! 1. v(t + dt/2) = v(t) + (dt/2) * a(t)
//! 2. r(t + dt) = r(t) + dt * v(t + dt/2)
//! 3. Compute a(t + dt) from r(t + dt)
//! 4. v(t + dt) = v(t + dt/2) + (dt/2) * a(t + dt)
//!
//! This implementation uses loop fusion and cache-friendly access patterns
//! for optimal performance.

use crate::types::{Atom, SimulationBox};
use super::traits::Integrator;

#[cfg(feature = "simd")]
use wide::f32x4;

/// Velocity Verlet integrator
#[derive(Debug, Clone, Copy, Default)]
pub struct VelocityVerlet {
    /// Whether to wrap positions into the box after each step
    wrap_positions: bool,
}

impl VelocityVerlet {
    /// Create a new velocity Verlet integrator
    pub fn new() -> Self {
        Self {
            wrap_positions: true,
        }
    }

    /// Set whether to wrap positions into the box
    pub fn with_wrap_positions(mut self, wrap: bool) -> Self {
        self.wrap_positions = wrap;
        self
    }
}

impl Integrator for VelocityVerlet {
    fn step<F>(&self, atoms: &mut [Atom], box_: &SimulationBox, dt: f32, mut compute_forces: F)
    where
        F: FnMut(&mut [Atom]),
    {
        let dt_half = dt * 0.5;

        // Pre-extract box parameters for faster wrapping
        let [lx, ly, lz] = box_.dimensions;
        let [lx_inv, ly_inv, lz_inv] = box_.inverse;

        // Step 1 & 2: Update velocities by half step and positions in single loop
        // Fuse steps to improve cache locality
        for atom in atoms.iter_mut() {
            let inv_mass = 1.0 / atom.mass;

            // v(t + dt/2) = v(t) + (dt/2) * F(t) / m
            let vx = atom.velocity[0] + dt_half * atom.force[0] * inv_mass;
            let vy = atom.velocity[1] + dt_half * atom.force[1] * inv_mass;
            let vz = atom.velocity[2] + dt_half * atom.force[2] * inv_mass;

            atom.velocity[0] = vx;
            atom.velocity[1] = vy;
            atom.velocity[2] = vz;

            // r(t + dt) = r(t) + dt * v(t + dt/2)
            let mut px = atom.position[0] + dt * vx;
            let mut py = atom.position[1] + dt * vy;
            let mut pz = atom.position[2] + dt * vz;

            // Inline position wrapping for better performance
            if self.wrap_positions {
                px -= lx * (px * lx_inv).floor();
                py -= ly * (py * ly_inv).floor();
                pz -= lz * (pz * lz_inv).floor();
            }

            atom.position[0] = px;
            atom.position[1] = py;
            atom.position[2] = pz;

            // Zero force for next calculation (fused into same loop)
            atom.force = [0.0; 3];
        }

        // Step 3: Compute new forces at r(t + dt)
        compute_forces(atoms);

        // Step 4: Complete velocity update
        for atom in atoms.iter_mut() {
            let inv_mass = 1.0 / atom.mass;

            // v(t + dt) = v(t + dt/2) + (dt/2) * F(t + dt) / m
            atom.velocity[0] += dt_half * atom.force[0] * inv_mass;
            atom.velocity[1] += dt_half * atom.force[1] * inv_mass;
            atom.velocity[2] += dt_half * atom.force[2] * inv_mass;
        }
    }

    fn name(&self) -> &str {
        "Velocity Verlet"
    }

    fn is_symplectic(&self) -> bool {
        true
    }
}

/// Batch integrator step for Structure-of-Arrays layout
///
/// More SIMD-friendly for large systems.
#[cfg(feature = "simd")]
#[inline]
pub fn velocity_verlet_step_soa(
    x: &mut [f32],
    y: &mut [f32],
    z: &mut [f32],
    vx: &mut [f32],
    vy: &mut [f32],
    vz: &mut [f32],
    fx: &[f32],
    fy: &[f32],
    fz: &[f32],
    mass: &[f32],
    dt: f32,
) {
    let dt_half = dt * 0.5;
    let n = x.len();
    let chunks = n / 4;
    let remainder = n % 4;

    let dt_v = f32x4::splat(dt);
    let dt_half_v = f32x4::splat(dt_half);

    // Process in SIMD chunks
    for chunk in 0..chunks {
        let base = chunk * 4;

        let mass_v = f32x4::from(&mass[base..base + 4]);
        let inv_mass_v = f32x4::ONE / mass_v;

        let vx_v = f32x4::from(&vx[base..base + 4]);
        let vy_v = f32x4::from(&vy[base..base + 4]);
        let vz_v = f32x4::from(&vz[base..base + 4]);

        let fx_v = f32x4::from(&fx[base..base + 4]);
        let fy_v = f32x4::from(&fy[base..base + 4]);
        let fz_v = f32x4::from(&fz[base..base + 4]);

        // Half-step velocity update
        let vx_half = vx_v + dt_half_v * fx_v * inv_mass_v;
        let vy_half = vy_v + dt_half_v * fy_v * inv_mass_v;
        let vz_half = vz_v + dt_half_v * fz_v * inv_mass_v;

        // Position update
        let x_v = f32x4::from(&x[base..base + 4]);
        let y_v = f32x4::from(&y[base..base + 4]);
        let z_v = f32x4::from(&z[base..base + 4]);

        let x_new = x_v + dt_v * vx_half;
        let y_new = y_v + dt_v * vy_half;
        let z_new = z_v + dt_v * vz_half;

        // Store results
        let vx_arr: [f32; 4] = vx_half.into();
        let vy_arr: [f32; 4] = vy_half.into();
        let vz_arr: [f32; 4] = vz_half.into();
        let x_arr: [f32; 4] = x_new.into();
        let y_arr: [f32; 4] = y_new.into();
        let z_arr: [f32; 4] = z_new.into();

        vx[base..base + 4].copy_from_slice(&vx_arr);
        vy[base..base + 4].copy_from_slice(&vy_arr);
        vz[base..base + 4].copy_from_slice(&vz_arr);
        x[base..base + 4].copy_from_slice(&x_arr);
        y[base..base + 4].copy_from_slice(&y_arr);
        z[base..base + 4].copy_from_slice(&z_arr);
    }

    // Scalar remainder
    let base = chunks * 4;
    for i in 0..remainder {
        let idx = base + i;
        let inv_mass = 1.0 / mass[idx];

        vx[idx] += dt_half * fx[idx] * inv_mass;
        vy[idx] += dt_half * fy[idx] * inv_mass;
        vz[idx] += dt_half * fz[idx] * inv_mass;

        x[idx] += dt * vx[idx];
        y[idx] += dt * vy[idx];
        z[idx] += dt * vz[idx];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_velocity_verlet_free_particle() {
        let integrator = VelocityVerlet::new().with_wrap_positions(false);
        let box_ = SimulationBox::non_periodic(100.0, 100.0, 100.0);

        let mut atoms = vec![
            Atom::new(0, 0, 1.0)
                .with_position(0.0, 0.0, 0.0)
                .with_velocity(1.0, 0.0, 0.0),
        ];

        let dt = 0.1;
        let steps = 10;

        for _ in 0..steps {
            integrator.step(&mut atoms, &box_, dt, |_| {});
        }

        // Free particle should move uniformly: x = v*t = 1.0 * 1.0 = 1.0
        assert!((atoms[0].position[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_velocity_verlet_harmonic_oscillator() {
        let integrator = VelocityVerlet::new().with_wrap_positions(false);
        let box_ = SimulationBox::non_periodic(100.0, 100.0, 100.0);

        // Harmonic oscillator: F = -k*x, with k=1, m=1
        // Period = 2*pi, so after one period x should return near initial
        let k = 1.0f32;

        let mut atoms = vec![
            Atom::new(0, 0, 1.0)
                .with_position(1.0, 0.0, 0.0)
                .with_velocity(0.0, 0.0, 0.0),
        ];

        // Initial force
        atoms[0].force[0] = -k * atoms[0].position[0];

        let dt = 0.01;
        let period = 2.0 * std::f32::consts::PI;
        let steps = (period / dt) as usize;

        for _ in 0..steps {
            integrator.step(&mut atoms, &box_, dt, |atoms| {
                atoms[0].force[0] = -k * atoms[0].position[0];
            });
        }

        // After one period, should be back near x=1
        assert!((atoms[0].position[0] - 1.0).abs() < 0.1);
    }
}
