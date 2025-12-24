//! Hyperconverged (Nutanix-style) topology
//!
//! Distributed compute+storage architecture:
//! - Nodes combine compute and storage
//! - Data locality optimization
//! - Replication for fault tolerance
//! - Storage-aware routing

use super::Topology;
use crate::error::{Result, WasmSimError};
use crate::scale::ScaleConfig;

/// Hyperconverged configuration
#[derive(Debug, Clone)]
pub struct HyperconvergedConfig {
    /// Nodes per cluster
    pub nodes_per_cluster: usize,

    /// Number of clusters
    pub num_clusters: usize,

    /// Replication factor (RF)
    pub replication_factor: usize,

    /// Storage bandwidth per node (Gbps)
    pub storage_bandwidth_gbps: f64,

    /// Network bandwidth per node (Gbps)
    pub network_bandwidth_gbps: f64,

    /// Inter-cluster bandwidth (Gbps)
    pub inter_cluster_bandwidth_gbps: f64,

    /// Enable data locality optimization
    pub data_locality: bool,

    /// Cache size per node (MB)
    pub cache_size_mb: usize,
}

impl Default for HyperconvergedConfig {
    fn default() -> Self {
        Self {
            nodes_per_cluster: 4,
            num_clusters: 4,
            replication_factor: 2,
            storage_bandwidth_gbps: 25.0,  // NVMe SSD speed
            network_bandwidth_gbps: 25.0,  // 25G NICs
            inter_cluster_bandwidth_gbps: 100.0, // Spine links
            data_locality: true,
            cache_size_mb: 1024, // 1GB cache per node
        }
    }
}

impl HyperconvergedConfig {
    /// Create Nutanix-style configuration
    pub fn nutanix_style() -> Self {
        Self {
            nodes_per_cluster: 4,
            num_clusters: 16,
            replication_factor: 2,
            storage_bandwidth_gbps: 50.0,   // High-speed NVMe
            network_bandwidth_gbps: 100.0,  // 100G NICs
            inter_cluster_bandwidth_gbps: 400.0,
            data_locality: true,
            cache_size_mb: 4096,
        }
    }

    /// Create configuration for small deployments
    pub fn small() -> Self {
        Self {
            nodes_per_cluster: 3, // Minimum for RF=2
            num_clusters: 1,
            replication_factor: 2,
            storage_bandwidth_gbps: 10.0,
            network_bandwidth_gbps: 10.0,
            inter_cluster_bandwidth_gbps: 10.0,
            data_locality: true,
            cache_size_mb: 256,
        }
    }

    /// Create configuration for specific node count
    pub fn for_nodes(num_nodes: usize) -> Self {
        let nodes_per_cluster = 4;
        let num_clusters = (num_nodes + nodes_per_cluster - 1) / nodes_per_cluster;

        Self {
            nodes_per_cluster,
            num_clusters,
            ..Default::default()
        }
    }
}

/// Hyperconverged topology implementation
pub struct HyperconvergedTopology {
    /// Configuration
    config: HyperconvergedConfig,

    /// Total compute nodes
    num_nodes: usize,

    /// Node to cluster mapping
    node_to_cluster: Vec<u16>,

    /// Primary replica location for each data block
    /// (simplified - in real system this would be distributed metadata)
    primary_locations: Vec<u16>,
}

impl HyperconvergedTopology {
    /// Create hyperconverged topology
    pub fn new(scale: &ScaleConfig, config: HyperconvergedConfig) -> Result<Self> {
        let num_nodes = scale.total_tiles();

        // Validate configuration
        let capacity = config.num_clusters * config.nodes_per_cluster;
        if capacity < num_nodes {
            return Err(WasmSimError::TopologyError(format!(
                "Hyperconverged capacity {} is less than required nodes {}",
                capacity, num_nodes
            )));
        }

        // Build node-to-cluster mapping
        let mut node_to_cluster = Vec::with_capacity(num_nodes);
        for node in 0..num_nodes {
            let cluster = (node / config.nodes_per_cluster) as u16;
            node_to_cluster.push(cluster);
        }

        // Initialize primary locations (distributed hash)
        let primary_locations = (0..num_nodes).map(|i| i as u16).collect();

        Ok(Self {
            config,
            num_nodes,
            node_to_cluster,
            primary_locations,
        })
    }

    /// Get cluster for a node
    pub fn cluster_for_node(&self, node: u16) -> u16 {
        self.node_to_cluster.get(node as usize).copied().unwrap_or(0)
    }

    /// Check if two nodes are in the same cluster
    pub fn same_cluster(&self, src: u16, dst: u16) -> bool {
        self.cluster_for_node(src) == self.cluster_for_node(dst)
    }

    /// Get replica nodes for a data block
    pub fn replica_nodes(&self, block_id: u64) -> Vec<u16> {
        let primary = (block_id % self.num_nodes as u64) as u16;
        let mut replicas = vec![primary];

        // Add RF-1 more replicas on different nodes
        for i in 1..self.config.replication_factor {
            let replica = ((primary as usize + i) % self.num_nodes) as u16;
            replicas.push(replica);
        }

        replicas
    }

    /// Find nearest replica for a read operation (data locality)
    pub fn nearest_replica(&self, reader: u16, block_id: u64) -> u16 {
        let replicas = self.replica_nodes(block_id);

        if !self.config.data_locality {
            return replicas[0]; // Always use primary
        }

        // Find replica in same cluster (if any)
        let reader_cluster = self.cluster_for_node(reader);
        for &replica in &replicas {
            if self.cluster_for_node(replica) == reader_cluster {
                return replica;
            }
        }

        // Fall back to primary
        replicas[0]
    }

    /// Calculate effective bandwidth for storage operation
    pub fn effective_storage_bandwidth(&self, src: u16, dst: u16) -> f64 {
        if self.same_cluster(src, dst) {
            // Local cluster - limited by storage bandwidth
            self.config.storage_bandwidth_gbps
        } else {
            // Cross-cluster - limited by network
            self.config.network_bandwidth_gbps
                .min(self.config.inter_cluster_bandwidth_gbps)
        }
    }
}

impl Topology for HyperconvergedTopology {
    fn name(&self) -> &str {
        "Hyperconverged"
    }

    fn node_count(&self) -> usize {
        self.num_nodes
    }

    fn base_latency_ns(&self) -> u64 {
        // Local storage access: ~100us
        // Network access: ~50us additional
        100_000 // 100 microseconds base (storage-dominated)
    }

    fn bandwidth_gbps(&self) -> f64 {
        // Total = sum of all node storage bandwidths
        // Limited by network for cross-node access
        let local_bw = self.num_nodes as f64 * self.config.storage_bandwidth_gbps;
        let network_bw = self.num_nodes as f64 * self.config.network_bandwidth_gbps;
        local_bw.min(network_bw)
    }

    fn buffer_depth(&self) -> usize {
        64 // Deep buffers for storage operations
    }

    fn path_length(&self, src: u16, dst: u16) -> usize {
        if src == dst {
            0
        } else if self.same_cluster(src, dst) {
            2 // Intra-cluster (via ToR switch)
        } else {
            4 // Inter-cluster (via spine)
        }
    }

    fn describe(&self) -> String {
        format!(
            "Hyperconverged (Nutanix-style)\n\
             - Compute/Storage nodes: {}\n\
             - Clusters: {} ({} nodes each)\n\
             - Replication factor: {}\n\
             - Storage bandwidth: {:.0}G per node\n\
             - Network bandwidth: {:.0}G per node\n\
             - Inter-cluster: {:.0}G\n\
             - Data locality: {}\n\
             - Cache per node: {} MB\n\
             - Aggregate bandwidth: {:.2} Tbps",
            self.num_nodes,
            self.config.num_clusters,
            self.config.nodes_per_cluster,
            self.config.replication_factor,
            self.config.storage_bandwidth_gbps,
            self.config.network_bandwidth_gbps,
            self.config.inter_cluster_bandwidth_gbps,
            if self.config.data_locality { "enabled" } else { "disabled" },
            self.config.cache_size_mb,
            self.bandwidth_gbps() / 1000.0
        )
    }

    fn directly_connected(&self, src: u16, dst: u16) -> bool {
        // Nodes in same cluster are directly connected
        self.same_cluster(src, dst)
    }

    fn neighbors(&self, node: u16) -> Vec<u16> {
        // All nodes in same cluster are neighbors
        let cluster = self.cluster_for_node(node);
        let start = (cluster as usize) * self.config.nodes_per_cluster;
        let end = (start + self.config.nodes_per_cluster).min(self.num_nodes);

        (start..end)
            .filter(|&n| n != node as usize)
            .map(|n| n as u16)
            .collect()
    }

    fn bisection_bandwidth(&self) -> f64 {
        // Limited by inter-cluster links
        (self.config.num_clusters / 2) as f64
            * self.config.nodes_per_cluster as f64
            * self.config.inter_cluster_bandwidth_gbps
    }

    fn diameter(&self) -> usize {
        if self.config.num_clusters > 1 {
            4 // Inter-cluster worst case
        } else {
            2 // Single cluster
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperconverged_creation() {
        let scale = ScaleConfig::from_tiles(16);
        let config = HyperconvergedConfig::default();
        let topo = HyperconvergedTopology::new(&scale, config).unwrap();

        assert_eq!(topo.node_count(), 16);
    }

    #[test]
    fn test_cluster_assignment() {
        let scale = ScaleConfig::from_tiles(16);
        let config = HyperconvergedConfig {
            nodes_per_cluster: 4,
            num_clusters: 4,
            ..Default::default()
        };
        let topo = HyperconvergedTopology::new(&scale, config).unwrap();

        assert_eq!(topo.cluster_for_node(0), 0);
        assert_eq!(topo.cluster_for_node(4), 1);
        assert_eq!(topo.cluster_for_node(15), 3);
    }

    #[test]
    fn test_replica_nodes() {
        let scale = ScaleConfig::from_tiles(16);
        let config = HyperconvergedConfig {
            nodes_per_cluster: 4,
            num_clusters: 4,
            replication_factor: 3,
            ..Default::default()
        };
        let topo = HyperconvergedTopology::new(&scale, config).unwrap();

        let replicas = topo.replica_nodes(0);
        assert_eq!(replicas.len(), 3);
    }

    #[test]
    fn test_data_locality() {
        let scale = ScaleConfig::from_tiles(16);
        let config = HyperconvergedConfig {
            nodes_per_cluster: 4,
            num_clusters: 4,
            replication_factor: 2,
            data_locality: true,
            ..Default::default()
        };
        let topo = HyperconvergedTopology::new(&scale, config).unwrap();

        // Node 0's nearest replica for block 0 should be local
        let nearest = topo.nearest_replica(0, 0);
        assert_eq!(nearest, 0);
    }

    #[test]
    fn test_nutanix_config() {
        let scale = ScaleConfig::from_tiles(64);
        let config = HyperconvergedConfig::nutanix_style();
        let topo = HyperconvergedTopology::new(&scale, config).unwrap();

        assert!(topo.bandwidth_gbps() > 1000.0); // > 1 Tbps
    }
}
