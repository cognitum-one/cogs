//! Coulomb electrostatic interactions
//!
//! V(r) = q_i * q_j / (4piε₀ε_r * r)
//! F(r) = q_i * q_j / (4piε₀ε_r * r²) * r_hat
//!
//! This implementation uses optimized cache access patterns and inline
//! calculations for high performance force computations.

use crate::types::{Atom, SimulationBox};
use crate::neighbor::NeighborList;
use super::traits::ForceField;

/// Method for handling long-range electrostatics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoulombMethod {
    /// Direct cutoff (not recommended for charged systems)
    Cutoff,
    /// Reaction field method
    ReactionField {
        /// Dielectric constant of surrounding medium
        epsilon_rf: f32,
    },
    /// Shifted force method
    ShiftedForce,
}

/// Coulomb electrostatic force field
#[derive(Debug, Clone)]
pub struct Coulomb {
    /// Coulomb constant (138.935458 kJ·nm/(mol·e²) in GROMACS units)
    pub coulomb_constant: f32,
    /// Cutoff radius
    cutoff: f32,
    /// Method for long-range interactions
    method: CoulombMethod,
    /// Relative dielectric constant
    epsilon_r: f32,
}

impl Coulomb {
    /// Create a new Coulomb force field
    ///
    /// Uses GROMACS units by default: kJ/mol, nm, e, ps
    pub fn new(cutoff: f32) -> Self {
        Self {
            // 1/(4πε₀) in kJ·nm/(mol·e²)
            coulomb_constant: 138.935458,
            cutoff,
            method: CoulombMethod::ReactionField { epsilon_rf: 78.5 },
            epsilon_r: 1.0,
        }
    }

    /// Create Coulomb in reduced units
    pub fn reduced_units(cutoff: f32) -> Self {
        Self {
            coulomb_constant: 1.0,
            cutoff,
            method: CoulombMethod::Cutoff,
            epsilon_r: 1.0,
        }
    }

    /// Set the method for long-range interactions
    pub fn with_method(mut self, method: CoulombMethod) -> Self {
        self.method = method;
        self
    }

    /// Set the relative dielectric constant
    pub fn with_epsilon_r(mut self, epsilon_r: f32) -> Self {
        self.epsilon_r = epsilon_r;
        self
    }

    /// Compute Coulomb interaction energy and force
    #[inline(always)]
    fn pair_interaction(&self, r: f32, q_i: f32, q_j: f32) -> (f32, f32) {
        let qq = q_i * q_j * self.coulomb_constant / self.epsilon_r;

        match self.method {
            CoulombMethod::Cutoff => {
                let r_inv = 1.0 / r;
                let r2_inv = r_inv * r_inv;
                let energy = qq * r_inv;
                let force_over_r = qq * r2_inv * r_inv;
                (energy, force_over_r)
            }
            CoulombMethod::ReactionField { epsilon_rf } => {
                let rc = self.cutoff;
                let rc3 = rc * rc * rc;
                let k_rf = (epsilon_rf - 1.0) / (2.0 * epsilon_rf + 1.0) / rc3;
                let c_rf = 1.0 / rc + k_rf * rc * rc;

                let r_inv = 1.0 / r;
                let r2 = r * r;
                let energy = qq * (r_inv + k_rf * r2 - c_rf);
                let force_over_r = qq * (r_inv * r_inv * r_inv - 2.0 * k_rf);
                (energy, force_over_r)
            }
            CoulombMethod::ShiftedForce => {
                let rc = self.cutoff;
                let rc_inv = 1.0 / rc;
                let rc2_inv = rc_inv * rc_inv;
                let r_inv = 1.0 / r;
                let energy = qq * (r_inv - rc_inv - (r - rc) * rc2_inv);
                let force_over_r = qq * (r_inv * r_inv * r_inv - rc2_inv * r_inv);
                (energy, force_over_r)
            }
        }
    }

    /// Compute Coulomb force only (skip energy calculation)
    #[inline(always)]
    fn pair_force_only(&self, r: f32, q_i: f32, q_j: f32) -> f32 {
        let qq = q_i * q_j * self.coulomb_constant / self.epsilon_r;

        match self.method {
            CoulombMethod::Cutoff => {
                let r_inv = 1.0 / r;
                qq * r_inv * r_inv * r_inv
            }
            CoulombMethod::ReactionField { epsilon_rf } => {
                let rc = self.cutoff;
                let rc3 = rc * rc * rc;
                let k_rf = (epsilon_rf - 1.0) / (2.0 * epsilon_rf + 1.0) / rc3;
                let r_inv = 1.0 / r;
                qq * (r_inv * r_inv * r_inv - 2.0 * k_rf)
            }
            CoulombMethod::ShiftedForce => {
                let rc = self.cutoff;
                let rc2_inv = 1.0 / (rc * rc);
                let r_inv = 1.0 / r;
                qq * (r_inv * r_inv * r_inv - rc2_inv * r_inv)
            }
        }
    }
}

impl ForceField for Coulomb {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) {
        let cutoff2 = self.cutoff * self.cutoff;
        let min_r2 = 1e-10_f32;
        let min_charge = 1e-10_f32;

        // Pre-extract box dimensions for faster PBC calculations
        let [lx, ly, lz] = box_.dimensions;
        let [lx_inv, ly_inv, lz_inv] = box_.inverse;
        let periodic = box_.periodic;

        if let Some(nl) = neighbor_list {
            for i in 0..atoms.len() {
                let neighbors = nl.get_neighbors(i);
                let pos_i = atoms[i].position;
                let q_i = atoms[i].charge;

                // Skip uncharged atoms
                if q_i.abs() < min_charge {
                    continue;
                }

                // Local force accumulator
                let mut fx_i = 0.0_f32;
                let mut fy_i = 0.0_f32;
                let mut fz_i = 0.0_f32;

                for &j in neighbors {
                    if j <= i {
                        continue;
                    }

                    let q_j = atoms[j].charge;
                    if q_j.abs() < min_charge {
                        continue;
                    }

                    let pos_j = atoms[j].position;

                    // Inline displacement calculation
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
                        let r = r2.sqrt();
                        let force_over_r = self.pair_force_only(r, q_i, q_j);

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
        } else {
            let n = atoms.len();
            for i in 0..n {
                let q_i = atoms[i].charge;
                if q_i.abs() < min_charge {
                    continue;
                }

                let pos_i = atoms[i].position;
                let mut fx_i = 0.0_f32;
                let mut fy_i = 0.0_f32;
                let mut fz_i = 0.0_f32;

                for j in (i + 1)..n {
                    let q_j = atoms[j].charge;
                    if q_j.abs() < min_charge {
                        continue;
                    }

                    let pos_j = atoms[j].position;

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
                        let r = r2.sqrt();
                        let force_over_r = self.pair_force_only(r, q_i, q_j);

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
                let q_i = atoms[i].charge;

                if q_i.abs() < 1e-10 {
                    continue;
                }

                for &j in neighbors {
                    if j <= i {
                        continue;
                    }

                    let q_j = atoms[j].charge;
                    if q_j.abs() < 1e-10 {
                        continue;
                    }

                    let pos_j = atoms[j].position;
                    let [dx, dy, dz] = box_.displacement(&pos_i, &pos_j);
                    let r2 = dx * dx + dy * dy + dz * dz;

                    if r2 < cutoff2 && r2 > 1e-10 {
                        let r = r2.sqrt();
                        let (e, _) = self.pair_interaction(r, q_i, q_j);
                        energy += e as f64;
                    }
                }
            }
        } else {
            let n = atoms.len();
            for i in 0..n {
                let q_i = atoms[i].charge;
                if q_i.abs() < 1e-10 {
                    continue;
                }

                for j in (i + 1)..n {
                    let q_j = atoms[j].charge;
                    if q_j.abs() < 1e-10 {
                        continue;
                    }

                    let pos_i = atoms[i].position;
                    let pos_j = atoms[j].position;
                    let [dx, dy, dz] = box_.displacement(&pos_i, &pos_j);
                    let r2 = dx * dx + dy * dy + dz * dz;

                    if r2 < cutoff2 && r2 > 1e-10 {
                        let r = r2.sqrt();
                        let (e, _) = self.pair_interaction(r, q_i, q_j);
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
        "Coulomb"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coulomb_opposite_charges() {
        let coulomb = Coulomb::reduced_units(5.0).with_method(CoulombMethod::Cutoff);
        let box_ = SimulationBox::cubic(10.0);

        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0).with_charge(1.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0).with_charge(-1.0),
        ];

        coulomb.compute_forces(&mut atoms, &box_, None);

        // Opposite charges attract
        assert!(atoms[0].force[0] > 0.0); // Pulled toward +x
        assert!(atoms[1].force[0] < 0.0); // Pulled toward -x
    }

    #[test]
    fn test_coulomb_same_charges() {
        let coulomb = Coulomb::reduced_units(5.0).with_method(CoulombMethod::Cutoff);
        let box_ = SimulationBox::cubic(10.0);

        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0).with_charge(1.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0).with_charge(1.0),
        ];

        coulomb.compute_forces(&mut atoms, &box_, None);

        // Same charges repel
        assert!(atoms[0].force[0] < 0.0); // Pushed toward -x
        assert!(atoms[1].force[0] > 0.0); // Pushed toward +x
    }
}
