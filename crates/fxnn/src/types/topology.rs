//! Molecular topology for bonded interactions

use serde::{Deserialize, Serialize};

/// Bond between two atoms
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bond {
    /// First atom index
    pub atom_i: usize,
    /// Second atom index
    pub atom_j: usize,
    /// Bond type (for force field parameters)
    pub bond_type: u16,
}

impl Bond {
    /// Create a new bond
    pub fn new(atom_i: usize, atom_j: usize, bond_type: u16) -> Self {
        Self {
            atom_i,
            atom_j,
            bond_type,
        }
    }
}

/// Angle between three atoms (i-j-k, j is central)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Angle {
    /// First atom index
    pub atom_i: usize,
    /// Central atom index
    pub atom_j: usize,
    /// Third atom index
    pub atom_k: usize,
    /// Angle type (for force field parameters)
    pub angle_type: u16,
}

impl Angle {
    /// Create a new angle
    pub fn new(atom_i: usize, atom_j: usize, atom_k: usize, angle_type: u16) -> Self {
        Self {
            atom_i,
            atom_j,
            atom_k,
            angle_type,
        }
    }
}

/// Dihedral angle between four atoms (i-j-k-l)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Dihedral {
    /// First atom index
    pub atom_i: usize,
    /// Second atom index
    pub atom_j: usize,
    /// Third atom index
    pub atom_k: usize,
    /// Fourth atom index
    pub atom_l: usize,
    /// Dihedral type (for force field parameters)
    pub dihedral_type: u16,
}

impl Dihedral {
    /// Create a new dihedral
    pub fn new(
        atom_i: usize,
        atom_j: usize,
        atom_k: usize,
        atom_l: usize,
        dihedral_type: u16,
    ) -> Self {
        Self {
            atom_i,
            atom_j,
            atom_k,
            atom_l,
            dihedral_type,
        }
    }
}

/// Molecular topology containing all bonded interactions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Topology {
    /// Number of atoms in the topology
    pub n_atoms: usize,

    /// All bonds
    pub bonds: Vec<Bond>,

    /// All angles
    pub angles: Vec<Angle>,

    /// All dihedrals
    pub dihedrals: Vec<Dihedral>,

    /// Exclusion list: pairs of atoms to exclude from non-bonded interactions
    /// Stored as a sorted list for each atom
    pub exclusions: Vec<Vec<usize>>,
}

impl Topology {
    /// Create an empty topology for n atoms
    pub fn new(n_atoms: usize) -> Self {
        Self {
            n_atoms,
            bonds: Vec::new(),
            angles: Vec::new(),
            dihedrals: Vec::new(),
            exclusions: vec![Vec::new(); n_atoms],
        }
    }

    /// Add a bond
    pub fn add_bond(&mut self, atom_i: usize, atom_j: usize, bond_type: u16) {
        self.bonds.push(Bond::new(atom_i, atom_j, bond_type));
    }

    /// Add an angle
    pub fn add_angle(&mut self, atom_i: usize, atom_j: usize, atom_k: usize, angle_type: u16) {
        self.angles.push(Angle::new(atom_i, atom_j, atom_k, angle_type));
    }

    /// Add a dihedral
    pub fn add_dihedral(
        &mut self,
        atom_i: usize,
        atom_j: usize,
        atom_k: usize,
        atom_l: usize,
        dihedral_type: u16,
    ) {
        self.dihedrals
            .push(Dihedral::new(atom_i, atom_j, atom_k, atom_l, dihedral_type));
    }

    /// Add an exclusion pair
    pub fn add_exclusion(&mut self, atom_i: usize, atom_j: usize) {
        if !self.exclusions[atom_i].contains(&atom_j) {
            self.exclusions[atom_i].push(atom_j);
            self.exclusions[atom_i].sort_unstable();
        }
        if !self.exclusions[atom_j].contains(&atom_i) {
            self.exclusions[atom_j].push(atom_i);
            self.exclusions[atom_j].sort_unstable();
        }
    }

    /// Generate exclusions from bonds (1-2 exclusions)
    pub fn generate_exclusions_from_bonds(&mut self) {
        let pairs: Vec<_> = self.bonds.iter().map(|b| (b.atom_i, b.atom_j)).collect();
        for (i, j) in pairs {
            self.add_exclusion(i, j);
        }
    }

    /// Generate angle-based exclusions (1-3 exclusions)
    pub fn generate_exclusions_from_angles(&mut self) {
        let pairs: Vec<_> = self.angles.iter().map(|a| (a.atom_i, a.atom_k)).collect();
        for (i, k) in pairs {
            self.add_exclusion(i, k);
        }
    }

    /// Check if a pair is excluded
    #[inline]
    pub fn is_excluded(&self, atom_i: usize, atom_j: usize) -> bool {
        self.exclusions[atom_i].binary_search(&atom_j).is_ok()
    }

    /// Build connectivity from bonds
    pub fn build_connectivity(&self) -> Vec<Vec<usize>> {
        let mut connectivity = vec![Vec::new(); self.n_atoms];

        for bond in &self.bonds {
            connectivity[bond.atom_i].push(bond.atom_j);
            connectivity[bond.atom_j].push(bond.atom_i);
        }

        // Sort for consistent ordering
        for neighbors in &mut connectivity {
            neighbors.sort_unstable();
        }

        connectivity
    }

    /// Automatically detect angles from bonds
    pub fn detect_angles(&mut self) {
        let connectivity = self.build_connectivity();

        for (j, neighbors) in connectivity.iter().enumerate() {
            // For each pair of neighbors of j, create an angle i-j-k
            for (idx_i, &i) in neighbors.iter().enumerate() {
                for &k in neighbors.iter().skip(idx_i + 1) {
                    self.add_angle(i, j, k, 0);
                }
            }
        }
    }

    /// Automatically detect dihedrals from bonds
    pub fn detect_dihedrals(&mut self) {
        let connectivity = self.build_connectivity();

        // Collect bond pairs first to avoid borrow issues
        let bond_pairs: Vec<_> = self.bonds.iter().map(|b| (b.atom_i, b.atom_j)).collect();

        // Collect dihedrals to add
        let mut new_dihedrals = Vec::new();

        for (j, k) in bond_pairs {
            // Find atoms bonded to j (not k) and atoms bonded to k (not j)
            for &i in &connectivity[j] {
                if i == k {
                    continue;
                }
                for &l in &connectivity[k] {
                    if l == j || l == i {
                        continue;
                    }
                    new_dihedrals.push((i, j, k, l, 0));
                }
            }
        }

        // Add dihedrals
        for (i, j, k, l, dtype) in new_dihedrals {
            self.add_dihedral(i, j, k, l, dtype);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_creation() {
        let mut topo = Topology::new(4);
        topo.add_bond(0, 1, 0);
        topo.add_bond(1, 2, 0);
        topo.add_bond(2, 3, 0);

        assert_eq!(topo.bonds.len(), 3);
    }

    #[test]
    fn test_exclusions() {
        let mut topo = Topology::new(4);
        topo.add_bond(0, 1, 0);
        topo.add_bond(1, 2, 0);
        topo.generate_exclusions_from_bonds();

        assert!(topo.is_excluded(0, 1));
        assert!(topo.is_excluded(1, 0));
        assert!(topo.is_excluded(1, 2));
        assert!(!topo.is_excluded(0, 2));
    }

    #[test]
    fn test_angle_detection() {
        let mut topo = Topology::new(4);
        topo.add_bond(0, 1, 0);
        topo.add_bond(1, 2, 0);
        topo.add_bond(1, 3, 0);
        topo.detect_angles();

        assert_eq!(topo.angles.len(), 3); // 0-1-2, 0-1-3, 2-1-3
    }
}
