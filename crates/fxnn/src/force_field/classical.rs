//! Classical force field implementations (Lennard-Jones, Coulomb).
//!
//! These are traditional pair potentials commonly used in molecular dynamics.

use crate::neighbor::NeighborList;
use crate::types::{Atom, SimulationBox};
use super::ForceField;
use serde::{Deserialize, Serialize};

/// Lennard-Jones 12-6 potential for non-bonded interactions.
///
/// The potential is:
/// V(r) = 4ε[(σ/r)^12 - (σ/r)^6]
///
/// where:
/// - ε (epsilon) is the well depth
/// - σ (sigma) is the distance at which the potential is zero
///
/// The force is:
/// F(r) = -dV/dr = 24ε/r[(σ/r)^6 - 2(σ/r)^12]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LennardJones {
    /// Epsilon (well depth) parameters indexed by (type_i, type_j)
    epsilon: Vec<Vec<f32>>,

    /// Sigma (zero-potential distance) parameters indexed by (type_i, type_j)
    sigma: Vec<Vec<f32>>,

    /// Cutoff distance for interactions
    cutoff: f32,

    /// Number of atom types
    num_types: usize,

    /// Whether to apply tail corrections for energy/pressure
    tail_correction: bool,
}

impl LennardJones {
    /// Create a new Lennard-Jones force field.
    ///
    /// # Arguments
    /// * `epsilon` - 2D array of epsilon parameters
    /// * `sigma` - 2D array of sigma parameters
    /// * `cutoff` - Cutoff distance for interactions
    pub fn new(epsilon: Vec<Vec<f32>>, sigma: Vec<Vec<f32>>, cutoff: f32) -> Self {
        assert!(!epsilon.is_empty(), "Epsilon array must not be empty");
        assert_eq!(epsilon.len(), sigma.len(), "Epsilon and sigma must have same dimensions");
        let num_types = epsilon.len();

        Self {
            epsilon,
            sigma,
            cutoff,
            num_types,
            tail_correction: false,
        }
    }

    /// Create Lennard-Jones parameters for Argon.
    ///
    /// Uses reduced units where σ = 1, ε = 1.
    pub fn argon() -> Self {
        Self::new(
            vec![vec![1.0]],
            vec![vec![1.0]],
            2.5, // Standard cutoff in reduced units
        )
    }

    /// Create Lennard-Jones parameters for Argon with real units.
    ///
    /// σ = 3.4 Å, ε = 0.0104 eV
    pub fn argon_real() -> Self {
        Self::new(
            vec![vec![0.0104]],
            vec![vec![3.4]],
            10.0, // 10 Angstroms
        )
    }

    /// Enable or disable tail corrections.
    pub fn with_tail_correction(mut self, enabled: bool) -> Self {
        self.tail_correction = enabled;
        self
    }

    /// Set the cutoff distance.
    pub fn with_cutoff(mut self, cutoff: f32) -> Self {
        self.cutoff = cutoff;
        self
    }

    /// Compute the LJ pair energy and force magnitude.
    ///
    /// # Returns
    /// (energy, force/r) where force/r can be multiplied by dr to get force vector
    #[inline]
    fn pair_interaction(&self, r2: f32, epsilon: f32, sigma: f32) -> (f32, f32) {
        let sigma2 = sigma * sigma;
        let sigma6 = sigma2 * sigma2 * sigma2;
        let r6 = r2 * r2 * r2;
        let sigma6_over_r6 = sigma6 / r6;
        let sigma12_over_r12 = sigma6_over_r6 * sigma6_over_r6;

        // Energy: 4ε[(σ/r)^12 - (σ/r)^6]
        let energy = 4.0 * epsilon * (sigma12_over_r12 - sigma6_over_r6);

        // Force/r: 24ε/r^2 * [2(σ/r)^12 - (σ/r)^6]
        let force_over_r = 24.0 * epsilon / r2 * (2.0 * sigma12_over_r12 - sigma6_over_r6);

        (energy, force_over_r)
    }

    /// Get epsilon for a pair of atom types.
    #[inline]
    fn get_epsilon(&self, type_i: u32, type_j: u32) -> f32 {
        let i = (type_i as usize).min(self.num_types - 1);
        let j = (type_j as usize).min(self.num_types - 1);
        self.epsilon[i][j]
    }

    /// Get sigma for a pair of atom types.
    #[inline]
    fn get_sigma(&self, type_i: u32, type_j: u32) -> f32 {
        let i = (type_i as usize).min(self.num_types - 1);
        let j = (type_j as usize).min(self.num_types - 1);
        self.sigma[i][j]
    }
}

impl ForceField for LennardJones {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) {
        let cutoff2 = self.cutoff * self.cutoff;

        for i in 0..atoms.len() {
            let neighbors = neighbor_list.get_neighbors(i);

            for &j in neighbors {
                if j <= i {
                    continue; // Avoid double counting
                }

                let dr = [
                    atoms[i].position[0] - atoms[j].position[0],
                    atoms[i].position[1] - atoms[j].position[1],
                    atoms[i].position[2] - atoms[j].position[2],
                ];
                let dr = simulation_box.minimum_image(dr);
                let r2 = dr[0].powi(2) + dr[1].powi(2) + dr[2].powi(2);

                if r2 < cutoff2 && r2 > 1e-10 {
                    let epsilon = self.get_epsilon(atoms[i].atom_type, atoms[j].atom_type);
                    let sigma = self.get_sigma(atoms[i].atom_type, atoms[j].atom_type);
                    let (_, force_over_r) = self.pair_interaction(r2, epsilon, sigma);

                    // Apply force to both atoms (Newton's third law)
                    for k in 0..3 {
                        let f = force_over_r * dr[k];
                        atoms[i].force[k] += f;
                        atoms[j].force[k] -= f;
                    }
                }
            }
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) -> f32 {
        let cutoff2 = self.cutoff * self.cutoff;
        let mut energy = 0.0;

        for i in 0..atoms.len() {
            let neighbors = neighbor_list.get_neighbors(i);

            for &j in neighbors {
                if j <= i {
                    continue;
                }

                let dr = [
                    atoms[i].position[0] - atoms[j].position[0],
                    atoms[i].position[1] - atoms[j].position[1],
                    atoms[i].position[2] - atoms[j].position[2],
                ];
                let dr = simulation_box.minimum_image(dr);
                let r2 = dr[0].powi(2) + dr[1].powi(2) + dr[2].powi(2);

                if r2 < cutoff2 && r2 > 1e-10 {
                    let epsilon = self.get_epsilon(atoms[i].atom_type, atoms[j].atom_type);
                    let sigma = self.get_sigma(atoms[i].atom_type, atoms[j].atom_type);
                    let (pair_energy, _) = self.pair_interaction(r2, epsilon, sigma);
                    energy += pair_energy;
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

/// Coulomb electrostatic potential.
///
/// V(r) = k * q_i * q_j / r
///
/// where k is Coulomb's constant (or 1 in reduced units).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coulomb {
    /// Coulomb constant (1.0 for reduced units, 332.0637 for kcal/mol with Angstroms)
    coulomb_constant: f32,

    /// Cutoff distance for interactions
    cutoff: f32,

    /// Dielectric constant (1.0 for vacuum)
    dielectric: f32,
}

impl Coulomb {
    /// Create a new Coulomb force field.
    ///
    /// # Arguments
    /// * `coulomb_constant` - The electrostatic constant
    /// * `cutoff` - Cutoff distance for interactions
    pub fn new(coulomb_constant: f32, cutoff: f32) -> Self {
        Self {
            coulomb_constant,
            cutoff,
            dielectric: 1.0,
        }
    }

    /// Create Coulomb with reduced units (k = 1).
    pub fn reduced_units(cutoff: f32) -> Self {
        Self::new(1.0, cutoff)
    }

    /// Create Coulomb with real units (kcal/mol with charges in elementary charge units).
    pub fn real_units(cutoff: f32) -> Self {
        Self::new(332.0637, cutoff)
    }

    /// Set the dielectric constant.
    pub fn with_dielectric(mut self, dielectric: f32) -> Self {
        self.dielectric = dielectric;
        self
    }
}

impl ForceField for Coulomb {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) {
        let cutoff2 = self.cutoff * self.cutoff;
        let k = self.coulomb_constant / self.dielectric;

        for i in 0..atoms.len() {
            if atoms[i].charge.abs() < 1e-10 {
                continue;
            }

            let neighbors = neighbor_list.get_neighbors(i);

            for &j in neighbors {
                if j <= i {
                    continue;
                }

                if atoms[j].charge.abs() < 1e-10 {
                    continue;
                }

                let dr = [
                    atoms[i].position[0] - atoms[j].position[0],
                    atoms[i].position[1] - atoms[j].position[1],
                    atoms[i].position[2] - atoms[j].position[2],
                ];
                let dr = simulation_box.minimum_image(dr);
                let r2 = dr[0].powi(2) + dr[1].powi(2) + dr[2].powi(2);

                if r2 < cutoff2 && r2 > 1e-10 {
                    let r = r2.sqrt();
                    let q_prod = atoms[i].charge * atoms[j].charge;

                    // Force: F = k * q_i * q_j / r^2, directed along r
                    let force_over_r = k * q_prod / (r2 * r);

                    for dim in 0..3 {
                        let f = force_over_r * dr[dim];
                        atoms[i].force[dim] += f;
                        atoms[j].force[dim] -= f;
                    }
                }
            }
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) -> f32 {
        let cutoff2 = self.cutoff * self.cutoff;
        let k = self.coulomb_constant / self.dielectric;
        let mut energy = 0.0;

        for i in 0..atoms.len() {
            if atoms[i].charge.abs() < 1e-10 {
                continue;
            }

            let neighbors = neighbor_list.get_neighbors(i);

            for &j in neighbors {
                if j <= i {
                    continue;
                }

                if atoms[j].charge.abs() < 1e-10 {
                    continue;
                }

                let dr = [
                    atoms[i].position[0] - atoms[j].position[0],
                    atoms[i].position[1] - atoms[j].position[1],
                    atoms[i].position[2] - atoms[j].position[2],
                ];
                let dr = simulation_box.minimum_image(dr);
                let r2 = dr[0].powi(2) + dr[1].powi(2) + dr[2].powi(2);

                if r2 < cutoff2 && r2 > 1e-10 {
                    let r = r2.sqrt();
                    let q_prod = atoms[i].charge * atoms[j].charge;
                    energy += k * q_prod / r;
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

/// Composite force field that combines multiple force fields.
///
/// Useful for combining LJ and Coulomb, or adding neural network corrections.
pub struct CompositeForceField {
    /// List of component force fields
    components: Vec<Box<dyn ForceField>>,

    /// Name of this composite
    name: String,
}

impl CompositeForceField {
    /// Create a new composite force field.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            components: Vec::new(),
            name: name.into(),
        }
    }

    /// Add a force field component.
    pub fn add(mut self, ff: impl ForceField + 'static) -> Self {
        self.components.push(Box::new(ff));
        self
    }

    /// Add a boxed force field component.
    pub fn add_boxed(mut self, ff: Box<dyn ForceField>) -> Self {
        self.components.push(ff);
        self
    }
}

impl ForceField for CompositeForceField {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) {
        for component in &self.components {
            component.compute_forces(atoms, simulation_box, neighbor_list);
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) -> f32 {
        self.components
            .iter()
            .map(|ff| ff.potential_energy(atoms, simulation_box, neighbor_list))
            .sum()
    }

    fn cutoff(&self) -> f32 {
        self.components
            .iter()
            .map(|ff| ff.cutoff())
            .fold(0.0, f32::max)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple neighbor list for testing
    struct AllPairsNeighborList {
        n: usize,
    }

    impl NeighborList for AllPairsNeighborList {
        fn neighbors(&self, i: usize) -> &[usize] {
            // This is inefficient but works for small tests
            // In real tests, would use proper neighbor list
            &[]
        }

        fn update(&mut self, _atoms: &[Atom], _box_: &SimulationBox) {}
        fn cutoff(&self) -> f32 { 10.0 }
    }

    #[test]
    fn test_lj_argon_creation() {
        let lj = LennardJones::argon();
        assert_eq!(lj.cutoff(), 2.5);
        assert_eq!(lj.name(), "Lennard-Jones");
    }

    #[test]
    fn test_lj_pair_interaction() {
        let lj = LennardJones::argon();

        // At r = sigma, potential should be zero
        let sigma2 = 1.0;
        let (energy, _) = lj.pair_interaction(sigma2, 1.0, 1.0);
        assert!(energy.abs() < 1e-6, "Energy at r=sigma should be ~0, got {}", energy);

        // At r = 2^(1/6) * sigma (minimum), potential should be -epsilon
        let r_min_squared = 2.0_f32.powf(1.0/3.0); // (2^(1/6))^2 = 2^(1/3)
        let (energy, _) = lj.pair_interaction(r_min_squared, 1.0, 1.0);
        assert!((energy - (-1.0)).abs() < 1e-5, "Energy at minimum should be -epsilon, got {}", energy);
    }

    #[test]
    fn test_coulomb_creation() {
        let coulomb = Coulomb::reduced_units(5.0);
        assert_eq!(coulomb.cutoff(), 5.0);
        assert_eq!(coulomb.name(), "Coulomb");
    }

    #[test]
    fn test_composite_force_field() {
        let composite = CompositeForceField::new("LJ+Coulomb")
            .add(LennardJones::argon())
            .add(Coulomb::reduced_units(2.5));

        assert_eq!(composite.name(), "LJ+Coulomb");
        assert_eq!(composite.cutoff(), 2.5);
    }
}
