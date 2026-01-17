//! Verlet neighbor list with skin distance
//!
//! Maintains a list of neighbors within cutoff + skin distance.
//! Only needs rebuilding when atoms have moved more than skin/2.

use crate::types::{Atom, SimulationBox};
use super::{NeighborList, NeighborSearch, CellList};

/// Verlet list with skin-based rebuild heuristic
#[derive(Debug, Clone)]
pub struct VerletList {
    /// The neighbor list
    neighbor_list: NeighborList,
    /// Reference positions from last build
    reference_positions: Vec<[f32; 3]>,
    /// Skin distance
    skin: f32,
    /// Underlying cell list for efficient building
    cell_list: Option<CellList>,
    /// Maximum displacement since last build
    max_displacement: f32,
    /// Number of rebuilds
    rebuild_count: usize,
    /// Number of steps since last rebuild
    steps_since_rebuild: usize,
}

impl VerletList {
    /// Create a new Verlet list
    pub fn new(n_atoms: usize, cutoff: f32, skin: f32) -> Self {
        Self {
            neighbor_list: NeighborList::new(n_atoms, cutoff, skin),
            reference_positions: Vec::new(),
            skin,
            cell_list: Some(CellList::new(n_atoms, cutoff, skin)),
            max_displacement: 0.0,
            rebuild_count: 0,
            steps_since_rebuild: 0,
        }
    }

    /// Create without cell list (for small systems, uses O(N²))
    pub fn new_direct(n_atoms: usize, cutoff: f32, skin: f32) -> Self {
        Self {
            neighbor_list: NeighborList::new(n_atoms, cutoff, skin),
            reference_positions: Vec::new(),
            skin,
            cell_list: None,
            max_displacement: 0.0,
            rebuild_count: 0,
            steps_since_rebuild: 0,
        }
    }

    /// Get rebuild statistics
    pub fn stats(&self) -> VerletListStats {
        VerletListStats {
            rebuild_count: self.rebuild_count,
            steps_since_rebuild: self.steps_since_rebuild,
            max_displacement: self.max_displacement,
            skin: self.skin,
            num_pairs: self.neighbor_list.num_pairs(),
        }
    }

    /// Update displacement tracking
    fn update_displacement(&mut self, atoms: &[Atom], box_: &SimulationBox) {
        if self.reference_positions.is_empty() {
            return;
        }

        self.max_displacement = 0.0;
        for (i, atom) in atoms.iter().enumerate() {
            let d2 = box_.distance_squared(&atom.position, &self.reference_positions[i]);
            let d = d2.sqrt();
            if d > self.max_displacement {
                self.max_displacement = d;
            }
        }
    }

    /// Check and track whether a step has been taken
    pub fn step(&mut self, atoms: &[Atom], box_: &SimulationBox) {
        self.steps_since_rebuild += 1;
        self.update_displacement(atoms, box_);
    }

    /// Get the skin distance
    pub fn skin(&self) -> f32 {
        self.skin
    }

    /// Set the skin distance (requires rebuild)
    pub fn set_skin(&mut self, skin: f32) {
        self.skin = skin;
        self.neighbor_list.skin = skin;
    }
}

impl NeighborSearch for VerletList {
    fn build(&mut self, atoms: &[Atom], box_: &SimulationBox, cutoff: f32) {
        if let Some(ref mut cell_list) = self.cell_list {
            cell_list.build(atoms, box_, cutoff);
            self.neighbor_list = cell_list.neighbor_list().clone();
        } else {
            self.neighbor_list.build_direct(atoms, box_);
        }

        self.reference_positions = atoms.iter().map(|a| a.position).collect();
        self.max_displacement = 0.0;
        self.rebuild_count += 1;
        self.steps_since_rebuild = 0;
    }

    fn needs_rebuild(&self, atoms: &[Atom], box_: &SimulationBox) -> bool {
        if self.reference_positions.len() != atoms.len() {
            return true;
        }

        // Conservative check: rebuild if 2 * max_displacement > skin
        // This ensures even the fastest atom pair can't miss each other
        let threshold = self.skin * 0.5;

        for (i, atom) in atoms.iter().enumerate() {
            let d2 = box_.distance_squared(&atom.position, &self.reference_positions[i]);
            if d2 > threshold * threshold {
                return true;
            }
        }

        false
    }

    fn neighbor_list(&self) -> &NeighborList {
        &self.neighbor_list
    }

    fn neighbor_list_mut(&mut self) -> &mut NeighborList {
        &mut self.neighbor_list
    }
}

/// Statistics about the Verlet list
#[derive(Debug, Clone, Copy)]
pub struct VerletListStats {
    /// Number of times the list has been rebuilt
    pub rebuild_count: usize,
    /// Steps since last rebuild
    pub steps_since_rebuild: usize,
    /// Maximum atom displacement since last rebuild
    pub max_displacement: f32,
    /// Skin distance
    pub skin: f32,
    /// Number of neighbor pairs
    pub num_pairs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verlet_list_creation() {
        let vl = VerletList::new(100, 2.5, 0.5);
        assert!((vl.skin() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_verlet_list_rebuild_detection() {
        let box_ = SimulationBox::cubic(10.0);
        let mut atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0),
        ];

        let mut vl = VerletList::new(2, 2.5, 0.5);
        vl.build(&atoms, &box_, 2.5);

        // Small movement - no rebuild needed
        atoms[0].position[0] = 0.1;
        assert!(!vl.needs_rebuild(&atoms, &box_));

        // Large movement - rebuild needed
        atoms[0].position[0] = 0.5;
        assert!(vl.needs_rebuild(&atoms, &box_));
    }

    #[test]
    fn test_verlet_list_stats() {
        let box_ = SimulationBox::cubic(10.0);
        let atoms = vec![
            Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
            Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0),
        ];

        let mut vl = VerletList::new(2, 2.5, 0.5);
        vl.build(&atoms, &box_, 2.5);
        vl.build(&atoms, &box_, 2.5);

        let stats = vl.stats();
        assert_eq!(stats.rebuild_count, 2);
    }
}
