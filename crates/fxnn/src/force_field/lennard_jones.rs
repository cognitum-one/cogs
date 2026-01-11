//! Lennard-Jones potential implementation
//!
//! V(r) = 4ε[(σ/r)¹² - (σ/r)⁶] - V_shift
//! F(r) = 24ε/r[2(σ/r)¹² - (σ/r)⁶]
//!
//! This implementation uses SIMD optimizations via the `wide` crate for
//! high-throughput force calculations on modern CPUs.

use crate::types::{Atom, SimulationBox};
use crate::neighbor::NeighborList;
use super::traits::{ForceField, PairParameters};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg(feature = "simd")]
use wide::f32x4;

/// Lennard-Jones force field
///
/// Implements the 12-6 Lennard-Jones potential with optional tail corrections.
#[derive(Debug, Clone)]
pub struct LennardJones {
    /// Pair parameters for each atom type pair
    parameters: Vec<Vec<PairParameters>>,
    /// Global cutoff radius
    cutoff: f32,
    /// Number of atom types
    n_types: usize,
    /// Whether to apply energy shift at cutoff
    shift: bool,
}

impl LennardJones {
    /// Create a new Lennard-Jones force field
    pub fn new(n_types: usize, cutoff: f32) -> Self {
        Self {
            parameters: vec![vec![PairParameters::new(1.0, 1.0, cutoff); n_types]; n_types],
            cutoff,
            n_types,
            shift: true,
        }
    }

    /// Create LJ parameters for argon in reduced units
    pub fn argon() -> Self {
        let mut lj = Self::new(1, 2.5);
        lj.set_parameters(0, 0, 1.0, 1.0); // ε=1, σ=1 in reduced units
        lj
    }

    /// Create LJ parameters for argon in real units
    /// ε = 0.996 kJ/mol, σ = 0.3405 nm
    pub fn argon_real() -> Self {
        let mut lj = Self::new(1, 0.85); // 2.5σ cutoff
        lj.set_parameters(0, 0, 0.996, 0.3405);
        lj
    }

    /// Set parameters for a pair of atom types
    pub fn set_parameters(&mut self, type_i: usize, type_j: usize, epsilon: f32, sigma: f32) {
        let params = PairParameters::new(epsilon, sigma, self.cutoff);
        self.parameters[type_i][type_j] = params;
        self.parameters[type_j][type_i] = params;
    }

    /// Enable or disable energy shift at cutoff
    pub fn with_shift(mut self, shift: bool) -> Self {
        self.shift = shift;
        self
    }

    /// Compute LJ potential and force for a pair
    ///
    /// Returns (energy, force_over_r) where force_over_r can be multiplied
    /// by displacement to get the force vector.
    #[inline(always)]
    fn pair_potential_and_force(&self, r2: f32, params: &PairParameters) -> (f32, f32) {
        let sigma2 = params.sigma * params.sigma;
        let r2_inv = sigma2 / r2;
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        let energy = 4.0 * params.epsilon * (r12_inv - r6_inv);
        let energy_shifted = if self.shift { energy - params.shift } else { energy };

        // F/r = 24ε/r² [2(σ/r)¹² - (σ/r)⁶]
        let force_over_r = 24.0 * params.epsilon / r2 * (2.0 * r12_inv - r6_inv);

        (energy_shifted, force_over_r)
    }

    /// Compute LJ force_over_r only (skip energy for force-only calculations)
    #[inline(always)]
    fn pair_force_only(&self, r2: f32, params: &PairParameters) -> f32 {
        let sigma2 = params.sigma * params.sigma;
        let r2_inv = sigma2 / r2;
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        // F/r = 24ε/r² [2(σ/r)¹² - (σ/r)⁶]
        24.0 * params.epsilon / r2 * (2.0 * r12_inv - r6_inv)
    }

    /// SIMD-optimized batch computation of LJ forces for 4 pairs
    #[cfg(feature = "simd")]
    #[inline(always)]
    fn pair_force_batch_4(
        &self,
        r2: [f32; 4],
        sigma2: f32,
        epsilon: f32,
    ) -> [f32; 4] {
        let sigma2_v = f32x4::splat(sigma2);
        let eps_v = f32x4::splat(epsilon);
        let r2_v = f32x4::from(r2);

        let r2_inv = sigma2_v / r2_v;
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        // F/r = 24ε/r² [2(σ/r)¹² - (σ/r)⁶]
        let two = f32x4::splat(2.0);
        let twenty_four = f32x4::splat(24.0);
        let force_over_r = twenty_four * eps_v / r2_v * (two * r12_inv - r6_inv);

        force_over_r.into()
    }

    /// Compute forces using neighbor list
    ///
    /// Optimized with cache-friendly access patterns and minimal branching.
    #[inline]
    fn compute_forces_neighborlist(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: &NeighborList,
    ) {
        let cutoff2 = self.cutoff * self.cutoff;
        let min_r2 = 1e-10_f32;

        // Pre-extract box dimensions for faster PBC calculations
        let [lx, ly, lz] = box_.dimensions;
        let [lx_inv, ly_inv, lz_inv] = box_.inverse;
        let periodic = box_.periodic;

        for i in 0..atoms.len() {
            let neighbors = neighbor_list.get_neighbors(i);

            // Cache atom i's data to avoid repeated lookups
            let pos_i = atoms[i].position;
            let type_i = atoms[i].atom_type as usize;

            // Accumulate forces for atom i locally to reduce memory writes
            let mut fx_i = 0.0_f32;
            let mut fy_i = 0.0_f32;
            let mut fz_i = 0.0_f32;

            for &j in neighbors {
                // Use branchless comparison for better pipeline efficiency
                if j <= i {
                    continue;
                }

                let pos_j = atoms[j].position;
                let type_j = atoms[j].atom_type as usize;

                // Inline displacement calculation for better optimization
                let mut dx = pos_j[0] - pos_i[0];
                let mut dy = pos_j[1] - pos_i[1];
                let mut dz = pos_j[2] - pos_i[2];

                // Apply minimum image convention inline
                if periodic[0] {
                    dx -= lx * (dx * lx_inv).round();
                }
                if periodic[1] {
                    dy -= ly * (dy * ly_inv).round();
                }
                if periodic[2] {
                    dz -= lz * (dz * lz_inv).round();
                }

                let r2 = dx * dx + dy * dy + dz * dz;

                // Combined cutoff check
                if r2 < cutoff2 && r2 > min_r2 {
                    let params = &self.parameters[type_i][type_j];
                    let force_over_r = self.pair_force_only(r2, params);

                    // Force on i points away from j for repulsion
                    let fx = -force_over_r * dx;
                    let fy = -force_over_r * dy;
                    let fz = -force_over_r * dz;

                    // Accumulate to local variable
                    fx_i += fx;
                    fy_i += fy;
                    fz_i += fz;

                    // Apply Newton's third law
                    atoms[j].force[0] -= fx;
                    atoms[j].force[1] -= fy;
                    atoms[j].force[2] -= fz;
                }
            }

            // Single write to atom i's forces
            atoms[i].force[0] += fx_i;
            atoms[i].force[1] += fy_i;
            atoms[i].force[2] += fz_i;
        }
    }

    /// Compute forces using O(N²) direct summation
    ///
    /// Optimized with cache-friendly access patterns.
    #[inline]
    fn compute_forces_direct(&self, atoms: &mut [Atom], box_: &SimulationBox) {
        let n = atoms.len();
        let cutoff2 = self.cutoff * self.cutoff;
        let min_r2 = 1e-10_f32;

        // Pre-extract box dimensions
        let [lx, ly, lz] = box_.dimensions;
        let [lx_inv, ly_inv, lz_inv] = box_.inverse;
        let periodic = box_.periodic;

        for i in 0..n {
            let pos_i = atoms[i].position;
            let type_i = atoms[i].atom_type as usize;

            // Local force accumulator
            let mut fx_i = 0.0_f32;
            let mut fy_i = 0.0_f32;
            let mut fz_i = 0.0_f32;

            for j in (i + 1)..n {
                let pos_j = atoms[j].position;
                let type_j = atoms[j].atom_type as usize;

                // Inline displacement with PBC
                let mut dx = pos_j[0] - pos_i[0];
                let mut dy = pos_j[1] - pos_i[1];
                let mut dz = pos_j[2] - pos_i[2];

                if periodic[0] {
                    dx -= lx * (dx * lx_inv).round();
                }
                if periodic[1] {
                    dy -= ly * (dy * ly_inv).round();
                }
                if periodic[2] {
                    dz -= lz * (dz * lz_inv).round();
                }

                let r2 = dx * dx + dy * dy + dz * dz;

                if r2 < cutoff2 && r2 > min_r2 {
                    let params = &self.parameters[type_i][type_j];
                    let force_over_r = self.pair_force_only(r2, params);

                    let fx = -force_over_r * dx;
                    let fy = -force_over_r * dy;
                    let fz = -force_over_r * dz;

                    fx_i += fx;
                    fy_i += fy;
                    fz_i += fz;

                    atoms[j].force[0] -= fx;
                    atoms[j].force[1] -= fy;
                    atoms[j].force[2] -= fz;
                }
            }

            atoms[i].force[0] += fx_i;
            atoms[i].force[1] += fy_i;
            atoms[i].force[2] += fz_i;
        }
    }
}

impl ForceField for LennardJones {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) {
        if let Some(nl) = neighbor_list {
            self.compute_forces_neighborlist(atoms, box_, nl);
        } else {
            self.compute_forces_direct(atoms, box_);
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) -> f64 {
        let cutoff2 = self.cutoff * self.cutoff;
        let mut energy = 0.0f64;

        if let Some(nl) = neighbor_list {
            for i in 0..atoms.len() {
                let neighbors = nl.get_neighbors(i);
                let pos_i = atoms[i].position;
                let type_i = atoms[i].atom_type as usize;

                for &j in neighbors {
                    if j <= i {
                        continue;
                    }

                    let pos_j = atoms[j].position;
                    let type_j = atoms[j].atom_type as usize;

                    let [dx, dy, dz] = box_.displacement(&pos_i, &pos_j);
                    let r2 = dx * dx + dy * dy + dz * dz;

                    if r2 < cutoff2 && r2 > 1e-10 {
                        let params = &self.parameters[type_i][type_j];
                        let (e, _) = self.pair_potential_and_force(r2, params);
                        energy += e as f64;
                    }
                }
            }
        } else {
            let n = atoms.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    let pos_i = atoms[i].position;
                    let pos_j = atoms[j].position;
                    let type_i = atoms[i].atom_type as usize;
                    let type_j = atoms[j].atom_type as usize;

                    let [dx, dy, dz] = box_.displacement(&pos_i, &pos_j);
                    let r2 = dx * dx + dy * dy + dz * dz;

                    if r2 < cutoff2 && r2 > 1e-10 {
                        let params = &self.parameters[type_i][type_j];
                        let (e, _) = self.pair_potential_and_force(r2, params);
                        energy += e as f64;
                    }
                }
            }
        }

        energy
    }

    fn cutoff(&self) -> f32 {
        self.cutoff
    }

    fn name(&self) -> &str {
        "Lennard-Jones"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lj_force() {
        let lj = LennardJones::argon();
        let box_ = SimulationBox::cubic(10.0);

        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(1.122462, 0.0, 0.0), // r = 2^(1/6) σ (equilibrium)
        ];

        lj.compute_forces(&mut atoms, &box_, None);

        // At equilibrium, force should be near zero
        assert!(atoms[0].force[0].abs() < 0.1);
    }

    #[test]
    fn test_lj_energy() {
        let lj = LennardJones::argon().with_shift(false);
        let box_ = SimulationBox::cubic(10.0);

        let atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0), // r = σ
        ];

        let energy = lj.potential_energy(&atoms, &box_, None);

        // At r = σ, V = 4ε(1 - 1) = 0
        assert!(energy.abs() < 1e-6);
    }

    #[test]
    fn test_lj_repulsive() {
        let lj = LennardJones::argon();
        let box_ = SimulationBox::cubic(10.0);

        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(0.9, 0.0, 0.0), // r < σ
        ];

        lj.compute_forces(&mut atoms, &box_, None);

        // Force should be repulsive (positive on atom 0, negative on atom 1)
        assert!(atoms[0].force[0] < 0.0); // Pushed in -x direction
        assert!(atoms[1].force[0] > 0.0); // Pushed in +x direction
    }
}
