//! Score plugins for the fabric scheduler.
//!
//! Score plugins rank feasible nodes after filtering.
//! Each plugin assigns a score (0-100) based on different criteria,
//! weighted by importance.

use crate::node::NodeInfo;
use crate::task::{TaskClass, TaskSpec};

/// Score value assigned by a scoring plugin (0-100).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Score(pub i64);

impl Score {
    /// Minimum score.
    pub const MIN: Score = Score(0);
    /// Maximum score.
    pub const MAX: Score = Score(100);

    /// Create a new score, clamping to valid range.
    pub fn new(value: i64) -> Self {
        Self(value.clamp(0, 100))
    }

    /// Get the score value.
    pub fn value(&self) -> i64 {
        self.0
    }
}

impl std::ops::Add for Score {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Mul<u32> for Score {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * rhs as i64)
    }
}

/// Trait for score plugins.
///
/// Score plugins evaluate how well a node fits a task's requirements.
/// Higher scores are better.
pub trait ScorePlugin: Send + Sync {
    /// Get the name of this scoring plugin.
    fn name(&self) -> &str;

    /// Get the weight of this scoring plugin (0-100).
    /// Higher weight means this score matters more.
    fn weight(&self) -> u32;

    /// Score a node for a task.
    /// Returns a score from 0 (worst) to 100 (best).
    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score;
}

/// Power efficiency scoring - prefer lower power consumption.
///
/// Scores nodes inversely to their power consumption,
/// encouraging tasks to run on the most power-efficient tier.
#[derive(Debug, Default)]
pub struct PowerScore;

impl ScorePlugin for PowerScore {
    fn name(&self) -> &str {
        "power"
    }

    fn weight(&self) -> u32 {
        30
    }

    fn score(&self, _task: &TaskSpec, node: &NodeInfo) -> Score {
        // Normalize power to 0-100 scale (lower power = higher score)
        // Max power assumed to be 300W (300,000 mW)
        const MAX_POWER_MW: u64 = 300_000;

        let power = node.power_draw_mw as u64;
        let normalized = (power * 100 / MAX_POWER_MW).min(100) as i64;
        let score = 100 - normalized;

        Score::new(score)
    }
}

/// Latency scoring - prefer lower wake latency.
///
/// Penalizes nodes with high wake latency, especially for
/// latency-critical task classes.
#[derive(Debug, Default)]
pub struct LatencyScore;

impl ScorePlugin for LatencyScore {
    fn name(&self) -> &str {
        "latency"
    }

    fn weight(&self) -> u32 {
        25
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        // Latency penalty varies by task class
        let latency_penalty = match task.class {
            // Latency-critical tasks penalize high latency heavily
            TaskClass::Reflex | TaskClass::Gating => node.wake_latency_ms * 2,
            // Interactive tasks have moderate penalty
            TaskClass::Cli | TaskClass::Network => node.wake_latency_ms,
            // Background tasks have low penalty
            _ => node.wake_latency_ms / 2,
        };

        let score = 100_i64.saturating_sub(latency_penalty.min(100) as i64);
        Score::new(score)
    }
}

/// Load balancing scoring - prefer less loaded nodes.
///
/// Scores nodes based on their current utilization,
/// preferring nodes with more available capacity.
#[derive(Debug, Default)]
pub struct LoadBalanceScore;

impl ScorePlugin for LoadBalanceScore {
    fn name(&self) -> &str {
        "load-balance"
    }

    fn weight(&self) -> u32 {
        20
    }

    fn score(&self, _task: &TaskSpec, node: &NodeInfo) -> Score {
        // Higher load = lower score
        let utilization = node.current_load as i64;
        Score::new(100 - utilization.min(100))
    }
}

/// Locality scoring - prefer nodes with local data.
///
/// Scores nodes based on whether they have a local workspace
/// for the task's capsule, reducing data transfer overhead.
#[derive(Debug, Default)]
pub struct LocalityScore;

impl ScorePlugin for LocalityScore {
    fn name(&self) -> &str {
        "locality"
    }

    fn weight(&self) -> u32 {
        15
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        if node.has_local_workspace(&task.capsule_id) {
            Score::MAX
        } else {
            Score::MIN
        }
    }
}

/// Risk scoring - prefer appropriate isolation levels.
///
/// Scores nodes based on whether their isolation level
/// matches the task's security requirements.
#[derive(Debug, Default)]
pub struct RiskScore;

impl ScorePlugin for RiskScore {
    fn name(&self) -> &str {
        "risk"
    }

    fn weight(&self) -> u32 {
        10
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        use crate::node::IsolationLevel;

        match (task.constraints.isolation, node.isolation_level) {
            // Perfect match for hardware isolation
            (IsolationLevel::Hardware, IsolationLevel::Hardware) => Score::MAX,
            // Hardware requested but not available
            (IsolationLevel::Hardware, _) => Score::MIN,
            // VM isolation requested, hardware or VM available
            (IsolationLevel::Vm, IsolationLevel::Hardware | IsolationLevel::Vm) => Score::MAX,
            // VM requested, only process available
            (IsolationLevel::Vm, IsolationLevel::Process) => Score::new(50),
            // Process isolation - any level is fine
            (IsolationLevel::Process, _) => Score::MAX,
        }
    }
}

/// Tier preference scoring - prefer the task's preferred tier.
///
/// Gives bonus points to nodes in the task's preferred tier,
/// while still allowing fallback to other tiers.
#[derive(Debug, Default)]
pub struct TierPreferenceScore;

impl ScorePlugin for TierPreferenceScore {
    fn name(&self) -> &str {
        "tier-preference"
    }

    fn weight(&self) -> u32 {
        15
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        let preferred_tier = task.class.preferred_tier();

        if node.tier == preferred_tier {
            Score::MAX
        } else if task.class.can_run_on(node.tier) {
            // Acceptable but not preferred
            Score::new(50)
        } else {
            Score::MIN
        }
    }
}

/// Memory availability scoring - prefer nodes with more free memory.
///
/// Scores nodes based on their available memory relative to
/// the task's requirements.
#[derive(Debug, Default)]
pub struct MemoryScore;

impl ScorePlugin for MemoryScore {
    fn name(&self) -> &str {
        "memory"
    }

    fn weight(&self) -> u32 {
        10
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        if node.available_memory == 0 {
            return Score::MIN;
        }

        // Score based on how much headroom we have
        let required = task.resources.memory_bytes;
        let available = node.available_memory;

        if required > available {
            return Score::MIN;
        }

        // More headroom = higher score
        let headroom_ratio = (available - required) * 100 / available;
        Score::new(headroom_ratio as i64)
    }
}

/// Health-aware scoring - prefer healthy nodes.
///
/// Gives lower scores to degraded nodes while still allowing
/// them to be used if no better options exist.
#[derive(Debug, Default)]
pub struct HealthScore;

impl ScorePlugin for HealthScore {
    fn name(&self) -> &str {
        "health"
    }

    fn weight(&self) -> u32 {
        20
    }

    fn score(&self, _task: &TaskSpec, node: &NodeInfo) -> Score {
        use crate::node::NodeHealth;

        match node.health {
            NodeHealth::Healthy => Score::MAX,
            NodeHealth::Degraded => Score::new(50),
            NodeHealth::Unhealthy | NodeHealth::Offline => Score::MIN,
        }
    }
}

/// Scored node result.
#[derive(Debug, Clone)]
pub struct ScoredNode {
    /// The node information.
    pub node: NodeInfo,
    /// Total weighted score.
    pub total_score: i64,
    /// Individual plugin scores.
    pub plugin_scores: Vec<(String, Score, u32)>, // (name, score, weight)
}

/// Collection of all scoring plugins with weighted aggregation.
pub struct ScoreChain {
    plugins: Vec<Box<dyn ScorePlugin>>,
}

impl ScoreChain {
    /// Create a new score chain with all default scoring plugins.
    pub fn default_chain() -> Self {
        Self {
            plugins: vec![
                Box::new(PowerScore),
                Box::new(LatencyScore),
                Box::new(LoadBalanceScore),
                Box::new(LocalityScore),
                Box::new(RiskScore),
                Box::new(TierPreferenceScore),
                Box::new(MemoryScore),
                Box::new(HealthScore),
            ],
        }
    }

    /// Create an empty score chain.
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Add a scoring plugin to the chain.
    pub fn add_plugin(mut self, plugin: Box<dyn ScorePlugin>) -> Self {
        self.plugins.push(plugin);
        self
    }

    /// Score a single node for a task.
    pub fn score(&self, task: &TaskSpec, node: &NodeInfo) -> ScoredNode {
        let mut total_score: i64 = 0;
        let mut total_weight: u32 = 0;
        let mut plugin_scores = Vec::with_capacity(self.plugins.len());

        for plugin in &self.plugins {
            let score = plugin.score(task, node);
            let weight = plugin.weight();

            total_score += score.value() * weight as i64;
            total_weight += weight;

            plugin_scores.push((plugin.name().to_string(), score, weight));

            tracing::trace!(
                plugin = plugin.name(),
                node = %node.id,
                score = score.value(),
                weight = weight,
                "Plugin scored node"
            );
        }

        // Normalize to 0-100 range
        let normalized_score = if total_weight > 0 {
            total_score / total_weight as i64
        } else {
            0
        };

        ScoredNode {
            node: node.clone(),
            total_score: normalized_score,
            plugin_scores,
        }
    }

    /// Score all nodes and return them sorted by score (highest first).
    pub fn score_nodes(&self, task: &TaskSpec, nodes: &[NodeInfo]) -> Vec<ScoredNode> {
        let mut scored: Vec<ScoredNode> = nodes
            .iter()
            .map(|node| self.score(task, node))
            .collect();

        // Sort by total score descending
        scored.sort_by(|a, b| b.total_score.cmp(&a.total_score));

        scored
    }

    /// Get the best-scoring node for a task.
    pub fn best_node(&self, task: &TaskSpec, nodes: &[NodeInfo]) -> Option<ScoredNode> {
        self.score_nodes(task, nodes).into_iter().next()
    }
}

impl Default for ScoreChain {
    fn default() -> Self {
        Self::default_chain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{IsolationLevel, NodeHealth, Tier};
    use crate::task::CapsuleId;

    fn create_test_node(tier: Tier) -> NodeInfo {
        NodeInfo::new("test-node", tier)
    }

    fn create_test_task(class: TaskClass) -> TaskSpec {
        TaskSpec::new(CapsuleId::new(), class)
    }

    #[test]
    fn test_power_score() {
        let scorer = PowerScore;

        let mut edge_node = create_test_node(Tier::Edge);
        edge_node.power_draw_mw = 5; // 5 mW

        let mut accel_node = create_test_node(Tier::Accel);
        accel_node.power_draw_mw = 150_000; // 150 W

        let task = create_test_task(TaskClass::Network);

        // Edge should score higher (lower power)
        let edge_score = scorer.score(&task, &edge_node);
        let accel_score = scorer.score(&task, &accel_node);

        assert!(edge_score.value() > accel_score.value());
        assert!(edge_score.value() >= 90); // Very low power
        assert!(accel_score.value() < 60); // High power
    }

    #[test]
    fn test_latency_score() {
        let scorer = LatencyScore;

        let mut fast_node = create_test_node(Tier::Edge);
        fast_node.wake_latency_ms = 0;

        let mut slow_node = create_test_node(Tier::Accel);
        slow_node.wake_latency_ms = 500;

        // Reflex task (latency-critical)
        let reflex_task = create_test_task(TaskClass::Reflex);
        let fast_score = scorer.score(&reflex_task, &fast_node);
        let slow_score = scorer.score(&reflex_task, &slow_node);

        assert!(fast_score.value() > slow_score.value());
        assert_eq!(fast_score.value(), 100); // Zero latency

        // CLI task (less latency-critical)
        let cli_task = create_test_task(TaskClass::Cli);
        let cli_slow_score = scorer.score(&cli_task, &slow_node);

        // Should be penalized less for CLI tasks
        assert!(cli_slow_score.value() >= slow_score.value());
    }

    #[test]
    fn test_load_balance_score() {
        let scorer = LoadBalanceScore;
        let task = create_test_task(TaskClass::Network);

        let mut idle_node = create_test_node(Tier::Host);
        idle_node.current_load = 10;

        let mut busy_node = create_test_node(Tier::Host);
        busy_node.current_load = 90;

        let idle_score = scorer.score(&task, &idle_node);
        let busy_score = scorer.score(&task, &busy_node);

        assert_eq!(idle_score.value(), 90);
        assert_eq!(busy_score.value(), 10);
    }

    #[test]
    fn test_locality_score() {
        let scorer = LocalityScore;
        let capsule_id = CapsuleId::new();
        let task = TaskSpec::new(capsule_id, TaskClass::Network);

        let mut local_node = create_test_node(Tier::Host);
        local_node.local_workspaces.push(capsule_id);

        let remote_node = create_test_node(Tier::Host);

        let local_score = scorer.score(&task, &local_node);
        let remote_score = scorer.score(&task, &remote_node);

        assert_eq!(local_score.value(), 100);
        assert_eq!(remote_score.value(), 0);
    }

    #[test]
    fn test_risk_score() {
        let scorer = RiskScore;

        let mut hw_node = create_test_node(Tier::Host);
        hw_node.isolation_level = IsolationLevel::Hardware;

        let mut vm_node = create_test_node(Tier::Host);
        vm_node.isolation_level = IsolationLevel::Vm;

        let mut proc_node = create_test_node(Tier::Host);
        proc_node.isolation_level = IsolationLevel::Process;

        // Task requiring hardware isolation
        let mut hw_task = create_test_task(TaskClass::Network);
        hw_task.constraints.isolation = IsolationLevel::Hardware;

        assert_eq!(scorer.score(&hw_task, &hw_node).value(), 100);
        assert_eq!(scorer.score(&hw_task, &vm_node).value(), 0);
        assert_eq!(scorer.score(&hw_task, &proc_node).value(), 0);

        // Task requiring VM isolation
        let mut vm_task = create_test_task(TaskClass::Network);
        vm_task.constraints.isolation = IsolationLevel::Vm;

        assert_eq!(scorer.score(&vm_task, &hw_node).value(), 100);
        assert_eq!(scorer.score(&vm_task, &vm_node).value(), 100);
        assert_eq!(scorer.score(&vm_task, &proc_node).value(), 50);
    }

    #[test]
    fn test_tier_preference_score() {
        let scorer = TierPreferenceScore;

        let edge_node = create_test_node(Tier::Edge);
        let host_node = create_test_node(Tier::Host);
        let accel_node = create_test_node(Tier::Accel);

        // Reflex task prefers edge
        let reflex_task = create_test_task(TaskClass::Reflex);
        assert_eq!(scorer.score(&reflex_task, &edge_node).value(), 100);
        assert_eq!(scorer.score(&reflex_task, &host_node).value(), 0); // Can't run on host

        // Network task prefers host
        let network_task = create_test_task(TaskClass::Network);
        assert_eq!(scorer.score(&network_task, &host_node).value(), 100);
        assert_eq!(scorer.score(&network_task, &accel_node).value(), 50); // Can run but not preferred
    }

    #[test]
    fn test_health_score() {
        let scorer = HealthScore;
        let task = create_test_task(TaskClass::Network);

        let mut healthy_node = create_test_node(Tier::Host);
        healthy_node.health = NodeHealth::Healthy;

        let mut degraded_node = create_test_node(Tier::Host);
        degraded_node.health = NodeHealth::Degraded;

        let mut unhealthy_node = create_test_node(Tier::Host);
        unhealthy_node.health = NodeHealth::Unhealthy;

        assert_eq!(scorer.score(&task, &healthy_node).value(), 100);
        assert_eq!(scorer.score(&task, &degraded_node).value(), 50);
        assert_eq!(scorer.score(&task, &unhealthy_node).value(), 0);
    }

    #[test]
    fn test_score_chain() {
        let chain = ScoreChain::default_chain();

        let mut good_node = create_test_node(Tier::Host);
        good_node.current_load = 10;
        good_node.health = NodeHealth::Healthy;
        good_node.available_memory = 8 * 1024 * 1024 * 1024;

        let mut bad_node = create_test_node(Tier::Host);
        bad_node.current_load = 90;
        bad_node.health = NodeHealth::Degraded;
        bad_node.available_memory = 512 * 1024 * 1024;

        let task = create_test_task(TaskClass::Network);
        let nodes = vec![bad_node, good_node.clone()];

        let scored = chain.score_nodes(&task, &nodes);

        // Good node should score higher
        assert_eq!(scored[0].node.id, good_node.id);
        assert!(scored[0].total_score > scored[1].total_score);
    }

    #[test]
    fn test_score_operations() {
        let s1 = Score::new(50);
        let s2 = Score::new(30);

        assert_eq!((s1 + s2).value(), 80);
        assert_eq!((s1 * 2).value(), 100);

        // Test clamping
        assert_eq!(Score::new(-10).value(), 0);
        assert_eq!(Score::new(150).value(), 100);
    }
}
