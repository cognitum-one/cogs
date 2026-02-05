//! Fabric scheduler for Agentic VM
//!
//! Implements hierarchical scheduling with constraint-based placement across
//! edge, host, and accelerator tiers.

pub mod filter;
pub mod node;
pub mod score;
pub mod task;

use std::collections::HashMap;
use std::sync::Arc;

pub use filter::{FilterPlugin, FilterResult};
pub use node::{NodeId, NodeInfo, NodeRegistry, Tier};
pub use score::{Score, ScorePlugin};
pub use task::{TaskClass, TaskConstraints, TaskId, TaskSpec};

/// Placement result
#[derive(Debug, Clone)]
pub struct Placement {
    /// Task that was placed
    pub task_id: TaskId,
    /// Node where task was placed
    pub node_id: NodeId,
    /// Tier of the placement
    pub tier: Tier,
    /// Placement timestamp
    pub timestamp: u64,
}

/// Scheduling error
#[derive(Debug, Clone)]
pub enum ScheduleError {
    NoFeasibleNodes,
    ScoringFailed,
    BindFailed(String),
    Timeout,
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFeasibleNodes => write!(f, "no feasible nodes"),
            Self::ScoringFailed => write!(f, "scoring failed"),
            Self::BindFailed(msg) => write!(f, "bind failed: {}", msg),
            Self::Timeout => write!(f, "scheduling timeout"),
        }
    }
}

impl std::error::Error for ScheduleError {}

/// Fabric scheduler
pub struct FabricScheduler {
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Filter plugins
    filters: Vec<Box<dyn FilterPlugin>>,
    /// Score plugins
    scorers: Vec<Box<dyn ScorePlugin>>,
    /// Active placements
    placements: HashMap<TaskId, Placement>,
}

impl FabricScheduler {
    /// Create a new scheduler
    pub fn new(node_registry: Arc<NodeRegistry>) -> Self {
        Self {
            node_registry,
            filters: Vec::new(),
            scorers: Vec::new(),
            placements: HashMap::new(),
        }
    }

    /// Add a filter plugin
    pub fn add_filter(&mut self, filter: Box<dyn FilterPlugin>) {
        self.filters.push(filter);
    }

    /// Add a score plugin
    pub fn add_scorer(&mut self, scorer: Box<dyn ScorePlugin>) {
        self.scorers.push(scorer);
    }

    /// Schedule a task
    pub fn schedule(&mut self, task: TaskSpec) -> Result<Placement, ScheduleError> {
        // Phase 1: Filter - eliminate infeasible nodes
        let feasible_nodes = self.filter_nodes(&task)?;

        if feasible_nodes.is_empty() {
            return Err(ScheduleError::NoFeasibleNodes);
        }

        // Phase 2: Score - rank feasible nodes
        let scored_nodes = self.score_nodes(&task, &feasible_nodes);

        // Phase 3: Select - choose best node
        let selected = scored_nodes
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(node, _)| node)
            .ok_or(ScheduleError::ScoringFailed)?;

        // Phase 4: Bind - assign task to node
        let placement = self.bind_task(&task, selected)?;

        self.placements.insert(task.id, placement.clone());

        Ok(placement)
    }

    /// Filter nodes for a task
    fn filter_nodes(&self, task: &TaskSpec) -> Result<Vec<NodeInfo>, ScheduleError> {
        let all_nodes = self.node_registry.get_all();
        let mut feasible = Vec::new();

        'outer: for node in all_nodes {
            // Skip unhealthy nodes
            if !node.healthy {
                continue;
            }

            // Apply all filter plugins
            for filter in &self.filters {
                match filter.filter(task, &node) {
                    FilterResult::Accept => continue,
                    FilterResult::Reject(_) => continue 'outer,
                }
            }

            feasible.push(node);
        }

        Ok(feasible)
    }

    /// Score feasible nodes
    fn score_nodes(&self, task: &TaskSpec, nodes: &[NodeInfo]) -> Vec<(NodeInfo, i64)> {
        nodes
            .iter()
            .map(|node| {
                let total_score: i64 = self
                    .scorers
                    .iter()
                    .map(|scorer| {
                        let score = scorer.score(task, node);
                        (score.0 * scorer.weight() as i64) / 100
                    })
                    .sum();

                (node.clone(), total_score)
            })
            .collect()
    }

    /// Bind task to node
    fn bind_task(&self, task: &TaskSpec, node: NodeInfo) -> Result<Placement, ScheduleError> {
        Ok(Placement {
            task_id: task.id,
            node_id: node.id,
            tier: node.tier,
            timestamp: current_time_ns(),
        })
    }

    /// Get placement for a task
    pub fn get_placement(&self, task_id: TaskId) -> Option<&Placement> {
        self.placements.get(&task_id)
    }

    /// Remove a placement
    pub fn remove_placement(&mut self, task_id: TaskId) -> Option<Placement> {
        self.placements.remove(&task_id)
    }

    /// Get all tasks on a node
    pub fn get_tasks_on_node(&self, node_id: NodeId) -> Vec<TaskId> {
        self.placements
            .iter()
            .filter(|(_, p)| p.node_id == node_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Handle node failure - reschedule affected tasks
    pub fn handle_node_failure(&mut self, node_id: NodeId) -> Vec<(TaskId, Result<Placement, ScheduleError>)> {
        self.node_registry.mark_unhealthy(node_id);

        let affected_tasks: Vec<TaskId> = self.get_tasks_on_node(node_id);
        let mut results = Vec::new();

        for task_id in affected_tasks {
            if let Some(old_placement) = self.remove_placement(task_id) {
                // Try to reschedule (would need task spec in real impl)
                // For now, we just mark as failed
                results.push((task_id, Err(ScheduleError::NoFeasibleNodes)));
            }
        }

        results
    }
}

fn current_time_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests;
