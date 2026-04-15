//! Node registry and node types

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use agentvm_types::CapabilityType;
use crate::task::{AcceleratorType, IsolationLevel};

/// Node identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Compute tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Edge tier (STM32, ESP32, nRF52)
    Edge,
    /// Host tier (Pi, x86, MicroVM)
    Host,
    /// Accelerator tier (GPU, TPU, v1 Chip)
    Accel,
}

impl Tier {
    /// Get typical power draw for this tier in mW
    pub fn typical_power_mw(&self) -> u32 {
        match self {
            Self::Edge => 5,      // 5 mA * 3.3V
            Self::Host => 5000,   // 5W
            Self::Accel => 100000, // 100W
        }
    }

    /// Get typical wake latency for this tier in ms
    pub fn typical_wake_latency_ms(&self) -> u32 {
        match self {
            Self::Edge => 1,
            Self::Host => 100,
            Self::Accel => 500,
        }
    }
}

/// Node capabilities
#[derive(Debug, Clone, Default)]
pub struct NodeCapabilities {
    /// Has network access
    pub network: bool,
    /// Available accelerators
    pub accelerators: Vec<AcceleratorType>,
    /// Available capability types
    pub capability_types: Vec<CapabilityType>,
    /// Isolation level
    pub isolation_level: IsolationLevel,
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node identifier
    pub id: NodeId,
    /// Tier this node belongs to
    pub tier: Tier,
    /// Human-readable name
    pub name: String,
    /// Available memory in bytes
    pub available_memory: u64,
    /// Total memory in bytes
    pub total_memory: u64,
    /// Available CPU time in ms
    pub available_cpu_ms: u64,
    /// Power draw in mW
    pub power_draw_mw: u32,
    /// Wake latency in ms
    pub wake_latency_ms: u32,
    /// Current load (0-100)
    pub current_load: u8,
    /// Node capabilities
    pub capabilities: NodeCapabilities,
    /// Whether node is healthy
    pub healthy: bool,
    /// Capsules with local workspace
    pub local_workspaces: Vec<[u8; 16]>,
}

impl NodeInfo {
    pub fn new(id: NodeId, tier: Tier, name: impl Into<String>) -> Self {
        Self {
            id,
            tier,
            name: name.into(),
            available_memory: match tier {
                Tier::Edge => 256 * 1024,       // 256 KB
                Tier::Host => 4 * 1024 * 1024 * 1024, // 4 GB
                Tier::Accel => 16 * 1024 * 1024 * 1024, // 16 GB
            },
            total_memory: match tier {
                Tier::Edge => 256 * 1024,
                Tier::Host => 4 * 1024 * 1024 * 1024,
                Tier::Accel => 16 * 1024 * 1024 * 1024,
            },
            available_cpu_ms: 1_000_000, // 1000 seconds
            power_draw_mw: tier.typical_power_mw(),
            wake_latency_ms: tier.typical_wake_latency_ms(),
            current_load: 0,
            capabilities: NodeCapabilities::default(),
            healthy: true,
            local_workspaces: Vec::new(),
        }
    }

    /// Check if this node has a local workspace for the capsule
    pub fn has_local_workspace(&self, capsule_id: &[u8; 16]) -> bool {
        self.local_workspaces.contains(capsule_id)
    }

    /// Get memory utilization as percentage (0-100)
    pub fn memory_utilization(&self) -> u8 {
        if self.total_memory == 0 {
            return 0;
        }
        let used = self.total_memory - self.available_memory;
        ((used * 100) / self.total_memory) as u8
    }
}

/// Node registry for tracking available nodes
pub struct NodeRegistry {
    nodes: RwLock<HashMap<NodeId, NodeInfo>>,
    next_id: RwLock<u64>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Register a new node
    pub fn register(&self, mut node: NodeInfo) -> NodeId {
        let id = {
            let mut next = self.next_id.write().unwrap();
            let id = NodeId::new(*next);
            *next += 1;
            id
        };

        node.id = id;
        self.nodes.write().unwrap().insert(id, node);
        id
    }

    /// Register a node with a specific ID
    pub fn register_with_id(&self, node: NodeInfo) {
        self.nodes.write().unwrap().insert(node.id, node);
    }

    /// Unregister a node
    pub fn unregister(&self, id: NodeId) -> Option<NodeInfo> {
        self.nodes.write().unwrap().remove(&id)
    }

    /// Get a node by ID
    pub fn get(&self, id: NodeId) -> Option<NodeInfo> {
        self.nodes.read().unwrap().get(&id).cloned()
    }

    /// Get all nodes
    pub fn get_all(&self) -> Vec<NodeInfo> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    /// Get all healthy nodes
    pub fn get_healthy(&self) -> Vec<NodeInfo> {
        self.nodes
            .read()
            .unwrap()
            .values()
            .filter(|n| n.healthy)
            .cloned()
            .collect()
    }

    /// Get nodes by tier
    pub fn get_by_tier(&self, tier: Tier) -> Vec<NodeInfo> {
        self.nodes
            .read()
            .unwrap()
            .values()
            .filter(|n| n.tier == tier)
            .cloned()
            .collect()
    }

    /// Mark a node as unhealthy
    pub fn mark_unhealthy(&self, id: NodeId) {
        if let Some(node) = self.nodes.write().unwrap().get_mut(&id) {
            node.healthy = false;
        }
    }

    /// Mark a node as healthy
    pub fn mark_healthy(&self, id: NodeId) {
        if let Some(node) = self.nodes.write().unwrap().get_mut(&id) {
            node.healthy = true;
        }
    }

    /// Update node load
    pub fn update_load(&self, id: NodeId, load: u8) {
        if let Some(node) = self.nodes.write().unwrap().get_mut(&id) {
            node.current_load = load.min(100);
        }
    }

    /// Update available resources
    pub fn update_resources(&self, id: NodeId, memory: u64, cpu_ms: u64) {
        if let Some(node) = self.nodes.write().unwrap().get_mut(&id) {
            node.available_memory = memory;
            node.available_cpu_ms = cpu_ms;
        }
    }

    /// Get node count
    pub fn len(&self) -> usize {
        self.nodes.read().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.read().unwrap().is_empty()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
