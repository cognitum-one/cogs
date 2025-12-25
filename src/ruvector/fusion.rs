//! Vector-Graph Fusion for Hybrid Routing
//!
//! Implements hybrid routing that combines vector similarity with graph topology
//! for intelligent tile selection and auto-rebalancing.
//!
//! # Architecture
//!
//! - **FusedEdgeWeight**: Combines vector similarity (0.6) with graph strength (0.4)
//! - **RelationType**: RaceWay topology relationships with different strengths
//! - **BrittlenessMonitor**: Detects graph fragmentation and islanding risks
//! - **GraphOptimizer**: Auto-rebalancing actions based on min-cut analysis
//!
//! # Example
//!
//! ```rust
//! use cognitum::ruvector::fusion::*;
//!
//! let fusion = FusedEdgeWeight::new(0.6, 0.4);
//! let capacity = fusion.compute(
//!     0.85,  // vector similarity
//!     0.90,  // graph strength
//!     RelationType::DirectNeighbor
//! );
//! ```

use crate::ruvector::types::*;
use crate::ruvector::partitioning::PartitionId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Relation types in RaceWay topology with associated strength values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    /// Direct neighbor (adjacent tiles)
    DirectNeighbor,
    /// Diagonal neighbor (one step diagonal)
    DiagonalNeighbor,
    /// Same quadrant but not neighbors
    SameQuadrant,
    /// Different quadrants
    CrossQuadrant,
    /// Remote tiles (far distance)
    Remote,
}

impl RelationType {
    /// Get the strength multiplier for this relation type
    ///
    /// Strengths:
    /// - DirectNeighbor: 1.0 (strongest connection)
    /// - DiagonalNeighbor: 0.7
    /// - SameQuadrant: 0.5
    /// - CrossQuadrant: 0.3
    /// - Remote: 0.1 (weakest connection)
    pub fn strength(&self) -> f64 {
        match self {
            RelationType::DirectNeighbor => 1.0,
            RelationType::DiagonalNeighbor => 0.7,
            RelationType::SameQuadrant => 0.5,
            RelationType::CrossQuadrant => 0.3,
            RelationType::Remote => 0.1,
        }
    }

    /// Determine relation type based on tile positions
    ///
    /// Assumes 4x4 grid layout (16 tiles)
    pub fn from_positions(tile_a: TileId, tile_b: TileId) -> Self {
        let (x1, y1) = ((tile_a.0 % 4) as i32, (tile_a.0 / 4) as i32);
        let (x2, y2) = ((tile_b.0 % 4) as i32, (tile_b.0 / 4) as i32);

        let dx = (x1 - x2).abs();
        let dy = (y1 - y2).abs();
        let distance = dx + dy;

        // Direct neighbors (Manhattan distance = 1)
        if distance == 1 {
            return RelationType::DirectNeighbor;
        }

        // Diagonal neighbors (Chebyshev distance = 1)
        if dx <= 1 && dy <= 1 {
            return RelationType::DiagonalNeighbor;
        }

        // Same quadrant (both in top/bottom, left/right)
        let q1 = (x1 / 2, y1 / 2);
        let q2 = (x2 / 2, y2 / 2);
        if q1 == q2 {
            return RelationType::SameQuadrant;
        }

        // Cross quadrant
        if distance <= 4 {
            return RelationType::CrossQuadrant;
        }

        // Remote
        RelationType::Remote
    }
}

/// Fused edge weight combining vector similarity and graph structure
///
/// # Formula
///
/// `c(u,v) = w_v × similarity(u,v) + w_g × graph_strength(u,v)`
///
/// Default weights: w_v = 0.6, w_g = 0.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedEdgeWeight {
    /// Weight for vector similarity component (default: 0.6)
    pub vector_weight: f64,
    /// Weight for graph structure component (default: 0.4)
    pub graph_weight: f64,
}

impl FusedEdgeWeight {
    /// Create new fused edge weight with custom weights
    ///
    /// # Panics
    ///
    /// Panics if weights don't sum to approximately 1.0
    pub fn new(vector_weight: f64, graph_weight: f64) -> Self {
        let sum = vector_weight + graph_weight;
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Weights must sum to 1.0, got {}",
            sum
        );
        Self {
            vector_weight,
            graph_weight,
        }
    }

    /// Create with default weights (0.6 vector, 0.4 graph)
    pub fn default_weights() -> Self {
        Self {
            vector_weight: 0.6,
            graph_weight: 0.4,
        }
    }

    /// Compute fused capacity for an edge
    ///
    /// # Arguments
    ///
    /// * `similarity` - Vector cosine similarity in [0, 1]
    /// * `graph_strength` - Base graph edge strength in [0, 1]
    /// * `relation_type` - Type of relationship (affects final strength)
    ///
    /// # Returns
    ///
    /// Fused capacity value in [0, 1]
    pub fn compute(
        &self,
        similarity: f64,
        graph_strength: f64,
        relation_type: RelationType,
    ) -> f64 {
        let adjusted_graph_strength = graph_strength * relation_type.strength();
        let capacity = self.vector_weight * similarity
            + self.graph_weight * adjusted_graph_strength;

        capacity.clamp(0.0, 1.0)
    }

    /// Compute batch capacities for multiple edges
    pub fn compute_batch(
        &self,
        edges: &[(f64, f64, RelationType)],
    ) -> Vec<f64> {
        edges
            .iter()
            .map(|&(sim, strength, rel)| self.compute(sim, strength, rel))
            .collect()
    }
}

/// Health signal indicating graph connectivity status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthSignal {
    /// Graph is healthy (min_cut >= threshold)
    Healthy,
    /// Graph is degrading but still functional
    Warning,
    /// Critical fragmentation risk
    Critical,
    /// Graph has disconnected components
    Disconnected,
}

/// Monitors graph brittleness and fragmentation risks
///
/// Uses min-cut analysis to detect potential partitioning and islanding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrittlenessMonitor {
    /// Threshold for healthy operation
    pub min_cut_threshold: f64,
    /// Warning level threshold
    pub warning_threshold: f64,
    /// Critical level threshold
    pub critical_threshold: f64,
    /// History of min-cut values for trend analysis
    min_cut_history: Vec<f64>,
    /// Maximum history size
    max_history: usize,
}

impl BrittlenessMonitor {
    /// Create new brittleness monitor with thresholds
    pub fn new(
        min_cut_threshold: f64,
        warning_threshold: f64,
        critical_threshold: f64,
    ) -> Self {
        Self {
            min_cut_threshold,
            warning_threshold,
            critical_threshold,
            min_cut_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Create with default thresholds
    pub fn default_thresholds() -> Self {
        Self::new(
            0.5, // healthy: min_cut >= 0.5
            0.3, // warning: min_cut >= 0.3
            0.1, // critical: min_cut >= 0.1
        )
    }

    /// Assess current graph health based on min-cut value
    pub fn assess(&mut self, min_cut: f64) -> HealthSignal {
        // Update history
        self.min_cut_history.push(min_cut);
        if self.min_cut_history.len() > self.max_history {
            self.min_cut_history.remove(0);
        }

        // Assess current state
        if min_cut <= 0.0 {
            HealthSignal::Disconnected
        } else if min_cut < self.critical_threshold {
            HealthSignal::Critical
        } else if min_cut < self.warning_threshold {
            HealthSignal::Warning
        } else if min_cut < self.min_cut_threshold {
            HealthSignal::Warning
        } else {
            HealthSignal::Healthy
        }
    }

    /// Detect if graph is at risk of islanding (forming isolated components)
    ///
    /// Uses trend analysis on min-cut history
    pub fn detect_islanding_risk(&self, graph: &FusionGraph) -> bool {
        // Check for disconnected components
        if !graph.is_connected() {
            return true;
        }

        // Check trend in min-cut values
        if self.min_cut_history.len() >= 5 {
            let recent: Vec<f64> = self.min_cut_history
                .iter()
                .rev()
                .take(5)
                .copied()
                .collect();

            // Check if consistently decreasing
            let mut decreasing = true;
            for i in 1..recent.len() {
                if recent[i] >= recent[i - 1] {
                    decreasing = false;
                    break;
                }
            }

            if decreasing && recent[0] < self.warning_threshold {
                return true;
            }
        }

        // Check current min-cut
        if let Some(&last_cut) = self.min_cut_history.last() {
            if last_cut < self.critical_threshold {
                return true;
            }
        }

        false
    }

    /// Get trend direction (-1: declining, 0: stable, 1: improving)
    pub fn trend(&self) -> i8 {
        if self.min_cut_history.len() < 3 {
            return 0;
        }

        let recent: Vec<f64> = self.min_cut_history
            .iter()
            .rev()
            .take(3)
            .copied()
            .collect();

        if recent[0] > recent[2] * 1.1 {
            1 // Improving
        } else if recent[0] < recent[2] * 0.9 {
            -1 // Declining
        } else {
            0 // Stable
        }
    }
}

/// Optimizer actions for auto-rebalancing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizerAction {
    /// Reindex with new similarity threshold
    Reindex { new_threshold: f64 },
    /// Strengthen specific edge
    StrengthenEdge { u: TileId, v: TileId, delta: f64 },
    /// Weaken specific edge
    WeakenEdge { u: TileId, v: TileId, delta: f64 },
    /// Split partition into smaller parts
    SplitPartition { partition: PartitionId },
    /// Merge two partitions
    MergePartitions {
        a: PartitionId,
        b: PartitionId,
    },
}

/// Graph optimizer for auto-rebalancing
///
/// Monitors graph health and applies corrective actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOptimizer {
    /// Queued actions to apply
    pub actions: Vec<OptimizerAction>,
    /// Learning rate for gradual adjustments
    pub learning_rate: f64,
    /// Brittleness monitor
    monitor: BrittlenessMonitor,
}

impl GraphOptimizer {
    /// Create new optimizer with learning rate
    pub fn new(learning_rate: f64) -> Self {
        Self {
            actions: Vec::new(),
            learning_rate,
            monitor: BrittlenessMonitor::default_thresholds(),
        }
    }

    /// Analyze graph and generate optimization actions
    pub fn analyze_and_optimize(&mut self, graph: &FusionGraph, min_cut: f64) {
        let health = self.monitor.assess(min_cut);
        self.actions.clear();

        match health {
            HealthSignal::Healthy => {
                // No action needed
            }
            HealthSignal::Warning => {
                // Strengthen weak edges
                self.strengthen_weak_edges(graph);
            }
            HealthSignal::Critical => {
                // Aggressive strengthening
                self.strengthen_weak_edges(graph);
                // Consider reindexing with lower threshold
                self.actions.push(OptimizerAction::Reindex {
                    new_threshold: 0.7, // Lower threshold to include more edges
                });
            }
            HealthSignal::Disconnected => {
                // Emergency: reindex with very low threshold
                self.actions.push(OptimizerAction::Reindex {
                    new_threshold: 0.5,
                });
            }
        }

        // Check for islanding risk
        if self.monitor.detect_islanding_risk(graph) {
            self.prevent_islanding(graph);
        }
    }

    /// Strengthen weak edges in the graph
    fn strengthen_weak_edges(&mut self, graph: &FusionGraph) {
        for (&(u, v), &weight) in &graph.edges {
            if weight < 0.3 {
                // Weak edge
                let delta = self.learning_rate * (0.5 - weight);
                self.actions.push(OptimizerAction::StrengthenEdge {
                    u,
                    v,
                    delta,
                });
            }
        }
    }

    /// Prevent islanding by strengthening inter-partition edges
    fn prevent_islanding(&mut self, graph: &FusionGraph) {
        // Find potential bridges
        let bridges = graph.find_bridges();

        for (u, v) in bridges {
            self.actions.push(OptimizerAction::StrengthenEdge {
                u,
                v,
                delta: 0.2,
            });
        }
    }

    /// Apply actions to graph
    pub fn apply_actions(&self, graph: &mut FusionGraph) -> usize {
        let mut applied = 0;

        for action in &self.actions {
            match action {
                OptimizerAction::StrengthenEdge { u, v, delta } => {
                    if let Some(weight) = graph.edges.get_mut(&(*u, *v)) {
                        *weight = (*weight + delta).min(1.0);
                        applied += 1;
                    }
                }
                OptimizerAction::WeakenEdge { u, v, delta } => {
                    if let Some(weight) = graph.edges.get_mut(&(*u, *v)) {
                        *weight = (*weight - delta).max(0.0);
                        applied += 1;
                    }
                }
                OptimizerAction::Reindex { .. } => {
                    // Reindexing handled externally
                    applied += 1;
                }
                OptimizerAction::SplitPartition { .. } => {
                    // Partition operations handled externally
                    applied += 1;
                }
                OptimizerAction::MergePartitions { .. } => {
                    applied += 1;
                }
            }
        }

        applied
    }
}

/// Fusion graph structure for hybrid routing topology analysis
///
/// This is a lightweight graph representation optimized for fusion operations,
/// separate from the full TileGraph in partitioning module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionGraph {
    /// Adjacency list representation with fused edge weights
    pub edges: HashMap<(TileId, TileId), f64>,
    /// Set of all tiles in the graph
    pub tiles: HashSet<TileId>,
}

impl FusionGraph {
    /// Create new empty graph
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            tiles: HashSet::new(),
        }
    }

    /// Add tile to graph
    pub fn add_tile(&mut self, tile: TileId) {
        self.tiles.insert(tile);
    }

    /// Add weighted edge between tiles
    pub fn add_edge(&mut self, u: TileId, v: TileId, weight: f64) {
        self.tiles.insert(u);
        self.tiles.insert(v);
        self.edges.insert((u, v), weight);
        self.edges.insert((v, u), weight); // Undirected
    }

    /// Check if graph is connected using DFS
    pub fn is_connected(&self) -> bool {
        if self.tiles.is_empty() {
            return true;
        }

        let start = *self.tiles.iter().next().unwrap();
        let visited = self.dfs_reachable(start);

        visited.len() == self.tiles.len()
    }

    /// DFS to find reachable nodes
    fn dfs_reachable(&self, start: TileId) -> HashSet<TileId> {
        let mut visited = HashSet::new();
        let mut stack = vec![start];

        while let Some(node) = stack.pop() {
            if visited.insert(node) {
                // Add neighbors
                for &(u, v) in self.edges.keys() {
                    if u == node && !visited.contains(&v) {
                        stack.push(v);
                    }
                }
            }
        }

        visited
    }

    /// Find bridge edges (edges whose removal disconnects graph)
    pub fn find_bridges(&self) -> Vec<(TileId, TileId)> {
        let mut bridges = Vec::new();

        // Try removing each edge and check connectivity
        let edges: Vec<_> = self.edges.keys().copied().collect();
        for (u, v) in edges {
            if u.0 >= v.0 {
                continue; // Skip reverse edges
            }

            let mut temp_graph = self.clone();
            temp_graph.edges.remove(&(u, v));
            temp_graph.edges.remove(&(v, u));

            // Check if still connected
            if !temp_graph.is_connected() {
                bridges.push((u, v));
            }
        }

        bridges
    }

    /// Compute approximate min-cut using edge weights
    ///
    /// Returns the minimum total weight needed to disconnect the graph
    pub fn min_cut_approximation(&self) -> f64 {
        if self.tiles.len() < 2 {
            return f64::INFINITY;
        }

        let mut min_cut = f64::INFINITY;

        // For each tile, compute cut separating it from others
        for &tile in &self.tiles {
            let mut cut_weight = 0.0;

            for (&(u, v), &weight) in &self.edges {
                if u == tile && v != tile {
                    cut_weight += weight;
                }
            }

            min_cut = min_cut.min(cut_weight);
        }

        min_cut
    }
}

impl Default for FusionGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_type_strength() {
        assert_eq!(RelationType::DirectNeighbor.strength(), 1.0);
        assert_eq!(RelationType::DiagonalNeighbor.strength(), 0.7);
        assert_eq!(RelationType::SameQuadrant.strength(), 0.5);
        assert_eq!(RelationType::CrossQuadrant.strength(), 0.3);
        assert_eq!(RelationType::Remote.strength(), 0.1);
    }

    #[test]
    fn test_relation_from_positions() {
        // Direct neighbors (4x4 grid)
        assert_eq!(
            RelationType::from_positions(TileId(0), TileId(1)),
            RelationType::DirectNeighbor
        );
        assert_eq!(
            RelationType::from_positions(TileId(0), TileId(4)),
            RelationType::DirectNeighbor
        );

        // Diagonal neighbors
        assert_eq!(
            RelationType::from_positions(TileId(0), TileId(5)),
            RelationType::DiagonalNeighbor
        );

        // Cross quadrant (0 is (0,0) in quadrant (0,0), 3 is (3,0) in quadrant (1,0))
        assert_eq!(
            RelationType::from_positions(TileId(0), TileId(3)),
            RelationType::CrossQuadrant
        );
    }

    #[test]
    fn test_fused_edge_weight_compute() {
        let fusion = FusedEdgeWeight::default_weights();

        // High similarity, high graph strength, direct neighbor
        let capacity = fusion.compute(0.9, 0.9, RelationType::DirectNeighbor);
        assert!(capacity > 0.85);

        // Low similarity, high graph strength
        let capacity = fusion.compute(0.1, 0.9, RelationType::DirectNeighbor);
        assert!(capacity < 0.5);
    }

    #[test]
    fn test_brittleness_monitor_healthy() {
        let mut monitor = BrittlenessMonitor::default_thresholds();

        let signal = monitor.assess(0.8);
        assert_eq!(signal, HealthSignal::Healthy);
    }

    #[test]
    fn test_brittleness_monitor_warning() {
        let mut monitor = BrittlenessMonitor::default_thresholds();

        let signal = monitor.assess(0.4);
        assert_eq!(signal, HealthSignal::Warning);
    }

    #[test]
    fn test_brittleness_monitor_critical() {
        let mut monitor = BrittlenessMonitor::default_thresholds();

        let signal = monitor.assess(0.05);
        assert_eq!(signal, HealthSignal::Critical);
    }

    #[test]
    fn test_brittleness_monitor_disconnected() {
        let mut monitor = BrittlenessMonitor::default_thresholds();

        let signal = monitor.assess(0.0);
        assert_eq!(signal, HealthSignal::Disconnected);
    }

    #[test]
    fn test_graph_connectivity() {
        let mut graph = FusionGraph::new();

        // Add connected component
        graph.add_edge(TileId(0), TileId(1), 1.0);
        graph.add_edge(TileId(1), TileId(2), 1.0);

        assert!(graph.is_connected());

        // Add disconnected tile
        graph.add_tile(TileId(10));

        assert!(!graph.is_connected());
    }

    #[test]
    fn test_find_bridges() {
        let mut graph = FusionGraph::new();

        // Create graph with bridge
        graph.add_edge(TileId(0), TileId(1), 1.0);
        graph.add_edge(TileId(1), TileId(2), 1.0); // Bridge
        graph.add_edge(TileId(2), TileId(3), 1.0);

        let bridges = graph.find_bridges();
        assert!(!bridges.is_empty());
    }

    #[test]
    fn test_min_cut_approximation() {
        let mut graph = FusionGraph::new();

        // Star topology (center is min-cut)
        // Node 0 connects to 1, 2, 3 with weight 0.5 each
        graph.add_edge(TileId(0), TileId(1), 0.5);
        graph.add_edge(TileId(0), TileId(2), 0.5);
        graph.add_edge(TileId(0), TileId(3), 0.5);

        let min_cut = graph.min_cut_approximation();
        // The min cut for any leaf node is 0.5 (single edge)
        // The cut for center node is 1.5 (three edges)
        // So minimum is 0.5
        assert!((min_cut - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_optimizer_strengthens_weak_edges() {
        let mut optimizer = GraphOptimizer::new(0.1);
        let mut graph = FusionGraph::new();

        graph.add_edge(TileId(0), TileId(1), 0.2); // Weak edge

        optimizer.analyze_and_optimize(&graph, 0.2);

        // Should have action to strengthen
        assert!(!optimizer.actions.is_empty());
        assert!(matches!(
            optimizer.actions[0],
            OptimizerAction::StrengthenEdge { .. }
        ));
    }

    #[test]
    fn test_optimizer_reindex_on_critical() {
        let mut optimizer = GraphOptimizer::new(0.1);
        let graph = FusionGraph::new();

        optimizer.analyze_and_optimize(&graph, 0.05); // Critical

        // Should have reindex action
        assert!(optimizer
            .actions
            .iter()
            .any(|a| matches!(a, OptimizerAction::Reindex { .. })));
    }

    #[test]
    fn test_fused_weight_batch_compute() {
        let fusion = FusedEdgeWeight::default_weights();

        let edges = vec![
            (0.9, 0.8, RelationType::DirectNeighbor),
            (0.5, 0.6, RelationType::DiagonalNeighbor),
            (0.3, 0.4, RelationType::Remote),
        ];

        let capacities = fusion.compute_batch(&edges);

        assert_eq!(capacities.len(), 3);
        assert!(capacities[0] > capacities[1]);
        assert!(capacities[1] > capacities[2]);
    }

    #[test]
    fn test_monitor_trend_detection() {
        let mut monitor = BrittlenessMonitor::default_thresholds();

        // Declining trend
        monitor.assess(0.8);
        monitor.assess(0.6);
        monitor.assess(0.4);

        assert_eq!(monitor.trend(), -1);
    }

    #[test]
    fn test_islanding_risk_detection() {
        let mut monitor = BrittlenessMonitor::default_thresholds();
        let mut graph = FusionGraph::new();

        // Create weak graph
        graph.add_edge(TileId(0), TileId(1), 0.05);

        // Declining min-cut
        for i in (1..=5).rev() {
            monitor.assess(i as f64 * 0.05);
        }

        assert!(monitor.detect_islanding_risk(&graph));
    }
}
