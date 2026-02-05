//! Task types for scheduling

use crate::node::{NodeId, Tier};
use agentvm_types::CapabilityType;

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Task classification for tier selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    /// Fast reflex response (edge tier)
    Reflex,
    /// Gating decision (edge tier)
    Gating,
    /// Anomaly detection (edge tier)
    Anomaly,
    /// Sensor polling (edge tier)
    Sensor,
    /// Network operations (host tier)
    Network,
    /// CLI execution (host tier)
    Cli,
    /// Repository operations (host tier)
    Repository,
    /// Model inference (accel tier)
    Inference,
    /// Heavy computation (accel tier)
    HeavyCompute,
}

impl TaskClass {
    /// Get preferred tier for this task class
    pub fn preferred_tier(&self) -> Tier {
        match self {
            Self::Reflex | Self::Gating | Self::Anomaly | Self::Sensor => Tier::Edge,
            Self::Network | Self::Cli | Self::Repository => Tier::Host,
            Self::Inference | Self::HeavyCompute => Tier::Accel,
        }
    }

    /// Check if this task class can run on the given tier
    pub fn can_run_on(&self, tier: Tier) -> bool {
        match (self, tier) {
            // Edge-only tasks
            (Self::Reflex | Self::Gating | Self::Sensor, Tier::Edge) => true,
            (Self::Reflex | Self::Gating | Self::Sensor, _) => false,

            // Anomaly can run on edge or host
            (Self::Anomaly, Tier::Edge | Tier::Host) => true,
            (Self::Anomaly, _) => false,

            // Host tasks can also run on accel tier
            (Self::Network | Self::Cli | Self::Repository, Tier::Host | Tier::Accel) => true,
            (Self::Network | Self::Cli | Self::Repository, _) => false,

            // Inference prefers accel but can fall back to host
            (Self::Inference, Tier::Accel | Tier::Host) => true,
            (Self::Inference, _) => false,

            // Heavy compute requires accel
            (Self::HeavyCompute, Tier::Accel) => true,
            (Self::HeavyCompute, _) => false,
        }
    }
}

/// Accelerator type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceleratorType {
    Gpu,
    Tpu,
    V1Chip,
    Custom(u32),
}

/// Storage requirements
#[derive(Debug, Clone, Default)]
pub struct StorageRequirements {
    /// Bytes of local storage needed
    pub local_bytes: u64,
    /// Whether persistent storage is required
    pub persistent: bool,
}

/// Resource requirements
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    /// CPU time estimate (ms)
    pub cpu_ms: u64,
    /// Memory required (bytes)
    pub memory_bytes: u64,
    /// Network access needed
    pub network: bool,
    /// GPU/accelerator needed
    pub accelerator: Option<AcceleratorType>,
    /// Storage requirements
    pub storage: StorageRequirements,
}

impl ResourceRequirements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cpu(mut self, cpu_ms: u64) -> Self {
        self.cpu_ms = cpu_ms;
        self
    }

    pub fn with_memory(mut self, memory_bytes: u64) -> Self {
        self.memory_bytes = memory_bytes;
        self
    }

    pub fn with_network(mut self, network: bool) -> Self {
        self.network = network;
        self
    }

    pub fn with_accelerator(mut self, accelerator: AcceleratorType) -> Self {
        self.accelerator = Some(accelerator);
        self
    }

    /// Check if requirements can be satisfied by available resources
    pub fn satisfied_by(&self, available: &ResourceRequirements) -> bool {
        if self.cpu_ms > available.cpu_ms {
            return false;
        }
        if self.memory_bytes > available.memory_bytes {
            return false;
        }
        if self.network && !available.network {
            return false;
        }
        if let Some(accel) = &self.accelerator {
            if available.accelerator.as_ref() != Some(accel) {
                return false;
            }
        }
        true
    }
}

/// Isolation level for security
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsolationLevel {
    /// Process-level isolation
    Process,
    /// VM-level isolation
    Vm,
    /// Hardware-level isolation
    Hardware,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::Vm
    }
}

/// Task constraints for scheduling
#[derive(Debug, Clone, Default)]
pub struct TaskConstraints {
    /// Required tier (if specified)
    pub tier_affinity: Option<Tier>,
    /// Avoid these tiers
    pub tier_anti_affinity: Vec<Tier>,
    /// Node affinity (specific nodes)
    pub node_affinity: Vec<NodeId>,
    /// Don't place on these nodes
    pub node_anti_affinity: Vec<NodeId>,
    /// Co-locate with these tasks
    pub pod_affinity: Vec<TaskId>,
    /// Don't co-locate with these tasks
    pub pod_anti_affinity: Vec<TaskId>,
    /// Maximum power budget (mW)
    pub power_budget_mw: Option<u32>,
    /// Security isolation level
    pub isolation: IsolationLevel,
}

impl TaskConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tier_affinity(mut self, tier: Tier) -> Self {
        self.tier_affinity = Some(tier);
        self
    }

    pub fn with_tier_anti_affinity(mut self, tier: Tier) -> Self {
        self.tier_anti_affinity.push(tier);
        self
    }

    pub fn with_node_affinity(mut self, node_id: NodeId) -> Self {
        self.node_affinity.push(node_id);
        self
    }

    pub fn with_node_anti_affinity(mut self, node_id: NodeId) -> Self {
        self.node_anti_affinity.push(node_id);
        self
    }

    pub fn with_power_budget(mut self, power_mw: u32) -> Self {
        self.power_budget_mw = Some(power_mw);
        self
    }

    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.isolation = level;
        self
    }
}

/// Task specification for scheduling
#[derive(Debug, Clone)]
pub struct TaskSpec {
    /// Unique task identifier
    pub id: TaskId,
    /// Capsule this task belongs to
    pub capsule_id: [u8; 16],
    /// Task classification
    pub class: TaskClass,
    /// Resource requirements
    pub resources: ResourceRequirements,
    /// Capability requirements
    pub capabilities: Vec<CapabilityType>,
    /// Constraints
    pub constraints: TaskConstraints,
    /// Priority (higher = more urgent)
    pub priority: u32,
    /// Deadline in nanoseconds (optional)
    pub deadline_ns: Option<u64>,
}

impl TaskSpec {
    pub fn new(id: TaskId, class: TaskClass) -> Self {
        Self {
            id,
            capsule_id: [0u8; 16],
            class,
            resources: ResourceRequirements::default(),
            capabilities: Vec::new(),
            constraints: TaskConstraints::default(),
            priority: 0,
            deadline_ns: None,
        }
    }

    pub fn with_capsule(mut self, capsule_id: [u8; 16]) -> Self {
        self.capsule_id = capsule_id;
        self
    }

    pub fn with_resources(mut self, resources: ResourceRequirements) -> Self {
        self.resources = resources;
        self
    }

    pub fn with_capability(mut self, cap: CapabilityType) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn with_constraints(mut self, constraints: TaskConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_deadline(mut self, deadline_ns: u64) -> Self {
        self.deadline_ns = Some(deadline_ns);
        self
    }
}
