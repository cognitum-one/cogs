//! Score plugins for scheduling

use crate::node::NodeInfo;
use crate::task::{IsolationLevel, TaskClass, TaskSpec};

/// Score value (higher is better)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(pub i64);

impl Score {
    pub const MIN: Score = Score(i64::MIN);
    pub const MAX: Score = Score(i64::MAX);
    pub const ZERO: Score = Score(0);

    pub fn new(value: i64) -> Self {
        Self(value)
    }
}

impl Default for Score {
    fn default() -> Self {
        Self::ZERO
    }
}

impl std::ops::Add for Score {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Score(self.0.saturating_add(rhs.0))
    }
}

/// Score plugin trait
pub trait ScorePlugin: Send + Sync {
    /// Name of this scorer
    fn name(&self) -> &str;

    /// Weight of this scorer (0-100)
    fn weight(&self) -> u32;

    /// Score a node for a task (0-100)
    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score;
}

/// Power efficiency scoring - prefer lower power
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
        let max_power = 300_000u64; // 300W in mW
        let power = node.power_draw_mw as u64;
        let score = 100i64 - ((power * 100) / max_power) as i64;
        Score(score.max(0))
    }
}

/// Latency scoring - prefer lower latency
pub struct LatencyScore;

impl ScorePlugin for LatencyScore {
    fn name(&self) -> &str {
        "latency"
    }

    fn weight(&self) -> u32 {
        25
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        // Penalize high wake latency for interactive tasks
        let latency_penalty = match task.class {
            TaskClass::Reflex | TaskClass::Gating => node.wake_latency_ms * 2,
            TaskClass::Cli | TaskClass::Network => node.wake_latency_ms,
            _ => node.wake_latency_ms / 2,
        };

        let score = 100i64 - (latency_penalty as i64).min(100);
        Score(score.max(0))
    }
}

/// Load balancing - prefer less loaded nodes
pub struct LoadBalanceScore;

impl ScorePlugin for LoadBalanceScore {
    fn name(&self) -> &str {
        "load_balance"
    }

    fn weight(&self) -> u32 {
        20
    }

    fn score(&self, _task: &TaskSpec, node: &NodeInfo) -> Score {
        let score = 100i64 - node.current_load as i64;
        Score(score)
    }
}

/// Locality scoring - prefer co-located data
pub struct LocalityScore;

impl ScorePlugin for LocalityScore {
    fn name(&self) -> &str {
        "locality"
    }

    fn weight(&self) -> u32 {
        15
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        // Check if workspace is local to node
        if node.has_local_workspace(&task.capsule_id) {
            Score(100)
        } else {
            Score(0)
        }
    }
}

/// Risk scoring - higher isolation for risky tasks
pub struct RiskScore;

impl ScorePlugin for RiskScore {
    fn name(&self) -> &str {
        "risk"
    }

    fn weight(&self) -> u32 {
        10
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        match (task.constraints.isolation, node.capabilities.isolation_level) {
            (IsolationLevel::Hardware, IsolationLevel::Hardware) => Score(100),
            (IsolationLevel::Hardware, _) => Score(0),
            (IsolationLevel::Vm, IsolationLevel::Hardware | IsolationLevel::Vm) => Score(100),
            (IsolationLevel::Vm, _) => Score(50),
            (IsolationLevel::Process, _) => Score(100),
        }
    }
}

/// Memory utilization scoring - prefer nodes with more available memory
pub struct MemoryScore;

impl ScorePlugin for MemoryScore {
    fn name(&self) -> &str {
        "memory"
    }

    fn weight(&self) -> u32 {
        15
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        if node.available_memory == 0 {
            return Score(0);
        }

        // How much memory would be left after scheduling this task
        let remaining = node.available_memory.saturating_sub(task.resources.memory_bytes);
        let remaining_pct = (remaining * 100) / node.total_memory;

        Score(remaining_pct as i64)
    }
}

/// Tier preference scoring - prefer preferred tier
pub struct TierPreferenceScore;

impl ScorePlugin for TierPreferenceScore {
    fn name(&self) -> &str {
        "tier_preference"
    }

    fn weight(&self) -> u32 {
        20
    }

    fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        if node.tier == task.class.preferred_tier() {
            Score(100)
        } else if task.class.can_run_on(node.tier) {
            Score(50)
        } else {
            Score(0)
        }
    }
}

/// Composite scorer combining multiple scorers
pub struct CompositeScorer {
    scorers: Vec<Box<dyn ScorePlugin>>,
}

impl CompositeScorer {
    pub fn new() -> Self {
        Self { scorers: Vec::new() }
    }

    pub fn add(&mut self, scorer: Box<dyn ScorePlugin>) {
        self.scorers.push(scorer);
    }

    /// Score a node using all scorers
    pub fn score(&self, task: &TaskSpec, node: &NodeInfo) -> Score {
        let mut total_score = 0i64;
        let mut total_weight = 0u32;

        for scorer in &self.scorers {
            let score = scorer.score(task, node);
            let weight = scorer.weight();
            total_score += (score.0 * weight as i64) / 100;
            total_weight += weight;
        }

        // Normalize to 0-100
        if total_weight > 0 {
            Score((total_score * 100) / total_weight as i64)
        } else {
            Score(0)
        }
    }

    /// Create with default scorers
    pub fn with_defaults() -> Self {
        let mut composite = Self::new();
        composite.add(Box::new(PowerScore));
        composite.add(Box::new(LatencyScore));
        composite.add(Box::new(LoadBalanceScore));
        composite.add(Box::new(LocalityScore));
        composite.add(Box::new(RiskScore));
        composite.add(Box::new(MemoryScore));
        composite.add(Box::new(TierPreferenceScore));
        composite
    }
}

impl Default for CompositeScorer {
    fn default() -> Self {
        Self::new()
    }
}
