//! Leaf-Spine (CLOS) topology
//!
//! Arista-style datacenter topology inspired by Tomahawk 5:
//! - Leaf switches connect to compute nodes
//! - Spine switches provide full mesh between leaves
//! - Non-blocking fabric with ECMP load balancing
//! - Supports up to 51.2 Tbps aggregate bandwidth

use super::Topology;
use crate::error::{Result, WasmSimError};
use crate::scale::ScaleConfig;

/// Leaf-spine configuration
#[derive(Debug, Clone)]
pub struct LeafSpineConfig {
    /// Number of leaf switches
    pub num_leaves: usize,

    /// Number of spine switches
    pub num_spines: usize,

    /// Compute nodes per leaf
    pub nodes_per_leaf: usize,

    /// Uplink bandwidth per leaf (Gbps)
    pub uplink_bandwidth_gbps: f64,

    /// Downlink bandwidth per port (Gbps)
    pub downlink_bandwidth_gbps: f64,

    /// Oversubscription ratio (1.0 = non-blocking)
    pub oversubscription: f64,
}

impl Default for LeafSpineConfig {
    fn default() -> Self {
        Self {
            num_leaves: 32,
            num_spines: 4,
            nodes_per_leaf: 8,
            uplink_bandwidth_gbps: 400.0,  // 400G uplinks
            downlink_bandwidth_gbps: 100.0, // 100G to nodes
            oversubscription: 1.0,
        }
    }
}

impl LeafSpineConfig {
    /// Create configuration for Arista 7060X6 style (Tomahawk 5)
    pub fn arista_7060x6() -> Self {
        Self {
            num_leaves: 32,
            num_spines: 8,
            nodes_per_leaf: 64,
            uplink_bandwidth_gbps: 800.0,  // 8x 800G uplinks
            downlink_bandwidth_gbps: 100.0,
            oversubscription: 1.0,
        }
    }

    /// Create small-scale configuration
    pub fn small() -> Self {
        Self {
            num_leaves: 4,
            num_spines: 2,
            nodes_per_leaf: 4,
            uplink_bandwidth_gbps: 100.0,
            downlink_bandwidth_gbps: 25.0,
            oversubscription: 1.0,
        }
    }

    /// Create configuration for specific node count
    pub fn for_nodes(num_nodes: usize) -> Self {
        let nodes_per_leaf = 8;
        let num_leaves = (num_nodes + nodes_per_leaf - 1) / nodes_per_leaf;
        let num_spines = (num_leaves / 4).max(2);

        Self {
            num_leaves,
            num_spines,
            nodes_per_leaf,
            uplink_bandwidth_gbps: 400.0,
            downlink_bandwidth_gbps: 100.0,
            oversubscription: 1.0,
        }
    }
}

/// Leaf-spine topology implementation
pub struct LeafSpineTopology {
    /// Configuration
    config: LeafSpineConfig,

    /// Total compute nodes
    num_nodes: usize,

    /// Node to leaf switch mapping
    node_to_leaf: Vec<u16>,
}

impl LeafSpineTopology {
    /// Create leaf-spine topology
    pub fn new(scale: &ScaleConfig, config: LeafSpineConfig) -> Result<Self> {
        let num_nodes = scale.total_tiles();

        // Validate configuration
        let capacity = config.num_leaves * config.nodes_per_leaf;
        if capacity < num_nodes {
            return Err(WasmSimError::TopologyError(format!(
                "Leaf-spine capacity {} is less than required nodes {}",
                capacity, num_nodes
            )));
        }

        // Build node-to-leaf mapping
        let mut node_to_leaf = Vec::with_capacity(num_nodes);
        for node in 0..num_nodes {
            let leaf = (node / config.nodes_per_leaf) as u16;
            node_to_leaf.push(leaf);
        }

        Ok(Self {
            config,
            num_nodes,
            node_to_leaf,
        })
    }

    /// Get leaf switch for a node
    pub fn leaf_for_node(&self, node: u16) -> u16 {
        self.node_to_leaf.get(node as usize).copied().unwrap_or(0)
    }

    /// Check if two nodes are on the same leaf
    pub fn same_leaf(&self, src: u16, dst: u16) -> bool {
        self.leaf_for_node(src) == self.leaf_for_node(dst)
    }

    /// Get total switch count
    pub fn switch_count(&self) -> usize {
        self.config.num_leaves + self.config.num_spines
    }

    /// Calculate ECMP paths between leaves
    pub fn ecmp_paths(&self, src_leaf: u16, dst_leaf: u16) -> usize {
        if src_leaf == dst_leaf {
            1 // Same leaf, no ECMP
        } else {
            self.config.num_spines // All spines provide equal paths
        }
    }
}

impl Topology for LeafSpineTopology {
    fn name(&self) -> &str {
        "LeafSpine"
    }

    fn node_count(&self) -> usize {
        self.num_nodes
    }

    fn base_latency_ns(&self) -> u64 {
        // Leaf-to-leaf via spine: ~500ns for high-speed datacenter
        // Assuming cut-through switching at 400G
        500
    }

    fn bandwidth_gbps(&self) -> f64 {
        // Total fabric bandwidth = spines * uplinks per spine * uplink BW
        (self.config.num_spines * self.config.num_leaves) as f64
            * self.config.uplink_bandwidth_gbps
    }

    fn buffer_depth(&self) -> usize {
        32 // Deeper buffers for datacenter switches
    }

    fn path_length(&self, src: u16, dst: u16) -> usize {
        if src == dst {
            0
        } else if self.same_leaf(src, dst) {
            2 // Node -> Leaf -> Node
        } else {
            4 // Node -> Leaf -> Spine -> Leaf -> Node
        }
    }

    fn describe(&self) -> String {
        format!(
            "Leaf-Spine (Arista-style CLOS)\n\
             - Compute nodes: {}\n\
             - Leaf switches: {} ({} nodes each)\n\
             - Spine switches: {}\n\
             - Uplink: {:.0}G x {} = {:.1}T per leaf\n\
             - Downlink: {:.0}G per node\n\
             - Oversubscription: {:.1}:1\n\
             - Aggregate bandwidth: {:.2} Tbps\n\
             - ECMP paths: {}",
            self.num_nodes,
            self.config.num_leaves,
            self.config.nodes_per_leaf,
            self.config.num_spines,
            self.config.uplink_bandwidth_gbps,
            self.config.num_spines,
            self.config.uplink_bandwidth_gbps * self.config.num_spines as f64 / 1000.0,
            self.config.downlink_bandwidth_gbps,
            self.config.oversubscription,
            self.bandwidth_gbps() / 1000.0,
            self.config.num_spines
        )
    }

    fn directly_connected(&self, src: u16, dst: u16) -> bool {
        // Nodes on same leaf are "directly" connected (1 switch hop)
        self.same_leaf(src, dst)
    }

    fn neighbors(&self, node: u16) -> Vec<u16> {
        // All nodes on same leaf are neighbors
        let leaf = self.leaf_for_node(node);
        let start = (leaf as usize) * self.config.nodes_per_leaf;
        let end = (start + self.config.nodes_per_leaf).min(self.num_nodes);

        (start..end)
            .filter(|&n| n != node as usize)
            .map(|n| n as u16)
            .collect()
    }

    fn bisection_bandwidth(&self) -> f64 {
        // Bisection = min(total spine uplinks, total leaf uplinks / 2)
        let spine_bw = (self.config.num_spines * self.config.num_leaves) as f64
            * self.config.uplink_bandwidth_gbps;
        let leaf_bw = (self.config.num_leaves / 2) as f64
            * self.config.num_spines as f64
            * self.config.uplink_bandwidth_gbps;
        spine_bw.min(leaf_bw)
    }

    fn diameter(&self) -> usize {
        4 // Node -> Leaf -> Spine -> Leaf -> Node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_spine_creation() {
        let scale = ScaleConfig::from_tiles(64);
        let config = LeafSpineConfig::for_nodes(64);
        let topo = LeafSpineTopology::new(&scale, config).unwrap();

        assert_eq!(topo.node_count(), 64);
    }

    #[test]
    fn test_path_length() {
        let scale = ScaleConfig::from_tiles(64);
        let config = LeafSpineConfig {
            num_leaves: 8,
            num_spines: 2,
            nodes_per_leaf: 8,
            uplink_bandwidth_gbps: 100.0,
            downlink_bandwidth_gbps: 25.0,
            oversubscription: 1.0,
        };
        let topo = LeafSpineTopology::new(&scale, config).unwrap();

        // Same node
        assert_eq!(topo.path_length(0, 0), 0);

        // Same leaf
        assert_eq!(topo.path_length(0, 1), 2);

        // Different leaves
        assert_eq!(topo.path_length(0, 8), 4);
    }

    #[test]
    fn test_ecmp_paths() {
        let scale = ScaleConfig::from_tiles(64);
        let config = LeafSpineConfig {
            num_leaves: 8,
            num_spines: 4,
            nodes_per_leaf: 8,
            uplink_bandwidth_gbps: 100.0,
            downlink_bandwidth_gbps: 25.0,
            oversubscription: 1.0,
        };
        let topo = LeafSpineTopology::new(&scale, config).unwrap();

        // Different leaves should have ECMP via all spines
        assert_eq!(topo.ecmp_paths(0, 1), 4);
    }

    #[test]
    fn test_arista_config() {
        let scale = ScaleConfig::from_tiles(2048);
        let config = LeafSpineConfig::arista_7060x6();
        let topo = LeafSpineTopology::new(&scale, config).unwrap();

        // Verify high bandwidth
        assert!(topo.bandwidth_gbps() > 100_000.0); // > 100 Tbps
    }
}
