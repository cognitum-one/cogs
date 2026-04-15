//! Packet routing algorithms
//!
//! Supports multiple routing strategies for different topologies

use std::collections::{HashMap, VecDeque};

/// Packet router with configurable algorithms
pub struct PacketRouter {
    /// Number of nodes
    num_nodes: usize,

    /// Routing table (source, dest) -> next hop
    routing_table: HashMap<(u16, u16), u16>,

    /// Adjacency list for path computation
    adjacency: Vec<Vec<u16>>,

    /// Routing algorithm
    algorithm: RoutingAlgorithm,
}

/// Available routing algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingAlgorithm {
    /// Dimension-ordered routing (X-Y for 2D mesh)
    DimensionOrder,

    /// Shortest path (BFS-based)
    ShortestPath,

    /// Equal-cost multipath (ECMP)
    Ecmp,

    /// Adaptive routing (congestion-aware)
    Adaptive,

    /// Valiant load balancing
    Valiant,
}

impl PacketRouter {
    /// Create new router
    pub fn new(num_nodes: usize) -> Self {
        // Default to mesh topology with dimension-order routing
        let mut adjacency = vec![Vec::new(); num_nodes];

        // Build 2D mesh adjacency (assuming square-ish layout)
        let dim = (num_nodes as f64).sqrt().ceil() as usize;

        for node in 0..num_nodes {
            let x = node % dim;
            let y = node / dim;

            // Right neighbor
            if x + 1 < dim && node + 1 < num_nodes {
                adjacency[node].push((node + 1) as u16);
            }

            // Left neighbor
            if x > 0 {
                adjacency[node].push((node - 1) as u16);
            }

            // Down neighbor
            if y + 1 < dim && node + dim < num_nodes {
                adjacency[node].push((node + dim) as u16);
            }

            // Up neighbor
            if y > 0 && node >= dim {
                adjacency[node].push((node - dim) as u16);
            }
        }

        Self {
            num_nodes,
            routing_table: HashMap::new(),
            adjacency,
            algorithm: RoutingAlgorithm::DimensionOrder,
        }
    }

    /// Create router for leaf-spine topology
    pub fn for_leaf_spine(num_leaves: usize, num_spines: usize, nodes_per_leaf: usize) -> Self {
        let total_nodes = num_leaves * nodes_per_leaf;
        let total_switches = num_leaves + num_spines;

        let mut adjacency = vec![Vec::new(); total_nodes + total_switches];

        // Connect nodes to their leaf switches
        for leaf in 0..num_leaves {
            let leaf_switch = total_nodes + leaf;
            for n in 0..nodes_per_leaf {
                let node = leaf * nodes_per_leaf + n;
                adjacency[node].push(leaf_switch as u16);
                adjacency[leaf_switch].push(node as u16);
            }
        }

        // Connect all leaf switches to all spine switches (full mesh)
        for leaf in 0..num_leaves {
            let leaf_switch = total_nodes + leaf;
            for spine in 0..num_spines {
                let spine_switch = total_nodes + num_leaves + spine;
                adjacency[leaf_switch].push(spine_switch as u16);
                adjacency[spine_switch].push(leaf_switch as u16);
            }
        }

        Self {
            num_nodes: total_nodes,
            routing_table: HashMap::new(),
            adjacency,
            algorithm: RoutingAlgorithm::Ecmp,
        }
    }

    /// Create router for fat-tree topology (used in hyperconverged)
    pub fn for_fat_tree(k: usize) -> Self {
        // k-ary fat tree
        // Pods: k
        // Servers per pod: k/2
        // Aggregate switches per pod: k/2
        // Core switches: (k/2)^2

        let servers_per_pod = (k / 2) * (k / 2);
        let total_servers = k * servers_per_pod;

        let edge_per_pod = k / 2;
        let agg_per_pod = k / 2;
        let core_switches = (k / 2) * (k / 2);

        let total_switches = k * (edge_per_pod + agg_per_pod) + core_switches;
        let total_nodes = total_servers + total_switches;

        let mut adjacency = vec![Vec::new(); total_nodes];

        // Build fat-tree connections (simplified)
        // Real implementation would be more complex

        Self {
            num_nodes: total_servers,
            routing_table: HashMap::new(),
            adjacency,
            algorithm: RoutingAlgorithm::Ecmp,
        }
    }

    /// Set routing algorithm
    pub fn set_algorithm(&mut self, algorithm: RoutingAlgorithm) {
        self.algorithm = algorithm;
        self.routing_table.clear(); // Invalidate cached routes
    }

    /// Compute path from source to destination
    pub fn compute_path(&self, source: u16, destination: u16) -> Vec<u16> {
        if source == destination {
            return vec![destination];
        }

        match self.algorithm {
            RoutingAlgorithm::DimensionOrder => {
                self.dimension_order_path(source, destination)
            }
            RoutingAlgorithm::ShortestPath | RoutingAlgorithm::Ecmp => {
                self.bfs_path(source, destination)
            }
            RoutingAlgorithm::Adaptive | RoutingAlgorithm::Valiant => {
                // Fall back to shortest path for now
                self.bfs_path(source, destination)
            }
        }
    }

    /// Dimension-ordered routing (X then Y)
    fn dimension_order_path(&self, source: u16, destination: u16) -> Vec<u16> {
        let dim = (self.num_nodes as f64).sqrt().ceil() as u16;

        let mut path = Vec::new();
        let mut current = source;

        let src_x = source % dim;
        let src_y = source / dim;
        let dst_x = destination % dim;
        let dst_y = destination / dim;

        // Move in X direction first
        let mut x = src_x;
        while x != dst_x {
            if x < dst_x {
                x += 1;
            } else {
                x -= 1;
            }
            current = src_y * dim + x;
            path.push(current);
        }

        // Then move in Y direction
        let mut y = src_y;
        while y != dst_y {
            if y < dst_y {
                y += 1;
            } else {
                y -= 1;
            }
            current = y * dim + x;
            path.push(current);
        }

        path
    }

    /// BFS shortest path
    fn bfs_path(&self, source: u16, destination: u16) -> Vec<u16> {
        if source as usize >= self.adjacency.len() || destination as usize >= self.adjacency.len() {
            return vec![destination];
        }

        let mut visited = vec![false; self.adjacency.len()];
        let mut parent = vec![None; self.adjacency.len()];
        let mut queue = VecDeque::new();

        visited[source as usize] = true;
        queue.push_back(source);

        while let Some(node) = queue.pop_front() {
            if node == destination {
                break;
            }

            for &neighbor in &self.adjacency[node as usize] {
                if !visited[neighbor as usize] {
                    visited[neighbor as usize] = true;
                    parent[neighbor as usize] = Some(node);
                    queue.push_back(neighbor);
                }
            }
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = destination;

        while let Some(p) = parent[current as usize] {
            path.push(current);
            current = p;
        }

        path.reverse();
        path
    }

    /// Get next hop for (source, destination) pair
    pub fn next_hop(&mut self, source: u16, destination: u16) -> Option<u16> {
        // Check cache first
        if let Some(&hop) = self.routing_table.get(&(source, destination)) {
            return Some(hop);
        }

        // Compute path and cache
        let path = self.compute_path(source, destination);
        if !path.is_empty() {
            let next = path[0];
            self.routing_table.insert((source, destination), next);
            Some(next)
        } else {
            None
        }
    }

    /// Get all neighbors of a node
    pub fn neighbors(&self, node: u16) -> &[u16] {
        self.adjacency.get(node as usize).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get path length (hop count)
    pub fn path_length(&self, source: u16, destination: u16) -> usize {
        self.compute_path(source, destination).len()
    }

    /// Update adjacency for custom topology
    pub fn set_adjacency(&mut self, node: u16, neighbors: Vec<u16>) {
        if (node as usize) < self.adjacency.len() {
            self.adjacency[node as usize] = neighbors;
            self.routing_table.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let router = PacketRouter::new(16);
        assert_eq!(router.num_nodes, 16);
    }

    #[test]
    fn test_dimension_order_routing() {
        let router = PacketRouter::new(16); // 4x4 mesh

        let path = router.compute_path(0, 15); // (0,0) to (3,3)
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), 15);
    }

    #[test]
    fn test_path_length() {
        let router = PacketRouter::new(16);

        // Adjacent nodes
        let len = router.path_length(0, 1);
        assert_eq!(len, 1);

        // Diagonal nodes in 4x4
        let len = router.path_length(0, 15);
        assert_eq!(len, 6); // 3 hops X + 3 hops Y
    }

    #[test]
    fn test_leaf_spine() {
        let router = PacketRouter::for_leaf_spine(4, 2, 8);
        // 4 leaves * 8 nodes = 32 compute nodes
        assert_eq!(router.num_nodes, 32);
    }
}
