//! Placement decisions and history tracking.
//!
//! This module defines:
//! - `Placement` struct for recording task-to-node assignments
//! - `PlacementHistory` for tracking placement decisions over time

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::node::{NodeId, Tier};
use crate::task::TaskId;

/// A placement decision assigning a task to a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    /// The task being placed.
    pub task_id: TaskId,
    /// The node the task is placed on.
    pub node_id: NodeId,
    /// The tier of the node.
    pub tier: Tier,
    /// When the placement was made.
    pub timestamp: DateTime<Utc>,
    /// Score that led to this placement.
    pub score: i64,
    /// Reason for placement (for debugging/auditing).
    pub reason: String,
}

impl Placement {
    /// Create a new placement.
    pub fn new(task_id: TaskId, node_id: NodeId, tier: Tier, score: i64) -> Self {
        Self {
            task_id,
            node_id,
            tier,
            timestamp: Utc::now(),
            score,
            reason: String::new(),
        }
    }

    /// Add a reason to the placement.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }
}

/// Reason for task reschedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RescheduleReason {
    /// Node failed.
    NodeFailure(NodeId),
    /// Task was preempted.
    Preemption,
    /// Manual reschedule request.
    Manual,
    /// Load rebalancing.
    LoadBalance,
    /// Resource constraints changed.
    ResourceChange,
}

impl std::fmt::Display for RescheduleReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RescheduleReason::NodeFailure(id) => write!(f, "node failure: {}", id),
            RescheduleReason::Preemption => write!(f, "preemption"),
            RescheduleReason::Manual => write!(f, "manual reschedule"),
            RescheduleReason::LoadBalance => write!(f, "load balancing"),
            RescheduleReason::ResourceChange => write!(f, "resource change"),
        }
    }
}

/// Historical record of a placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRecord {
    /// The placement.
    pub placement: Placement,
    /// Whether the task completed successfully.
    pub completed: bool,
    /// Time task completed (if it did).
    pub completion_time: Option<DateTime<Utc>>,
    /// Reason for reschedule (if rescheduled).
    pub reschedule_reason: Option<RescheduleReason>,
}

impl PlacementRecord {
    /// Create a new placement record.
    pub fn new(placement: Placement) -> Self {
        Self {
            placement,
            completed: false,
            completion_time: None,
            reschedule_reason: None,
        }
    }

    /// Mark the placement as completed.
    pub fn mark_completed(&mut self) {
        self.completed = true;
        self.completion_time = Some(Utc::now());
    }

    /// Mark the placement as rescheduled.
    pub fn mark_rescheduled(&mut self, reason: RescheduleReason) {
        self.reschedule_reason = Some(reason);
    }

    /// Get the duration of the task execution.
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completion_time
            .map(|end| end - self.placement.timestamp)
    }
}

/// Statistics about placements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlacementStats {
    /// Total placements.
    pub total_placements: u64,
    /// Successful completions.
    pub successful_completions: u64,
    /// Reschedules.
    pub reschedules: u64,
    /// Placements by tier.
    pub by_tier: HashMap<Tier, u64>,
    /// Reschedules by reason.
    pub reschedules_by_reason: HashMap<String, u64>,
    /// Average score.
    pub average_score: f64,
}

impl PlacementStats {
    /// Calculate success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_placements == 0 {
            return 0.0;
        }
        self.successful_completions as f64 / self.total_placements as f64
    }

    /// Calculate reschedule rate.
    pub fn reschedule_rate(&self) -> f64 {
        if self.total_placements == 0 {
            return 0.0;
        }
        self.reschedules as f64 / self.total_placements as f64
    }
}

/// History of placement decisions.
#[derive(Debug)]
pub struct PlacementHistory {
    /// Recent placement records (bounded size).
    records: Arc<RwLock<VecDeque<PlacementRecord>>>,
    /// Current active placements by task ID.
    active: Arc<RwLock<HashMap<TaskId, PlacementRecord>>>,
    /// Maximum number of records to keep.
    max_records: usize,
    /// Running statistics.
    stats: Arc<RwLock<PlacementStats>>,
}

impl PlacementHistory {
    /// Create a new placement history.
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(VecDeque::with_capacity(max_records))),
            active: Arc::new(RwLock::new(HashMap::new())),
            max_records,
            stats: Arc::new(RwLock::new(PlacementStats::default())),
        }
    }

    /// Record a new placement.
    pub async fn record(&self, placement: Placement) {
        let task_id = placement.task_id;
        let tier = placement.tier;
        let score = placement.score;

        let record = PlacementRecord::new(placement);

        // Add to active placements
        self.active.write().await.insert(task_id, record.clone());

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_placements += 1;
        *stats.by_tier.entry(tier).or_insert(0) += 1;

        // Update rolling average score
        let n = stats.total_placements as f64;
        stats.average_score = stats.average_score * (n - 1.0) / n + score as f64 / n;

        tracing::debug!(
            task = %task_id,
            tier = ?tier,
            score = score,
            "Placement recorded"
        );
    }

    /// Mark a task as completed.
    pub async fn complete(&self, task_id: TaskId) -> Option<PlacementRecord> {
        let mut active = self.active.write().await;

        if let Some(mut record) = active.remove(&task_id) {
            record.mark_completed();

            // Update statistics
            self.stats.write().await.successful_completions += 1;

            // Add to history
            let mut records = self.records.write().await;
            if records.len() >= self.max_records {
                records.pop_front();
            }
            records.push_back(record.clone());

            tracing::debug!(
                task = %task_id,
                duration = ?record.duration(),
                "Task completed"
            );

            Some(record)
        } else {
            None
        }
    }

    /// Mark a task as rescheduled.
    pub async fn reschedule(
        &self,
        task_id: TaskId,
        reason: RescheduleReason,
    ) -> Option<PlacementRecord> {
        let mut active = self.active.write().await;

        if let Some(mut record) = active.remove(&task_id) {
            record.mark_rescheduled(reason.clone());

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.reschedules += 1;
            *stats
                .reschedules_by_reason
                .entry(reason.to_string())
                .or_insert(0) += 1;

            // Add to history
            let mut records = self.records.write().await;
            if records.len() >= self.max_records {
                records.pop_front();
            }
            records.push_back(record.clone());

            tracing::debug!(
                task = %task_id,
                reason = %reason,
                "Task rescheduled"
            );

            Some(record)
        } else {
            None
        }
    }

    /// Get the current placement for a task.
    pub async fn get_current(&self, task_id: TaskId) -> Option<PlacementRecord> {
        self.active.read().await.get(&task_id).cloned()
    }

    /// Get all active placements.
    pub async fn get_active(&self) -> Vec<PlacementRecord> {
        self.active.read().await.values().cloned().collect()
    }

    /// Get all tasks on a specific node.
    pub async fn get_tasks_on_node(&self, node_id: NodeId) -> Vec<TaskId> {
        self.active
            .read()
            .await
            .iter()
            .filter(|(_, record)| record.placement.node_id == node_id)
            .map(|(task_id, _)| *task_id)
            .collect()
    }

    /// Get recent placement history.
    pub async fn get_history(&self, limit: usize) -> Vec<PlacementRecord> {
        let records = self.records.read().await;
        records.iter().rev().take(limit).cloned().collect()
    }

    /// Get placement statistics.
    pub async fn get_stats(&self) -> PlacementStats {
        self.stats.read().await.clone()
    }

    /// Clear all history and reset statistics.
    pub async fn clear(&self) {
        self.records.write().await.clear();
        self.active.write().await.clear();
        *self.stats.write().await = PlacementStats::default();
    }
}

impl Default for PlacementHistory {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeId;

    #[tokio::test]
    async fn test_placement_record() {
        let history = PlacementHistory::new(100);

        let task_id = TaskId::new();
        let node_id = NodeId::new();
        let placement = Placement::new(task_id, node_id, Tier::Host, 85);

        // Record placement
        history.record(placement).await;

        // Check it's active
        let active = history.get_current(task_id).await;
        assert!(active.is_some());
        assert!(!active.unwrap().completed);

        // Complete it
        let completed = history.complete(task_id).await;
        assert!(completed.is_some());
        assert!(completed.unwrap().completed);

        // No longer active
        assert!(history.get_current(task_id).await.is_none());
    }

    #[tokio::test]
    async fn test_placement_reschedule() {
        let history = PlacementHistory::new(100);

        let task_id = TaskId::new();
        let node_id = NodeId::new();
        let placement = Placement::new(task_id, node_id, Tier::Host, 75);

        history.record(placement).await;

        // Reschedule due to node failure
        let record = history
            .reschedule(task_id, RescheduleReason::NodeFailure(node_id))
            .await;

        assert!(record.is_some());
        let record = record.unwrap();
        assert!(record.reschedule_reason.is_some());

        // Check stats
        let stats = history.get_stats().await;
        assert_eq!(stats.reschedules, 1);
    }

    #[tokio::test]
    async fn test_placement_stats() {
        let history = PlacementHistory::new(100);

        // Record some placements
        for i in 0..10 {
            let task_id = TaskId::new();
            let node_id = NodeId::new();
            let tier = if i % 2 == 0 { Tier::Host } else { Tier::Edge };
            let placement = Placement::new(task_id, node_id, tier, 80 + i as i64);
            history.record(placement).await;

            if i < 8 {
                history.complete(task_id).await;
            }
        }

        let stats = history.get_stats().await;
        assert_eq!(stats.total_placements, 10);
        assert_eq!(stats.successful_completions, 8);
        assert_eq!(stats.by_tier.get(&Tier::Host), Some(&5));
        assert_eq!(stats.by_tier.get(&Tier::Edge), Some(&5));
        assert!(stats.average_score > 80.0);
        assert!(stats.average_score < 90.0);
    }

    #[tokio::test]
    async fn test_get_tasks_on_node() {
        let history = PlacementHistory::new(100);
        let node_id = NodeId::new();

        // Add multiple tasks to same node
        let task_ids: Vec<TaskId> = (0..5).map(|_| TaskId::new()).collect();
        for task_id in &task_ids {
            let placement = Placement::new(*task_id, node_id, Tier::Host, 80);
            history.record(placement).await;
        }

        // Add one task to different node
        let other_node = NodeId::new();
        let other_task = TaskId::new();
        history
            .record(Placement::new(other_task, other_node, Tier::Host, 80))
            .await;

        let tasks_on_node = history.get_tasks_on_node(node_id).await;
        assert_eq!(tasks_on_node.len(), 5);
        for task_id in &task_ids {
            assert!(tasks_on_node.contains(task_id));
        }
    }

    #[tokio::test]
    async fn test_history_limit() {
        let history = PlacementHistory::new(5);

        // Record more placements than the limit
        for _ in 0..10 {
            let task_id = TaskId::new();
            let placement = Placement::new(task_id, NodeId::new(), Tier::Host, 80);
            history.record(placement).await;
            history.complete(task_id).await;
        }

        // Should only have 5 records in history
        let records = history.get_history(100).await;
        assert_eq!(records.len(), 5);
    }

    #[test]
    fn test_placement_with_reason() {
        let placement = Placement::new(TaskId::new(), NodeId::new(), Tier::Host, 90)
            .with_reason("Best scoring node");

        assert_eq!(placement.reason, "Best scoring node");
    }
}
