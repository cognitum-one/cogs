//! Domain Management for Parallel Molecular Dynamics
//!
//! This module handles spatial domains, ghost atoms at boundaries, and load
//! balancing for parallel simulations. Each domain contains a subset of atoms
//! and is responsible for computing forces on its atoms.
//!
//! # Ghost Atoms
//!
//! When atoms interact across domain boundaries, we need "ghost" copies of
//! atoms from neighboring domains. Ghost atoms are read-only copies used only
//! for force computation.
//!
//! # Load Balancing
//!
//! As atoms move during simulation, domains can become imbalanced. The load
//! balancer monitors imbalance and triggers re-partitioning when necessary.

use super::mincut::{InteractionGraph, MinCutPartitioner, PartitionResult, VertexId};
use super::{DecompositionConfig, DecompositionError, Result};
use indexmap::IndexSet;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Unique identifier for a domain
pub type DomainId = u32;

/// Index of an atom within its domain
pub type LocalAtomIndex = u32;

/// Global atom index
pub type GlobalAtomIndex = u32;

/// A ghost atom copied from another domain for boundary interactions
#[derive(Debug, Clone, Copy)]
pub struct GhostAtom {
    /// Global atom ID
    pub global_id: GlobalAtomIndex,
    /// Source domain that owns this atom
    pub source_domain: DomainId,
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Velocity (vx, vy, vz) - may be needed for some potentials
    pub velocity: [f32; 3],
    /// Atom type/species
    pub atom_type: u32,
    /// Charge
    pub charge: f32,
    /// Mass
    pub mass: f32,
}

impl GhostAtom {
    /// Create a new ghost atom
    pub fn new(
        global_id: GlobalAtomIndex,
        source_domain: DomainId,
        position: [f32; 3],
        velocity: [f32; 3],
        atom_type: u32,
        charge: f32,
        mass: f32,
    ) -> Self {
        Self {
            global_id,
            source_domain,
            position,
            velocity,
            atom_type,
            charge,
            mass,
        }
    }
}

/// An atom at a domain boundary that needs to be shared as ghost
#[derive(Debug, Clone)]
pub struct BoundaryAtom {
    /// Local index within this domain
    pub local_index: LocalAtomIndex,
    /// Global atom ID
    pub global_id: GlobalAtomIndex,
    /// List of neighboring domains that need this atom as ghost
    pub neighbor_domains: SmallVec<[DomainId; 4]>,
    /// Distance to boundary
    pub boundary_distance: f32,
}

/// Domain boundary information
#[derive(Debug, Clone)]
pub struct DomainBoundary {
    /// ID of the neighboring domain
    pub neighbor_id: DomainId,
    /// Atoms on this side of the boundary
    pub boundary_atoms: Vec<BoundaryAtom>,
    /// Ghost atoms received from neighbor
    pub ghost_atoms: Vec<GhostAtom>,
    /// Number of cross-boundary interactions
    pub interaction_count: usize,
    /// Total communication volume (bytes)
    pub comm_volume: usize,
}

impl DomainBoundary {
    /// Create a new empty boundary
    pub fn new(neighbor_id: DomainId) -> Self {
        Self {
            neighbor_id,
            boundary_atoms: Vec::new(),
            ghost_atoms: Vec::new(),
            interaction_count: 0,
            comm_volume: 0,
        }
    }

    /// Clear ghost atoms (call before receiving new data)
    pub fn clear_ghosts(&mut self) {
        self.ghost_atoms.clear();
    }

    /// Add a ghost atom
    pub fn add_ghost(&mut self, ghost: GhostAtom) {
        self.ghost_atoms.push(ghost);
    }
}

/// Statistics about a domain
#[derive(Debug, Clone, Default)]
pub struct DomainStats {
    /// Number of owned atoms
    pub num_atoms: usize,
    /// Number of ghost atoms
    pub num_ghosts: usize,
    /// Number of neighbor domains
    pub num_neighbors: usize,
    /// Total boundary atoms (sent as ghosts)
    pub num_boundary_atoms: usize,
    /// Total cross-boundary interactions
    pub num_boundary_interactions: usize,
    /// Estimated computation cost
    pub computation_cost: f64,
    /// Estimated communication cost
    pub communication_cost: f64,
}

/// A spatial domain containing a subset of atoms
#[derive(Debug)]
pub struct AtomDomain {
    /// Domain identifier
    pub id: DomainId,
    /// Global indices of owned atoms
    pub atom_indices: IndexSet<GlobalAtomIndex>,
    /// Boundaries with neighboring domains
    pub boundaries: HashMap<DomainId, DomainBoundary>,
    /// Statistics
    pub stats: DomainStats,
}

impl AtomDomain {
    /// Create a new empty domain
    pub fn new(id: DomainId) -> Self {
        Self {
            id,
            atom_indices: IndexSet::new(),
            boundaries: HashMap::new(),
            stats: DomainStats::default(),
        }
    }

    /// Create a domain with given atoms
    pub fn with_atoms(id: DomainId, atoms: impl IntoIterator<Item = GlobalAtomIndex>) -> Self {
        let mut domain = Self::new(id);
        domain.atom_indices = atoms.into_iter().collect();
        domain.stats.num_atoms = domain.atom_indices.len();
        domain
    }

    /// Add an atom to this domain
    pub fn add_atom(&mut self, global_id: GlobalAtomIndex) {
        if self.atom_indices.insert(global_id) {
            self.stats.num_atoms += 1;
        }
    }

    /// Remove an atom from this domain
    pub fn remove_atom(&mut self, global_id: GlobalAtomIndex) -> bool {
        if self.atom_indices.swap_remove(&global_id) {
            self.stats.num_atoms -= 1;
            true
        } else {
            false
        }
    }

    /// Check if this domain contains an atom
    pub fn contains(&self, global_id: GlobalAtomIndex) -> bool {
        self.atom_indices.contains(&global_id)
    }

    /// Get the number of owned atoms
    pub fn num_atoms(&self) -> usize {
        self.atom_indices.len()
    }

    /// Add a boundary with a neighbor domain
    pub fn add_boundary(&mut self, neighbor_id: DomainId) {
        if !self.boundaries.contains_key(&neighbor_id) {
            self.boundaries
                .insert(neighbor_id, DomainBoundary::new(neighbor_id));
            self.stats.num_neighbors += 1;
        }
    }

    /// Get boundary with a neighbor
    pub fn get_boundary(&self, neighbor_id: DomainId) -> Option<&DomainBoundary> {
        self.boundaries.get(&neighbor_id)
    }

    /// Get mutable boundary with a neighbor
    pub fn get_boundary_mut(&mut self, neighbor_id: DomainId) -> Option<&mut DomainBoundary> {
        self.boundaries.get_mut(&neighbor_id)
    }

    /// Iterate over all ghost atoms from all neighbors
    pub fn all_ghosts(&self) -> impl Iterator<Item = &GhostAtom> {
        self.boundaries.values().flat_map(|b| b.ghost_atoms.iter())
    }

    /// Get total number of ghost atoms
    pub fn num_ghosts(&self) -> usize {
        self.boundaries.values().map(|b| b.ghost_atoms.len()).sum()
    }

    /// Update statistics
    pub fn update_stats(&mut self) {
        self.stats.num_atoms = self.atom_indices.len();
        self.stats.num_neighbors = self.boundaries.len();
        self.stats.num_ghosts = self.num_ghosts();
        self.stats.num_boundary_atoms = self
            .boundaries
            .values()
            .map(|b| b.boundary_atoms.len())
            .sum();
        self.stats.num_boundary_interactions = self
            .boundaries
            .values()
            .map(|b| b.interaction_count)
            .sum();

        // Simple cost model
        self.stats.computation_cost =
            self.stats.num_atoms as f64 + 0.5 * self.stats.num_boundary_interactions as f64;
        self.stats.communication_cost = self.stats.num_boundary_atoms as f64 * 48.0; // ~48 bytes per atom
    }
}

/// Manager for ghost atom synchronization
#[derive(Debug)]
pub struct GhostManager {
    /// Ghost cutoff distance
    pub cutoff: f32,
    /// Skin distance for neighbor list updates
    pub skin: f32,
    /// Maximum displacement since last ghost update
    max_displacement: f32,
    /// Whether ghost update is needed
    needs_update: bool,
}

impl GhostManager {
    /// Create a new ghost manager
    pub fn new(cutoff: f32, skin: f32) -> Self {
        Self {
            cutoff,
            skin,
            max_displacement: 0.0,
            needs_update: true,
        }
    }

    /// Check if ghost atoms need updating based on atom displacement
    pub fn needs_update(&self) -> bool {
        self.needs_update || self.max_displacement > self.skin * 0.5
    }

    /// Mark ghosts as updated
    pub fn mark_updated(&mut self) {
        self.max_displacement = 0.0;
        self.needs_update = false;
    }

    /// Force a ghost update on next check
    pub fn invalidate(&mut self) {
        self.needs_update = true;
    }

    /// Update maximum displacement tracking
    pub fn update_displacement(&mut self, displacement: f32) {
        self.max_displacement = self.max_displacement.max(displacement);
    }

    /// Identify boundary atoms based on distance to domain boundary
    ///
    /// # Arguments
    /// * `positions` - All atom positions
    /// * `domain` - The domain to find boundary atoms for
    /// * `neighbor_domains` - Neighboring domains
    /// * `partition` - Current partitioning result
    pub fn identify_boundary_atoms(
        &self,
        positions: &[[f32; 3]],
        domain: &mut AtomDomain,
        neighbor_domains: &[DomainId],
        partition: &PartitionResult,
    ) {
        // Clear existing boundary information
        for boundary in domain.boundaries.values_mut() {
            boundary.boundary_atoms.clear();
        }

        // Collect atom indices first to avoid borrow issues
        let atom_list: Vec<(usize, GlobalAtomIndex)> = domain
            .atom_indices
            .iter()
            .enumerate()
            .map(|(idx, &gid)| (idx, gid))
            .collect();

        // Collect boundary atoms to add
        let mut boundary_updates: Vec<(DomainId, BoundaryAtom)> = Vec::new();

        // For each atom in this domain
        for (local_idx, global_id) in atom_list {
            let pos = positions[global_id as usize];

            // Check distance to atoms in neighboring domains
            let mut neighbor_distances: SmallVec<[(DomainId, f32); 4]> = SmallVec::new();

            for &neighbor_id in neighbor_domains {
                if neighbor_id == domain.id {
                    continue;
                }

                // Find minimum distance to any atom in neighbor domain
                let mut min_dist = f32::MAX;
                if let Some(neighbor_atoms) = partition.partitions.get(neighbor_id as usize) {
                    for &neighbor_global in neighbor_atoms {
                        let neighbor_pos = positions[neighbor_global as usize];
                        let dx = pos[0] - neighbor_pos[0];
                        let dy = pos[1] - neighbor_pos[1];
                        let dz = pos[2] - neighbor_pos[2];
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                        min_dist = min_dist.min(dist);
                    }
                }

                if min_dist <= self.cutoff + self.skin {
                    neighbor_distances.push((neighbor_id, min_dist));
                }
            }

            // If this atom is within cutoff of any neighbor domain, mark as boundary
            if !neighbor_distances.is_empty() {
                let neighbor_domain_ids: SmallVec<[DomainId; 4]> =
                    neighbor_distances.iter().map(|(id, _)| *id).collect();
                let min_distance = neighbor_distances
                    .iter()
                    .map(|(_, d)| *d)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(f32::MAX);

                let boundary_atom = BoundaryAtom {
                    local_index: local_idx as LocalAtomIndex,
                    global_id,
                    neighbor_domains: neighbor_domain_ids.clone(),
                    boundary_distance: min_distance,
                };

                // Collect for later update
                for neighbor_id in neighbor_domain_ids {
                    boundary_updates.push((neighbor_id, boundary_atom.clone()));
                }
            }
        }

        // Now apply updates
        for (neighbor_id, boundary_atom) in boundary_updates {
            domain.add_boundary(neighbor_id);
            if let Some(boundary) = domain.boundaries.get_mut(&neighbor_id) {
                boundary.boundary_atoms.push(boundary_atom);
            }
        }

        domain.update_stats();
    }

    /// Exchange ghost atoms between domains
    ///
    /// In a real parallel implementation, this would use MPI or similar.
    /// Here we provide a sequential reference implementation.
    pub fn exchange_ghosts(
        domains: &mut [AtomDomain],
        positions: &[[f32; 3]],
        velocities: &[[f32; 3]],
        atom_types: &[u32],
        charges: &[f32],
        masses: &[f32],
    ) {
        // First, clear all ghost atoms
        for domain in domains.iter_mut() {
            for boundary in domain.boundaries.values_mut() {
                boundary.clear_ghosts();
            }
        }

        // Collect boundary atoms to send
        let mut sends: Vec<(DomainId, DomainId, Vec<GhostAtom>)> = Vec::new();

        for domain in domains.iter() {
            for (neighbor_id, boundary) in &domain.boundaries {
                let mut ghosts = Vec::with_capacity(boundary.boundary_atoms.len());

                for boundary_atom in &boundary.boundary_atoms {
                    let gid = boundary_atom.global_id as usize;
                    ghosts.push(GhostAtom::new(
                        boundary_atom.global_id,
                        domain.id,
                        positions[gid],
                        velocities[gid],
                        atom_types[gid],
                        charges[gid],
                        masses[gid],
                    ));
                }

                sends.push((domain.id, *neighbor_id, ghosts));
            }
        }

        // Receive ghost atoms
        for (source_domain, target_domain, ghosts) in sends {
            if let Some(domain) = domains.iter_mut().find(|d| d.id == target_domain) {
                if let Some(boundary) = domain.boundaries.get_mut(&source_domain) {
                    for ghost in ghosts {
                        boundary.add_ghost(ghost);
                    }
                }
            }
        }

        // Update stats
        for domain in domains.iter_mut() {
            domain.update_stats();
        }
    }
}

/// Load balancer for dynamic re-partitioning
#[derive(Debug)]
pub struct LoadBalancer {
    /// Maximum allowed imbalance before rebalancing
    pub max_imbalance: f32,
    /// Minimum timesteps between rebalancing
    pub min_interval: usize,
    /// Timesteps since last rebalance
    timesteps_since_rebalance: usize,
    /// Historical imbalance values
    imbalance_history: Vec<f32>,
    /// Whether rebalancing is enabled
    enabled: bool,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(max_imbalance: f32, min_interval: usize) -> Self {
        Self {
            max_imbalance,
            min_interval,
            timesteps_since_rebalance: 0,
            imbalance_history: Vec::with_capacity(100),
            enabled: true,
        }
    }

    /// Enable or disable load balancing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if rebalancing is needed
    pub fn should_rebalance(&self, domains: &[AtomDomain]) -> bool {
        if !self.enabled {
            return false;
        }

        if self.timesteps_since_rebalance < self.min_interval {
            return false;
        }

        let imbalance = self.compute_imbalance(domains);
        imbalance > self.max_imbalance
    }

    /// Compute current load imbalance
    pub fn compute_imbalance(&self, domains: &[AtomDomain]) -> f32 {
        if domains.is_empty() {
            return 1.0;
        }

        let loads: Vec<f64> = domains.iter().map(|d| d.stats.computation_cost).collect();

        let total: f64 = loads.iter().sum();
        let avg = total / domains.len() as f64;
        let max = loads.iter().copied().fold(0.0, f64::max);

        if avg > 0.0 {
            (max / avg) as f32
        } else {
            1.0
        }
    }

    /// Record a timestep
    pub fn tick(&mut self) {
        self.timesteps_since_rebalance += 1;
    }

    /// Mark that rebalancing occurred
    pub fn mark_rebalanced(&mut self, imbalance: f32) {
        self.timesteps_since_rebalance = 0;
        self.imbalance_history.push(imbalance);

        // Keep only recent history
        if self.imbalance_history.len() > 100 {
            self.imbalance_history.remove(0);
        }
    }

    /// Get average imbalance from history
    pub fn average_imbalance(&self) -> f32 {
        if self.imbalance_history.is_empty() {
            1.0
        } else {
            self.imbalance_history.iter().sum::<f32>() / self.imbalance_history.len() as f32
        }
    }
}

/// Main domain decomposer for parallel molecular dynamics
#[derive(Debug)]
pub struct DomainDecomposer {
    /// Configuration
    config: DecompositionConfig,
    /// Current partitioning result
    partition: Option<PartitionResult>,
    /// Active domains
    domains: Vec<AtomDomain>,
    /// Ghost atom manager
    ghost_manager: GhostManager,
    /// Load balancer
    load_balancer: LoadBalancer,
}

impl DomainDecomposer {
    /// Create a new domain decomposer
    pub fn new(config: DecompositionConfig) -> Self {
        let ghost_manager = GhostManager::new(config.ghost_cutoff, config.ghost_skin);
        let load_balancer = LoadBalancer::new(config.max_imbalance, config.rebalance_frequency);

        Self {
            config,
            partition: None,
            domains: Vec::new(),
            ghost_manager,
            load_balancer,
        }
    }

    /// Perform initial domain decomposition
    ///
    /// # Arguments
    /// * `positions` - Atom positions
    /// * `neighbor_pairs` - Iterator of (i, j) neighbor pairs
    pub fn decompose<I>(&mut self, positions: &[[f32; 3]], neighbor_pairs: I) -> Result<()>
    where
        I: Iterator<Item = (usize, usize)>,
    {
        self.config.validate()?;

        let num_atoms = positions.len();
        if num_atoms < self.config.num_domains {
            return Err(DecompositionError::InsufficientAtoms(
                num_atoms,
                self.config.num_domains,
            ));
        }

        // Build interaction graph
        let graph = InteractionGraph::from_neighbors(
            positions,
            neighbor_pairs,
            self.config.edge_weight_strategy,
        );

        // Partition the graph
        let mut partitioner = MinCutPartitioner::for_partitions(self.config.num_domains);
        let partition = partitioner.partition(&graph)?;

        // Create domains from partition
        self.domains.clear();
        for (part_idx, vertex_set) in partition.partitions.iter().enumerate() {
            let domain = AtomDomain::with_atoms(
                part_idx as DomainId,
                vertex_set.iter().map(|&v| v as GlobalAtomIndex),
            );
            self.domains.push(domain);
        }

        // Store partition result
        self.partition = Some(partition);

        // Identify boundaries and ghost atoms
        self.update_boundaries(positions);

        Ok(())
    }

    /// Update domain boundaries based on current positions
    fn update_boundaries(&mut self, positions: &[[f32; 3]]) {
        let partition = match &self.partition {
            Some(p) => p,
            None => return,
        };

        // Get all domain IDs
        let domain_ids: Vec<DomainId> = self.domains.iter().map(|d| d.id).collect();

        // Update boundaries for each domain
        for domain in &mut self.domains {
            // Find neighboring domains (those with adjacent atoms)
            let mut neighbors = HashSet::new();
            for &global_id in &domain.atom_indices {
                if partition.get_partition(global_id as VertexId).is_some() {
                    // Check which other partitions have atoms within cutoff
                    for &other_id in &domain_ids {
                        if other_id == domain.id {
                            continue;
                        }
                        neighbors.insert(other_id);
                    }
                }
            }

            let neighbor_vec: Vec<_> = neighbors.into_iter().collect();
            self.ghost_manager
                .identify_boundary_atoms(positions, domain, &neighbor_vec, partition);
        }

        self.ghost_manager.mark_updated();
    }

    /// Update ghost atoms for all domains
    pub fn update_ghosts(
        &mut self,
        positions: &[[f32; 3]],
        velocities: &[[f32; 3]],
        atom_types: &[u32],
        charges: &[f32],
        masses: &[f32],
    ) {
        if self.ghost_manager.needs_update() {
            GhostManager::exchange_ghosts(
                &mut self.domains,
                positions,
                velocities,
                atom_types,
                charges,
                masses,
            );
            self.ghost_manager.mark_updated();
        }
    }

    /// Force ghost update on next call
    pub fn invalidate_ghosts(&mut self) {
        self.ghost_manager.invalidate();
    }

    /// Check if load rebalancing is needed and perform if so
    pub fn maybe_rebalance<I>(
        &mut self,
        positions: &[[f32; 3]],
        neighbor_pairs_fn: impl Fn() -> I,
    ) -> Result<bool>
    where
        I: Iterator<Item = (usize, usize)>,
    {
        self.load_balancer.tick();

        if !self.load_balancer.should_rebalance(&self.domains) {
            return Ok(false);
        }

        // Perform rebalancing
        let imbalance = self.load_balancer.compute_imbalance(&self.domains);
        self.decompose(positions, neighbor_pairs_fn())?;
        self.load_balancer.mark_rebalanced(imbalance);

        Ok(true)
    }

    /// Get a reference to all domains
    pub fn domains(&self) -> &[AtomDomain] {
        &self.domains
    }

    /// Get a mutable reference to all domains
    pub fn domains_mut(&mut self) -> &mut [AtomDomain] {
        &mut self.domains
    }

    /// Get a specific domain by ID
    pub fn get_domain(&self, id: DomainId) -> Option<&AtomDomain> {
        self.domains.iter().find(|d| d.id == id)
    }

    /// Get mutable domain by ID
    pub fn get_domain_mut(&mut self, id: DomainId) -> Option<&mut AtomDomain> {
        self.domains.iter_mut().find(|d| d.id == id)
    }

    /// Get the current partitioning result
    pub fn partition(&self) -> Option<&PartitionResult> {
        self.partition.as_ref()
    }

    /// Get number of domains
    pub fn num_domains(&self) -> usize {
        self.domains.len()
    }

    /// Get total number of ghost atoms across all domains
    pub fn total_ghosts(&self) -> usize {
        self.domains.iter().map(|d| d.num_ghosts()).sum()
    }

    /// Get overall statistics
    pub fn stats(&self) -> DecompositionStats {
        let num_domains = self.domains.len();
        let total_atoms: usize = self.domains.iter().map(|d| d.num_atoms()).sum();
        let total_ghosts: usize = self.domains.iter().map(|d| d.num_ghosts()).sum();
        let total_boundary_atoms: usize = self
            .domains
            .iter()
            .map(|d| d.stats.num_boundary_atoms)
            .sum();

        let partition_cut = self
            .partition
            .as_ref()
            .map(|p| p.cut_weight)
            .unwrap_or(0.0);
        let imbalance = self.load_balancer.compute_imbalance(&self.domains);

        DecompositionStats {
            num_domains,
            total_atoms,
            total_ghosts,
            total_boundary_atoms,
            partition_cut,
            imbalance,
            avg_atoms_per_domain: if num_domains > 0 {
                total_atoms as f32 / num_domains as f32
            } else {
                0.0
            },
            ghost_ratio: if total_atoms > 0 {
                total_ghosts as f32 / total_atoms as f32
            } else {
                0.0
            },
        }
    }

    /// Get reference to ghost manager
    pub fn ghost_manager(&self) -> &GhostManager {
        &self.ghost_manager
    }

    /// Get reference to load balancer
    pub fn load_balancer(&self) -> &LoadBalancer {
        &self.load_balancer
    }

    /// Get mutable reference to load balancer
    pub fn load_balancer_mut(&mut self) -> &mut LoadBalancer {
        &mut self.load_balancer
    }
}

/// Overall decomposition statistics
#[derive(Debug, Clone, Default)]
pub struct DecompositionStats {
    /// Number of domains
    pub num_domains: usize,
    /// Total owned atoms
    pub total_atoms: usize,
    /// Total ghost atoms
    pub total_ghosts: usize,
    /// Total boundary atoms
    pub total_boundary_atoms: usize,
    /// Partition cut weight
    pub partition_cut: f32,
    /// Current load imbalance
    pub imbalance: f32,
    /// Average atoms per domain
    pub avg_atoms_per_domain: f32,
    /// Ghost to owned atom ratio
    pub ghost_ratio: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_positions() -> Vec<[f32; 3]> {
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 0.0, 0.0],
            [12.0, 0.0, 0.0],
            [13.0, 0.0, 0.0],
        ]
    }

    fn create_test_neighbors() -> Vec<(usize, usize)> {
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (4, 5),
            (5, 6),
            (6, 7),
            (3, 4), // Bridge between clusters
        ]
    }

    #[test]
    fn test_atom_domain() {
        let mut domain = AtomDomain::new(0);

        domain.add_atom(1);
        domain.add_atom(2);
        domain.add_atom(3);

        assert_eq!(domain.num_atoms(), 3);
        assert!(domain.contains(1));
        assert!(domain.contains(2));
        assert!(!domain.contains(10));

        domain.remove_atom(2);
        assert_eq!(domain.num_atoms(), 2);
        assert!(!domain.contains(2));
    }

    #[test]
    fn test_domain_boundary() {
        let mut domain = AtomDomain::new(0);
        domain.add_boundary(1);
        domain.add_boundary(2);

        assert_eq!(domain.boundaries.len(), 2);
        assert!(domain.get_boundary(1).is_some());
        assert!(domain.get_boundary(3).is_none());
    }

    #[test]
    fn test_ghost_atom() {
        let ghost = GhostAtom::new(42, 1, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3], 0, 0.0, 1.0);

        assert_eq!(ghost.global_id, 42);
        assert_eq!(ghost.source_domain, 1);
        assert_eq!(ghost.position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_ghost_manager() {
        let mut manager = GhostManager::new(3.0, 0.5);

        assert!(manager.needs_update());
        manager.mark_updated();
        assert!(!manager.needs_update());

        manager.update_displacement(0.3);
        assert!(manager.needs_update()); // > 0.5 * 0.5

        manager.invalidate();
        assert!(manager.needs_update());
    }

    #[test]
    fn test_load_balancer() {
        let mut balancer = LoadBalancer::new(1.2, 10);

        // Create test domains with varying loads
        let mut domains = vec![
            AtomDomain::with_atoms(0, [0, 1, 2].iter().copied()),
            AtomDomain::with_atoms(1, [3].iter().copied()),
        ];

        for domain in &mut domains {
            domain.update_stats();
        }

        // Should not rebalance initially (interval not met)
        for _ in 0..5 {
            balancer.tick();
        }
        assert!(!balancer.should_rebalance(&domains));

        // After enough timesteps
        for _ in 0..10 {
            balancer.tick();
        }
        assert!(balancer.should_rebalance(&domains));
    }

    #[test]
    fn test_decomposer_creation() {
        let config = DecompositionConfig::default().with_num_domains(2);
        let decomposer = DomainDecomposer::new(config);

        assert_eq!(decomposer.num_domains(), 0);
    }

    #[test]
    fn test_decomposition() {
        let config = DecompositionConfig::default()
            .with_num_domains(2)
            .with_ghost_cutoff(2.0);
        let mut decomposer = DomainDecomposer::new(config);

        let positions = create_test_positions();
        let neighbors = create_test_neighbors();

        decomposer
            .decompose(&positions, neighbors.into_iter())
            .unwrap();

        assert_eq!(decomposer.num_domains(), 2);

        let stats = decomposer.stats();
        assert_eq!(stats.total_atoms, 8);
        assert!(stats.imbalance >= 1.0);
    }

    #[test]
    fn test_decomposition_stats() {
        let config = DecompositionConfig::default()
            .with_num_domains(2)
            .with_ghost_cutoff(2.0);
        let mut decomposer = DomainDecomposer::new(config);

        let positions = create_test_positions();
        let neighbors = create_test_neighbors();

        decomposer
            .decompose(&positions, neighbors.into_iter())
            .unwrap();

        let stats = decomposer.stats();

        assert_eq!(stats.num_domains, 2);
        assert_eq!(stats.total_atoms, 8);
        assert_eq!(stats.avg_atoms_per_domain, 4.0);
    }

    #[test]
    fn test_insufficient_atoms() {
        let config = DecompositionConfig::default().with_num_domains(10);
        let mut decomposer = DomainDecomposer::new(config);

        let positions = vec![[0.0, 0.0, 0.0]; 5];
        let neighbors: Vec<(usize, usize)> = vec![];

        let result = decomposer.decompose(&positions, neighbors.into_iter());
        assert!(matches!(
            result,
            Err(DecompositionError::InsufficientAtoms(5, 10))
        ));
    }
}
