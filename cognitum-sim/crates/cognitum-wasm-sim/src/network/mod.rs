//! Network fabric simulation
//!
//! Provides configurable network simulation for different topologies:
//! - RaceWay: Hierarchical mesh NoC (native Cognitum)
//! - LeafSpine: Arista-style datacenter CLOS topology
//! - Hyperconverged: Nutanix-style distributed compute+storage

pub mod fabric;
pub mod router;
pub mod packet;
pub mod stats;

pub use fabric::{NetworkFabric, NetworkConfig};
pub use router::PacketRouter;
pub use packet::Packet;
pub use stats::NetworkStats;

use crate::error::Result;
use crate::topology::Topology;

impl NetworkConfig {
    /// Create configuration from topology
    pub fn from_topology(topology: &dyn Topology) -> Self {
        Self {
            num_nodes: topology.node_count(),
            latency_ns: topology.base_latency_ns(),
            bandwidth_gbps: topology.bandwidth_gbps(),
            buffer_depth: topology.buffer_depth(),
            topology_name: topology.name().to_string(),
        }
    }
}
