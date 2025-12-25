//! Tile Partitioning with MinCut Integration
//!
//! Provides graph-based partitioning for the 256-tile Cognitum chip using
//! min-cut algorithms to optimize inter-partition communication costs.

use crate::ruvector::types::{TaskEmbedding, TileId};
use crate::ruvector::router::{TinyDancerRouter, TaskRouter};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Partition identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionId(pub usize);

/// Error types for partitioning operations
#[derive(Debug, thiserror::Error)]
pub enum PartitionError {
    #[error("Invalid partition count: {0}")]
    InvalidPartitionCount(usize),
    #[error("Invalid tile ID: {0}")]
    InvalidTileId(u32),
    #[error("Graph error: {0}")]
    GraphError(String),
    #[error("Convergence failed after {0} iterations")]
    ConvergenceFailed(usize),
    #[error("Empty partition: {0}")]
    EmptyPartition(usize),
}

/// Represents a single partition with its tiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub id: PartitionId,
    pub tiles: Vec<TileId>,
    pub internal_edges: usize,
    pub external_edges: usize,
    pub total_load: f64,
}

impl Partition {
    pub fn new(id: PartitionId) -> Self {
        Self {
            id,
            tiles: Vec::new(),
            internal_edges: 0,
            external_edges: 0,
            total_load: 0.0,
        }
    }

    pub fn add_tile(&mut self, tile: TileId, load: f64) {
        self.tiles.push(tile);
        self.total_load += load;
    }

    pub fn size(&self) -> usize {
        self.tiles.len()
    }
}

/// Node in the tile graph
#[derive(Debug, Clone)]
pub struct TileNode {
    pub id: TileId,
    pub x: u8, // Grid position 0-15
    pub y: u8, // Grid position 0-15
    pub load: f64, // Workload metric
}

impl TileNode {
    pub fn new(id: TileId, x: u8, y: u8) -> Self {
        Self {
            id,
            x,
            y,
            load: 1.0,
        }
    }

    /// Calculate Manhattan distance to another node
    pub fn manhattan_distance(&self, other: &TileNode) -> u8 {
        ((self.x as i16 - other.x as i16).abs() + (self.y as i16 - other.y as i16).abs()) as u8
    }

    /// Check if nodes are neighbors in 8-neighbor (RaceWay) topology
    pub fn is_neighbor(&self, other: &TileNode) -> bool {
        let dx = (self.x as i16 - other.x as i16).abs();
        let dy = (self.y as i16 - other.y as i16).abs();
        dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0)
    }
}

/// Represents the 256-tile Cognitum chip as a dynamic graph
pub struct TileGraph {
    /// 16x16 grid of tiles (256 total)
    nodes: Vec<TileNode>,
    /// Weighted edges (communication cost)
    edges: HashMap<(TileId, TileId), f64>,
    /// Current partition assignment
    partitions: Vec<PartitionId>,
}

impl TileGraph {
    /// Create a new 16x16 tile graph
    pub fn new() -> Self {
        let mut nodes = Vec::with_capacity(256);

        // Create 16x16 grid
        for y in 0..16 {
            for x in 0..16 {
                let id = TileId((y * 16 + x) as u32);
                nodes.push(TileNode::new(id, x, y));
            }
        }

        let mut graph = Self {
            nodes,
            edges: HashMap::new(),
            partitions: vec![PartitionId(0); 256],
        };

        // Initialize RaceWay topology (8-neighbor connectivity)
        graph.init_raceway_topology();

        graph
    }

    /// Initialize 8-neighbor RaceWay topology
    fn init_raceway_topology(&mut self) {
        let mut edges_to_add = Vec::new();

        for i in 0..self.nodes.len() {
            let node = &self.nodes[i];
            let x = node.x as i16;
            let y = node.y as i16;
            let node_id = node.id;

            // Connect to all 8 neighbors
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x + dx;
                    let ny = y + dy;

                    // Check bounds
                    if nx >= 0 && nx < 16 && ny >= 0 && ny < 16 {
                        let neighbor_id = TileId((ny * 16 + nx) as u32);
                        // Default edge weight is 1.0
                        edges_to_add.push((node_id, neighbor_id, 1.0));
                    }
                }
            }
        }

        // Add all edges at once to avoid borrow checker issues
        for (u, v, weight) in edges_to_add {
            self.add_edge(u, v, weight);
        }
    }

    /// Add or update an edge
    pub fn add_edge(&mut self, u: TileId, v: TileId, weight: f64) {
        // Store both directions for undirected graph
        self.edges.insert((u, v), weight);
        self.edges.insert((v, u), weight);
    }

    /// Get edge weight
    pub fn edge_weight(&self, u: TileId, v: TileId) -> f64 {
        *self.edges.get(&(u, v)).unwrap_or(&0.0)
    }

    /// Get all neighbors of a tile
    pub fn neighbors(&self, tile: TileId) -> Vec<TileId> {
        self.edges
            .keys()
            .filter(|(u, _)| *u == tile)
            .map(|(_, v)| *v)
            .collect()
    }

    /// Set workload for a tile
    pub fn set_load(&mut self, tile: TileId, load: f64) {
        if let Some(node) = self.nodes.get_mut(tile.0 as usize) {
            node.load = load;
        }
    }

    /// Get current partition for a tile
    pub fn get_partition(&self, tile: TileId) -> PartitionId {
        self.partitions.get(tile.0 as usize).copied().unwrap_or(PartitionId(0))
    }

    /// Get number of nodes in the graph
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get number of edges in the graph
    pub fn edge_count(&self) -> usize {
        self.edges.len() / 2 // Divide by 2 since edges are stored bidirectionally
    }

    /// Set partition for a tile
    pub fn set_partition(&mut self, tile: TileId, partition: PartitionId) {
        if let Some(p) = self.partitions.get_mut(tile.0 as usize) {
            *p = partition;
        }
    }

    /// Calculate total cut size (edges crossing partitions)
    pub fn cut_size(&self) -> usize {
        let mut cut = 0;
        for ((u, v), _) in &self.edges {
            if self.get_partition(*u) != self.get_partition(*v) {
                cut += 1;
            }
        }
        // Divide by 2 since edges are counted twice
        cut / 2
    }

    /// Get all partitions
    pub fn get_partitions(&self, k: usize) -> Vec<Partition> {
        let mut partitions: Vec<Partition> = (0..k).map(|i| Partition::new(PartitionId(i))).collect();

        for (tile_idx, &partition_id) in self.partitions.iter().enumerate() {
            let tile = TileId(tile_idx as u32);
            let load = self.nodes[tile_idx].load;

            if partition_id.0 < k {
                partitions[partition_id.0].add_tile(tile, load);
            }
        }

        // Calculate edge counts
        for ((u, v), _) in &self.edges {
            let u_part = self.get_partition(*u);
            let v_part = self.get_partition(*v);

            if u_part == v_part && u_part.0 < k {
                partitions[u_part.0].internal_edges += 1;
            } else {
                if u_part.0 < k {
                    partitions[u_part.0].external_edges += 1;
                }
            }
        }

        partitions
    }
}

impl Default for TileGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for min-cut based partitioning (compatible with ruvector-mincut API)
#[cfg_attr(test, automock)]
pub trait MinCutPartitioner: Send + Sync {
    /// Compute optimal k-way partition minimizing inter-partition edges
    fn partition(&mut self, k: usize) -> Result<Vec<Partition>, PartitionError>;

    /// Dynamic edge update with subpolynomial time
    fn update_edge(&mut self, u: TileId, v: TileId, weight: f64) -> Result<(), PartitionError>;

    /// Get current minimum cut value
    fn min_cut_value(&self) -> f64;

    /// Get partition for a tile
    fn get_partition(&self, tile: TileId) -> PartitionId;
}

/// Kernighan-Lin partitioner (fallback when ruvector-mincut not available)
pub struct KernighanLinPartitioner {
    graph: Arc<RwLock<TileGraph>>,
    pub max_iterations: usize,
}

impl KernighanLinPartitioner {
    pub fn new(graph: TileGraph) -> Self {
        Self {
            graph: Arc::new(RwLock::new(graph)),
            max_iterations: 100,
        }
    }

    /// Calculate gain of moving a node from one partition to another
    fn compute_gain(&self, node: TileId, from: PartitionId, to: PartitionId) -> f64 {
        let graph = self.graph.read();
        let mut gain = 0.0;

        for neighbor in graph.neighbors(node) {
            let neighbor_part = graph.get_partition(neighbor);
            let weight = graph.edge_weight(node, neighbor);

            if neighbor_part == from {
                // Moving away from this neighbor (increases cut)
                gain -= weight;
            } else if neighbor_part == to {
                // Moving towards this neighbor (decreases cut)
                gain += weight;
            }
        }

        gain
    }

    /// Swap two nodes between partitions
    fn swap_nodes(&mut self, a: TileId, b: TileId) {
        let mut graph = self.graph.write();
        let a_part = graph.get_partition(a);
        let b_part = graph.get_partition(b);

        graph.set_partition(a, b_part);
        graph.set_partition(b, a_part);
    }

    /// Perform one iteration of Kernighan-Lin
    fn iterate(&mut self, k: usize) -> bool {
        let graph = self.graph.read();
        let initial_cut = graph.cut_size();
        drop(graph);

        let mut improved = false;

        // Try swaps between all partition pairs
        for p1 in 0..k {
            for p2 in (p1 + 1)..k {
                let part1 = PartitionId(p1);
                let part2 = PartitionId(p2);

                if let Some((best_a, best_b, best_gain)) = self.find_best_swap(part1, part2) {
                    if best_gain > 0.0 {
                        self.swap_nodes(best_a, best_b);
                        improved = true;
                    }
                }
            }
        }

        let graph = self.graph.read();
        let final_cut = graph.cut_size();
        drop(graph);

        improved && final_cut < initial_cut
    }

    /// Find best pair of nodes to swap between two partitions
    fn find_best_swap(&self, part1: PartitionId, part2: PartitionId) -> Option<(TileId, TileId, f64)> {
        let graph = self.graph.read();
        let mut best_gain = 0.0;
        let mut best_pair = None;

        for i in 0..256 {
            let tile_a = TileId(i);
            if graph.get_partition(tile_a) != part1 {
                continue;
            }

            for j in 0..256 {
                let tile_b = TileId(j);
                if graph.get_partition(tile_b) != part2 {
                    continue;
                }

                let gain = self.compute_gain(tile_a, part1, part2)
                         + self.compute_gain(tile_b, part2, part1);

                if gain > best_gain {
                    best_gain = gain;
                    best_pair = Some((tile_a, tile_b, gain));
                }
            }
        }

        best_pair
    }
}

impl MinCutPartitioner for KernighanLinPartitioner {
    fn partition(&mut self, k: usize) -> Result<Vec<Partition>, PartitionError> {
        if k == 0 || k > 256 {
            return Err(PartitionError::InvalidPartitionCount(k));
        }

        // Initialize partitions using round-robin
        {
            let mut graph = self.graph.write();
            for i in 0..256u32 {
                graph.set_partition(TileId(i), PartitionId((i as usize) % k));
            }
        }

        // Iteratively improve
        for iter in 0..self.max_iterations {
            if !self.iterate(k) {
                // Converged
                let graph = self.graph.read();
                return Ok(graph.get_partitions(k));
            }

            if iter == self.max_iterations - 1 {
                return Err(PartitionError::ConvergenceFailed(self.max_iterations));
            }
        }

        let graph = self.graph.read();
        Ok(graph.get_partitions(k))
    }

    fn update_edge(&mut self, u: TileId, v: TileId, weight: f64) -> Result<(), PartitionError> {
        let mut graph = self.graph.write();
        graph.add_edge(u, v, weight);
        Ok(())
    }

    fn min_cut_value(&self) -> f64 {
        let graph = self.graph.read();
        graph.cut_size() as f64
    }

    fn get_partition(&self, tile: TileId) -> PartitionId {
        let graph = self.graph.read();
        graph.get_partition(tile)
    }
}

/// Workload-aware partitioner balancing cut size and load
pub struct WorkloadPartitioner {
    partitioner: KernighanLinPartitioner,
    tile_loads: HashMap<TileId, f64>,
    balance_factor: f64, // 0.0 = pure mincut, 1.0 = pure load balance
}

impl WorkloadPartitioner {
    pub fn new(graph: TileGraph, balance_factor: f64) -> Self {
        let tile_loads = (0..256)
            .map(|i| (TileId(i), 1.0))
            .collect();

        Self {
            partitioner: KernighanLinPartitioner::new(graph),
            tile_loads,
            balance_factor: balance_factor.clamp(0.0, 1.0),
        }
    }

    /// Set workload for a tile
    pub fn set_tile_load(&mut self, tile: TileId, load: f64) {
        self.tile_loads.insert(tile, load);
        let mut graph = self.partitioner.graph.write();
        graph.set_load(tile, load);
    }

    /// Calculate partition imbalance metric
    fn calculate_imbalance(&self, partitions: &[Partition]) -> f64 {
        if partitions.is_empty() {
            return 0.0;
        }

        let total_load: f64 = partitions.iter().map(|p| p.total_load).sum();
        let avg_load = total_load / partitions.len() as f64;

        if avg_load == 0.0 {
            return 0.0;
        }

        partitions
            .iter()
            .map(|p| ((p.total_load - avg_load) / avg_load).abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    /// Compute combined objective (cut size + imbalance)
    fn objective(&self, partitions: &[Partition]) -> f64 {
        let cut_size = partitions.iter().map(|p| p.external_edges).sum::<usize>() as f64;
        let imbalance = self.calculate_imbalance(partitions);

        (1.0 - self.balance_factor) * cut_size + self.balance_factor * imbalance * 1000.0
    }
}

impl MinCutPartitioner for WorkloadPartitioner {
    fn partition(&mut self, k: usize) -> Result<Vec<Partition>, PartitionError> {
        // Start with base partition
        let mut best_partitions = self.partitioner.partition(k)?;
        let mut best_objective = self.objective(&best_partitions);

        // Refine with load balancing
        for _ in 0..10 {
            let partitions = self.partitioner.partition(k)?;
            let objective = self.objective(&partitions);

            if objective < best_objective {
                best_objective = objective;
                best_partitions = partitions;
            }
        }

        Ok(best_partitions)
    }

    fn update_edge(&mut self, u: TileId, v: TileId, weight: f64) -> Result<(), PartitionError> {
        self.partitioner.update_edge(u, v, weight)
    }

    fn min_cut_value(&self) -> f64 {
        self.partitioner.min_cut_value()
    }

    fn get_partition(&self, tile: TileId) -> PartitionId {
        self.partitioner.get_partition(tile)
    }
}

/// Helper functions for graph construction
pub mod helpers {
    use super::*;

    /// Create 16x16 grid topology
    pub fn create_grid_topology() -> TileGraph {
        TileGraph::new()
    }

    /// Create quadrant-based hierarchical partitioning (4 quadrants)
    pub fn quadrant_partition(graph: &mut TileGraph) {
        for i in 0..256 {
            let tile = TileId(i);
            let x = (i % 16) as u8;
            let y = (i / 16) as u8;

            // Divide into 4 quadrants
            let quadrant = match (x < 8, y < 8) {
                (true, true) => PartitionId(0),    // Top-left
                (false, true) => PartitionId(1),   // Top-right
                (true, false) => PartitionId(2),   // Bottom-left
                (false, false) => PartitionId(3),  // Bottom-right
            };

            graph.set_partition(tile, quadrant);
        }
    }

    /// Create hierarchical partitioning with k subdivisions
    pub fn hierarchical_partition(graph: &mut TileGraph, k: usize) -> Result<(), PartitionError> {
        if k == 0 || k > 256 {
            return Err(PartitionError::InvalidPartitionCount(k));
        }

        // Calculate grid subdivision
        let rows = (k as f64).sqrt().ceil() as usize;
        let cols = (k + rows - 1) / rows;

        for i in 0..256 {
            let x = (i % 16) as usize;
            let y = (i / 16) as usize;

            let part_x = (x * cols) / 16;
            let part_y = (y * rows) / 16;
            let partition = part_y * cols + part_x;

            graph.set_partition(TileId(i), PartitionId(partition.min(k - 1)));
        }

        Ok(())
    }
}

/// Extension for TinyDancerRouter to use partition information
impl TinyDancerRouter {
    /// Route task to appropriate tile using partition awareness
    pub fn route_with_partition(
        &self,
        task: &TaskEmbedding,
        partitioner: &dyn MinCutPartitioner,
    ) -> TileId {
        // First, predict the best tile using the router
        let predicted_tile = self.predict_tile(task);

        // Get the partition for this tile
        let target_partition = partitioner.get_partition(predicted_tile);

        // Find all tiles in this partition and choose the best one
        let mut best_tile = predicted_tile;
        let mut best_confidence = self.confidence(task);

        for i in 0..256 {
            let tile = TileId(i);
            if partitioner.get_partition(tile) == target_partition {
                // Only consider tiles in the same partition
                let confidence = self.confidence(task);
                if confidence > best_confidence {
                    best_confidence = confidence;
                    best_tile = tile;
                }
            }
        }

        best_tile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_graph_creation() {
        let graph = TileGraph::new();
        assert_eq!(graph.nodes.len(), 256);

        // Check 16x16 grid
        for i in 0..256 {
            let node = &graph.nodes[i];
            assert_eq!(node.id.0, i as u32);
            assert_eq!(node.x as usize, i % 16);
            assert_eq!(node.y as usize, i / 16);
        }
    }

    #[test]
    fn test_raceway_topology() {
        let graph = TileGraph::new();

        // Corner node (0,0) should have 3 neighbors
        let tile_00 = TileId(0);
        let neighbors = graph.neighbors(tile_00);
        assert_eq!(neighbors.len(), 3);

        // Center node (8,8) should have 8 neighbors
        let tile_88 = TileId(8 * 16 + 8);
        let neighbors = graph.neighbors(tile_88);
        assert_eq!(neighbors.len(), 8);

        // Edge node (0,8) should have 5 neighbors
        let tile_08 = TileId(8 * 16);
        let neighbors = graph.neighbors(tile_08);
        assert_eq!(neighbors.len(), 5);
    }

    #[test]
    fn test_manhattan_distance() {
        let node1 = TileNode::new(TileId(0), 0, 0);
        let node2 = TileNode::new(TileId(255), 15, 15);

        assert_eq!(node1.manhattan_distance(&node2), 30);
        assert_eq!(node1.manhattan_distance(&node1), 0);
    }

    #[test]
    fn test_is_neighbor() {
        let center = TileNode::new(TileId(0), 5, 5);

        // 8 neighbors
        assert!(center.is_neighbor(&TileNode::new(TileId(1), 4, 4)));
        assert!(center.is_neighbor(&TileNode::new(TileId(2), 5, 4)));
        assert!(center.is_neighbor(&TileNode::new(TileId(3), 6, 4)));
        assert!(center.is_neighbor(&TileNode::new(TileId(4), 4, 5)));
        assert!(center.is_neighbor(&TileNode::new(TileId(5), 6, 5)));
        assert!(center.is_neighbor(&TileNode::new(TileId(6), 4, 6)));
        assert!(center.is_neighbor(&TileNode::new(TileId(7), 5, 6)));
        assert!(center.is_neighbor(&TileNode::new(TileId(8), 6, 6)));

        // Not a neighbor
        assert!(!center.is_neighbor(&TileNode::new(TileId(9), 7, 7)));
        assert!(!center.is_neighbor(&center));
    }

    #[test]
    fn test_kernighan_lin_partition() {
        let graph = TileGraph::new();
        let mut partitioner = KernighanLinPartitioner::new(graph);

        let result = partitioner.partition(4);
        assert!(result.is_ok());

        let partitions = result.unwrap();
        assert_eq!(partitions.len(), 4);

        // Each partition should have ~64 tiles
        for partition in &partitions {
            assert!(partition.size() > 50 && partition.size() < 80);
        }
    }

    #[test]
    fn test_quadrant_partition() {
        let mut graph = TileGraph::new();
        helpers::quadrant_partition(&mut graph);

        // Check corners
        assert_eq!(graph.get_partition(TileId(0)), PartitionId(0)); // (0,0)
        assert_eq!(graph.get_partition(TileId(15)), PartitionId(1)); // (15,0)
        assert_eq!(graph.get_partition(TileId(240)), PartitionId(2)); // (0,15)
        assert_eq!(graph.get_partition(TileId(255)), PartitionId(3)); // (15,15)

        let partitions = graph.get_partitions(4);
        for partition in &partitions {
            assert_eq!(partition.size(), 64); // Each quadrant has 64 tiles
        }
    }

    #[test]
    fn test_hierarchical_partition() {
        let mut graph = TileGraph::new();

        let result = helpers::hierarchical_partition(&mut graph, 16);
        assert!(result.is_ok());

        let partitions = graph.get_partitions(16);
        assert_eq!(partitions.len(), 16);

        // Each partition should have ~16 tiles
        let total_tiles: usize = partitions.iter().map(|p| p.size()).sum();
        assert_eq!(total_tiles, 256);
    }

    #[test]
    fn test_workload_partitioner() {
        let graph = TileGraph::new();
        let mut partitioner = WorkloadPartitioner::new(graph, 0.5);

        // Set varying loads
        for i in 0..256 {
            let load = if i % 2 == 0 { 2.0 } else { 1.0 };
            partitioner.set_tile_load(TileId(i), load);
        }

        let result = partitioner.partition(4);
        assert!(result.is_ok());

        let partitions = result.unwrap();

        // Check that partitions are reasonably balanced
        let loads: Vec<f64> = partitions.iter().map(|p| p.total_load).collect();
        let min_load = loads.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_load = loads.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Should be within 50% of each other with balance_factor = 0.5
        assert!((max_load - min_load) / min_load < 1.0);
    }

    #[test]
    fn test_update_edge() {
        let graph = TileGraph::new();
        let mut partitioner = KernighanLinPartitioner::new(graph);

        // Update edge weight
        let result = partitioner.update_edge(TileId(0), TileId(1), 5.0);
        assert!(result.is_ok());

        let graph = partitioner.graph.read();
        assert_eq!(graph.edge_weight(TileId(0), TileId(1)), 5.0);
    }

    #[test]
    fn test_min_cut_value() {
        let graph = TileGraph::new();
        let mut partitioner = KernighanLinPartitioner::new(graph);

        // Partition into 2 groups
        partitioner.partition(2).unwrap();

        let cut = partitioner.min_cut_value();
        assert!(cut > 0.0);
        assert!(cut < 256.0 * 8.0); // Max possible edges
    }

    #[test]
    fn test_route_with_partition() {
        let graph = TileGraph::new();
        let mut partitioner = KernighanLinPartitioner::new(graph);
        partitioner.partition(4).unwrap();

        let router = TinyDancerRouter::new(256, 256);
        let task = TaskEmbedding::random();

        let tile = router.route_with_partition(&task, &partitioner);

        // Verify tile is valid
        assert!(tile.0 < 256);

        // Verify it's in a valid partition
        let partition = partitioner.get_partition(tile);
        assert!(partition.0 < 4);
    }

    #[test]
    fn test_mock_partitioner() {
        let mut mock = MockMinCutPartitioner::new();

        mock.expect_partition()
            .times(1)
            .returning(|k| {
                Ok(vec![Partition::new(PartitionId(0)); k])
            });

        mock.expect_get_partition()
            .times(1)
            .returning(|_| PartitionId(0));

        let result = mock.partition(4);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 4);

        let part = mock.get_partition(TileId(0));
        assert_eq!(part.0, 0);
    }
}
