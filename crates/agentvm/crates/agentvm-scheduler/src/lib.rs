//! Heterogeneous Fabric Scheduler for Agentic VM
//!
//! This crate implements the fabric scheduler as specified in ADR-007,
//! providing task placement across a heterogeneous compute fabric spanning:
//!
//! - **Edge tier**: Always-on low-power devices (STM32, ESP32, nRF52)
//! - **Host tier**: General-purpose compute (Raspberry Pi, x86, MicroVMs)
//! - **Accelerator tier**: High-performance devices (GPU, TPU, custom ASICs)
//!
//! # Architecture
//!
//! The scheduler uses a four-phase algorithm inspired by Kubernetes:
//!
//! 1. **Filter**: Eliminate infeasible nodes using filter plugins
//! 2. **Score**: Rank feasible nodes using weighted score plugins
//! 3. **Select**: Choose the highest-scoring node
//! 4. **Bind**: Assign the task to the selected node
//!
//! # Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use agentvm_scheduler::{
//!     FabricScheduler, NodeRegistry, NodeInfo, Tier,
//!     TaskSpec, TaskClass, CapsuleId,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create node registry and add nodes
//!     let registry = Arc::new(NodeRegistry::new());
//!     registry.add(NodeInfo::new("edge-1", Tier::Edge)).await?;
//!     registry.add(NodeInfo::new("host-1", Tier::Host)).await?;
//!     registry.add(NodeInfo::new("accel-1", Tier::Accel)).await?;
//!
//!     // Create scheduler
//!     let scheduler = FabricScheduler::new(registry);
//!
//!     // Create and schedule a task
//!     let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
//!     let placement = scheduler.schedule(task).await?;
//!
//!     println!("Task placed on node {} (tier {:?})", placement.node_id, placement.tier);
//!     Ok(())
//! }
//! ```
//!
//! # Modules
//!
//! - [`task`]: Task specification and classification
//! - [`node`]: Node registry and tier definitions
//! - [`filter`]: Filter plugins for eliminating infeasible nodes
//! - [`score`]: Score plugins for ranking nodes
//! - [`scheduler`]: Main scheduler implementation
//! - [`placement`]: Placement decisions and history
//! - [`metrics`]: Scheduler metrics and observability

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod filter;
pub mod metrics;
pub mod node;
pub mod placement;
pub mod scheduler;
pub mod score;
pub mod task;

// Re-export commonly used types from filter module
pub use filter::{
    CapabilityFilter, FilterChain, FilterPlugin, FilterResult, HealthFilter, IsolationFilter,
    NodeAffinityFilter, PowerFilter, ResourceFilter, TierFilter,
};

// Re-export commonly used types from metrics module
pub use metrics::{LatencyHistogram, MetricsAggregator, SchedulerMetrics};

// Re-export commonly used types from node module
pub use node::{
    IsolationLevel, NodeCapabilities, NodeHealth, NodeId, NodeInfo, NodeRegistry,
    NodeRegistryError, Tier,
};

// Re-export commonly used types from placement module
pub use placement::{Placement, PlacementHistory, PlacementRecord, PlacementStats, RescheduleReason};

// Re-export commonly used types from scheduler module
pub use scheduler::{
    FabricScheduler, PreemptError, ScheduleError, SchedulerConfig, PREEMPTION_THRESHOLD,
};

// Re-export commonly used types from score module
pub use score::{
    HealthScore, LatencyScore, LoadBalanceScore, LocalityScore, MemoryScore, PowerScore,
    RiskScore, Score, ScoreChain, ScorePlugin, ScoredNode, TierPreferenceScore,
};

// Re-export commonly used types from task module
pub use task::{
    AcceleratorType, CapabilityType, CapsuleId, ResourceRequirements, StorageRequirements,
    TaskClass, TaskConstraints, TaskId, TaskSpec,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Create a new scheduler with default configuration.
///
/// This is a convenience function that creates a scheduler with
/// the default filter and score chains.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use agentvm_scheduler::{create_scheduler, NodeRegistry, NodeInfo, Tier};
///
/// #[tokio::main]
/// async fn main() {
///     let registry = Arc::new(NodeRegistry::new());
///     registry.add(NodeInfo::new("host-1", Tier::Host)).await.unwrap();
///
///     let scheduler = create_scheduler(registry);
/// }
/// ```
pub fn create_scheduler(registry: std::sync::Arc<NodeRegistry>) -> FabricScheduler {
    FabricScheduler::new(registry)
}

/// Create a scheduler with custom configuration.
pub fn create_scheduler_with_config(
    registry: std::sync::Arc<NodeRegistry>,
    config: SchedulerConfig,
) -> FabricScheduler {
    FabricScheduler::with_config(registry, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[tokio::test]
    async fn test_create_scheduler() {
        let registry = std::sync::Arc::new(NodeRegistry::new());
        let _scheduler = create_scheduler(registry);
    }

    #[tokio::test]
    async fn test_full_scheduling_flow() {
        // Create registry with all tier types
        let registry = std::sync::Arc::new(NodeRegistry::new());
        registry
            .add(NodeInfo::new("edge-1", Tier::Edge))
            .await
            .unwrap();
        registry
            .add(NodeInfo::new("host-1", Tier::Host))
            .await
            .unwrap();
        registry
            .add(NodeInfo::new("accel-1", Tier::Accel))
            .await
            .unwrap();

        let scheduler = create_scheduler(registry);

        // Test different task classes
        let test_cases = vec![
            (TaskClass::Reflex, Tier::Edge),
            (TaskClass::Gating, Tier::Edge),
            (TaskClass::Network, Tier::Host),
            (TaskClass::Cli, Tier::Host),
            (TaskClass::HeavyCompute, Tier::Accel),
        ];

        for (class, expected_tier) in test_cases {
            let task = TaskSpec::new(CapsuleId::new(), class);
            let placement = scheduler.schedule(task).await.unwrap();
            assert_eq!(
                placement.tier, expected_tier,
                "Task class {:?} should be placed on {:?}",
                class, expected_tier
            );
        }
    }
}
