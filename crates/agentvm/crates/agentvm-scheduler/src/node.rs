//! Node registry for the heterogeneous fabric scheduler.
//!
//! This module provides:
//! - `NodeId` and `NodeInfo` for node identification and metadata
//! - `Tier` enum representing compute tiers (Edge, Host, Accel)
//! - `NodeCapabilities` for describing node features
//! - `NodeRegistry` for managing the set of available nodes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::task::{AcceleratorType, CapsuleId, CapabilityType};

/// Unique identifier for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// Create a new random node ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a node ID from a UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node-{}", self.0)
    }
}

/// Compute tier classification.
///
/// The fabric consists of three tiers with different power/performance profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tier {
    /// Edge tier: STM32, ESP32, nRF52 - always-on, low power (uA-mA).
    Edge,
    /// Host tier: Raspberry Pi, x86, MicroVM - general compute (W).
    Host,
    /// Accelerator tier: GPU, TPU, custom ASIC - heavy compute (10-100W).
    Accel,
}

impl Tier {
    /// Get the typical power consumption range for this tier in milliwatts.
    pub fn power_range_mw(&self) -> (u32, u32) {
        match self {
            Tier::Edge => (0, 50),       // 0-50 mW (sub-mA to few mA at 3.3V)
            Tier::Host => (2000, 15000), // 2-15 W
            Tier::Accel => (50000, 300000), // 50-300 W
        }
    }

    /// Get the typical wake latency for this tier in milliseconds.
    pub fn typical_wake_latency_ms(&self) -> u64 {
        match self {
            Tier::Edge => 0,    // Always on
            Tier::Host => 100,  // Resume from sleep
            Tier::Accel => 500, // GPU allocation
        }
    }
}

/// Security isolation level provided by a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum IsolationLevel {
    /// Hardware isolation (separate physical machine or HSM).
    Hardware,
    /// VM-level isolation (MicroVM, hypervisor).
    Vm,
    /// Process-level isolation (containers, namespaces).
    #[default]
    Process,
}

impl IsolationLevel {
    /// Check if this isolation level meets the required level.
    pub fn satisfies(&self, required: IsolationLevel) -> bool {
        match (self, required) {
            (IsolationLevel::Hardware, _) => true,
            (IsolationLevel::Vm, IsolationLevel::Hardware) => false,
            (IsolationLevel::Vm, _) => true,
            (IsolationLevel::Process, IsolationLevel::Process) => true,
            (IsolationLevel::Process, _) => false,
        }
    }
}

/// Capabilities available on a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Network access available.
    pub network: bool,
    /// Available accelerators.
    pub accelerators: Vec<AcceleratorType>,
    /// Available capability types.
    pub available_capabilities: Vec<CapabilityType>,
    /// Supports memory snapshots.
    pub memory_snapshot: bool,
    /// Supports disk snapshots.
    pub disk_snapshot: bool,
    /// Has local workspace storage.
    pub local_workspace: bool,
    /// Maximum concurrent tasks.
    pub max_concurrent_tasks: u32,
}

impl NodeCapabilities {
    /// Create minimal edge node capabilities.
    pub fn edge() -> Self {
        Self {
            network: false,
            accelerators: Vec::new(),
            available_capabilities: vec![],
            memory_snapshot: false,
            disk_snapshot: false,
            local_workspace: false,
            max_concurrent_tasks: 4,
        }
    }

    /// Create typical host node capabilities.
    pub fn host() -> Self {
        Self {
            network: true,
            accelerators: Vec::new(),
            available_capabilities: vec![
                CapabilityType::NetworkHttp,
                CapabilityType::NetworkSocket,
                CapabilityType::FileRead,
                CapabilityType::FileWrite,
                CapabilityType::ProcessSpawn,
                CapabilityType::SecretRead,
            ],
            memory_snapshot: true,
            disk_snapshot: true,
            local_workspace: true,
            max_concurrent_tasks: 16,
        }
    }

    /// Create accelerator node capabilities.
    pub fn accel(accelerator: AcceleratorType) -> Self {
        Self {
            network: true,
            accelerators: vec![accelerator],
            available_capabilities: vec![
                CapabilityType::NetworkHttp,
                CapabilityType::FileRead,
                CapabilityType::FileWrite,
                CapabilityType::GpuCompute,
            ],
            memory_snapshot: false,
            disk_snapshot: true,
            local_workspace: true,
            max_concurrent_tasks: 8,
        }
    }
}

/// Health status of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealth {
    /// Node is healthy and accepting tasks.
    Healthy,
    /// Node is degraded but functional.
    Degraded,
    /// Node is unhealthy and should not receive new tasks.
    Unhealthy,
    /// Node is offline.
    Offline,
}

impl Default for NodeHealth {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Complete information about a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier.
    pub id: NodeId,
    /// Human-readable name.
    pub name: String,
    /// Compute tier.
    pub tier: Tier,
    /// Node capabilities.
    pub capabilities: NodeCapabilities,
    /// Available memory in bytes.
    pub available_memory: u64,
    /// Total memory in bytes.
    pub total_memory: u64,
    /// Current power draw in milliwatts.
    pub power_draw_mw: u32,
    /// Wake latency in milliseconds.
    pub wake_latency_ms: u64,
    /// Current load percentage (0-100).
    pub current_load: u8,
    /// Isolation level provided.
    pub isolation_level: IsolationLevel,
    /// Current health status.
    pub health: NodeHealth,
    /// Capsules with local workspaces on this node.
    pub local_workspaces: Vec<CapsuleId>,
    /// Number of currently running tasks.
    pub running_tasks: u32,
}

impl NodeInfo {
    /// Create a new node with the given tier and name.
    pub fn new(name: impl Into<String>, tier: Tier) -> Self {
        let capabilities = match tier {
            Tier::Edge => NodeCapabilities::edge(),
            Tier::Host => NodeCapabilities::host(),
            Tier::Accel => NodeCapabilities::accel(AcceleratorType::Gpu),
        };

        let (total_memory, power_draw) = match tier {
            Tier::Edge => (64 * 1024, 5),                     // 64KB, 5mW
            Tier::Host => (4 * 1024 * 1024 * 1024, 5000),     // 4GB, 5W
            Tier::Accel => (16 * 1024 * 1024 * 1024, 100000), // 16GB, 100W
        };

        Self {
            id: NodeId::new(),
            name: name.into(),
            tier,
            capabilities,
            available_memory: total_memory,
            total_memory,
            power_draw_mw: power_draw,
            wake_latency_ms: tier.typical_wake_latency_ms(),
            current_load: 0,
            isolation_level: match tier {
                Tier::Edge => IsolationLevel::Process,
                Tier::Host => IsolationLevel::Vm,
                Tier::Accel => IsolationLevel::Vm,
            },
            health: NodeHealth::Healthy,
            local_workspaces: Vec::new(),
            running_tasks: 0,
        }
    }

    /// Check if this node has a local workspace for the given capsule.
    pub fn has_local_workspace(&self, capsule_id: &CapsuleId) -> bool {
        self.local_workspaces.contains(capsule_id)
    }

    /// Check if this node can accept more tasks.
    pub fn can_accept_task(&self) -> bool {
        self.health == NodeHealth::Healthy
            && self.running_tasks < self.capabilities.max_concurrent_tasks
    }

    /// Get the remaining capacity as a percentage.
    pub fn remaining_capacity(&self) -> f64 {
        if self.capabilities.max_concurrent_tasks == 0 {
            return 0.0;
        }
        let used = self.running_tasks as f64 / self.capabilities.max_concurrent_tasks as f64;
        (1.0 - used) * 100.0
    }
}

/// Error types for node registry operations.
#[derive(Debug, thiserror::Error)]
pub enum NodeRegistryError {
    /// The specified node was not found in the registry.
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
    /// A node with the same ID already exists in the registry.
    #[error("Node already exists: {0}")]
    NodeAlreadyExists(NodeId),
    /// The node is marked as unhealthy and cannot accept tasks.
    #[error("Node is unhealthy: {0}")]
    NodeUnhealthy(NodeId),
}

/// Registry of available nodes in the fabric.
#[derive(Debug)]
pub struct NodeRegistry {
    /// Nodes indexed by ID.
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
    /// Nodes indexed by tier for fast lookup.
    nodes_by_tier: Arc<RwLock<HashMap<Tier, Vec<NodeId>>>>,
}

impl NodeRegistry {
    /// Create a new empty node registry.
    pub fn new() -> Self {
        let mut nodes_by_tier = HashMap::new();
        nodes_by_tier.insert(Tier::Edge, Vec::new());
        nodes_by_tier.insert(Tier::Host, Vec::new());
        nodes_by_tier.insert(Tier::Accel, Vec::new());

        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            nodes_by_tier: Arc::new(RwLock::new(nodes_by_tier)),
        }
    }

    /// Add a node to the registry.
    pub async fn add(&self, node: NodeInfo) -> Result<(), NodeRegistryError> {
        let node_id = node.id;
        let tier = node.tier;

        let mut nodes = self.nodes.write().await;
        if nodes.contains_key(&node_id) {
            return Err(NodeRegistryError::NodeAlreadyExists(node_id));
        }

        nodes.insert(node_id, node);
        drop(nodes);

        let mut by_tier = self.nodes_by_tier.write().await;
        by_tier.entry(tier).or_default().push(node_id);

        tracing::info!(%node_id, ?tier, "Node added to registry");
        Ok(())
    }

    /// Remove a node from the registry.
    pub async fn remove(&self, node_id: NodeId) -> Result<NodeInfo, NodeRegistryError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .remove(&node_id)
            .ok_or(NodeRegistryError::NodeNotFound(node_id))?;

        drop(nodes);

        let mut by_tier = self.nodes_by_tier.write().await;
        if let Some(tier_nodes) = by_tier.get_mut(&node.tier) {
            tier_nodes.retain(|id| *id != node_id);
        }

        tracing::info!(%node_id, tier = ?node.tier, "Node removed from registry");
        Ok(node)
    }

    /// Get a node by ID.
    pub async fn get(&self, node_id: NodeId) -> Option<NodeInfo> {
        self.nodes.read().await.get(&node_id).cloned()
    }

    /// Get all nodes in a specific tier.
    pub async fn get_by_tier(&self, tier: Tier) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().await;
        let by_tier = self.nodes_by_tier.read().await;

        by_tier
            .get(&tier)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| nodes.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all healthy nodes.
    pub async fn get_healthy(&self) -> Vec<NodeInfo> {
        self.nodes
            .read()
            .await
            .values()
            .filter(|n| n.health == NodeHealth::Healthy)
            .cloned()
            .collect()
    }

    /// Get all nodes.
    pub async fn get_all(&self) -> Vec<NodeInfo> {
        self.nodes.read().await.values().cloned().collect()
    }

    /// Mark a node as unhealthy.
    pub async fn mark_unhealthy(&self, node_id: NodeId) -> Result<(), NodeRegistryError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(NodeRegistryError::NodeNotFound(node_id))?;

        node.health = NodeHealth::Unhealthy;
        tracing::warn!(%node_id, "Node marked as unhealthy");
        Ok(())
    }

    /// Mark a node as healthy.
    pub async fn mark_healthy(&self, node_id: NodeId) -> Result<(), NodeRegistryError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(NodeRegistryError::NodeNotFound(node_id))?;

        node.health = NodeHealth::Healthy;
        tracing::info!(%node_id, "Node marked as healthy");
        Ok(())
    }

    /// Update node load and running task count.
    pub async fn update_load(
        &self,
        node_id: NodeId,
        load: u8,
        running_tasks: u32,
    ) -> Result<(), NodeRegistryError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(NodeRegistryError::NodeNotFound(node_id))?;

        node.current_load = load.min(100);
        node.running_tasks = running_tasks;
        Ok(())
    }

    /// Update node available memory.
    pub async fn update_memory(
        &self,
        node_id: NodeId,
        available_memory: u64,
    ) -> Result<(), NodeRegistryError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(&node_id)
            .ok_or(NodeRegistryError::NodeNotFound(node_id))?;

        node.available_memory = available_memory.min(node.total_memory);
        Ok(())
    }

    /// Get the number of nodes in the registry.
    pub async fn len(&self) -> usize {
        self.nodes.read().await.len()
    }

    /// Check if the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.nodes.read().await.is_empty()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_registry_add_remove() {
        let registry = NodeRegistry::new();
        let node = NodeInfo::new("test-node", Tier::Host);
        let node_id = node.id;

        // Add node
        registry.add(node).await.unwrap();
        assert_eq!(registry.len().await, 1);

        // Get node
        let retrieved = registry.get(node_id).await.unwrap();
        assert_eq!(retrieved.name, "test-node");
        assert_eq!(retrieved.tier, Tier::Host);

        // Remove node
        registry.remove(node_id).await.unwrap();
        assert!(registry.is_empty().await);
    }

    #[tokio::test]
    async fn test_node_registry_by_tier() {
        let registry = NodeRegistry::new();

        let edge_node = NodeInfo::new("edge-1", Tier::Edge);
        let host_node1 = NodeInfo::new("host-1", Tier::Host);
        let host_node2 = NodeInfo::new("host-2", Tier::Host);
        let accel_node = NodeInfo::new("accel-1", Tier::Accel);

        registry.add(edge_node).await.unwrap();
        registry.add(host_node1).await.unwrap();
        registry.add(host_node2).await.unwrap();
        registry.add(accel_node).await.unwrap();

        assert_eq!(registry.get_by_tier(Tier::Edge).await.len(), 1);
        assert_eq!(registry.get_by_tier(Tier::Host).await.len(), 2);
        assert_eq!(registry.get_by_tier(Tier::Accel).await.len(), 1);
    }

    #[tokio::test]
    async fn test_node_health_management() {
        let registry = NodeRegistry::new();
        let node = NodeInfo::new("test-node", Tier::Host);
        let node_id = node.id;

        registry.add(node).await.unwrap();

        // Initially healthy
        let healthy = registry.get_healthy().await;
        assert_eq!(healthy.len(), 1);

        // Mark unhealthy
        registry.mark_unhealthy(node_id).await.unwrap();
        let healthy = registry.get_healthy().await;
        assert_eq!(healthy.len(), 0);

        // Mark healthy again
        registry.mark_healthy(node_id).await.unwrap();
        let healthy = registry.get_healthy().await;
        assert_eq!(healthy.len(), 1);
    }

    #[test]
    fn test_isolation_level_satisfies() {
        assert!(IsolationLevel::Hardware.satisfies(IsolationLevel::Hardware));
        assert!(IsolationLevel::Hardware.satisfies(IsolationLevel::Vm));
        assert!(IsolationLevel::Hardware.satisfies(IsolationLevel::Process));

        assert!(!IsolationLevel::Vm.satisfies(IsolationLevel::Hardware));
        assert!(IsolationLevel::Vm.satisfies(IsolationLevel::Vm));
        assert!(IsolationLevel::Vm.satisfies(IsolationLevel::Process));

        assert!(!IsolationLevel::Process.satisfies(IsolationLevel::Hardware));
        assert!(!IsolationLevel::Process.satisfies(IsolationLevel::Vm));
        assert!(IsolationLevel::Process.satisfies(IsolationLevel::Process));
    }

    #[test]
    fn test_tier_properties() {
        // Edge should have lowest power
        let (_edge_min, edge_max) = Tier::Edge.power_range_mw();
        let (host_min, _) = Tier::Host.power_range_mw();
        assert!(edge_max < host_min);

        // Edge should have lowest latency
        assert!(Tier::Edge.typical_wake_latency_ms() < Tier::Host.typical_wake_latency_ms());
        assert!(Tier::Host.typical_wake_latency_ms() < Tier::Accel.typical_wake_latency_ms());
    }
}
