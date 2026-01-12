//! # Layer 4: MEMORY
//!
//! The memory layer provides persistent storage and learning capabilities for agents.
//! Key components:
//!
//! - **SONA Substrate**: Self-Optimizing Neural Architecture for adaptive learning
//! - **ReasoningBank**: Experience storage with trajectory tracking
//! - **EWC++ Protection**: Elastic Weight Consolidation to prevent catastrophic forgetting
//! - **Trajectory Storage**: Record of past observations and actions
//!
//! ## Design Philosophy
//!
//! Memory enables agents to learn from experience while protecting important
//! learned knowledge from being overwritten by new experiences.

use crate::error::{FxnnError, Result};
use super::agency::AgentId;
use super::perception::Observation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

// ============================================================================
// Core Types
// ============================================================================

/// Error type for memory operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum MemoryError {
    /// Memory capacity exceeded
    #[error("Memory capacity exceeded: {0}")]
    CapacityExceeded(String),

    /// Memory not found
    #[error("Memory not found: {0}")]
    NotFound(String),

    /// Invalid memory format
    #[error("Invalid memory format: {0}")]
    InvalidFormat(String),

    /// EWC protection prevented update
    #[error("EWC protection prevented update: importance = {0}")]
    EWCProtected(f32),
}

/// Entry in agent memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique entry ID
    pub id: u64,
    /// Agent that owns this memory
    pub agent_id: AgentId,
    /// Observation that created this memory
    pub observation: Vec<f32>,
    /// Action taken
    pub action: Vec<f32>,
    /// Reward received
    pub reward: f32,
    /// Importance weight (for EWC)
    pub importance: f32,
    /// Timestamp
    pub timestamp: u64,
    /// Tags for retrieval
    pub tags: Vec<String>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(agent_id: AgentId, observation: Vec<f32>, action: Vec<f32>, reward: f32) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self {
            id: COUNTER.fetch_add(1, Ordering::SeqCst),
            agent_id,
            observation,
            action,
            reward,
            importance: 1.0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tags: Vec::new(),
        }
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set importance
    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance;
        self
    }
}

/// Query for memory retrieval
#[derive(Debug, Clone)]
pub struct MemoryQuery {
    /// Maximum number of results
    pub limit: usize,
    /// Minimum importance threshold
    pub min_importance: f32,
    /// Time range (start, end) in seconds
    pub time_range: Option<(u64, u64)>,
    /// Required tags
    pub tags: Vec<String>,
    /// Query vector for similarity search
    pub query_vector: Option<Vec<f32>>,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            limit: 10,
            min_importance: 0.0,
            time_range: None,
            tags: Vec::new(),
            query_vector: None,
        }
    }
}

impl MemoryQuery {
    /// Create a new query
    pub fn new() -> Self {
        Self::default()
    }

    /// Set limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set minimum importance
    pub fn with_min_importance(mut self, min: f32) -> Self {
        self.min_importance = min;
        self
    }

    /// Set time range
    pub fn with_time_range(mut self, start: u64, end: u64) -> Self {
        self.time_range = Some((start, end));
        self
    }

    /// Add required tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set query vector for similarity search
    pub fn with_query(mut self, query: Vec<f32>) -> Self {
        self.query_vector = Some(query);
        self
    }
}

// ============================================================================
// Trajectory Storage
// ============================================================================

/// A trajectory of observations and actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Trajectory ID
    pub id: u64,
    /// Agent that generated this trajectory
    pub agent_id: AgentId,
    /// Sequence of (observation, action, reward) tuples
    pub steps: Vec<TrajectoryStep>,
    /// Total reward
    pub total_reward: f32,
    /// Whether trajectory was successful
    pub success: bool,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Single step in a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Step index
    pub step: usize,
    /// Observation
    pub observation: Vec<f32>,
    /// Action taken
    pub action: Vec<f32>,
    /// Reward received
    pub reward: f32,
    /// Value estimate (for advantage computation)
    pub value: f32,
    /// Action log probability
    pub log_prob: f32,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(agent_id: AgentId) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self {
            id: COUNTER.fetch_add(1, Ordering::SeqCst),
            agent_id,
            steps: Vec::new(),
            total_reward: 0.0,
            success: false,
            metadata: HashMap::new(),
        }
    }

    /// Add a step
    pub fn add_step(&mut self, observation: Vec<f32>, action: Vec<f32>, reward: f32) {
        let step = TrajectoryStep {
            step: self.steps.len(),
            observation,
            action,
            reward,
            value: 0.0,
            log_prob: 0.0,
        };
        self.total_reward += reward;
        self.steps.push(step);
    }

    /// Mark as successful
    pub fn mark_success(&mut self) {
        self.success = true;
    }

    /// Compute discounted returns
    pub fn compute_returns(&mut self, gamma: f32) {
        let mut returns = 0.0;
        for step in self.steps.iter_mut().rev() {
            returns = step.reward + gamma * returns;
            step.value = returns;
        }
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

// ============================================================================
// EWC++ Protection
// ============================================================================

/// Elastic Weight Consolidation for preventing catastrophic forgetting
#[derive(Debug, Clone)]
pub struct EWCProtection {
    /// Protection threshold (0 to 1)
    threshold: f32,
    /// Importance weights for each parameter
    fisher_information: Vec<f32>,
    /// Reference parameter values
    reference_params: Vec<f32>,
    /// Lambda scaling factor
    lambda: f32,
    /// Whether online mode is enabled
    online_mode: bool,
    /// Decay factor for online EWC
    online_decay: f32,
}

impl EWCProtection {
    /// Create new EWC protection
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            fisher_information: Vec::new(),
            reference_params: Vec::new(),
            lambda: 1.0,
            online_mode: false,
            online_decay: 0.9,
        }
    }

    /// Enable online mode (EWC++)
    pub fn with_online_mode(mut self, decay: f32) -> Self {
        self.online_mode = true;
        self.online_decay = decay;
        self
    }

    /// Set lambda scaling
    pub fn with_lambda(mut self, lambda: f32) -> Self {
        self.lambda = lambda;
        self
    }

    /// Initialize with reference parameters
    pub fn initialize(&mut self, params: &[f32]) {
        self.reference_params = params.to_vec();
        self.fisher_information = vec![1.0; params.len()];
    }

    /// Update Fisher information from gradients
    pub fn update_fisher(&mut self, gradients: &[f32]) {
        if self.fisher_information.len() != gradients.len() {
            self.fisher_information = vec![0.0; gradients.len()];
        }

        for (i, &grad) in gradients.iter().enumerate() {
            if self.online_mode {
                // Online EWC: exponential moving average
                self.fisher_information[i] = self.online_decay * self.fisher_information[i]
                    + (1.0 - self.online_decay) * grad * grad;
            } else {
                // Standard EWC: accumulate
                self.fisher_information[i] += grad * grad;
            }
        }
    }

    /// Compute EWC penalty for parameter update
    pub fn compute_penalty(&self, current_params: &[f32]) -> f32 {
        if self.reference_params.len() != current_params.len() {
            return 0.0;
        }

        let mut penalty = 0.0;
        for (i, (&current, &reference)) in current_params.iter().zip(self.reference_params.iter()).enumerate() {
            let diff = current - reference;
            let fisher = self.fisher_information.get(i).copied().unwrap_or(1.0);
            penalty += fisher * diff * diff;
        }

        0.5 * self.lambda * penalty
    }

    /// Check if update should be blocked
    pub fn should_block(&self, importance: f32) -> bool {
        importance > self.threshold
    }

    /// Get protection level for a parameter
    pub fn protection_level(&self, param_idx: usize) -> f32 {
        self.fisher_information.get(param_idx).copied().unwrap_or(0.0)
    }
}

impl Default for EWCProtection {
    fn default() -> Self {
        Self::new(0.95)
    }
}

// ============================================================================
// ReasoningBank
// ============================================================================

/// Bank of reasoning experiences for learning
#[derive(Debug)]
pub struct ReasoningBank {
    /// Memory entries by agent
    memories: HashMap<AgentId, VecDeque<MemoryEntry>>,
    /// Maximum entries per agent
    max_entries: usize,
    /// Trajectory storage
    trajectories: HashMap<AgentId, VecDeque<Trajectory>>,
    /// Maximum trajectories per agent
    max_trajectories: usize,
    /// EWC protection
    ewc: EWCProtection,
}

impl ReasoningBank {
    /// Create a new reasoning bank
    pub fn new() -> Self {
        Self {
            memories: HashMap::new(),
            max_entries: 10000,
            trajectories: HashMap::new(),
            max_trajectories: 100,
            ewc: EWCProtection::default(),
        }
    }

    /// Set maximum entries per agent
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Set maximum trajectories per agent
    pub fn with_max_trajectories(mut self, max: usize) -> Self {
        self.max_trajectories = max;
        self
    }

    /// Set EWC protection
    pub fn with_ewc(mut self, ewc: EWCProtection) -> Self {
        self.ewc = ewc;
        self
    }

    /// Store a memory entry
    pub fn store(&mut self, entry: MemoryEntry) -> Result<()> {
        // Check EWC protection
        if self.ewc.should_block(entry.importance) {
            return Err(FxnnError::invalid_parameter(
                format!("EWC blocked memory with importance {}", entry.importance)
            ));
        }

        let agent_memories = self.memories
            .entry(entry.agent_id)
            .or_insert_with(VecDeque::new);

        // Remove oldest if at capacity
        if agent_memories.len() >= self.max_entries {
            agent_memories.pop_front();
        }

        agent_memories.push_back(entry);
        Ok(())
    }

    /// Store a trajectory
    pub fn store_trajectory(&mut self, trajectory: Trajectory) -> Result<()> {
        let agent_trajectories = self.trajectories
            .entry(trajectory.agent_id)
            .or_insert_with(VecDeque::new);

        // Remove oldest if at capacity
        if agent_trajectories.len() >= self.max_trajectories {
            agent_trajectories.pop_front();
        }

        agent_trajectories.push_back(trajectory);
        Ok(())
    }

    /// Retrieve memories matching query
    pub fn retrieve(&self, agent_id: AgentId, query: &MemoryQuery) -> Vec<MemoryEntry> {
        let Some(memories) = self.memories.get(&agent_id) else {
            return Vec::new();
        };

        let mut results: Vec<_> = memories.iter()
            .filter(|m| m.importance >= query.min_importance)
            .filter(|m| {
                if let Some((start, end)) = query.time_range {
                    m.timestamp >= start && m.timestamp <= end
                } else {
                    true
                }
            })
            .filter(|m| {
                query.tags.is_empty() || query.tags.iter().all(|t| m.tags.contains(t))
            })
            .cloned()
            .collect();

        // Sort by similarity if query vector provided
        if let Some(ref query_vec) = query.query_vector {
            results.sort_by(|a, b| {
                let sim_a = cosine_similarity(&a.observation, query_vec);
                let sim_b = cosine_similarity(&b.observation, query_vec);
                sim_b.partial_cmp(&sim_a).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        results.truncate(query.limit);
        results
    }

    /// Get trajectories for an agent
    pub fn get_trajectories(&self, agent_id: AgentId) -> Vec<&Trajectory> {
        self.trajectories
            .get(&agent_id)
            .map(|t| t.iter().collect())
            .unwrap_or_default()
    }

    /// Get successful trajectories
    pub fn get_successful_trajectories(&self, agent_id: AgentId) -> Vec<&Trajectory> {
        self.trajectories
            .get(&agent_id)
            .map(|t| t.iter().filter(|tr| tr.success).collect())
            .unwrap_or_default()
    }

    /// Get EWC protection
    pub fn ewc(&self) -> &EWCProtection {
        &self.ewc
    }

    /// Get mutable EWC protection
    pub fn ewc_mut(&mut self) -> &mut EWCProtection {
        &mut self.ewc
    }

    /// Get total memory count
    pub fn memory_count(&self) -> usize {
        self.memories.values().map(|m| m.len()).sum()
    }

    /// Get total trajectory count
    pub fn trajectory_count(&self) -> usize {
        self.trajectories.values().map(|t| t.len()).sum()
    }
}

impl Default for ReasoningBank {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between vectors
/// Uses SIMD-friendly loop structure for better performance on large vectors
#[inline]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let n = a.len();
    let chunks = n / 8;

    // Accumulate dot product, norm_a, norm_b in parallel
    let mut dot = 0.0f32;
    let mut norm_a_sq = 0.0f32;
    let mut norm_b_sq = 0.0f32;

    // Process in chunks of 8 for SIMD optimization
    for chunk in 0..chunks {
        let base = chunk * 8;
        let mut chunk_dot = 0.0f32;
        let mut chunk_norm_a = 0.0f32;
        let mut chunk_norm_b = 0.0f32;

        // Unrolled inner loop - compiler can vectorize this
        for k in 0..8 {
            let i = base + k;
            let ai = a[i];
            let bi = b[i];
            chunk_dot += ai * bi;
            chunk_norm_a += ai * ai;
            chunk_norm_b += bi * bi;
        }

        dot += chunk_dot;
        norm_a_sq += chunk_norm_a;
        norm_b_sq += chunk_norm_b;
    }

    // Handle remainder
    for i in (chunks * 8)..n {
        let ai = a[i];
        let bi = b[i];
        dot += ai * bi;
        norm_a_sq += ai * ai;
        norm_b_sq += bi * bi;
    }

    let norm_a = norm_a_sq.sqrt();
    let norm_b = norm_b_sq.sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

// ============================================================================
// SONA Substrate
// ============================================================================

/// Self-Optimizing Neural Architecture substrate
#[derive(Debug)]
pub struct SONASubstrate {
    /// Neural weights (simplified representation)
    weights: Vec<f32>,
    /// Learning rate
    learning_rate: f32,
    /// Adaptation rate (<0.05ms target)
    adaptation_rate: f32,
    /// Memory bank
    reasoning_bank: ReasoningBank,
    /// Performance history
    performance_history: VecDeque<f32>,
    /// Maximum history length
    max_history: usize,
}

impl SONASubstrate {
    /// Create a new SONA substrate
    pub fn new() -> Self {
        Self {
            weights: Vec::new(),
            learning_rate: 0.001,
            adaptation_rate: 0.1,
            reasoning_bank: ReasoningBank::new(),
            performance_history: VecDeque::new(),
            max_history: 1000,
        }
    }

    /// Set learning rate
    pub fn with_learning_rate(mut self, rate: f32) -> Self {
        self.learning_rate = rate;
        self
    }

    /// Set reasoning bank
    pub fn with_reasoning_bank(mut self, bank: ReasoningBank) -> Self {
        self.reasoning_bank = bank;
        self
    }

    /// Set EWC protection level
    pub fn with_ewc_protection(mut self, threshold: f32) -> Self {
        self.reasoning_bank.ewc = EWCProtection::new(threshold);
        self
    }

    /// Initialize weights
    pub fn initialize(&mut self, n_params: usize) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.weights = (0..n_params)
            .map(|_| rng.gen_range(-0.1..0.1))
            .collect();
        self.reasoning_bank.ewc.initialize(&self.weights);
    }

    /// Adapt weights based on feedback
    /// Uses SIMD-friendly loop patterns for better vectorization
    #[inline]
    pub fn adapt(&mut self, gradients: &[f32], performance: f32) {
        // Record performance
        if self.performance_history.len() >= self.max_history {
            self.performance_history.pop_front();
        }
        self.performance_history.push_back(performance);

        // Update EWC Fisher information
        self.reasoning_bank.ewc.update_fisher(gradients);

        // Compute EWC penalty
        let ewc_penalty = self.reasoning_bank.ewc.compute_penalty(&self.weights);

        // Adapt learning rate based on performance trend
        if self.performance_history.len() > 10 {
            let recent: f32 = self.performance_history.iter().rev().take(10).sum::<f32>() / 10.0;
            let old: f32 = self.performance_history.iter().take(10).sum::<f32>() / 10.0;

            if recent > old {
                // Improving: increase learning rate slightly
                self.learning_rate = (self.learning_rate * 1.01).min(0.1);
            } else {
                // Getting worse: decrease learning rate
                self.learning_rate = (self.learning_rate * 0.99).max(0.0001);
            }
        }

        // Apply gradient update with EWC regularization
        for (i, &grad) in gradients.iter().enumerate() {
            if i < self.weights.len() {
                let protection = self.reasoning_bank.ewc.protection_level(i);
                let effective_lr = self.learning_rate / (1.0 + protection);
                self.weights[i] -= effective_lr * (grad + ewc_penalty * (self.weights[i] - self.reasoning_bank.ewc.reference_params.get(i).copied().unwrap_or(0.0)));
            }
        }
    }

    /// Get current weights
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Get reasoning bank
    pub fn reasoning_bank(&self) -> &ReasoningBank {
        &self.reasoning_bank
    }

    /// Get mutable reasoning bank
    pub fn reasoning_bank_mut(&mut self) -> &mut ReasoningBank {
        &mut self.reasoning_bank
    }

    /// Get average performance
    pub fn average_performance(&self) -> f32 {
        if self.performance_history.is_empty() {
            return 0.0;
        }
        self.performance_history.iter().sum::<f32>() / self.performance_history.len() as f32
    }
}

impl Default for SONASubstrate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ADR-001 Learning Safety Bounds
// ============================================================================

/// Learning safety configuration from ADR-001 Part II-B
///
/// Implements safety bounds on the learning process:
/// - Max policy update magnitude (gradient clipping)
/// - Reward signal bound (|R| < R_max)
/// - Memory modification rate limit
/// - Drift spike rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSafetyConfig {
    /// Maximum policy update L2 norm (gradient clipping)
    pub max_policy_update_norm: f32,
    /// Maximum absolute reward signal
    pub max_reward_magnitude: f32,
    /// Maximum memory modifications per tick
    pub max_memory_mods_per_tick: u32,
    /// Drift spike threshold (triggers rollback if exceeded)
    pub drift_spike_threshold: f32,
    /// Number of steps to keep for rollback
    pub rollback_buffer_size: usize,
    /// EWC lambda for catastrophic forgetting prevention
    pub ewc_lambda: f32,
    /// Enable strict mode (drop violations vs. clip)
    pub strict_mode: bool,
}

impl Default for LearningSafetyConfig {
    fn default() -> Self {
        Self {
            max_policy_update_norm: 0.01,
            max_reward_magnitude: 100.0,
            max_memory_mods_per_tick: 5,
            drift_spike_threshold: 10.0,  // 10x baseline drift triggers rollback
            rollback_buffer_size: 100,
            ewc_lambda: 0.5,
            strict_mode: false,
        }
    }
}

/// Learning safety enforcer (ADR-001 compliant)
///
/// Implements all learning safety bounds from ADR-001:
/// - Gradient clipping to max L2 norm
/// - Reward clipping to bounded range
/// - Memory write rate limiting
/// - Drift spike detection and rollback
#[derive(Debug)]
pub struct LearningSafetyEnforcer {
    /// Configuration
    config: LearningSafetyConfig,
    /// Memory modifications this tick
    memory_mods_this_tick: u32,
    /// Rollback checkpoints (parameter snapshots)
    rollback_checkpoints: VecDeque<Vec<f32>>,
    /// Performance history for drift detection
    performance_history: VecDeque<f32>,
    /// Baseline performance (for drift calculation)
    baseline_performance: Option<f32>,
    /// Total violations detected
    violation_count: u64,
    /// Last violation description
    last_violation: Option<String>,
}

impl LearningSafetyEnforcer {
    /// Create a new safety enforcer with default config
    pub fn new() -> Self {
        Self::with_config(LearningSafetyConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: LearningSafetyConfig) -> Self {
        Self {
            config,
            memory_mods_this_tick: 0,
            rollback_checkpoints: VecDeque::new(),
            performance_history: VecDeque::new(),
            baseline_performance: None,
            violation_count: 0,
            last_violation: None,
        }
    }

    /// Clip gradient to maximum L2 norm (ADR-001: Max policy update magnitude)
    ///
    /// Returns the original norm before clipping.
    pub fn clip_gradient(&self, gradient: &mut [f32]) -> f32 {
        let norm_sq: f32 = gradient.iter().map(|x| x * x).sum();
        let norm = norm_sq.sqrt();

        if norm > self.config.max_policy_update_norm {
            let scale = self.config.max_policy_update_norm / norm;
            for g in gradient.iter_mut() {
                *g *= scale;
            }
        }

        norm
    }

    /// Clip reward to bounded range (ADR-001: Reward signal bound |R| < R_max)
    ///
    /// Returns the original reward before clipping.
    pub fn clip_reward(&self, reward: f32) -> (f32, f32) {
        let original = reward;
        let clipped = reward.clamp(
            -self.config.max_reward_magnitude,
            self.config.max_reward_magnitude,
        );
        (clipped, original)
    }

    /// Check if a memory write is allowed this tick
    ///
    /// Returns true if the write is allowed, false if rate limit exceeded.
    pub fn can_write_memory(&self) -> bool {
        self.memory_mods_this_tick < self.config.max_memory_mods_per_tick
    }

    /// Record a memory write
    ///
    /// Returns true if the write was allowed, false if it should be blocked.
    pub fn record_memory_write(&mut self) -> bool {
        if self.can_write_memory() {
            self.memory_mods_this_tick += 1;
            true
        } else {
            self.violation_count += 1;
            self.last_violation = Some("Memory write rate limit exceeded".to_string());
            false
        }
    }

    /// Reset tick counters
    pub fn reset_tick(&mut self) {
        self.memory_mods_this_tick = 0;
    }

    /// Save a checkpoint for potential rollback
    pub fn save_checkpoint(&mut self, params: &[f32]) {
        if self.rollback_checkpoints.len() >= self.config.rollback_buffer_size {
            self.rollback_checkpoints.pop_front();
        }
        self.rollback_checkpoints.push_back(params.to_vec());
    }

    /// Record performance for drift detection
    pub fn record_performance(&mut self, performance: f32) {
        if self.performance_history.len() >= 100 {
            self.performance_history.pop_front();
        }
        self.performance_history.push_back(performance);

        // Update baseline if not set
        if self.baseline_performance.is_none() && self.performance_history.len() >= 10 {
            self.baseline_performance = Some(
                self.performance_history.iter().sum::<f32>() / self.performance_history.len() as f32
            );
        }
    }

    /// Check for drift spike (ADR-001: Drift spike rollback)
    ///
    /// Returns true if a drift spike is detected and rollback is recommended.
    pub fn detect_drift_spike(&self) -> bool {
        let Some(baseline) = self.baseline_performance else {
            return false;
        };

        if self.performance_history.len() < 5 {
            return false;
        }

        // Check recent performance against baseline
        let recent: f32 = self.performance_history.iter().rev().take(5).sum::<f32>() / 5.0;
        let drift = (recent - baseline).abs() / baseline.abs().max(1e-6);

        drift > self.config.drift_spike_threshold
    }

    /// Get rollback checkpoint if drift spike detected
    ///
    /// Returns the checkpoint to restore to, or None if no rollback needed.
    pub fn get_rollback_checkpoint(&self) -> Option<&[f32]> {
        if self.detect_drift_spike() {
            // Return checkpoint from 10 steps ago if available
            let idx = self.rollback_checkpoints.len().saturating_sub(10);
            self.rollback_checkpoints.get(idx).map(|v| v.as_slice())
        } else {
            None
        }
    }

    /// Validate a policy update (combines gradient clipping + drift check)
    ///
    /// Returns a validation result with any necessary corrections.
    pub fn validate_policy_update(
        &mut self,
        gradient: &mut [f32],
        reward: f32,
    ) -> PolicyUpdateValidation {
        let original_norm = self.clip_gradient(gradient);
        let (clipped_reward, original_reward) = self.clip_reward(reward);

        let gradient_clipped = original_norm > self.config.max_policy_update_norm;
        let reward_clipped = original_reward.abs() > self.config.max_reward_magnitude;

        if gradient_clipped {
            self.violation_count += 1;
            self.last_violation = Some(format!(
                "Gradient clipped: {:.4} -> {:.4}",
                original_norm, self.config.max_policy_update_norm
            ));
        }

        if reward_clipped {
            self.violation_count += 1;
            self.last_violation = Some(format!(
                "Reward clipped: {:.4} -> {:.4}",
                original_reward, clipped_reward
            ));
        }

        PolicyUpdateValidation {
            allowed: true,  // Updates are allowed but clipped
            gradient_clipped,
            gradient_original_norm: original_norm,
            reward_clipped,
            reward_original: original_reward,
            reward_final: clipped_reward,
            drift_spike_detected: self.detect_drift_spike(),
            rollback_recommended: self.detect_drift_spike(),
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> LearningSafetyStats {
        LearningSafetyStats {
            memory_mods_this_tick: self.memory_mods_this_tick,
            max_memory_mods: self.config.max_memory_mods_per_tick,
            checkpoints_stored: self.rollback_checkpoints.len(),
            violation_count: self.violation_count,
            drift_spike_detected: self.detect_drift_spike(),
            baseline_performance: self.baseline_performance,
        }
    }
}

impl Default for LearningSafetyEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of policy update validation
#[derive(Debug, Clone)]
pub struct PolicyUpdateValidation {
    /// Whether the update is allowed
    pub allowed: bool,
    /// Whether gradient was clipped
    pub gradient_clipped: bool,
    /// Original gradient L2 norm before clipping
    pub gradient_original_norm: f32,
    /// Whether reward was clipped
    pub reward_clipped: bool,
    /// Original reward before clipping
    pub reward_original: f32,
    /// Final reward after clipping
    pub reward_final: f32,
    /// Whether a drift spike was detected
    pub drift_spike_detected: bool,
    /// Whether rollback is recommended
    pub rollback_recommended: bool,
}

/// Statistics for learning safety
#[derive(Debug, Clone, Copy)]
pub struct LearningSafetyStats {
    pub memory_mods_this_tick: u32,
    pub max_memory_mods: u32,
    pub checkpoints_stored: usize,
    pub violation_count: u64,
    pub drift_spike_detected: bool,
    pub baseline_performance: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry() {
        let entry = MemoryEntry::new(
            AgentId(0),
            vec![1.0, 2.0, 3.0],
            vec![0.5],
            1.0,
        ).with_tag("test");

        assert_eq!(entry.tags, vec!["test"]);
        assert_eq!(entry.reward, 1.0);
    }

    #[test]
    fn test_trajectory() {
        let mut traj = Trajectory::new(AgentId(0));
        traj.add_step(vec![1.0], vec![0.5], 1.0);
        traj.add_step(vec![2.0], vec![0.3], 2.0);

        assert_eq!(traj.len(), 2);
        assert_eq!(traj.total_reward, 3.0);
    }

    #[test]
    fn test_ewc_protection() {
        let mut ewc = EWCProtection::new(0.5);
        ewc.initialize(&[0.0, 0.0, 0.0]);

        let penalty = ewc.compute_penalty(&[0.1, 0.1, 0.1]);
        assert!(penalty > 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.01);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.01);
    }

    // =========================================================================
    // ADR-001 Learning Safety Tests
    // =========================================================================

    #[test]
    fn test_gradient_clipping() {
        let enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            max_policy_update_norm: 1.0,
            ..Default::default()
        });

        // Gradient with norm 5.0 (3,4 => sqrt(9+16) = 5)
        let mut gradient = vec![3.0, 4.0];
        let original_norm = enforcer.clip_gradient(&mut gradient);

        assert!((original_norm - 5.0).abs() < 0.01);
        let new_norm: f32 = gradient.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((new_norm - 1.0).abs() < 0.01); // Should be clipped to 1.0
    }

    #[test]
    fn test_gradient_no_clip_needed() {
        let enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            max_policy_update_norm: 10.0,
            ..Default::default()
        });

        let mut gradient = vec![1.0, 1.0];
        let original_norm = enforcer.clip_gradient(&mut gradient);

        assert!(original_norm < 10.0);
        // Values should be unchanged
        assert!((gradient[0] - 1.0).abs() < 0.01);
        assert!((gradient[1] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_reward_clipping() {
        let enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            max_reward_magnitude: 10.0,
            ..Default::default()
        });

        let (clipped, original) = enforcer.clip_reward(50.0);
        assert_eq!(original, 50.0);
        assert_eq!(clipped, 10.0);

        let (clipped, original) = enforcer.clip_reward(-50.0);
        assert_eq!(original, -50.0);
        assert_eq!(clipped, -10.0);

        let (clipped, original) = enforcer.clip_reward(5.0);
        assert_eq!(original, 5.0);
        assert_eq!(clipped, 5.0); // No clipping needed
    }

    #[test]
    fn test_memory_write_rate_limiting() {
        let mut enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            max_memory_mods_per_tick: 3,
            ..Default::default()
        });

        assert!(enforcer.record_memory_write());
        assert!(enforcer.record_memory_write());
        assert!(enforcer.record_memory_write());
        assert!(!enforcer.record_memory_write()); // Should be blocked

        enforcer.reset_tick();
        assert!(enforcer.record_memory_write()); // Should work again
    }

    #[test]
    fn test_checkpoint_rollback() {
        let mut enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            rollback_buffer_size: 5,
            ..Default::default()
        });

        // Save some checkpoints
        enforcer.save_checkpoint(&[1.0, 2.0]);
        enforcer.save_checkpoint(&[2.0, 3.0]);
        enforcer.save_checkpoint(&[3.0, 4.0]);

        assert_eq!(enforcer.stats().checkpoints_stored, 3);

        // No drift spike yet
        assert!(enforcer.get_rollback_checkpoint().is_none());
    }

    #[test]
    fn test_policy_update_validation() {
        let mut enforcer = LearningSafetyEnforcer::with_config(LearningSafetyConfig {
            max_policy_update_norm: 1.0,
            max_reward_magnitude: 10.0,
            ..Default::default()
        });

        let mut gradient = vec![3.0, 4.0];  // norm = 5
        let validation = enforcer.validate_policy_update(&mut gradient, 50.0);

        assert!(validation.allowed);
        assert!(validation.gradient_clipped);
        assert!(validation.reward_clipped);
        assert_eq!(validation.reward_final, 10.0);
    }

    #[test]
    fn test_learning_safety_stats() {
        let enforcer = LearningSafetyEnforcer::new();
        let stats = enforcer.stats();

        assert_eq!(stats.memory_mods_this_tick, 0);
        assert_eq!(stats.max_memory_mods, 5);
        assert_eq!(stats.checkpoints_stored, 0);
        assert_eq!(stats.violation_count, 0);
        assert!(!stats.drift_spike_detected);
    }
}
