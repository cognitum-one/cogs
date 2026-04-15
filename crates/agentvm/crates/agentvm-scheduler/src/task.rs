//! Task specification and classification for the heterogeneous fabric scheduler.
//!
//! This module defines:
//! - `TaskId` and `TaskSpec` for task identification and requirements
//! - `TaskClass` for tier-based routing decisions
//! - `ResourceRequirements` and `TaskConstraints` for placement constraints

use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

use crate::node::{IsolationLevel, NodeId, Tier};

/// Unique identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Create a new random task ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a task ID from a UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

/// Unique identifier for a capsule (agent container).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapsuleId(pub [u8; 16]);

impl CapsuleId {
    /// Create a new random capsule ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().into_bytes())
    }

    /// Create a capsule ID from bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl Default for CapsuleId {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of accelerator required by a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcceleratorType {
    /// NVIDIA GPU
    Gpu,
    /// Google TPU
    Tpu,
    /// Custom ASIC (v1 chip)
    CustomAsic,
    /// FPGA accelerator
    Fpga,
}

/// Storage requirements for a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageRequirements {
    /// Bytes to read
    pub read_bytes: u64,
    /// Bytes to write
    pub write_bytes: u64,
    /// Requires local storage (not network-attached)
    pub local_required: bool,
}

/// Resource requirements for scheduling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Estimated CPU time in milliseconds.
    pub cpu_ms: u64,
    /// Memory required in bytes.
    pub memory_bytes: u64,
    /// Whether network access is needed.
    pub network: bool,
    /// GPU/accelerator needed (if any).
    pub accelerator: Option<AcceleratorType>,
    /// Storage access requirements.
    pub storage: StorageRequirements,
}

impl ResourceRequirements {
    /// Create minimal resource requirements (for host-tier tasks).
    pub fn minimal() -> Self {
        Self {
            cpu_ms: 10,
            memory_bytes: 1024 * 1024, // 1 MB
            network: false,
            accelerator: None,
            storage: StorageRequirements::default(),
        }
    }

    /// Create minimal resource requirements for edge-tier tasks.
    pub fn minimal_edge() -> Self {
        Self {
            cpu_ms: 1,
            memory_bytes: 32 * 1024, // 32 KB - fits in edge nodes
            network: false,
            accelerator: None,
            storage: StorageRequirements::default(),
        }
    }

    /// Create resource requirements appropriate for a task class.
    pub fn for_task_class(class: TaskClass) -> Self {
        match class {
            // Edge tasks have minimal requirements
            TaskClass::Reflex | TaskClass::Gating | TaskClass::Sensor => Self::minimal_edge(),
            TaskClass::Anomaly => Self {
                cpu_ms: 5,
                memory_bytes: 64 * 1024, // 64 KB
                network: false,
                accelerator: None,
                storage: StorageRequirements::default(),
            },
            // Host tasks need more resources
            TaskClass::Network => Self {
                cpu_ms: 100,
                memory_bytes: 1024 * 1024, // 1 MB
                network: true,
                accelerator: None,
                storage: StorageRequirements::default(),
            },
            TaskClass::Cli | TaskClass::Repository => Self::minimal(),
            // Accelerator tasks need significant resources
            TaskClass::Inference => Self {
                cpu_ms: 1000,
                memory_bytes: 512 * 1024 * 1024, // 512 MB
                network: false,
                accelerator: None, // Accelerator is optional preference
                storage: StorageRequirements::default(),
            },
            TaskClass::HeavyCompute => Self {
                cpu_ms: 10000,
                memory_bytes: 1024 * 1024 * 1024, // 1 GB
                network: false,
                accelerator: Some(AcceleratorType::Gpu),
                storage: StorageRequirements::default(),
            },
        }
    }

    /// Check if these requirements are satisfied by available resources.
    pub fn satisfies(&self, available: &ResourceRequirements) -> bool {
        self.cpu_ms <= available.cpu_ms
            && self.memory_bytes <= available.memory_bytes
            && (!self.network || available.network)
            && self.accelerator == available.accelerator
    }
}

impl std::ops::AddAssign for ResourceRequirements {
    fn add_assign(&mut self, rhs: Self) {
        self.cpu_ms += rhs.cpu_ms;
        self.memory_bytes += rhs.memory_bytes;
        self.network = self.network || rhs.network;
        // For accelerator, keep the more demanding one
        if self.accelerator.is_none() {
            self.accelerator = rhs.accelerator;
        }
        self.storage.read_bytes += rhs.storage.read_bytes;
        self.storage.write_bytes += rhs.storage.write_bytes;
        self.storage.local_required = self.storage.local_required || rhs.storage.local_required;
    }
}

/// Task classification for tier selection.
///
/// Each task class has a preferred tier and constraints on where it can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskClass {
    /// Fast reflex response (edge tier) - sub-millisecond latency required.
    Reflex,
    /// Gating decision (edge tier) - decides whether to wake higher tiers.
    Gating,
    /// Anomaly detection (edge tier) - lightweight pattern matching.
    Anomaly,
    /// Sensor polling (edge tier) - periodic data collection.
    Sensor,
    /// Network operations (host tier) - HTTP/TCP/UDP.
    Network,
    /// CLI execution (host tier) - shell commands, scripts.
    Cli,
    /// Repository operations (host tier) - git, package managers.
    Repository,
    /// Model inference (accel tier) - ML model execution.
    Inference,
    /// Heavy computation (accel tier) - batch processing, training.
    HeavyCompute,
}

impl TaskClass {
    /// Get the preferred tier for this task class.
    ///
    /// This is the tier where the task will run most efficiently.
    pub fn preferred_tier(&self) -> Tier {
        match self {
            // Edge tier tasks - low latency, low power
            TaskClass::Reflex => Tier::Edge,
            TaskClass::Gating => Tier::Edge,
            TaskClass::Anomaly => Tier::Edge,
            TaskClass::Sensor => Tier::Edge,

            // Host tier tasks - general compute
            TaskClass::Network => Tier::Host,
            TaskClass::Cli => Tier::Host,
            TaskClass::Repository => Tier::Host,

            // Accelerator tier tasks - heavy compute
            TaskClass::Inference => Tier::Accel,
            TaskClass::HeavyCompute => Tier::Accel,
        }
    }

    /// Check if this task class can run on the given tier.
    ///
    /// Some tasks can fall back to higher-capability tiers, while others
    /// are strictly bound to specific tiers.
    pub fn can_run_on(&self, tier: Tier) -> bool {
        match (self, tier) {
            // Edge tasks can only run on edge (latency-critical)
            (TaskClass::Reflex | TaskClass::Gating | TaskClass::Sensor, Tier::Edge) => true,
            (TaskClass::Reflex | TaskClass::Gating | TaskClass::Sensor, _) => false,

            // Anomaly can run on edge or host (has fallback)
            (TaskClass::Anomaly, Tier::Edge | Tier::Host) => true,
            (TaskClass::Anomaly, Tier::Accel) => false,

            // Network/CLI/Repository require host or higher
            (TaskClass::Network | TaskClass::Cli | TaskClass::Repository, Tier::Host) => true,
            (TaskClass::Network | TaskClass::Cli | TaskClass::Repository, Tier::Accel) => true,
            (TaskClass::Network | TaskClass::Cli | TaskClass::Repository, Tier::Edge) => false,

            // Inference prefers accel but can fall back to host
            (TaskClass::Inference, Tier::Accel | Tier::Host) => true,
            (TaskClass::Inference, Tier::Edge) => false,

            // Heavy compute requires accel
            (TaskClass::HeavyCompute, Tier::Accel) => true,
            (TaskClass::HeavyCompute, _) => false,
        }
    }

    /// Get the maximum acceptable latency for this task class in milliseconds.
    pub fn max_latency_ms(&self) -> u64 {
        match self {
            TaskClass::Reflex => 1,
            TaskClass::Gating => 5,
            TaskClass::Anomaly => 100,
            TaskClass::Sensor => 1000,
            TaskClass::Network => 5000,
            TaskClass::Cli => 30000,
            TaskClass::Repository => 60000,
            TaskClass::Inference => 10000,
            TaskClass::HeavyCompute => 300000,
        }
    }
}

/// Required capability types for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityType {
    /// Network HTTP access
    NetworkHttp,
    /// Network raw socket access
    NetworkSocket,
    /// File read access
    FileRead,
    /// File write access
    FileWrite,
    /// Process spawn capability
    ProcessSpawn,
    /// Secret read access
    SecretRead,
    /// GPU compute access
    GpuCompute,
    /// Custom capability
    Custom(String),
}

/// Task constraints for placement decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskConstraints {
    /// Required tier (if specified).
    pub tier_affinity: Option<Tier>,
    /// Tiers to avoid.
    pub tier_anti_affinity: Vec<Tier>,
    /// Preferred nodes (soft constraint).
    pub node_affinity: Vec<NodeId>,
    /// Nodes to avoid.
    pub node_anti_affinity: Vec<NodeId>,
    /// Co-locate with these tasks.
    pub pod_affinity: Vec<TaskId>,
    /// Don't co-locate with these tasks.
    pub pod_anti_affinity: Vec<TaskId>,
    /// Maximum power budget in milliwatts.
    pub power_budget_mw: Option<u32>,
    /// Required security isolation level.
    pub isolation: IsolationLevel,
}

impl TaskConstraints {
    /// Create constraints with no restrictions.
    pub fn unconstrained() -> Self {
        Self::default()
    }

    /// Create constraints requiring a specific tier.
    pub fn with_tier(tier: Tier) -> Self {
        Self {
            tier_affinity: Some(tier),
            ..Default::default()
        }
    }

    /// Create constraints with power budget.
    pub fn with_power_budget(power_mw: u32) -> Self {
        Self {
            power_budget_mw: Some(power_mw),
            ..Default::default()
        }
    }
}

/// Complete task specification for scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique task identifier.
    pub id: TaskId,
    /// Capsule this task belongs to.
    pub capsule_id: CapsuleId,
    /// Task classification.
    pub class: TaskClass,
    /// Resource requirements.
    pub resources: ResourceRequirements,
    /// Required capabilities.
    pub capabilities: Vec<CapabilityType>,
    /// Placement constraints.
    pub constraints: TaskConstraints,
    /// Priority (higher = more urgent, 0-1000).
    pub priority: u32,
    /// Deadline (optional).
    #[serde(skip)]
    pub deadline: Option<Instant>,
    /// Node currently running this task (if scheduled).
    pub current_node: Option<NodeId>,
}

impl TaskSpec {
    /// Create a new task specification with default resources for the task class.
    pub fn new(capsule_id: CapsuleId, class: TaskClass) -> Self {
        Self {
            id: TaskId::new(),
            capsule_id,
            class,
            resources: ResourceRequirements::for_task_class(class),
            capabilities: Vec::new(),
            constraints: TaskConstraints::unconstrained(),
            priority: 100,
            deadline: None,
            current_node: None,
        }
    }

    /// Builder method to set resource requirements.
    pub fn with_resources(mut self, resources: ResourceRequirements) -> Self {
        self.resources = resources;
        self
    }

    /// Builder method to set constraints.
    pub fn with_constraints(mut self, constraints: TaskConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Builder method to set priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority.min(1000);
        self
    }

    /// Builder method to set deadline.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Builder method to add a capability requirement.
    pub fn with_capability(mut self, capability: CapabilityType) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Check if this task is deadline-critical.
    pub fn is_deadline_critical(&self) -> bool {
        if let Some(deadline) = self.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            remaining.as_millis() < self.class.max_latency_ms() as u128 * 2
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_creation() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_class_preferred_tier() {
        assert_eq!(TaskClass::Reflex.preferred_tier(), Tier::Edge);
        assert_eq!(TaskClass::Network.preferred_tier(), Tier::Host);
        assert_eq!(TaskClass::Inference.preferred_tier(), Tier::Accel);
    }

    #[test]
    fn test_task_class_can_run_on() {
        // Reflex can only run on edge
        assert!(TaskClass::Reflex.can_run_on(Tier::Edge));
        assert!(!TaskClass::Reflex.can_run_on(Tier::Host));
        assert!(!TaskClass::Reflex.can_run_on(Tier::Accel));

        // Anomaly can run on edge or host
        assert!(TaskClass::Anomaly.can_run_on(Tier::Edge));
        assert!(TaskClass::Anomaly.can_run_on(Tier::Host));
        assert!(!TaskClass::Anomaly.can_run_on(Tier::Accel));

        // Network requires host or higher
        assert!(!TaskClass::Network.can_run_on(Tier::Edge));
        assert!(TaskClass::Network.can_run_on(Tier::Host));
        assert!(TaskClass::Network.can_run_on(Tier::Accel));

        // HeavyCompute requires accel
        assert!(!TaskClass::HeavyCompute.can_run_on(Tier::Edge));
        assert!(!TaskClass::HeavyCompute.can_run_on(Tier::Host));
        assert!(TaskClass::HeavyCompute.can_run_on(Tier::Accel));
    }

    #[test]
    fn test_resource_requirements_add() {
        let mut req1 = ResourceRequirements {
            cpu_ms: 100,
            memory_bytes: 1024,
            network: false,
            accelerator: None,
            storage: StorageRequirements::default(),
        };

        let req2 = ResourceRequirements {
            cpu_ms: 50,
            memory_bytes: 512,
            network: true,
            accelerator: Some(AcceleratorType::Gpu),
            storage: StorageRequirements {
                read_bytes: 100,
                write_bytes: 50,
                local_required: true,
            },
        };

        req1 += req2;

        assert_eq!(req1.cpu_ms, 150);
        assert_eq!(req1.memory_bytes, 1536);
        assert!(req1.network);
        assert_eq!(req1.accelerator, Some(AcceleratorType::Gpu));
        assert_eq!(req1.storage.read_bytes, 100);
        assert!(req1.storage.local_required);
    }

    #[test]
    fn test_task_spec_builder() {
        let capsule_id = CapsuleId::new();
        let task = TaskSpec::new(capsule_id, TaskClass::Network)
            .with_priority(500)
            .with_capability(CapabilityType::NetworkHttp)
            .with_constraints(TaskConstraints::with_tier(Tier::Host));

        assert_eq!(task.priority, 500);
        assert_eq!(task.capabilities.len(), 1);
        assert_eq!(task.constraints.tier_affinity, Some(Tier::Host));
    }
}
