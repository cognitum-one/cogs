//! Network topology implementations
//!
//! Provides three enterprise-grade topologies:
//! - RaceWay: Native Cognitum hierarchical mesh NoC
//! - LeafSpine: Arista-style datacenter CLOS (Tomahawk-inspired)
//! - Hyperconverged: Nutanix-style distributed compute+storage

pub mod raceway;
pub mod leaf_spine;
pub mod hyperconverged;

pub use raceway::RaceWayTopology;
pub use leaf_spine::{LeafSpineTopology, LeafSpineConfig};
pub use hyperconverged::{HyperconvergedTopology, HyperconvergedConfig};

use crate::error::Result;
use crate::scale::ScaleConfig;

/// Topology trait for network configuration
pub trait Topology: Send + Sync {
    /// Get topology name
    fn name(&self) -> &str;

    /// Get total node count
    fn node_count(&self) -> usize;

    /// Get base latency in nanoseconds
    fn base_latency_ns(&self) -> u64;

    /// Get bandwidth in Gbps
    fn bandwidth_gbps(&self) -> f64;

    /// Get buffer depth per port
    fn buffer_depth(&self) -> usize;

    /// Get path length between two nodes
    fn path_length(&self, src: u16, dst: u16) -> usize;

    /// Get description of topology
    fn describe(&self) -> String;

    /// Check if two nodes are directly connected
    fn directly_connected(&self, src: u16, dst: u16) -> bool;

    /// Get all neighbors of a node
    fn neighbors(&self, node: u16) -> Vec<u16>;

    /// Get bisection bandwidth in Gbps
    fn bisection_bandwidth(&self) -> f64;

    /// Get diameter (maximum path length)
    fn diameter(&self) -> usize;
}

/// Topology kind with configuration
#[derive(Debug, Clone)]
pub enum TopologyKind {
    /// RaceWay hierarchical mesh
    RaceWay,

    /// Arista-style leaf-spine CLOS
    LeafSpine(LeafSpineConfig),

    /// Nutanix-style hyperconverged
    Hyperconverged(HyperconvergedConfig),
}

impl Default for TopologyKind {
    fn default() -> Self {
        TopologyKind::RaceWay
    }
}
