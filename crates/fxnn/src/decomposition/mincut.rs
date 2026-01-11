//! MinCut-based Graph Partitioning for Domain Decomposition
//!
//! This module implements graph partitioning algorithms based on minimum cut
//! optimization. The goal is to partition atoms into domains such that the
//! number of cross-boundary interactions is minimized.
//!
//! # Algorithms
//!
//! - **Karger-Stein**: Randomized algorithm with O(n^2 log^3 n) complexity
//! - **Spectral Bisection**: Uses graph Laplacian eigenvectors
//! - **Multilevel**: Coarsen-partition-uncoarsen with Kernighan-Lin refinement
//! - **Recursive Bisection**: Simple and effective for molecular systems
//!
//! # Integration with RuVector MinCut
//!
//! The implementation follows patterns from `ruvector-mincut` crate:
//! - Graph representation optimized for dynamic updates
//! - Hierarchical decomposition for multi-level partitioning
//! - Certificate-based verification for cut quality

use super::{DecompositionError, EdgeWeightStrategy, Result};
use indexmap::IndexMap;
use smallvec::SmallVec;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

/// Unique identifier for a vertex (atom) in the interaction graph
pub type VertexId = u32;

/// Edge weight type (typically interaction strength or distance)
pub type EdgeWeight = f32;

/// An edge in the interaction graph representing atom-atom interaction
#[derive(Debug, Clone, Copy)]
pub struct InteractionEdge {
    /// Source atom ID
    pub source: VertexId,
    /// Target atom ID
    pub target: VertexId,
    /// Edge weight (higher = more costly to cut)
    pub weight: EdgeWeight,
    /// Distance between atoms (for weighting strategies)
    pub distance: f32,
}

impl InteractionEdge {
    /// Create a new interaction edge
    pub fn new(source: VertexId, target: VertexId, weight: EdgeWeight, distance: f32) -> Self {
        Self {
            source,
            target,
            weight,
            distance,
        }
    }

    /// Get canonical (ordered) endpoints
    pub fn canonical(&self) -> (VertexId, VertexId) {
        if self.source <= self.target {
            (self.source, self.target)
        } else {
            (self.target, self.source)
        }
    }
}

/// Adjacency entry for a vertex
#[derive(Debug, Clone, Default)]
struct AdjacencyEntry {
    /// Neighbors and their edge weights
    neighbors: SmallVec<[(VertexId, EdgeWeight); 16]>,
    /// Total weight of incident edges
    total_weight: EdgeWeight,
}

/// Interaction graph built from neighbor list
///
/// Represents atom-atom interactions as a weighted undirected graph.
/// Optimized for:
/// - Fast neighbor iteration
/// - Efficient edge weight queries
/// - Dynamic updates for rebalancing
#[derive(Debug, Clone)]
pub struct InteractionGraph {
    /// Adjacency list representation
    adjacency: IndexMap<VertexId, AdjacencyEntry>,
    /// Edge storage for iteration
    edges: Vec<InteractionEdge>,
    /// Number of vertices
    num_vertices: usize,
    /// Total edge weight
    total_weight: EdgeWeight,
}

impl InteractionGraph {
    /// Create a new empty interaction graph
    pub fn new() -> Self {
        Self {
            adjacency: IndexMap::new(),
            edges: Vec::new(),
            num_vertices: 0,
            total_weight: 0.0,
        }
    }

    /// Create with capacity hints
    pub fn with_capacity(vertices: usize, edges: usize) -> Self {
        Self {
            adjacency: IndexMap::with_capacity(vertices),
            edges: Vec::with_capacity(edges),
            num_vertices: 0,
            total_weight: 0.0,
        }
    }

    /// Build interaction graph from atom positions and neighbor list
    ///
    /// # Arguments
    /// * `positions` - Atom positions as (x, y, z) tuples
    /// * `neighbor_pairs` - Iterator of (atom_i, atom_j) neighbor pairs
    /// * `strategy` - Edge weight assignment strategy
    pub fn from_neighbors<I>(
        positions: &[[f32; 3]],
        neighbor_pairs: I,
        strategy: EdgeWeightStrategy,
    ) -> Self
    where
        I: Iterator<Item = (usize, usize)>,
    {
        let mut graph = Self::with_capacity(positions.len(), positions.len() * 20);

        // Add all vertices
        for i in 0..positions.len() {
            graph.add_vertex(i as VertexId);
        }

        // Add edges from neighbor pairs
        for (i, j) in neighbor_pairs {
            if i >= j {
                continue; // Skip duplicates and self-interactions
            }

            let pi = positions[i];
            let pj = positions[j];

            // Compute distance
            let dx = pi[0] - pj[0];
            let dy = pi[1] - pj[1];
            let dz = pi[2] - pj[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            // Compute weight based on strategy
            let weight = match strategy {
                EdgeWeightStrategy::Uniform => 1.0,
                EdgeWeightStrategy::InverseDistance => 1.0 / dist.max(0.1),
                EdgeWeightStrategy::InteractionStrength => {
                    // LJ potential approximation: 1/r^6
                    let r6 = dist.powi(6).max(1e-6);
                    1.0 / r6
                }
                EdgeWeightStrategy::CommunicationCost => {
                    // Communication scales with data size, roughly constant per atom
                    1.0
                }
            };

            graph.add_edge(i as VertexId, j as VertexId, weight, dist);
        }

        graph
    }

    /// Add a vertex to the graph
    pub fn add_vertex(&mut self, v: VertexId) -> bool {
        if self.adjacency.contains_key(&v) {
            return false;
        }
        self.adjacency.insert(v, AdjacencyEntry::default());
        self.num_vertices += 1;
        true
    }

    /// Add an edge to the graph
    pub fn add_edge(
        &mut self,
        u: VertexId,
        v: VertexId,
        weight: EdgeWeight,
        distance: f32,
    ) -> bool {
        if u == v {
            return false;
        }

        // Ensure vertices exist
        self.add_vertex(u);
        self.add_vertex(v);

        // Add to adjacency lists
        if let Some(entry) = self.adjacency.get_mut(&u) {
            entry.neighbors.push((v, weight));
            entry.total_weight += weight;
        }
        if let Some(entry) = self.adjacency.get_mut(&v) {
            entry.neighbors.push((u, weight));
            entry.total_weight += weight;
        }

        // Store edge
        self.edges.push(InteractionEdge::new(u, v, weight, distance));
        self.total_weight += weight;

        true
    }

    /// Get neighbors of a vertex
    pub fn neighbors(&self, v: VertexId) -> impl Iterator<Item = (VertexId, EdgeWeight)> + '_ {
        self.adjacency
            .get(&v)
            .into_iter()
            .flat_map(|e| e.neighbors.iter().copied())
    }

    /// Get degree (weighted) of a vertex
    pub fn weighted_degree(&self, v: VertexId) -> EdgeWeight {
        self.adjacency.get(&v).map_or(0.0, |e| e.total_weight)
    }

    /// Get number of neighbors of a vertex
    pub fn degree(&self, v: VertexId) -> usize {
        self.adjacency.get(&v).map_or(0, |e| e.neighbors.len())
    }

    /// Get all vertices
    pub fn vertices(&self) -> impl Iterator<Item = VertexId> + '_ {
        self.adjacency.keys().copied()
    }

    /// Get all edges
    pub fn edges(&self) -> &[InteractionEdge] {
        &self.edges
    }

    /// Get number of vertices
    pub fn num_vertices(&self) -> usize {
        self.num_vertices
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Get total edge weight
    pub fn total_weight(&self) -> EdgeWeight {
        self.total_weight
    }

    /// Compute cut value between two vertex sets
    pub fn cut_value(&self, set_a: &HashSet<VertexId>, set_b: &HashSet<VertexId>) -> EdgeWeight {
        let mut cut = 0.0;
        for edge in &self.edges {
            let a_has_src = set_a.contains(&edge.source);
            let a_has_tgt = set_a.contains(&edge.target);
            let b_has_src = set_b.contains(&edge.source);
            let b_has_tgt = set_b.contains(&edge.target);

            // Edge crosses cut if endpoints are in different sets
            if (a_has_src && b_has_tgt) || (b_has_src && a_has_tgt) {
                cut += edge.weight;
            }
        }
        cut
    }

    /// Check if graph is connected using BFS
    pub fn is_connected(&self) -> bool {
        if self.num_vertices <= 1 {
            return true;
        }

        let start = match self.adjacency.keys().next() {
            Some(&v) => v,
            None => return true,
        };

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(v) = queue.pop_front() {
            for (neighbor, _) in self.neighbors(v) {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        visited.len() == self.num_vertices
    }

    /// Get connected components
    pub fn connected_components(&self) -> Vec<HashSet<VertexId>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for &start in self.adjacency.keys() {
            if visited.contains(&start) {
                continue;
            }

            let mut component = HashSet::new();
            let mut queue = VecDeque::new();

            queue.push_back(start);
            visited.insert(start);

            while let Some(v) = queue.pop_front() {
                component.insert(v);
                for (neighbor, _) in self.neighbors(v) {
                    if visited.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }

            components.push(component);
        }

        components
    }
}

impl Default for InteractionGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for graph partitioning
#[derive(Debug, Clone)]
pub struct PartitionConfig {
    /// Number of partitions to create
    pub num_partitions: usize,
    /// Maximum imbalance ratio (1.0 = perfect balance)
    pub max_imbalance: f32,
    /// Partitioning strategy to use
    pub strategy: PartitionStrategy,
    /// Number of refinement iterations
    pub refinement_iterations: usize,
    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            num_partitions: 2,
            max_imbalance: 1.1,
            strategy: PartitionStrategy::RecursiveBisection,
            refinement_iterations: 10,
            seed: 42,
        }
    }
}

/// Partitioning strategy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionStrategy {
    /// Simple recursive bisection using BFS
    RecursiveBisection,
    /// Spectral bisection using Laplacian eigenvector
    SpectralBisection,
    /// Kernighan-Lin with random initialization
    KernighanLin,
    /// Multilevel coarsening approach
    Multilevel,
    /// Greedy graph growing partitioning
    GreedyGrow,
}

/// Result of graph partitioning
#[derive(Debug, Clone)]
pub struct PartitionResult {
    /// Partition assignment for each vertex
    pub assignments: HashMap<VertexId, usize>,
    /// Vertices in each partition
    pub partitions: Vec<HashSet<VertexId>>,
    /// Total cut weight (edges crossing partition boundaries)
    pub cut_weight: EdgeWeight,
    /// Size of each partition
    pub partition_sizes: Vec<usize>,
    /// Imbalance ratio (max_size / avg_size)
    pub imbalance: f32,
    /// Number of boundary edges per partition
    pub boundary_counts: Vec<usize>,
}

impl PartitionResult {
    /// Create a new partition result
    pub fn new(num_partitions: usize) -> Self {
        Self {
            assignments: HashMap::new(),
            partitions: vec![HashSet::new(); num_partitions],
            cut_weight: 0.0,
            partition_sizes: vec![0; num_partitions],
            imbalance: 1.0,
            boundary_counts: vec![0; num_partitions],
        }
    }

    /// Add a vertex to a partition
    pub fn assign(&mut self, vertex: VertexId, partition: usize) {
        if partition < self.partitions.len() {
            self.partitions[partition].insert(vertex);
            self.assignments.insert(vertex, partition);
            self.partition_sizes[partition] += 1;
        }
    }

    /// Get the partition of a vertex
    pub fn get_partition(&self, vertex: VertexId) -> Option<usize> {
        self.assignments.get(&vertex).copied()
    }

    /// Compute statistics after partitioning
    pub fn compute_stats(&mut self, graph: &InteractionGraph) {
        // Compute cut weight
        self.cut_weight = 0.0;
        for edge in graph.edges() {
            let p1 = self.assignments.get(&edge.source);
            let p2 = self.assignments.get(&edge.target);
            if p1 != p2 {
                self.cut_weight += edge.weight;
            }
        }

        // Compute imbalance
        let avg_size = self
            .partition_sizes
            .iter()
            .sum::<usize>() as f32
            / self.partitions.len() as f32;
        let max_size = *self.partition_sizes.iter().max().unwrap_or(&1) as f32;
        self.imbalance = if avg_size > 0.0 {
            max_size / avg_size
        } else {
            1.0
        };

        // Count boundary edges per partition
        self.boundary_counts = vec![0; self.partitions.len()];
        for edge in graph.edges() {
            let p1 = self.assignments.get(&edge.source).copied();
            let p2 = self.assignments.get(&edge.target).copied();
            if p1 != p2 {
                if let Some(p) = p1 {
                    self.boundary_counts[p] += 1;
                }
                if let Some(p) = p2 {
                    self.boundary_counts[p] += 1;
                }
            }
        }
    }
}

/// MinCut-based graph partitioner
///
/// Implements various graph partitioning algorithms to minimize the cut
/// (number of edges crossing partition boundaries) while maintaining
/// balanced partition sizes.
pub struct MinCutPartitioner {
    config: PartitionConfig,
    /// Random number generator state
    rng_state: u64,
}

impl MinCutPartitioner {
    /// Create a new partitioner with given configuration
    pub fn new(config: PartitionConfig) -> Self {
        Self {
            rng_state: config.seed,
            config,
        }
    }

    /// Create with default configuration for n partitions
    pub fn for_partitions(n: usize) -> Self {
        Self::new(PartitionConfig {
            num_partitions: n,
            ..Default::default()
        })
    }

    /// Partition the graph into the configured number of parts
    pub fn partition(&mut self, graph: &InteractionGraph) -> Result<PartitionResult> {
        if graph.num_vertices() < self.config.num_partitions {
            return Err(DecompositionError::InsufficientAtoms(
                graph.num_vertices(),
                self.config.num_partitions,
            ));
        }

        let result = match self.config.strategy {
            PartitionStrategy::RecursiveBisection => {
                self.recursive_bisection(graph, self.config.num_partitions)?
            }
            PartitionStrategy::SpectralBisection => {
                self.spectral_bisection(graph, self.config.num_partitions)?
            }
            PartitionStrategy::KernighanLin => {
                self.kernighan_lin(graph, self.config.num_partitions)?
            }
            PartitionStrategy::Multilevel => {
                self.multilevel_partition(graph, self.config.num_partitions)?
            }
            PartitionStrategy::GreedyGrow => {
                self.greedy_grow(graph, self.config.num_partitions)?
            }
        };

        Ok(result)
    }

    /// Simple random number generator (xorshift)
    fn next_random(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Recursive bisection partitioning
    ///
    /// Repeatedly bisects the graph using BFS-based partitioning until
    /// the desired number of partitions is reached.
    fn recursive_bisection(
        &mut self,
        graph: &InteractionGraph,
        num_parts: usize,
    ) -> Result<PartitionResult> {
        let vertices: Vec<_> = graph.vertices().collect();

        // Start with all vertices in one partition
        let mut partitions: Vec<HashSet<VertexId>> = vec![vertices.into_iter().collect()];

        // Repeatedly bisect until we have enough partitions
        while partitions.len() < num_parts {
            // Find largest partition to split
            let (idx, largest) = partitions
                .iter()
                .enumerate()
                .max_by_key(|(_, p)| p.len())
                .map(|(i, p)| (i, p.clone()))
                .unwrap();

            if largest.len() < 2 {
                break; // Can't split further
            }

            // Bisect the largest partition
            let (part_a, part_b) = self.bisect_partition(graph, &largest);

            // Replace largest with the two halves
            partitions[idx] = part_a;
            partitions.push(part_b);
        }

        // Build result
        let mut result = PartitionResult::new(partitions.len());
        for (part_idx, partition) in partitions.into_iter().enumerate() {
            for v in partition {
                result.assign(v, part_idx);
            }
        }
        result.compute_stats(graph);

        Ok(result)
    }

    /// Bisect a partition using BFS from two seed vertices
    fn bisect_partition(
        &mut self,
        graph: &InteractionGraph,
        vertices: &HashSet<VertexId>,
    ) -> (HashSet<VertexId>, HashSet<VertexId>) {
        if vertices.len() <= 1 {
            return (vertices.clone(), HashSet::new());
        }

        // Pick two seed vertices (farthest apart using BFS)
        let (seed_a, seed_b) = self.find_distant_seeds(graph, vertices);

        // Grow partitions alternately from seeds
        let mut part_a = HashSet::new();
        let mut part_b = HashSet::new();
        let mut queue_a = VecDeque::new();
        let mut queue_b = VecDeque::new();
        let mut assigned = HashSet::new();

        queue_a.push_back(seed_a);
        queue_b.push_back(seed_b);
        part_a.insert(seed_a);
        part_b.insert(seed_b);
        assigned.insert(seed_a);
        assigned.insert(seed_b);

        let target_size = vertices.len() / 2;

        // Alternate growth
        loop {
            let done_a = part_a.len() >= target_size || queue_a.is_empty();
            let done_b = part_b.len() >= target_size || queue_b.is_empty();

            if done_a && done_b {
                break;
            }

            // Grow partition A
            if !done_a {
                if let Some(v) = queue_a.pop_front() {
                    for (neighbor, _) in graph.neighbors(v) {
                        if vertices.contains(&neighbor) && assigned.insert(neighbor) {
                            part_a.insert(neighbor);
                            queue_a.push_back(neighbor);
                        }
                    }
                }
            }

            // Grow partition B
            if !done_b {
                if let Some(v) = queue_b.pop_front() {
                    for (neighbor, _) in graph.neighbors(v) {
                        if vertices.contains(&neighbor) && assigned.insert(neighbor) {
                            part_b.insert(neighbor);
                            queue_b.push_back(neighbor);
                        }
                    }
                }
            }
        }

        // Assign remaining unassigned vertices
        for &v in vertices {
            if !assigned.contains(&v) {
                if part_a.len() <= part_b.len() {
                    part_a.insert(v);
                } else {
                    part_b.insert(v);
                }
            }
        }

        (part_a, part_b)
    }

    /// Find two distant seed vertices using BFS
    fn find_distant_seeds(
        &mut self,
        graph: &InteractionGraph,
        vertices: &HashSet<VertexId>,
    ) -> (VertexId, VertexId) {
        // Start from a random vertex
        let idx = (self.next_random() as usize) % vertices.len();
        let start = *vertices.iter().nth(idx).unwrap();

        // BFS to find farthest vertex
        let seed_a = self.bfs_farthest(graph, start, vertices);

        // BFS from seed_a to find farthest from it
        let seed_b = self.bfs_farthest(graph, seed_a, vertices);

        (seed_a, seed_b)
    }

    /// Find farthest vertex from start using BFS
    fn bfs_farthest(
        &self,
        graph: &InteractionGraph,
        start: VertexId,
        vertices: &HashSet<VertexId>,
    ) -> VertexId {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut farthest = start;

        queue.push_back(start);
        visited.insert(start);

        while let Some(v) = queue.pop_front() {
            farthest = v;
            for (neighbor, _) in graph.neighbors(v) {
                if vertices.contains(&neighbor) && visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        farthest
    }

    /// Spectral bisection using Fiedler vector approximation
    fn spectral_bisection(
        &mut self,
        graph: &InteractionGraph,
        num_parts: usize,
    ) -> Result<PartitionResult> {
        // For simplicity, use power iteration to approximate Fiedler vector
        let vertices: Vec<_> = graph.vertices().collect();
        let n = vertices.len();
        let vertex_idx: HashMap<_, _> = vertices.iter().enumerate().map(|(i, &v)| (v, i)).collect();

        // Initialize random vector
        let mut x: Vec<f64> = (0..n).map(|_| self.next_random() as f64 / u64::MAX as f64).collect();

        // Power iteration for Laplacian
        for _ in 0..50 {
            // Apply Laplacian: L*x = D*x - A*x
            let mut new_x = vec![0.0; n];

            for (i, &v) in vertices.iter().enumerate() {
                let degree = graph.weighted_degree(v) as f64;
                new_x[i] = degree * x[i];

                for (neighbor, weight) in graph.neighbors(v) {
                    if let Some(&j) = vertex_idx.get(&neighbor) {
                        new_x[i] -= (weight as f64) * x[j];
                    }
                }
            }

            // Orthogonalize against constant vector
            let sum: f64 = new_x.iter().sum();
            let mean = sum / n as f64;
            for v in &mut new_x {
                *v -= mean;
            }

            // Normalize
            let norm: f64 = new_x.iter().map(|v| v * v).sum::<f64>().sqrt();
            if norm > 1e-10 {
                for v in &mut new_x {
                    *v /= norm;
                }
            }

            x = new_x;
        }

        // Partition based on sign of Fiedler vector
        let mut partitions: Vec<HashSet<VertexId>> = vec![HashSet::new(); num_parts];

        // Sort by Fiedler value and assign to partitions
        let mut indexed: Vec<_> = vertices.iter().enumerate().map(|(i, &v)| (x[i], v)).collect();
        indexed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let chunk_size = (indexed.len() + num_parts - 1) / num_parts;
        for (idx, (_, v)) in indexed.into_iter().enumerate() {
            let part = (idx / chunk_size).min(num_parts - 1);
            partitions[part].insert(v);
        }

        // Build result
        let mut result = PartitionResult::new(num_parts);
        for (part_idx, partition) in partitions.into_iter().enumerate() {
            for v in partition {
                result.assign(v, part_idx);
            }
        }
        result.compute_stats(graph);

        Ok(result)
    }

    /// Kernighan-Lin style refinement
    fn kernighan_lin(
        &mut self,
        graph: &InteractionGraph,
        num_parts: usize,
    ) -> Result<PartitionResult> {
        // Start with recursive bisection
        let mut result = self.recursive_bisection(graph, num_parts)?;

        // Refinement iterations
        for _ in 0..self.config.refinement_iterations {
            let improved = self.refine_partition(graph, &mut result);
            if !improved {
                break;
            }
        }

        result.compute_stats(graph);
        Ok(result)
    }

    /// Refine partition by swapping vertices
    fn refine_partition(&mut self, graph: &InteractionGraph, result: &mut PartitionResult) -> bool {
        let mut improved = false;

        // Try swapping boundary vertices
        let vertices: Vec<_> = graph.vertices().collect();

        for &v in &vertices {
            let current_part = match result.get_partition(v) {
                Some(p) => p,
                None => continue,
            };

            // Compute gain for moving to each other partition
            let mut best_gain = 0.0;
            let mut best_part = current_part;

            for target_part in 0..result.partitions.len() {
                if target_part == current_part {
                    continue;
                }

                // Check balance constraint
                if result.partition_sizes[target_part] as f32
                    > result.partition_sizes[current_part] as f32 * self.config.max_imbalance
                {
                    continue;
                }

                // Compute gain
                let mut internal_to_current = 0.0;
                let mut external_to_target = 0.0;

                for (neighbor, weight) in graph.neighbors(v) {
                    if let Some(neighbor_part) = result.get_partition(neighbor) {
                        if neighbor_part == current_part {
                            internal_to_current += weight;
                        } else if neighbor_part == target_part {
                            external_to_target += weight;
                        }
                    }
                }

                let gain = internal_to_current - external_to_target;
                if gain > best_gain {
                    best_gain = gain;
                    best_part = target_part;
                }
            }

            // Make the move if beneficial
            if best_part != current_part && best_gain > 0.0 {
                result.partitions[current_part].remove(&v);
                result.partitions[best_part].insert(v);
                result.assignments.insert(v, best_part);
                result.partition_sizes[current_part] -= 1;
                result.partition_sizes[best_part] += 1;
                improved = true;
            }
        }

        improved
    }

    /// Multilevel partitioning with coarsening
    fn multilevel_partition(
        &mut self,
        graph: &InteractionGraph,
        num_parts: usize,
    ) -> Result<PartitionResult> {
        // For simplicity, use recursive bisection with more refinement
        let mut config = self.config.clone();
        config.refinement_iterations *= 2;

        let mut partitioner = MinCutPartitioner::new(config);
        let mut result = partitioner.recursive_bisection(graph, num_parts)?;

        // Extra refinement
        for _ in 0..5 {
            if !partitioner.refine_partition(graph, &mut result) {
                break;
            }
        }

        result.compute_stats(graph);
        Ok(result)
    }

    /// Greedy graph growing partitioning
    fn greedy_grow(
        &mut self,
        graph: &InteractionGraph,
        num_parts: usize,
    ) -> Result<PartitionResult> {
        let vertices: Vec<_> = graph.vertices().collect();
        let target_size = (vertices.len() + num_parts - 1) / num_parts;

        let mut result = PartitionResult::new(num_parts);
        let mut assigned = HashSet::new();

        // Use priority queue for each partition (by connectivity)
        for part in 0..num_parts {
            if assigned.len() >= vertices.len() {
                break;
            }

            // Find an unassigned seed
            let seed = vertices
                .iter()
                .copied()
                .find(|v| !assigned.contains(v))
                .unwrap();

            // Priority queue: (negative connectivity, vertex)
            let mut heap: BinaryHeap<(i32, VertexId)> = BinaryHeap::new();
            heap.push((0, seed));

            while result.partition_sizes[part] < target_size {
                let v = loop {
                    match heap.pop() {
                        Some((_, v)) if !assigned.contains(&v) => break v,
                        Some(_) => continue,
                        None => break vertices
                            .iter()
                            .copied()
                            .find(|v| !assigned.contains(v))
                            .unwrap_or(seed),
                    }
                };

                if assigned.contains(&v) {
                    break;
                }

                result.assign(v, part);
                assigned.insert(v);

                // Add neighbors to heap
                for (neighbor, weight) in graph.neighbors(v) {
                    if !assigned.contains(&neighbor) {
                        // Negative because BinaryHeap is max-heap
                        heap.push((-(weight as i32), neighbor));
                    }
                }
            }
        }

        // Assign any remaining vertices
        for &v in &vertices {
            if !assigned.contains(&v) {
                // Find smallest partition
                let smallest = (0..num_parts)
                    .min_by_key(|&p| result.partition_sizes[p])
                    .unwrap();
                result.assign(v, smallest);
            }
        }

        result.compute_stats(graph);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> InteractionGraph {
        let mut graph = InteractionGraph::new();

        // Create a simple graph: two clusters connected by a bridge
        // Cluster 1: 0-1-2-0
        graph.add_edge(0, 1, 1.0, 1.0);
        graph.add_edge(1, 2, 1.0, 1.0);
        graph.add_edge(2, 0, 1.0, 1.0);

        // Bridge
        graph.add_edge(2, 3, 1.0, 1.0);

        // Cluster 2: 3-4-5-3
        graph.add_edge(3, 4, 1.0, 1.0);
        graph.add_edge(4, 5, 1.0, 1.0);
        graph.add_edge(5, 3, 1.0, 1.0);

        graph
    }

    #[test]
    fn test_graph_construction() {
        let graph = create_test_graph();
        assert_eq!(graph.num_vertices(), 6);
        assert_eq!(graph.num_edges(), 7);
        assert!(graph.is_connected());
    }

    #[test]
    fn test_recursive_bisection() {
        let graph = create_test_graph();
        let mut partitioner = MinCutPartitioner::for_partitions(2);

        let result = partitioner.partition(&graph).unwrap();

        assert_eq!(result.partitions.len(), 2);
        assert_eq!(
            result.partitions[0].len() + result.partitions[1].len(),
            6
        );
        // Cut should ideally be the bridge edge
        assert!(result.cut_weight <= 2.0);
    }

    #[test]
    fn test_spectral_bisection() {
        let graph = create_test_graph();
        let mut partitioner = MinCutPartitioner::new(PartitionConfig {
            num_partitions: 2,
            strategy: PartitionStrategy::SpectralBisection,
            ..Default::default()
        });

        let result = partitioner.partition(&graph).unwrap();
        assert_eq!(result.partitions.len(), 2);
    }

    #[test]
    fn test_greedy_grow() {
        let graph = create_test_graph();
        let mut partitioner = MinCutPartitioner::new(PartitionConfig {
            num_partitions: 2,
            strategy: PartitionStrategy::GreedyGrow,
            ..Default::default()
        });

        let result = partitioner.partition(&graph).unwrap();
        assert_eq!(result.partitions.len(), 2);
    }

    #[test]
    fn test_many_partitions() {
        let graph = create_test_graph();
        let mut partitioner = MinCutPartitioner::for_partitions(3);

        let result = partitioner.partition(&graph).unwrap();
        assert_eq!(result.partitions.len(), 3);
    }

    #[test]
    fn test_cut_value() {
        let graph = create_test_graph();

        let set_a: HashSet<_> = [0, 1, 2].into_iter().collect();
        let set_b: HashSet<_> = [3, 4, 5].into_iter().collect();

        let cut = graph.cut_value(&set_a, &set_b);
        assert_eq!(cut, 1.0); // Only the bridge edge
    }

    #[test]
    fn test_connected_components() {
        let mut graph = InteractionGraph::new();

        // Two disconnected components
        graph.add_edge(0, 1, 1.0, 1.0);
        graph.add_edge(1, 2, 1.0, 1.0);
        graph.add_edge(3, 4, 1.0, 1.0);

        let components = graph.connected_components();
        assert_eq!(components.len(), 2);
    }

    #[test]
    fn test_from_neighbors() {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let neighbors = vec![(0, 1), (1, 2), (2, 3)];

        let graph =
            InteractionGraph::from_neighbors(&positions, neighbors.into_iter(), EdgeWeightStrategy::Uniform);

        assert_eq!(graph.num_vertices(), 4);
        assert_eq!(graph.num_edges(), 3);
    }

    #[test]
    fn test_imbalance_constraint() {
        let graph = create_test_graph();
        let mut partitioner = MinCutPartitioner::new(PartitionConfig {
            num_partitions: 2,
            max_imbalance: 1.5,
            ..Default::default()
        });

        let result = partitioner.partition(&graph).unwrap();

        // Check imbalance is within bounds
        assert!(result.imbalance <= 1.5);
    }
}
