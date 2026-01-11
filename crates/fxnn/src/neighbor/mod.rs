//! Neighbor list algorithms for efficient force computation.
//!
//! This module provides data structures and algorithms for maintaining
//! lists of neighboring atom pairs. Neighbor lists reduce force calculation
//! from O(N^2) to O(N) complexity for short-range interactions.
//!
//! # Overview
//!
//! In molecular dynamics, most potentials have a finite cutoff distance
//! beyond which interactions are negligible. Instead of checking all N^2
//! pairs, we only need to compute interactions for pairs within the cutoff.
//!
//! FXNN provides two main algorithms:
//!
//! - [`CellList`]: Spatial decomposition into cells for O(N) neighbor finding
//! - [`VerletList`]: Neighbor list with skin distance for amortized rebuilds
//!
//! # The NeighborSearch Trait
//!
//! Both algorithms implement [`NeighborSearch`]:
//!
//! ```rust,ignore
//! pub trait NeighborSearch {
//!     fn build(&mut self, atoms: &[Atom], box_: &SimulationBox, cutoff: f32);
//!     fn needs_rebuild(&self, atoms: &[Atom], box_: &SimulationBox) -> bool;
//!     fn neighbor_list(&self) -> &NeighborList;
//! }
//! ```
//!
//! # Examples
//!
//! ## Using the Verlet list (recommended)
//!
//! ```rust
//! use fxnn::neighbor::{VerletList, NeighborSearch};
//! use fxnn::types::{Atom, SimulationBox};
//!
//! let box_ = SimulationBox::cubic(10.0);
//! let atoms = vec![
//!     Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
//!     Atom::new(1, 0, 1.0).with_position(1.0, 0.0, 0.0),
//!     Atom::new(2, 0, 1.0).with_position(5.0, 5.0, 5.0),
//! ];
//!
//! let cutoff = 2.5;
//! let skin = 0.5;
//!
//! let mut verlet = VerletList::new(atoms.len(), cutoff, skin);
//! verlet.build(&atoms, &box_, cutoff);
//!
//! // Get neighbors of atom 0
//! let neighbors = verlet.neighbor_list().get_neighbors(0);
//! assert!(neighbors.contains(&1));  // Atom 1 is nearby
//! ```
//!
//! ## Using the cell list directly
//!
//! ```rust
//! use fxnn::neighbor::{CellList, NeighborSearch};
//! use fxnn::types::{Atom, SimulationBox};
//!
//! let box_ = SimulationBox::cubic(10.0);
//! let atoms: Vec<Atom> = (0..100)
//!     .map(|i| Atom::new(i, 0, 1.0).with_position(
//!         (i % 10) as f32,
//!         ((i / 10) % 10) as f32,
//!         (i / 100) as f32,
//!     ))
//!     .collect();
//!
//! let mut cell_list = CellList::new(atoms.len(), 2.5, 0.5);
//! cell_list.build(&atoms, &box_, 2.5);
//!
//! // Check statistics
//! let stats = cell_list.stats();
//! println!("Cells: {} x {} x {}", stats.n_cells[0], stats.n_cells[1], stats.n_cells[2]);
//! ```
//!
//! # Algorithms
//!
//! ## Cell List Algorithm
//!
//! The simulation box is divided into cells with side length >= cutoff.
//! Each atom is assigned to its cell, and neighbors are only searched
//! in the 27 neighboring cells (including self).
//!
//! ```text
//! +---+---+---+
//! | 6 | 7 | 8 |   For cell 4:
//! +---+---+---+   - Check cells 0-8 (including self)
//! | 3 | 4 | 5 |   - Only pairs within cutoff are neighbors
//! +---+---+---+
//! | 0 | 1 | 2 |
//! +---+---+---+
//! ```
//!
//! Complexity: O(N) average case when atoms are uniformly distributed.
//!
//! ## Verlet List with Skin Distance
//!
//! The Verlet list stores all pairs within (cutoff + skin) distance.
//! The list only needs rebuilding when atoms have moved more than skin/2
//! since the last build.
//!
//! ```text
//! |<------- cutoff ------->|<-- skin -->|
//! +------------------------+------------+
//! |   Interaction zone     |   Buffer   |
//! +------------------------+------------+
//! ```
//!
//! Typical skin values: 0.3 - 1.0 in reduced units (or 0.1 - 0.3 nm).
//!
//! # Performance Tips
//!
//! 1. **Choose skin wisely**: Too small = frequent rebuilds, too large = many non-interacting pairs
//! 2. **Check rebuild frequency**: Aim for rebuilds every 10-50 steps
//! 3. **Use Verlet for production**: Combines cell list O(N) build with amortized updates
//!
//! # Rebuild Heuristic
//!
//! The `needs_rebuild()` method uses a conservative criterion:
//!
//! ```text
//! 2 * max_displacement > skin
//! ```
//!
//! This ensures even if two atoms move toward each other at maximum speed,
//! they cannot miss each other in the neighbor list.

mod cell_list;
mod verlet_list;

pub use cell_list::CellList;
pub use verlet_list::VerletList;

use crate::types::{Atom, SimulationBox};

/// Generic neighbor list structure storing pairs within cutoff.
///
/// The neighbor list stores, for each atom i, the indices of all atoms j
/// that are within (cutoff + skin) distance. This data structure is used
/// by force fields to efficiently iterate over interacting pairs.
///
/// # Structure
///
/// Each atom has a vector of neighbor indices. The list is symmetric:
/// if j is in neighbors\[i\], then i is in neighbors\[j\].
///
/// # Examples
///
/// ```rust
/// use fxnn::neighbor::NeighborList;
///
/// let mut nl = NeighborList::new(100, 2.5, 0.5);
/// assert_eq!(nl.neighbors.len(), 100);
///
/// // After building, access neighbors
/// // let neighbors_of_0 = nl.get_neighbors(0);
/// ```
#[derive(Debug, Clone)]
pub struct NeighborList {
    /// Neighbors for each atom (indices of neighboring atoms).
    pub neighbors: Vec<Vec<usize>>,
    /// Cutoff radius used to build the list.
    pub cutoff: f32,
    /// Skin distance for Verlet list buffer.
    pub skin: f32,
}

impl NeighborList {
    /// Create an empty neighbor list for n_atoms.
    ///
    /// # Arguments
    ///
    /// * `n_atoms` - Number of atoms in the system
    /// * `cutoff` - Interaction cutoff radius
    /// * `skin` - Additional buffer distance for Verlet list
    ///
    /// # Returns
    ///
    /// Empty neighbor list ready to be built.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::neighbor::NeighborList;
    ///
    /// let nl = NeighborList::new(1000, 2.5, 0.5);
    /// assert_eq!(nl.neighbors.len(), 1000);
    /// ```
    pub fn new(n_atoms: usize, cutoff: f32, skin: f32) -> Self {
        Self {
            neighbors: vec![Vec::new(); n_atoms],
            cutoff,
            skin,
        }
    }

    /// Clear all neighbor entries.
    ///
    /// Retains the allocated capacity for efficiency during rebuilds.
    pub fn clear(&mut self) {
        for neighbors in &mut self.neighbors {
            neighbors.clear();
        }
    }

    /// Get neighbors of atom i.
    ///
    /// # Arguments
    ///
    /// * `i` - Atom index
    ///
    /// # Returns
    ///
    /// Slice of neighbor indices for atom i.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let neighbors = neighbor_list.get_neighbors(0);
    /// for &j in neighbors {
    ///     // Process pair (0, j)
    /// }
    /// ```
    pub fn get_neighbors(&self, i: usize) -> &[usize] {
        &self.neighbors[i]
    }

    /// Get neighbors of atom i (alias for `get_neighbors`).
    ///
    /// # Arguments
    ///
    /// * `i` - Atom index
    ///
    /// # Returns
    ///
    /// Slice of neighbor indices for atom i.
    pub fn neighbors(&self, i: usize) -> &[usize] {
        &self.neighbors[i]
    }

    /// Get total number of neighbor pairs.
    ///
    /// Since pairs are stored symmetrically (both i->j and j->i),
    /// the actual number of unique pairs is half the sum of all neighbor counts.
    ///
    /// # Returns
    ///
    /// Number of unique neighbor pairs.
    pub fn num_pairs(&self) -> usize {
        self.neighbors.iter().map(|n| n.len()).sum::<usize>() / 2
    }

    /// Build neighbor list using O(N^2) direct enumeration.
    ///
    /// This method checks all pairs directly without spatial decomposition.
    /// Suitable for small systems (N < 100) or debugging.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Slice of atoms
    /// * `box_` - Simulation box with periodic boundary conditions
    pub fn build_direct(&mut self, atoms: &[Atom], box_: &SimulationBox) {
        self.clear();
        let cutoff2 = (self.cutoff + self.skin) * (self.cutoff + self.skin);
        let n = atoms.len();

        for i in 0..n {
            for j in (i + 1)..n {
                let d2 = box_.distance_squared(&atoms[i].position, &atoms[j].position);
                if d2 < cutoff2 {
                    self.neighbors[i].push(j);
                    self.neighbors[j].push(i);
                }
            }
        }
    }
}

/// Trait for neighbor search algorithms.
///
/// Implementations of this trait provide efficient neighbor finding
/// with automatic rebuild detection based on atom displacements.
pub trait NeighborSearch {
    /// Build neighbor list from current atom positions.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Current atom positions
    /// * `box_` - Simulation box
    /// * `cutoff` - Interaction cutoff radius
    fn build(&mut self, atoms: &[Atom], box_: &SimulationBox, cutoff: f32);

    /// Check if rebuild is needed based on atom displacement.
    ///
    /// Returns true if any atom has moved more than skin/2 since
    /// the last build, indicating the neighbor list may be stale.
    ///
    /// # Arguments
    ///
    /// * `atoms` - Current atom positions
    /// * `box_` - Simulation box
    ///
    /// # Returns
    ///
    /// `true` if the neighbor list should be rebuilt.
    fn needs_rebuild(&self, atoms: &[Atom], box_: &SimulationBox) -> bool;

    /// Get a reference to the neighbor list.
    fn neighbor_list(&self) -> &NeighborList;

    /// Get a mutable reference to the neighbor list.
    fn neighbor_list_mut(&mut self) -> &mut NeighborList;
}
