//! Goal and reward system for agent behavior.
//!
//! This module provides goal definitions and reward functions that
//! drive agent behavior. Goals can be hierarchical, compositional,
//! and include intrinsic motivation mechanisms.
//!
//! # Goal Types
//!
//! | Goal | Description | Reward Structure |
//! |------|-------------|-----------------|
//! | [`DistanceGoal`] | Reach or avoid positions | Distance-based |
//! | [`ResourceGoal`] | Collect or consume resources | Quantity-based |
//! | [`SurvivalGoal`] | Maintain health/energy | Threshold-based |
//! | [`SocialGoal`] | Interact with other agents | Interaction-based |
//! | [`CompositeGoal`] | Combination of goals | Weighted sum |
//!
//! # Intrinsic Motivation
//!
//! Beyond explicit goals, agents can be driven by intrinsic motivation:
//!
//! - **Curiosity**: Seek novel states (prediction error)
//! - **Competence**: Master skills (learning progress)
//! - **Autonomy**: Maintain control over outcomes
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::agency::goal::{DistanceGoal, GoalType, RewardFunction};
//!
//! // Create a goal to reach a position
//! let target = [10.0, 0.0, 0.0];
//! let reach_goal = DistanceGoal::reach_position(target, 0.5);
//!
//! // Create a goal to avoid a position
//! let danger = [5.0, 5.0, 0.0];
//! let avoid_goal = DistanceGoal::avoid_position(danger, 3.0);
//! ```

use super::agent::{AgentState, WorldState};

/// Status of a goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    /// Goal is active and being pursued.
    Active,
    /// Goal has been achieved.
    Achieved,
    /// Goal has failed (cannot be achieved).
    Failed,
    /// Goal is suspended (temporarily inactive).
    Suspended,
}

/// Type of goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalType {
    /// Distance-based goal (reach or avoid).
    Distance,
    /// Resource collection/consumption goal.
    Resource,
    /// Survival/maintenance goal.
    Survival,
    /// Social interaction goal.
    Social,
    /// Exploration/curiosity goal.
    Exploration,
    /// Composite goal (combination).
    Composite,
    /// Custom goal type.
    Custom,
}

/// Trait for goal implementations.
///
/// Goals define objectives for agent behavior and provide reward signals.
pub trait Goal: Send + Sync {
    /// Get the type of this goal.
    fn goal_type(&self) -> GoalType;

    /// Evaluate the goal and return a reward.
    ///
    /// # Arguments
    ///
    /// * `agent_state` - Current state of the agent
    /// * `world_state` - Current state of the world
    ///
    /// # Returns
    ///
    /// Reward value (positive for progress, negative for regression).
    fn evaluate(&self, agent_state: &AgentState, world_state: &WorldState) -> f32;

    /// Check if the goal has been achieved.
    ///
    /// # Arguments
    ///
    /// * `agent_state` - Current state of the agent
    /// * `world_state` - Current state of the world
    ///
    /// # Returns
    ///
    /// `true` if the goal is achieved.
    fn is_achieved(&self, agent_state: &AgentState, world_state: &WorldState) -> bool;

    /// Get the current status of the goal.
    fn status(&self) -> GoalStatus;

    /// Get the priority of this goal (higher = more important).
    fn priority(&self) -> f32 {
        1.0
    }

    /// Get a description of the goal.
    fn description(&self) -> &str;

    /// Reset the goal to its initial state.
    fn reset(&mut self);

    /// Get progress towards the goal (0.0 to 1.0).
    fn progress(&self, agent_state: &AgentState, world_state: &WorldState) -> f32;
}

/// Trait for reward function implementations.
///
/// Reward functions transform state information into scalar rewards.
pub trait RewardFunction: Send + Sync {
    /// Compute the reward for a state transition.
    ///
    /// # Arguments
    ///
    /// * `prev_state` - Previous agent state
    /// * `action` - Action taken
    /// * `curr_state` - Current agent state
    /// * `world_state` - Current world state
    ///
    /// # Returns
    ///
    /// Scalar reward value.
    fn compute(
        &self,
        prev_state: &AgentState,
        curr_state: &AgentState,
        world_state: &WorldState,
    ) -> f32;

    /// Get the name of this reward function.
    fn name(&self) -> &str;

    /// Get the scale factor for this reward.
    fn scale(&self) -> f32 {
        1.0
    }
}

// ============================================================================
// Distance Goal
// ============================================================================

/// Goal based on distance to a target position.
#[derive(Debug, Clone)]
pub struct DistanceGoal {
    /// Target position.
    target: [f32; 3],
    /// Threshold distance for achievement.
    threshold: f32,
    /// Whether to reach (true) or avoid (false) the target.
    reach: bool,
    /// Current status.
    status: GoalStatus,
    /// Description.
    description: String,
    /// Reward scale.
    reward_scale: f32,
    /// Whether to use shaping (continuous reward based on distance).
    use_shaping: bool,
    /// Previous distance (for shaping).
    prev_distance: Option<f32>,
}

impl DistanceGoal {
    /// Create a goal to reach a position.
    ///
    /// # Arguments
    ///
    /// * `target` - Target position to reach
    /// * `threshold` - Distance at which goal is considered achieved
    ///
    /// # Returns
    ///
    /// A new `DistanceGoal` for reaching the target.
    pub fn reach_position(target: [f32; 3], threshold: f32) -> Self {
        Self {
            target,
            threshold,
            reach: true,
            status: GoalStatus::Active,
            description: format!("Reach position {:?}", target),
            reward_scale: 1.0,
            use_shaping: true,
            prev_distance: None,
        }
    }

    /// Create a goal to avoid a position.
    ///
    /// # Arguments
    ///
    /// * `target` - Position to avoid
    /// * `threshold` - Minimum safe distance
    ///
    /// # Returns
    ///
    /// A new `DistanceGoal` for avoiding the target.
    pub fn avoid_position(target: [f32; 3], threshold: f32) -> Self {
        Self {
            target,
            threshold,
            reach: false,
            status: GoalStatus::Active,
            description: format!("Avoid position {:?}", target),
            reward_scale: 1.0,
            use_shaping: true,
            prev_distance: None,
        }
    }

    /// Set the reward scale (builder pattern).
    pub fn with_reward_scale(mut self, scale: f32) -> Self {
        self.reward_scale = scale;
        self
    }

    /// Disable reward shaping (builder pattern).
    pub fn without_shaping(mut self) -> Self {
        self.use_shaping = false;
        self
    }

    /// Calculate distance from agent to target.
    fn distance_to_target(&self, agent_state: &AgentState) -> f32 {
        let dx = self.target[0] - agent_state.position[0];
        let dy = self.target[1] - agent_state.position[1];
        let dz = self.target[2] - agent_state.position[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl Goal for DistanceGoal {
    fn goal_type(&self) -> GoalType {
        GoalType::Distance
    }

    fn evaluate(&self, agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        let distance = self.distance_to_target(agent_state);

        if self.reach {
            // Reaching goal
            if distance <= self.threshold {
                // Achievement bonus
                10.0 * self.reward_scale
            } else if self.use_shaping {
                // Shaping reward: reward for getting closer
                if let Some(prev) = self.prev_distance {
                    (prev - distance) * self.reward_scale
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            // Avoidance goal
            if distance < self.threshold {
                // Penalty for being too close
                -10.0 * self.reward_scale * (1.0 - distance / self.threshold)
            } else if self.use_shaping {
                // Small reward for staying away
                0.1 * self.reward_scale
            } else {
                0.0
            }
        }
    }

    fn is_achieved(&self, agent_state: &AgentState, _world_state: &WorldState) -> bool {
        let distance = self.distance_to_target(agent_state);

        if self.reach {
            distance <= self.threshold
        } else {
            distance > self.threshold
        }
    }

    fn status(&self) -> GoalStatus {
        self.status
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn reset(&mut self) {
        self.status = GoalStatus::Active;
        self.prev_distance = None;
    }

    fn progress(&self, agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        let distance = self.distance_to_target(agent_state);

        if self.reach {
            // Progress is inverse of distance (clamped to 0-1)
            (1.0 - distance / (distance + self.threshold)).clamp(0.0, 1.0)
        } else {
            // Progress is how far beyond threshold
            (distance / self.threshold).min(1.0)
        }
    }
}

// ============================================================================
// Resource Goal
// ============================================================================

/// Goal based on resource collection or consumption.
#[derive(Debug, Clone)]
pub struct ResourceGoal {
    /// Target resource amount.
    target_amount: f32,
    /// Current collected amount.
    current_amount: f32,
    /// Resource type identifier.
    resource_type: u32,
    /// Whether to collect (true) or consume (false).
    collect: bool,
    /// Current status.
    status: GoalStatus,
    /// Description.
    description: String,
    /// Reward per unit resource.
    reward_per_unit: f32,
}

impl ResourceGoal {
    /// Create a goal to collect resources.
    pub fn collect(resource_type: u32, target_amount: f32) -> Self {
        Self {
            target_amount,
            current_amount: 0.0,
            resource_type,
            collect: true,
            status: GoalStatus::Active,
            description: format!("Collect {} units of resource {}", target_amount, resource_type),
            reward_per_unit: 1.0,
        }
    }

    /// Create a goal to consume resources.
    pub fn consume(resource_type: u32, target_amount: f32) -> Self {
        Self {
            target_amount,
            current_amount: target_amount,
            resource_type,
            collect: false,
            status: GoalStatus::Active,
            description: format!("Consume {} units of resource {}", target_amount, resource_type),
            reward_per_unit: 1.0,
        }
    }

    /// Add resources to the collected amount.
    pub fn add_resource(&mut self, amount: f32) {
        if self.collect {
            self.current_amount += amount;
        } else {
            self.current_amount -= amount;
        }

        // Check for achievement
        if self.is_target_met() {
            self.status = GoalStatus::Achieved;
        }
    }

    /// Check if the target is met.
    fn is_target_met(&self) -> bool {
        if self.collect {
            self.current_amount >= self.target_amount
        } else {
            self.current_amount <= 0.0
        }
    }

    /// Get the current amount.
    pub fn current_amount(&self) -> f32 {
        self.current_amount
    }
}

impl Goal for ResourceGoal {
    fn goal_type(&self) -> GoalType {
        GoalType::Resource
    }

    fn evaluate(&self, _agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        // Reward is based on change in resources (handled externally)
        0.0
    }

    fn is_achieved(&self, _agent_state: &AgentState, _world_state: &WorldState) -> bool {
        self.is_target_met()
    }

    fn status(&self) -> GoalStatus {
        self.status
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn reset(&mut self) {
        self.status = GoalStatus::Active;
        if self.collect {
            self.current_amount = 0.0;
        } else {
            self.current_amount = self.target_amount;
        }
    }

    fn progress(&self, _agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        if self.collect {
            (self.current_amount / self.target_amount).clamp(0.0, 1.0)
        } else {
            (1.0 - self.current_amount / self.target_amount).clamp(0.0, 1.0)
        }
    }
}

// ============================================================================
// Survival Goal
// ============================================================================

/// Goal to maintain health or energy above a threshold.
#[derive(Debug, Clone)]
pub struct SurvivalGoal {
    /// Minimum acceptable level.
    threshold: f32,
    /// Whether this tracks health (true) or energy (false).
    track_health: bool,
    /// Status.
    status: GoalStatus,
    /// Description.
    description: String,
    /// Penalty for falling below threshold.
    penalty_scale: f32,
}

impl SurvivalGoal {
    /// Create a health maintenance goal.
    pub fn maintain_health(threshold: f32) -> Self {
        Self {
            threshold,
            track_health: true,
            status: GoalStatus::Active,
            description: format!("Maintain health above {:.1}", threshold),
            penalty_scale: 10.0,
        }
    }

    /// Create an energy maintenance goal.
    pub fn maintain_energy(threshold: f32) -> Self {
        Self {
            threshold,
            track_health: false,
            status: GoalStatus::Active,
            description: format!("Maintain energy above {:.1}", threshold),
            penalty_scale: 5.0,
        }
    }

    /// Get the tracked value from agent state.
    fn get_value(&self, agent_state: &AgentState) -> f32 {
        if self.track_health {
            agent_state.health
        } else {
            agent_state.energy
        }
    }
}

impl Goal for SurvivalGoal {
    fn goal_type(&self) -> GoalType {
        GoalType::Survival
    }

    fn evaluate(&self, agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        let value = self.get_value(agent_state);

        if value <= 0.0 {
            // Death penalty
            -100.0
        } else if value < self.threshold {
            // Penalty for being below threshold
            -(self.threshold - value) * self.penalty_scale
        } else {
            // Small reward for staying healthy
            0.1
        }
    }

    fn is_achieved(&self, agent_state: &AgentState, _world_state: &WorldState) -> bool {
        // Survival goals are never "achieved" - they're ongoing
        self.get_value(agent_state) > 0.0
    }

    fn status(&self) -> GoalStatus {
        self.status
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn reset(&mut self) {
        self.status = GoalStatus::Active;
    }

    fn progress(&self, agent_state: &AgentState, _world_state: &WorldState) -> f32 {
        self.get_value(agent_state).clamp(0.0, 1.0)
    }
}

// ============================================================================
// Composite Goal
// ============================================================================

/// A composite goal combining multiple sub-goals.
pub struct CompositeGoal {
    /// Sub-goals with their weights.
    goals: Vec<(Box<dyn Goal>, f32)>,
    /// How to combine goals.
    combination: GoalCombination,
    /// Status.
    status: GoalStatus,
    /// Description.
    description: String,
}

impl std::fmt::Debug for CompositeGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeGoal")
            .field("goals_count", &self.goals.len())
            .field("combination", &self.combination)
            .field("status", &self.status)
            .field("description", &self.description)
            .finish()
    }
}

/// How to combine multiple goals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalCombination {
    /// Weighted sum of rewards.
    WeightedSum,
    /// Maximum reward across goals.
    Maximum,
    /// Minimum reward across goals.
    Minimum,
    /// All goals must be achieved.
    All,
    /// Any goal can be achieved.
    Any,
}

impl CompositeGoal {
    /// Create a new composite goal.
    pub fn new(combination: GoalCombination) -> Self {
        Self {
            goals: Vec::new(),
            combination,
            status: GoalStatus::Active,
            description: "Composite goal".to_string(),
        }
    }

    /// Add a sub-goal with weight (builder pattern).
    pub fn with_goal(mut self, goal: Box<dyn Goal>, weight: f32) -> Self {
        self.goals.push((goal, weight));
        self
    }

    /// Add a sub-goal with default weight.
    pub fn add_goal(&mut self, goal: Box<dyn Goal>) {
        self.goals.push((goal, 1.0));
    }

    /// Set the description (builder pattern).
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

impl Goal for CompositeGoal {
    fn goal_type(&self) -> GoalType {
        GoalType::Composite
    }

    fn evaluate(&self, agent_state: &AgentState, world_state: &WorldState) -> f32 {
        if self.goals.is_empty() {
            return 0.0;
        }

        match self.combination {
            GoalCombination::WeightedSum => {
                let total_weight: f32 = self.goals.iter().map(|(_, w)| w).sum();
                if total_weight == 0.0 {
                    return 0.0;
                }
                self.goals
                    .iter()
                    .map(|(g, w)| g.evaluate(agent_state, world_state) * w)
                    .sum::<f32>()
                    / total_weight
            }
            GoalCombination::Maximum => {
                self.goals
                    .iter()
                    .map(|(g, _)| g.evaluate(agent_state, world_state))
                    .fold(f32::NEG_INFINITY, f32::max)
            }
            GoalCombination::Minimum => {
                self.goals
                    .iter()
                    .map(|(g, _)| g.evaluate(agent_state, world_state))
                    .fold(f32::INFINITY, f32::min)
            }
            GoalCombination::All | GoalCombination::Any => {
                // For achievement-based combinations, sum rewards
                self.goals
                    .iter()
                    .map(|(g, _)| g.evaluate(agent_state, world_state))
                    .sum()
            }
        }
    }

    fn is_achieved(&self, agent_state: &AgentState, world_state: &WorldState) -> bool {
        if self.goals.is_empty() {
            return false;
        }

        match self.combination {
            GoalCombination::All | GoalCombination::WeightedSum | GoalCombination::Minimum => {
                self.goals
                    .iter()
                    .all(|(g, _)| g.is_achieved(agent_state, world_state))
            }
            GoalCombination::Any | GoalCombination::Maximum => {
                self.goals
                    .iter()
                    .any(|(g, _)| g.is_achieved(agent_state, world_state))
            }
        }
    }

    fn status(&self) -> GoalStatus {
        self.status
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn reset(&mut self) {
        self.status = GoalStatus::Active;
        for (goal, _) in &mut self.goals {
            goal.reset();
        }
    }

    fn progress(&self, agent_state: &AgentState, world_state: &WorldState) -> f32 {
        if self.goals.is_empty() {
            return 0.0;
        }

        let progresses: Vec<f32> = self.goals
            .iter()
            .map(|(g, _)| g.progress(agent_state, world_state))
            .collect();

        match self.combination {
            GoalCombination::WeightedSum | GoalCombination::All => {
                progresses.iter().sum::<f32>() / progresses.len() as f32
            }
            GoalCombination::Maximum | GoalCombination::Any => {
                progresses.iter().cloned().fold(0.0, f32::max)
            }
            GoalCombination::Minimum => {
                progresses.iter().cloned().fold(1.0, f32::min)
            }
        }
    }
}

// ============================================================================
// Intrinsic Motivation
// ============================================================================

/// Type of intrinsic motivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicType {
    /// Curiosity: seek novel states.
    Curiosity,
    /// Competence: improve skills.
    Competence,
    /// Autonomy: maintain control.
    Autonomy,
}

/// Intrinsic motivation system.
///
/// Provides rewards based on internal drives rather than external objectives.
#[derive(Debug, Clone)]
pub struct IntrinsicMotivation {
    /// Type of intrinsic motivation.
    motivation_type: IntrinsicType,
    /// Reward scale.
    scale: f32,
    /// State visitation counts (for curiosity).
    state_counts: Vec<u32>,
    /// State discretization resolution.
    resolution: f32,
    /// Prediction model error (for curiosity).
    prediction_error: f32,
    /// Learning progress (for competence).
    learning_progress: f32,
}

impl IntrinsicMotivation {
    /// Create curiosity-driven motivation.
    ///
    /// Rewards visiting novel states that are difficult to predict.
    pub fn curiosity(scale: f32) -> Self {
        Self {
            motivation_type: IntrinsicType::Curiosity,
            scale,
            state_counts: Vec::new(),
            resolution: 1.0,
            prediction_error: 0.0,
            learning_progress: 0.0,
        }
    }

    /// Create competence-driven motivation.
    ///
    /// Rewards improvement in skill/prediction ability.
    pub fn competence(scale: f32) -> Self {
        Self {
            motivation_type: IntrinsicType::Competence,
            scale,
            state_counts: Vec::new(),
            resolution: 1.0,
            prediction_error: 0.0,
            learning_progress: 0.0,
        }
    }

    /// Create autonomy-driven motivation.
    ///
    /// Rewards being in control of outcomes.
    pub fn autonomy(scale: f32) -> Self {
        Self {
            motivation_type: IntrinsicType::Autonomy,
            scale,
            state_counts: Vec::new(),
            resolution: 1.0,
            prediction_error: 0.0,
            learning_progress: 0.0,
        }
    }

    /// Set the discretization resolution (builder pattern).
    pub fn with_resolution(mut self, resolution: f32) -> Self {
        self.resolution = resolution;
        self
    }

    /// Discretize a position to a state index.
    fn discretize(&self, position: [f32; 3]) -> usize {
        let ix = (position[0] / self.resolution).floor() as i32;
        let iy = (position[1] / self.resolution).floor() as i32;
        let iz = (position[2] / self.resolution).floor() as i32;

        // Simple hash function
        let hash = (ix.wrapping_mul(73856093))
            ^ (iy.wrapping_mul(19349663))
            ^ (iz.wrapping_mul(83492791));
        hash.unsigned_abs() as usize % 10000
    }

    /// Update with a new state observation.
    pub fn observe(&mut self, agent_state: &AgentState, prediction_error: f32) {
        // Update state counts for curiosity
        let state_idx = self.discretize(agent_state.position);
        if state_idx >= self.state_counts.len() {
            self.state_counts.resize(state_idx + 1, 0);
        }
        self.state_counts[state_idx] += 1;

        // Update prediction error
        let alpha = 0.1;
        self.prediction_error = (1.0 - alpha) * self.prediction_error + alpha * prediction_error;

        // Update learning progress (derivative of prediction error)
        let prev_error = self.prediction_error;
        self.learning_progress = prev_error - prediction_error;
    }

    /// Compute the intrinsic reward.
    pub fn compute_reward(&self, agent_state: &AgentState) -> f32 {
        match self.motivation_type {
            IntrinsicType::Curiosity => {
                // Reward inversely proportional to state visitation count
                let state_idx = self.discretize(agent_state.position);
                let count = self.state_counts.get(state_idx).copied().unwrap_or(0);
                let novelty = 1.0 / (1.0 + count as f32);

                // Also reward prediction errors
                let curiosity_reward = novelty + self.prediction_error;
                curiosity_reward * self.scale
            }
            IntrinsicType::Competence => {
                // Reward learning progress (improvement in predictions)
                self.learning_progress * self.scale
            }
            IntrinsicType::Autonomy => {
                // Reward being able to control outcomes
                // Simplified: reward for having options (low force resistance)
                let force_mag = (agent_state.force[0].powi(2)
                    + agent_state.force[1].powi(2)
                    + agent_state.force[2].powi(2))
                .sqrt();
                let autonomy = 1.0 / (1.0 + force_mag);
                autonomy * self.scale
            }
        }
    }

    /// Reset the motivation state.
    pub fn reset(&mut self) {
        self.state_counts.clear();
        self.prediction_error = 0.0;
        self.learning_progress = 0.0;
    }

    /// Get the motivation type.
    pub fn motivation_type(&self) -> IntrinsicType {
        self.motivation_type
    }
}

impl RewardFunction for IntrinsicMotivation {
    fn compute(
        &self,
        _prev_state: &AgentState,
        curr_state: &AgentState,
        _world_state: &WorldState,
    ) -> f32 {
        self.compute_reward(curr_state)
    }

    fn name(&self) -> &str {
        match self.motivation_type {
            IntrinsicType::Curiosity => "CuriosityReward",
            IntrinsicType::Competence => "CompetenceReward",
            IntrinsicType::Autonomy => "AutonomyReward",
        }
    }

    fn scale(&self) -> f32 {
        self.scale
    }
}

// ============================================================================
// Standard Reward Functions
// ============================================================================

/// Distance-based reward function.
#[derive(Debug, Clone)]
pub struct DistanceReward {
    /// Target position.
    target: [f32; 3],
    /// Scale factor.
    scale: f32,
    /// Name.
    name: String,
}

impl DistanceReward {
    /// Create a new distance reward.
    pub fn new(target: [f32; 3], scale: f32) -> Self {
        Self {
            target,
            scale,
            name: "DistanceReward".to_string(),
        }
    }
}

impl RewardFunction for DistanceReward {
    fn compute(
        &self,
        prev_state: &AgentState,
        curr_state: &AgentState,
        _world_state: &WorldState,
    ) -> f32 {
        // Reward for getting closer
        let prev_dist = {
            let dx = self.target[0] - prev_state.position[0];
            let dy = self.target[1] - prev_state.position[1];
            let dz = self.target[2] - prev_state.position[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        let curr_dist = {
            let dx = self.target[0] - curr_state.position[0];
            let dy = self.target[1] - curr_state.position[1];
            let dz = self.target[2] - curr_state.position[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        (prev_dist - curr_dist) * self.scale
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn scale(&self) -> f32 {
        self.scale
    }
}

/// Energy efficiency reward.
#[derive(Debug, Clone)]
pub struct EfficiencyReward {
    /// Scale factor.
    scale: f32,
    /// Name.
    name: String,
}

impl EfficiencyReward {
    /// Create a new efficiency reward.
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            name: "EfficiencyReward".to_string(),
        }
    }
}

impl RewardFunction for EfficiencyReward {
    fn compute(
        &self,
        _prev_state: &AgentState,
        curr_state: &AgentState,
        _world_state: &WorldState,
    ) -> f32 {
        // Penalize energy expenditure (high forces)
        let force_mag = (curr_state.force[0].powi(2)
            + curr_state.force[1].powi(2)
            + curr_state.force[2].powi(2))
        .sqrt();
        -force_mag * self.scale
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn scale(&self) -> f32 {
        self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_goal_reach() {
        let target = [10.0, 0.0, 0.0];
        let goal = DistanceGoal::reach_position(target, 1.0);

        let mut state = AgentState::new();

        // Not at target
        assert!(!goal.is_achieved(&state, &WorldState::default()));

        // At target
        state.position = [10.0, 0.0, 0.0];
        assert!(goal.is_achieved(&state, &WorldState::default()));

        // Near target
        state.position = [9.5, 0.0, 0.0];
        assert!(goal.is_achieved(&state, &WorldState::default()));
    }

    #[test]
    fn test_distance_goal_avoid() {
        let danger = [0.0, 0.0, 0.0];
        let goal = DistanceGoal::avoid_position(danger, 5.0);

        let mut state = AgentState::new();

        // At danger position
        assert!(!goal.is_achieved(&state, &WorldState::default()));

        // Far from danger
        state.position = [10.0, 0.0, 0.0];
        assert!(goal.is_achieved(&state, &WorldState::default()));
    }

    #[test]
    fn test_survival_goal() {
        let goal = SurvivalGoal::maintain_health(0.5);

        let mut state = AgentState::new();
        state.health = 1.0;

        // Healthy
        let reward = goal.evaluate(&state, &WorldState::default());
        assert!(reward > 0.0);

        // Low health
        state.health = 0.3;
        let reward = goal.evaluate(&state, &WorldState::default());
        assert!(reward < 0.0);
    }

    #[test]
    fn test_composite_goal() {
        // Use closer targets with larger thresholds so a single position can satisfy both
        let goal = CompositeGoal::new(GoalCombination::All)
            .with_goal(Box::new(DistanceGoal::reach_position([1.0, 0.0, 0.0], 1.5)), 1.0)
            .with_goal(Box::new(DistanceGoal::reach_position([0.0, 1.0, 0.0], 1.5)), 1.0);

        let mut state = AgentState::new();

        // Neither achieved (at origin)
        // Distance to [1.0, 0.0, 0.0] = 1.0, Distance to [0.0, 1.0, 0.0] = 1.0
        // Both are within threshold 1.5, so both would be achieved at origin
        state.position = [0.0, 0.0, 5.0]; // Far from both targets
        assert!(!goal.is_achieved(&state, &WorldState::default()));

        // One achieved
        state.position = [1.0, 0.0, 0.0]; // At first target
        // Distance to [0.0, 1.0, 0.0] = sqrt(1 + 1) = 1.414 < 1.5, so both achieved
        // Need to use tighter thresholds
        let goal_tight = CompositeGoal::new(GoalCombination::All)
            .with_goal(Box::new(DistanceGoal::reach_position([2.0, 0.0, 0.0], 0.5)), 1.0)
            .with_goal(Box::new(DistanceGoal::reach_position([0.0, 2.0, 0.0], 0.5)), 1.0);

        state.position = [0.0, 0.0, 0.0]; // At origin, far from both (distance = 2.0 each)
        assert!(!goal_tight.is_achieved(&state, &WorldState::default()));

        // One achieved - at first target
        state.position = [2.0, 0.0, 0.0];
        assert!(!goal_tight.is_achieved(&state, &WorldState::default())); // Second still not achieved

        // Both achieved - position that's within 0.5 of both [2,0,0] and [0,2,0]
        // Such position doesn't exist for distance 0.5. Use diagonal approach:
        // Position [1, 1, 0] is sqrt(2) ≈ 1.414 away from both - need larger threshold
        let goal_diag = CompositeGoal::new(GoalCombination::All)
            .with_goal(Box::new(DistanceGoal::reach_position([2.0, 0.0, 0.0], 1.5)), 1.0)
            .with_goal(Box::new(DistanceGoal::reach_position([0.0, 2.0, 0.0], 1.5)), 1.0);

        state.position = [1.0, 1.0, 0.0]; // sqrt((2-1)^2 + 1^2) = sqrt(2) ≈ 1.414 < 1.5 for both
        assert!(goal_diag.is_achieved(&state, &WorldState::default()));
    }

    #[test]
    fn test_intrinsic_motivation() {
        let mut curiosity = IntrinsicMotivation::curiosity(1.0);

        let state = AgentState::new();

        // First visit to a state should have high reward
        let reward1 = curiosity.compute_reward(&state);
        curiosity.observe(&state, 0.5);

        // Subsequent visits should have lower reward
        let reward2 = curiosity.compute_reward(&state);
        curiosity.observe(&state, 0.4);

        assert!(reward1 > reward2);
    }
}
