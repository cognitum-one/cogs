//! Bonded force field terms (bonds, angles, dihedrals)

use crate::types::{Atom, SimulationBox, Topology, Bond, Angle, Dihedral};
use crate::neighbor::NeighborList;
use super::traits::ForceField;

/// Harmonic bond potential: V = 0.5 * k * (r - r0)²
#[derive(Debug, Clone, Copy)]
pub struct HarmonicBondParams {
    /// Force constant (kJ/(mol·nm²))
    pub k: f32,
    /// Equilibrium bond length (nm)
    pub r0: f32,
}

impl HarmonicBondParams {
    /// Create new harmonic bond parameters
    pub fn new(k: f32, r0: f32) -> Self {
        Self { k, r0 }
    }
}

/// Harmonic angle potential: V = 0.5 * k * (θ - θ0)²
#[derive(Debug, Clone, Copy)]
pub struct HarmonicAngleParams {
    /// Force constant (kJ/(mol·rad²))
    pub k: f32,
    /// Equilibrium angle (radians)
    pub theta0: f32,
}

impl HarmonicAngleParams {
    /// Create new harmonic angle parameters
    pub fn new(k: f32, theta0: f32) -> Self {
        Self { k, theta0 }
    }

    /// Create from degrees
    pub fn from_degrees(k: f32, theta0_deg: f32) -> Self {
        Self {
            k,
            theta0: theta0_deg.to_radians(),
        }
    }
}

/// Periodic dihedral potential: V = k * (1 + cos(n*φ - δ))
#[derive(Debug, Clone, Copy)]
pub struct PeriodicDihedralParams {
    /// Force constant (kJ/mol)
    pub k: f32,
    /// Periodicity
    pub n: i32,
    /// Phase shift (radians)
    pub delta: f32,
}

impl PeriodicDihedralParams {
    /// Create new periodic dihedral parameters
    pub fn new(k: f32, n: i32, delta: f32) -> Self {
        Self { k, n, delta }
    }
}

/// Harmonic bond force field component
#[derive(Debug, Clone)]
pub struct HarmonicBond {
    /// Parameters for each bond type
    pub params: Vec<HarmonicBondParams>,
}

impl HarmonicBond {
    /// Create with default parameters
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Add parameters for a bond type
    pub fn add_type(&mut self, k: f32, r0: f32) -> usize {
        self.params.push(HarmonicBondParams::new(k, r0));
        self.params.len() - 1
    }

    /// Compute bond energy and forces
    pub fn compute(&self, atoms: &mut [Atom], box_: &SimulationBox, bonds: &[Bond]) -> f64 {
        let mut energy = 0.0f64;

        for bond in bonds {
            let params = &self.params[bond.bond_type as usize];
            let pos_i = atoms[bond.atom_i].position;
            let pos_j = atoms[bond.atom_j].position;

            let [dx, dy, dz] = box_.displacement(&pos_i, &pos_j);
            let r = (dx * dx + dy * dy + dz * dz).sqrt();

            if r < 1e-10 {
                continue;
            }

            let dr = r - params.r0;
            energy += 0.5 * params.k as f64 * dr as f64 * dr as f64;

            // F_i = k * (r - r0) / r * (r_ij) pulls atom_i toward atom_j when stretched
            // displacement [dx,dy,dz] = r_ij points from i to j
            let force_mag = params.k * dr / r;
            let fx = force_mag * dx;
            let fy = force_mag * dy;
            let fz = force_mag * dz;

            atoms[bond.atom_i].add_force(fx, fy, fz);
            atoms[bond.atom_j].add_force(-fx, -fy, -fz);
        }

        energy
    }
}

impl Default for HarmonicBond {
    fn default() -> Self {
        Self::new()
    }
}

/// Harmonic angle force field component
#[derive(Debug, Clone)]
pub struct HarmonicAngle {
    /// Parameters for each angle type
    pub params: Vec<HarmonicAngleParams>,
}

impl HarmonicAngle {
    /// Create with default parameters
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Add parameters for an angle type
    pub fn add_type(&mut self, k: f32, theta0: f32) -> usize {
        self.params.push(HarmonicAngleParams::new(k, theta0));
        self.params.len() - 1
    }

    /// Compute angle energy and forces
    pub fn compute(&self, atoms: &mut [Atom], box_: &SimulationBox, angles: &[Angle]) -> f64 {
        let mut energy = 0.0f64;

        for angle in angles {
            let params = &self.params[angle.angle_type as usize];

            let pos_i = atoms[angle.atom_i].position;
            let pos_j = atoms[angle.atom_j].position;
            let pos_k = atoms[angle.atom_k].position;

            // Vectors from central atom j to i and k
            let [rji_x, rji_y, rji_z] = box_.displacement(&pos_j, &pos_i);
            let [rjk_x, rjk_y, rjk_z] = box_.displacement(&pos_j, &pos_k);

            let rji2 = rji_x * rji_x + rji_y * rji_y + rji_z * rji_z;
            let rjk2 = rjk_x * rjk_x + rjk_y * rjk_y + rjk_z * rjk_z;
            let rji = rji2.sqrt();
            let rjk = rjk2.sqrt();

            if rji < 1e-10 || rjk < 1e-10 {
                continue;
            }

            // cos(θ) = (rji · rjk) / (|rji| * |rjk|)
            let dot = rji_x * rjk_x + rji_y * rjk_y + rji_z * rjk_z;
            let cos_theta = (dot / (rji * rjk)).clamp(-1.0, 1.0);
            let theta = cos_theta.acos();

            let dtheta = theta - params.theta0;
            energy += 0.5 * params.k as f64 * dtheta as f64 * dtheta as f64;

            // Force calculation
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt().max(1e-10);
            let prefactor = -params.k * dtheta / sin_theta;

            // Forces on atoms i and k
            let rji_inv = 1.0 / rji;
            let rjk_inv = 1.0 / rjk;

            let fi_x = prefactor * rji_inv * (rjk_x * rjk_inv - cos_theta * rji_x * rji_inv);
            let fi_y = prefactor * rji_inv * (rjk_y * rjk_inv - cos_theta * rji_y * rji_inv);
            let fi_z = prefactor * rji_inv * (rjk_z * rjk_inv - cos_theta * rji_z * rji_inv);

            let fk_x = prefactor * rjk_inv * (rji_x * rji_inv - cos_theta * rjk_x * rjk_inv);
            let fk_y = prefactor * rjk_inv * (rji_y * rji_inv - cos_theta * rjk_y * rjk_inv);
            let fk_z = prefactor * rjk_inv * (rji_z * rji_inv - cos_theta * rjk_z * rjk_inv);

            atoms[angle.atom_i].add_force(fi_x, fi_y, fi_z);
            atoms[angle.atom_k].add_force(fk_x, fk_y, fk_z);
            atoms[angle.atom_j].add_force(-fi_x - fk_x, -fi_y - fk_y, -fi_z - fk_z);
        }

        energy
    }
}

impl Default for HarmonicAngle {
    fn default() -> Self {
        Self::new()
    }
}

/// Periodic dihedral force field component
#[derive(Debug, Clone)]
pub struct PeriodicDihedral {
    /// Parameters for each dihedral type
    pub params: Vec<PeriodicDihedralParams>,
}

impl PeriodicDihedral {
    /// Create with default parameters
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Add parameters for a dihedral type
    pub fn add_type(&mut self, k: f32, n: i32, delta: f32) -> usize {
        self.params.push(PeriodicDihedralParams::new(k, n, delta));
        self.params.len() - 1
    }

    /// Compute dihedral energy and forces
    pub fn compute(&self, atoms: &mut [Atom], box_: &SimulationBox, dihedrals: &[Dihedral]) -> f64 {
        let mut energy = 0.0f64;

        for dihedral in dihedrals {
            let params = &self.params[dihedral.dihedral_type as usize];

            let pos_i = atoms[dihedral.atom_i].position;
            let pos_j = atoms[dihedral.atom_j].position;
            let pos_k = atoms[dihedral.atom_k].position;
            let pos_l = atoms[dihedral.atom_l].position;

            // Bond vectors
            let [b1_x, b1_y, b1_z] = box_.displacement(&pos_i, &pos_j);
            let [b2_x, b2_y, b2_z] = box_.displacement(&pos_j, &pos_k);
            let [b3_x, b3_y, b3_z] = box_.displacement(&pos_k, &pos_l);

            // Cross products for normal vectors
            let n1_x = b1_y * b2_z - b1_z * b2_y;
            let n1_y = b1_z * b2_x - b1_x * b2_z;
            let n1_z = b1_x * b2_y - b1_y * b2_x;

            let n2_x = b2_y * b3_z - b2_z * b3_y;
            let n2_y = b2_z * b3_x - b2_x * b3_z;
            let n2_z = b2_x * b3_y - b2_y * b3_x;

            let n1_len = (n1_x * n1_x + n1_y * n1_y + n1_z * n1_z).sqrt();
            let n2_len = (n2_x * n2_x + n2_y * n2_y + n2_z * n2_z).sqrt();

            if n1_len < 1e-10 || n2_len < 1e-10 {
                continue;
            }

            // Dihedral angle
            let cos_phi = (n1_x * n2_x + n1_y * n2_y + n1_z * n2_z) / (n1_len * n2_len);
            let cos_phi = cos_phi.clamp(-1.0, 1.0);

            // Sign of dihedral
            let b2_len = (b2_x * b2_x + b2_y * b2_y + b2_z * b2_z).sqrt();
            let m_x = n1_y * b2_z / b2_len - n1_z * b2_y / b2_len;
            let m_y = n1_z * b2_x / b2_len - n1_x * b2_z / b2_len;
            let m_z = n1_x * b2_y / b2_len - n1_y * b2_x / b2_len;
            let sin_phi = (m_x * n2_x + m_y * n2_y + m_z * n2_z) / n2_len;

            let phi = sin_phi.atan2(cos_phi);

            // Energy: V = k * (1 + cos(n*φ - δ))
            let n_f = params.n as f32;
            energy += params.k as f64 * (1.0 + (n_f * phi - params.delta).cos()) as f64;

            // Force: F = k * n * sin(n*φ - δ) * dφ/dr
            // This is a simplified force calculation
            let dphi = params.k * n_f * (n_f * phi - params.delta).sin();

            // Distribute forces (simplified - proper implementation needs full derivation)
            let scale = dphi / (n1_len * n2_len).max(1e-10);

            atoms[dihedral.atom_i].add_force(scale * n2_x, scale * n2_y, scale * n2_z);
            atoms[dihedral.atom_l].add_force(-scale * n1_x, -scale * n1_y, -scale * n1_z);
        }

        energy
    }
}

impl Default for PeriodicDihedral {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined bonded forces
#[derive(Debug, Clone, Default)]
pub struct BondedForces {
    /// Harmonic bond parameters
    pub bonds: HarmonicBond,
    /// Harmonic angle parameters
    pub angles: HarmonicAngle,
    /// Periodic dihedral parameters
    pub dihedrals: PeriodicDihedral,
    /// Topology with connectivity information
    pub topology: Topology,
}

impl BondedForces {
    /// Create new bonded forces
    pub fn new(topology: Topology) -> Self {
        Self {
            bonds: HarmonicBond::new(),
            angles: HarmonicAngle::new(),
            dihedrals: PeriodicDihedral::new(),
            topology,
        }
    }
}

impl ForceField for BondedForces {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        _neighbor_list: Option<&NeighborList>,
    ) {
        self.bonds.compute(atoms, box_, &self.topology.bonds);
        self.angles.compute(atoms, box_, &self.topology.angles);
        self.dihedrals.compute(atoms, box_, &self.topology.dihedrals);
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        _neighbor_list: Option<&NeighborList>,
    ) -> f64 {
        // Create temporary copy for force calculation (which also computes energy)
        let mut atoms_copy = atoms.to_vec();
        let e_bond = self.bonds.compute(&mut atoms_copy, box_, &self.topology.bonds);
        let e_angle = self.angles.compute(&mut atoms_copy, box_, &self.topology.angles);
        let e_dihedral = self.dihedrals.compute(&mut atoms_copy, box_, &self.topology.dihedrals);
        e_bond + e_angle + e_dihedral
    }

    fn cutoff(&self) -> f32 {
        0.0 // Bonded interactions have no cutoff
    }

    fn name(&self) -> &str {
        "Bonded"
    }

    fn requires_neighbor_list(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harmonic_bond() {
        let mut bonds = HarmonicBond::new();
        bonds.add_type(1000.0, 0.15); // k = 1000 kJ/(mol·nm²), r0 = 0.15 nm

        let box_ = SimulationBox::cubic(10.0);
        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(0.16, 0.0, 0.0), // r = 0.16 > r0
        ];

        let topology_bonds = vec![Bond::new(0, 1, 0)];
        let energy = bonds.compute(&mut atoms, &box_, &topology_bonds);

        // E = 0.5 * 1000 * (0.16 - 0.15)² = 0.5 * 1000 * 0.0001 = 0.05
        assert!((energy - 0.05).abs() < 1e-6);

        // Bond is stretched, so atoms should be pulled together
        assert!(atoms[0].force[0] > 0.0);
        assert!(atoms[1].force[0] < 0.0);
    }

    #[test]
    fn test_harmonic_angle() {
        let mut angles = HarmonicAngle::new();
        angles.add_type(100.0, std::f32::consts::PI); // k = 100, θ0 = 180°

        let box_ = SimulationBox::cubic(10.0);
        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(0.1, 0.0, 0.0),
            Atom::new(2, 0, 1.0).with_position(0.2, 0.0, 0.0), // Linear arrangement
        ];

        let topology_angles = vec![Angle::new(0, 1, 2, 0)];
        let energy = angles.compute(&mut atoms, &box_, &topology_angles);

        // Atoms are linear (180°), so energy should be near zero
        assert!(energy.abs() < 1e-4);
    }
}
