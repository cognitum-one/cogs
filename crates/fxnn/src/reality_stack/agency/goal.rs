//! Goals for agents

use super::AgentState;
use serde::{Deserialize, Serialize};

/// Status of goal achievement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    /// Goal is being pursued
    Active,
    /// Goal has been achieved
    Achieved,
    /// Goal has failed
    Failed,
    /// Goal is paused
    Paused,
}

/// Trait for goal implementations
pub trait Goal: Send + Sync {
    /// Get goal name
    fn name(&self) -> &str;

    /// Evaluate current progress toward goal
    /// Returns value in [0, 1] where 1 = achieved
    fn evaluate(&self, state: &AgentState) -> f32;

    /// Get reward signal for learning
    fn reward(&self, state: &AgentState, prev_state: &AgentState) -> f32;

    /// Check if goal is achieved
    fn is_achieved(&self, state: &AgentState) -> bool {
        self.evaluate(state) >= 0.99
    }

    /// Get goal status
    fn status(&self, state: &AgentState) -> GoalStatus {
        if self.is_achieved(state) {
            GoalStatus::Achieved
        } else {
            GoalStatus::Active
        }
    }

    /// Get priority of this goal
    fn priority(&self) -> f32 {
        1.0
    }
}

/// Goal evaluator that combines multiple goals
pub struct GoalEvaluator {
    goals: Vec<Box<dyn Goal>>,
}

impl GoalEvaluator {
    /// Create a new goal evaluator
    pub fn new() -> Self {
        Self { goals: Vec::new() }
    }

    /// Add a goal
    pub fn add_goal(&mut self, goal: Box<dyn Goal>) {
        self.goals.push(goal);
    }

    /// Evaluate all goals
    pub fn evaluate(&self, state: &AgentState) -> Vec<(String, f32)> {
        self.goals
            .iter()
            .map(|g| (g.name().to_string(), g.evaluate(state)))
            .collect()
    }

    /// Get total reward
    pub fn total_reward(&self, state: &AgentState, prev_state: &AgentState) -> f32 {
        self.goals
            .iter()
            .map(|g| g.reward(state, prev_state) * g.priority())
            .sum()
    }
}

impl Default for GoalEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Built-in Goals
// ============================================================================

/// Goal to minimize energy expenditure
pub struct MinimizeEnergyGoal {
    target_energy: f32,
}

impl MinimizeEnergyGoal {
    /// Create a new minimize energy goal
    pub fn new(target_energy: f32) -> Self {
        Self { target_energy }
    }
}

impl Goal for MinimizeEnergyGoal {
    fn name(&self) -> &str {
        "MinimizeEnergy"
    }

    fn evaluate(&self, state: &AgentState) -> f32 {
        // Higher energy = lower score
        let excess = (state.energy - self.target_energy).max(0.0);
        1.0 / (1.0 + excess / 10.0)
    }

    fn reward(&self, state: &AgentState, prev_state: &AgentState) -> f32 {
        // Reward for reducing energy
        (prev_state.energy - state.energy).max(0.0) * 0.1
    }
}

/// Goal to reach a target position
pub struct ReachPositionGoal {
    target: [f32; 3],
    tolerance: f32,
}

impl ReachPositionGoal {
    /// Create a new reach position goal
    pub fn new(target: [f32; 3], tolerance: f32) -> Self {
        Self { target, tolerance }
    }
}

impl Goal for ReachPositionGoal {
    fn name(&self) -> &str {
        "ReachPosition"
    }

    fn evaluate(&self, state: &AgentState) -> f32 {
        let dx = state.position[0] - self.target[0];
        let dy = state.position[1] - self.target[1];
        let dz = state.position[2] - self.target[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if dist < self.tolerance {
            1.0
        } else {
            self.tolerance / (dist + self.tolerance)
        }
    }

    fn reward(&self, state: &AgentState, prev_state: &AgentState) -> f32 {
        let prev_dist = {
            let dx = prev_state.position[0] - self.target[0];
            let dy = prev_state.position[1] - self.target[1];
            let dz = prev_state.position[2] - self.target[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        let curr_dist = {
            let dx = state.position[0] - self.target[0];
            let dy = state.position[1] - self.target[1];
            let dz = state.position[2] - self.target[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        // Reward for moving closer
        (prev_dist - curr_dist).max(-1.0).min(1.0)
    }
}

/// Goal to maintain survival (health > 0)
pub struct SurvivalGoal {
    health_threshold: f32,
}

impl SurvivalGoal {
    /// Create a new survival goal
    pub fn new() -> Self {
        Self {
            health_threshold: 10.0,
        }
    }

    /// Set health threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.health_threshold = threshold;
        self
    }
}

impl Default for SurvivalGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal for SurvivalGoal {
    fn name(&self) -> &str {
        "Survival"
    }

    fn evaluate(&self, state: &AgentState) -> f32 {
        if state.health <= 0.0 {
            0.0
        } else if state.health >= self.health_threshold {
            1.0
        } else {
            state.health / self.health_threshold
        }
    }

    fn reward(&self, state: &AgentState, prev_state: &AgentState) -> f32 {
        // Large penalty for dying
        if state.health <= 0.0 && prev_state.health > 0.0 {
            -10.0
        } else {
            // Small reward for maintaining health
            (state.health - prev_state.health).clamp(-1.0, 0.1)
        }
    }

    fn priority(&self) -> f32 {
        10.0 // Survival is high priority
    }
}

/// Goal to explore the environment
pub struct ExplorationGoal {
    /// Visited positions (discretized)
    visited: std::collections::HashSet<(i32, i32, i32)>,
    /// Grid resolution
    resolution: f32,
}

impl ExplorationGoal {
    /// Create a new exploration goal
    pub fn new(resolution: f32) -> Self {
        Self {
            visited: std::collections::HashSet::new(),
            resolution,
        }
    }

    /// Discretize position to grid cell
    fn to_cell(&self, pos: [f32; 3]) -> (i32, i32, i32) {
        (
            (pos[0] / self.resolution).floor() as i32,
            (pos[1] / self.resolution).floor() as i32,
            (pos[2] / self.resolution).floor() as i32,
        )
    }

    /// Mark position as visited
    pub fn visit(&mut self, pos: [f32; 3]) {
        self.visited.insert(self.to_cell(pos));
    }

    /// Get number of visited cells
    pub fn visited_count(&self) -> usize {
        self.visited.len()
    }
}

impl Goal for ExplorationGoal {
    fn name(&self) -> &str {
        "Exploration"
    }

    fn evaluate(&self, _state: &AgentState) -> f32 {
        // Never fully achieved - always room to explore
        (self.visited.len() as f32 / 100.0).min(0.99)
    }

    fn reward(&self, state: &AgentState, _prev_state: &AgentState) -> f32 {
        let cell = self.to_cell(state.position);
        if self.visited.contains(&cell) {
            0.0 // No reward for revisiting
        } else {
            1.0 // Reward for new cell
        }
    }

    fn priority(&self) -> f32 {
        0.5 // Medium priority
    }
}

/// Goal to follow another agent/entity
pub struct FollowGoal {
    /// Target entity ID (could be another agent or atom)
    target_id: u64,
    /// Desired following distance
    desired_distance: f32,
    /// Current target position (updated externally)
    target_position: [f32; 3],
}

impl FollowGoal {
    /// Create a new follow goal
    pub fn new(target_id: u64, desired_distance: f32) -> Self {
        Self {
            target_id,
            desired_distance,
            target_position: [0.0; 3],
        }
    }

    /// Update target position
    pub fn update_target(&mut self, position: [f32; 3]) {
        self.target_position = position;
    }

    /// Get target ID
    pub fn target_id(&self) -> u64 {
        self.target_id
    }
}

impl Goal for FollowGoal {
    fn name(&self) -> &str {
        "Follow"
    }

    fn evaluate(&self, state: &AgentState) -> f32 {
        let dx = state.position[0] - self.target_position[0];
        let dy = state.position[1] - self.target_position[1];
        let dz = state.position[2] - self.target_position[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        let error = (dist - self.desired_distance).abs();
        1.0 / (1.0 + error / self.desired_distance)
    }

    fn reward(&self, state: &AgentState, prev_state: &AgentState) -> f32 {
        let prev_error = {
            let dx = prev_state.position[0] - self.target_position[0];
            let dy = prev_state.position[1] - self.target_position[1];
            let dz = prev_state.position[2] - self.target_position[2];
            ((dx * dx + dy * dy + dz * dz).sqrt() - self.desired_distance).abs()
        };

        let curr_error = {
            let dx = state.position[0] - self.target_position[0];
            let dy = state.position[1] - self.target_position[1];
            let dz = state.position[2] - self.target_position[2];
            ((dx * dx + dy * dy + dz * dz).sqrt() - self.desired_distance).abs()
        };

        (prev_error - curr_error).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reach_position_goal() {
        let goal = ReachPositionGoal::new([10.0, 0.0, 0.0], 0.5);

        let mut state = AgentState::default();
        state.position = [10.0, 0.0, 0.0];

        assert!(goal.is_achieved(&state));
        assert!((goal.evaluate(&state) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_survival_goal() {
        let goal = SurvivalGoal::new();

        let mut state = AgentState::default();
        state.health = 100.0;
        assert!((goal.evaluate(&state) - 1.0).abs() < 0.01);

        state.health = 0.0;
        assert!((goal.evaluate(&state) - 0.0).abs() < 0.01);
    }
}
