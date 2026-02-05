//! Main fabric scheduler implementation.
//!
//! The `FabricScheduler` implements a four-phase scheduling algorithm:
//! 1. **Filter**: Eliminate infeasible nodes
//! 2. **Score**: Rank feasible nodes
//! 3. **Select**: Choose the best node
//! 4. **Bind**: Assign the task to the node

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::filter::{FilterChain, FilterResult};
use crate::metrics::SchedulerMetrics;
use crate::node::{NodeId, NodeInfo, NodeRegistry, NodeRegistryError};
use crate::placement::{Placement, PlacementHistory, RescheduleReason};
use crate::score::{ScoreChain, ScoredNode};
use crate::task::{ResourceRequirements, TaskId, TaskSpec};

/// Errors that can occur during scheduling.
#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    /// No nodes pass the filter phase.
    #[error("No feasible nodes available for task")]
    NoFeasibleNodes,

    /// Scoring phase failed (no nodes could be scored).
    #[error("Scoring failed: {0}")]
    ScoringFailed(String),

    /// Failed to bind task to node.
    #[error("Failed to bind task to node: {0}")]
    BindFailed(String),

    /// Node registry error.
    #[error("Node registry error: {0}")]
    NodeRegistry(#[from] NodeRegistryError),

    /// Task not found.
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    /// Internal scheduler error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Errors that can occur during preemption.
#[derive(Debug, thiserror::Error)]
pub enum PreemptError {
    /// Task priority too low for preemption.
    #[error("Task priority too low for preemption")]
    PriorityTooLow,

    /// Not enough lower-priority tasks to preempt.
    #[error("Insufficient victims for preemption")]
    InsufficientVictims,

    /// Preemption failed.
    #[error("Preemption failed: {0}")]
    Failed(String),
}

/// Minimum priority required for preemption.
pub const PREEMPTION_THRESHOLD: u32 = 500;

/// Configuration for the fabric scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Enable preemption for high-priority tasks.
    pub enable_preemption: bool,
    /// Maximum number of scheduling attempts per task.
    pub max_attempts: u32,
    /// Default timeout for binding (milliseconds).
    pub bind_timeout_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enable_preemption: true,
            max_attempts: 3,
            bind_timeout_ms: 5000,
        }
    }
}

/// The main fabric scheduler.
///
/// Implements a four-phase scheduling algorithm inspired by Kubernetes,
/// extended with power-awareness and heterogeneous tier support.
pub struct FabricScheduler {
    /// Node registry.
    node_registry: Arc<NodeRegistry>,
    /// Filter chain.
    filter_chain: FilterChain,
    /// Score chain.
    score_chain: ScoreChain,
    /// Placement history.
    placement_history: Arc<PlacementHistory>,
    /// Scheduler metrics.
    metrics: Arc<RwLock<SchedulerMetrics>>,
    /// Configuration.
    config: SchedulerConfig,
}

impl FabricScheduler {
    /// Create a new fabric scheduler.
    pub fn new(node_registry: Arc<NodeRegistry>) -> Self {
        Self {
            node_registry,
            filter_chain: FilterChain::default_chain(),
            score_chain: ScoreChain::default_chain(),
            placement_history: Arc::new(PlacementHistory::default()),
            metrics: Arc::new(RwLock::new(SchedulerMetrics::new())),
            config: SchedulerConfig::default(),
        }
    }

    /// Create a new scheduler with custom configuration.
    pub fn with_config(node_registry: Arc<NodeRegistry>, config: SchedulerConfig) -> Self {
        Self {
            node_registry,
            filter_chain: FilterChain::default_chain(),
            score_chain: ScoreChain::default_chain(),
            placement_history: Arc::new(PlacementHistory::default()),
            metrics: Arc::new(RwLock::new(SchedulerMetrics::new())),
            config,
        }
    }

    /// Set custom filter chain.
    pub fn with_filters(mut self, filter_chain: FilterChain) -> Self {
        self.filter_chain = filter_chain;
        self
    }

    /// Set custom score chain.
    pub fn with_scorers(mut self, score_chain: ScoreChain) -> Self {
        self.score_chain = score_chain;
        self
    }

    /// Schedule a task to an appropriate node.
    ///
    /// This implements the four-phase scheduling algorithm:
    /// 1. Filter - eliminate infeasible nodes
    /// 2. Score - rank feasible nodes
    /// 3. Select - choose best node
    /// 4. Bind - assign task to node
    pub async fn schedule(&self, task: TaskSpec) -> Result<Placement, ScheduleError> {
        let start = std::time::Instant::now();

        tracing::debug!(
            task = %task.id,
            class = ?task.class,
            priority = task.priority,
            "Starting scheduling"
        );

        // Phase 1: Filter - eliminate infeasible nodes
        let feasible_nodes = self.filter_nodes(&task).await?;

        if feasible_nodes.is_empty() {
            // Try preemption if enabled
            if self.config.enable_preemption && task.priority >= PREEMPTION_THRESHOLD {
                tracing::debug!(task = %task.id, "No feasible nodes, attempting preemption");
                if let Ok(preempted) = self.preempt_for_task(&task).await {
                    tracing::info!(
                        task = %task.id,
                        preempted = ?preempted,
                        "Preempted tasks to make room"
                    );
                    // Retry scheduling
                    return Box::pin(self.schedule(task)).await;
                }
            }

            self.metrics.write().await.record_filter_rejection("no_feasible_nodes");
            return Err(ScheduleError::NoFeasibleNodes);
        }

        tracing::debug!(
            task = %task.id,
            feasible_count = feasible_nodes.len(),
            "Filter phase complete"
        );

        // Phase 2: Score - rank feasible nodes
        let scored_nodes = self.score_nodes(&task, &feasible_nodes).await;

        // Phase 3: Select - choose best node
        let selected = scored_nodes
            .into_iter()
            .next()
            .ok_or_else(|| ScheduleError::ScoringFailed("No nodes scored".to_string()))?;

        tracing::debug!(
            task = %task.id,
            node = %selected.node.id,
            score = selected.total_score,
            "Selected node"
        );

        // Phase 4: Bind - assign task to node
        let placement = self.bind_task(&task, &selected).await?;

        // Record metrics
        let duration = start.elapsed();
        self.metrics
            .write()
            .await
            .record_scheduling(placement.tier, duration);

        tracing::info!(
            task = %task.id,
            node = %placement.node_id,
            tier = ?placement.tier,
            score = placement.score,
            duration_ms = duration.as_millis(),
            "Task scheduled successfully"
        );

        Ok(placement)
    }

    /// Filter nodes to find feasible candidates.
    async fn filter_nodes(&self, task: &TaskSpec) -> Result<Vec<NodeInfo>, ScheduleError> {
        let all_nodes = self.node_registry.get_all().await;

        let feasible: Vec<NodeInfo> = all_nodes
            .into_iter()
            .filter(|node| {
                let result = self.filter_chain.filter(task, node);
                if let FilterResult::Reject(reason) = &result {
                    self.record_filter_rejection_sync(&reason);
                }
                result.is_accept()
            })
            .collect();

        Ok(feasible)
    }

    /// Record filter rejection (sync version for use in iterator).
    fn record_filter_rejection_sync(&self, reason: &str) {
        // Extract the filter name from the reason
        let filter_name = reason.split(':').next().unwrap_or(reason);
        // This is a best-effort metric recording
        // In production, we'd use a lock-free counter
        tracing::trace!(filter = filter_name, "Filter rejection");
    }

    /// Score nodes and return them sorted by score.
    async fn score_nodes(&self, task: &TaskSpec, nodes: &[NodeInfo]) -> Vec<ScoredNode> {
        self.score_chain.score_nodes(task, nodes)
    }

    /// Bind a task to the selected node.
    async fn bind_task(
        &self,
        task: &TaskSpec,
        scored_node: &ScoredNode,
    ) -> Result<Placement, ScheduleError> {
        let node_id = scored_node.node.id;
        let tier = scored_node.node.tier;

        // Update node load
        let new_running_tasks = scored_node.node.running_tasks + 1;
        let new_load = ((new_running_tasks as f64
            / scored_node.node.capabilities.max_concurrent_tasks as f64)
            * 100.0) as u8;

        self.node_registry
            .update_load(node_id, new_load, new_running_tasks)
            .await?;

        // Update node available memory
        let new_memory = scored_node
            .node
            .available_memory
            .saturating_sub(task.resources.memory_bytes);
        self.node_registry
            .update_memory(node_id, new_memory)
            .await?;

        // Create placement
        let placement = Placement::new(task.id, node_id, tier, scored_node.total_score)
            .with_reason(format!(
                "Score: {}, Node: {}",
                scored_node.total_score, scored_node.node.name
            ));

        // Record in history
        self.placement_history.record(placement.clone()).await;

        Ok(placement)
    }

    /// Handle node failure by rescheduling affected tasks.
    pub async fn handle_node_failure(&self, node_id: NodeId) -> Result<Vec<Placement>, ScheduleError> {
        // Mark node as unhealthy
        self.node_registry.mark_unhealthy(node_id).await?;

        // Get affected tasks
        let affected_tasks = self.placement_history.get_tasks_on_node(node_id).await;

        tracing::warn!(
            node = %node_id,
            affected_count = affected_tasks.len(),
            "Node failure detected, rescheduling tasks"
        );

        let new_placements = Vec::new();

        for task_id in affected_tasks {
            // Mark as rescheduled in history
            self.placement_history
                .reschedule(task_id, RescheduleReason::NodeFailure(node_id))
                .await;

            // Update metrics
            self.metrics.write().await.record_reschedule();

            // Note: In a real system, we'd need to retrieve the full TaskSpec
            // from a task store. For now, we just log the reschedule.
            tracing::info!(
                task = %task_id,
                "Task marked for rescheduling due to node failure"
            );
        }

        Ok(new_placements)
    }

    /// Reschedule a task, avoiding its current node.
    pub async fn reschedule_task(&self, mut task: TaskSpec) -> Result<Placement, ScheduleError> {
        if let Some(current_node) = task.current_node {
            // Avoid the failed node
            if !task.constraints.node_anti_affinity.contains(&current_node) {
                task.constraints.node_anti_affinity.push(current_node);
            }
        }

        self.schedule(task).await
    }

    /// Attempt to preempt lower-priority tasks to make room for a high-priority task.
    pub async fn preempt_for_task(&self, task: &TaskSpec) -> Result<Vec<TaskId>, PreemptError> {
        if task.priority < PREEMPTION_THRESHOLD {
            return Err(PreemptError::PriorityTooLow);
        }

        let candidates = self.find_preemption_candidates(task).await;

        if candidates.is_empty() {
            return Err(PreemptError::InsufficientVictims);
        }

        let mut preempted = Vec::new();
        let mut freed_resources = ResourceRequirements::default();

        for (task_id, resources, priority) in candidates {
            // Can't preempt higher or equal priority
            if priority >= task.priority {
                continue;
            }

            preempted.push(task_id);
            freed_resources += resources;

            // Check if we've freed enough resources
            if freed_resources.satisfies(&task.resources) {
                break;
            }
        }

        if !freed_resources.satisfies(&task.resources) {
            return Err(PreemptError::InsufficientVictims);
        }

        // Actually preempt the tasks
        for task_id in &preempted {
            self.preempt_task(*task_id).await?;
        }

        self.metrics.write().await.record_preemption(preempted.len());

        Ok(preempted)
    }

    /// Find tasks that could be preempted.
    async fn find_preemption_candidates(
        &self,
        _task: &TaskSpec,
    ) -> Vec<(TaskId, ResourceRequirements, u32)> {
        let _active = self.placement_history.get_active().await;

        // For now, return empty since we don't have full task specs stored
        // In production, this would query a task store
        let candidates: Vec<(TaskId, ResourceRequirements, u32)> = Vec::new();

        candidates
    }

    /// Preempt a single task.
    async fn preempt_task(&self, task_id: TaskId) -> Result<(), PreemptError> {
        // Mark as rescheduled
        self.placement_history
            .reschedule(task_id, RescheduleReason::Preemption)
            .await;

        tracing::info!(task = %task_id, "Task preempted");

        Ok(())
    }

    /// Mark a task as completed.
    pub async fn complete_task(&self, task_id: TaskId) -> Result<(), ScheduleError> {
        let record = self.placement_history.complete(task_id).await;

        if record.is_none() {
            return Err(ScheduleError::TaskNotFound(task_id));
        }

        let record = record.unwrap();

        // Update node resources
        // In production, we'd release the resources back to the node
        if let Ok(node) = self
            .node_registry
            .get(record.placement.node_id)
            .await
            .ok_or(ScheduleError::Internal("Node not found".to_string()))
        {
            let new_running = node.running_tasks.saturating_sub(1);
            let new_load = if node.capabilities.max_concurrent_tasks > 0 {
                ((new_running as f64 / node.capabilities.max_concurrent_tasks as f64) * 100.0)
                    as u8
            } else {
                0
            };

            self.node_registry
                .update_load(record.placement.node_id, new_load, new_running)
                .await
                .ok();
        }

        Ok(())
    }

    /// Get placement history.
    pub fn placement_history(&self) -> Arc<PlacementHistory> {
        Arc::clone(&self.placement_history)
    }

    /// Get scheduler metrics.
    pub async fn get_metrics(&self) -> SchedulerMetrics {
        self.metrics.read().await.clone()
    }

    /// Export metrics (for observability).
    pub async fn export_metrics(&self) -> SchedulerMetrics {
        self.get_metrics().await
    }

    /// Get the node registry.
    pub fn node_registry(&self) -> Arc<NodeRegistry> {
        Arc::clone(&self.node_registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Tier;
    use crate::task::{CapsuleId, TaskClass};

    async fn create_test_scheduler() -> FabricScheduler {
        let registry = Arc::new(NodeRegistry::new());

        // Add some test nodes
        let edge_node = NodeInfo::new("edge-1", Tier::Edge);
        let host_node = NodeInfo::new("host-1", Tier::Host);
        let accel_node = NodeInfo::new("accel-1", Tier::Accel);

        registry.add(edge_node).await.unwrap();
        registry.add(host_node).await.unwrap();
        registry.add(accel_node).await.unwrap();

        FabricScheduler::new(registry)
    }

    #[tokio::test]
    async fn test_schedule_network_task() {
        let scheduler = create_test_scheduler().await;

        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let placement = scheduler.schedule(task).await.unwrap();

        // Network tasks should be scheduled on Host tier
        assert_eq!(placement.tier, Tier::Host);
    }

    #[tokio::test]
    async fn test_schedule_reflex_task() {
        let scheduler = create_test_scheduler().await;

        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Reflex);
        let placement = scheduler.schedule(task).await.unwrap();

        // Reflex tasks must run on Edge tier
        assert_eq!(placement.tier, Tier::Edge);
    }

    #[tokio::test]
    async fn test_schedule_inference_task() {
        let scheduler = create_test_scheduler().await;

        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Inference);
        let placement = scheduler.schedule(task).await.unwrap();

        // Inference prefers Accel but can run on Host
        assert!(matches!(placement.tier, Tier::Accel | Tier::Host));
    }

    #[tokio::test]
    async fn test_schedule_heavy_compute_task() {
        let scheduler = create_test_scheduler().await;

        let task = TaskSpec::new(CapsuleId::new(), TaskClass::HeavyCompute);
        let placement = scheduler.schedule(task).await.unwrap();

        // Heavy compute requires Accel tier
        assert_eq!(placement.tier, Tier::Accel);
    }

    #[tokio::test]
    async fn test_no_feasible_nodes() {
        let registry = Arc::new(NodeRegistry::new());
        // Only add edge node
        let edge_node = NodeInfo::new("edge-1", Tier::Edge);
        registry.add(edge_node).await.unwrap();

        let scheduler = FabricScheduler::new(registry);

        // Network task cannot run on edge
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let result = scheduler.schedule(task).await;

        assert!(matches!(result, Err(ScheduleError::NoFeasibleNodes)));
    }

    #[tokio::test]
    async fn test_placement_history() {
        let scheduler = create_test_scheduler().await;

        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let task_id = task.id;

        let placement = scheduler.schedule(task).await.unwrap();
        assert_eq!(placement.task_id, task_id);

        // Check placement is recorded
        let history = scheduler.placement_history();
        let current = history.get_current(task_id).await;
        assert!(current.is_some());

        // Complete the task
        scheduler.complete_task(task_id).await.unwrap();

        // Should no longer be active
        let current = history.get_current(task_id).await;
        assert!(current.is_none());
    }

    #[tokio::test]
    async fn test_node_failure_handling() {
        let scheduler = create_test_scheduler().await;

        // Schedule a task
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let placement = scheduler.schedule(task).await.unwrap();
        let node_id = placement.node_id;

        // Simulate node failure
        let result = scheduler.handle_node_failure(node_id).await;
        assert!(result.is_ok());

        // Node should be marked unhealthy
        let node = scheduler.node_registry().get(node_id).await;
        assert!(node.is_some());
        assert!(matches!(
            node.unwrap().health,
            crate::node::NodeHealth::Unhealthy
        ));
    }

    #[tokio::test]
    async fn test_metrics_recording() {
        let scheduler = create_test_scheduler().await;

        // Schedule several tasks
        for _ in 0..5 {
            let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
            scheduler.schedule(task).await.unwrap();
        }

        let metrics = scheduler.export_metrics().await;
        assert!(metrics.tasks_by_tier.get(&Tier::Host).unwrap_or(&0) >= &5);
    }

    #[tokio::test]
    async fn test_custom_config() {
        let registry = Arc::new(NodeRegistry::new());
        let config = SchedulerConfig {
            enable_preemption: false,
            max_attempts: 5,
            bind_timeout_ms: 10000,
        };

        let scheduler = FabricScheduler::with_config(registry, config.clone());
        assert!(!scheduler.config.enable_preemption);
        assert_eq!(scheduler.config.max_attempts, 5);
    }
}
