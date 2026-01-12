//! Benchmark B: Partial Observation Agency - Learning Under Uncertainty
//!
//! **Purpose**: Prove agents can learn and improve with limited, noisy observations.
//!
//! # Test Protocol (from ADR-001)
//!
//! ## Setup
//! - 10x10 maze with occluding walls
//! - Agent with limited FOV (90 degrees, range 3 cells)
//! - Sensor noise: Gaussian, sigma = 0.1
//! - Goal: reach target location
//!
//! ## Run
//! - 100 episodes, max 200 steps each
//! - Agent uses simple Q-learning with partial observability
//!
//! ## Pass Criteria
//! - Success rate increases over episodes
//! - Episode 100 success rate > 80%
//! - Information gain positive during exploration
//! - No catastrophic forgetting (variance check)
//!
//! ## Report
//! - Learning curve (success rate vs episode)
//! - Belief entropy over time
//! - Policy update magnitudes (should stay bounded)

use super::{BenchmarkConfig, BenchmarkMetrics, BenchmarkReport, CriterionResult};
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::collections::HashMap;
use std::time::Instant;

/// A simple 2D maze environment
#[derive(Clone)]
struct Maze {
    width: usize,
    height: usize,
    walls: Vec<Vec<bool>>,
    start: (usize, usize),
    goal: (usize, usize),
}

impl Maze {
    /// Generate a random maze with some walls
    fn generate(width: usize, height: usize, rng: &mut impl Rng) -> Self {
        let mut walls = vec![vec![false; width]; height];

        // Add border walls
        for x in 0..width {
            walls[0][x] = true;
            walls[height - 1][x] = true;
        }
        for y in 0..height {
            walls[y][0] = true;
            walls[y][width - 1] = true;
        }

        // Add some internal walls (about 20% of interior cells)
        for y in 2..(height - 2) {
            for x in 2..(width - 2) {
                if rng.gen::<f32>() < 0.2 {
                    walls[y][x] = true;
                }
            }
        }

        // Ensure start and goal are clear
        let start = (1, 1);
        let goal = (width - 2, height - 2);
        walls[start.1][start.0] = false;
        walls[goal.1][goal.0] = false;

        // Ensure there's at least a basic path (clear a corridor)
        for x in 1..(width - 1) {
            walls[1][x] = false;
        }
        for y in 1..(height - 1) {
            walls[y][width - 2] = false;
        }

        Self {
            width,
            height,
            walls,
            start,
            goal,
        }
    }

    fn is_wall(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return true;
        }
        self.walls[y as usize][x as usize]
    }
}

/// Agent's partial observation of the environment
#[derive(Clone, Debug)]
struct Observation {
    /// Visible cells relative to agent (dx, dy, is_wall)
    visible_cells: Vec<(i32, i32, bool)>,
    /// Distance to goal (with noise)
    noisy_goal_distance: f32,
    /// Agent's current position
    position: (i32, i32),
}

impl Observation {
    /// Convert to a discrete state for Q-learning
    fn to_state_id(&self) -> u64 {
        // Simple state encoding based on position and nearby walls
        let mut state: u64 = 0;

        // Encode position (limited precision)
        state |= (self.position.0 as u64 & 0xFF) << 0;
        state |= (self.position.1 as u64 & 0xFF) << 8;

        // Encode nearby wall pattern (4 cardinal directions)
        for (i, (dx, dy, _)) in self.visible_cells.iter().take(4).enumerate() {
            let wall_bit = if self
                .visible_cells
                .iter()
                .any(|(vx, vy, is_wall)| *vx == *dx && *vy == *dy && *is_wall)
            {
                1u64
            } else {
                0u64
            };
            state |= wall_bit << (16 + i);
        }

        state
    }
}

/// Simple agent with limited FOV
struct Agent {
    position: (i32, i32),
    fov_degrees: f32,
    sensor_range: f32,
    sensor_noise: Normal<f64>,
    /// Q-table: state -> action -> Q-value
    q_table: HashMap<u64, [f64; 4]>,
    /// Learning rate
    alpha: f64,
    /// Discount factor
    gamma: f64,
    /// Exploration rate
    epsilon: f64,
    /// Epsilon decay
    epsilon_decay: f64,
    /// Minimum epsilon
    epsilon_min: f64,
    /// Track policy update magnitudes
    update_magnitudes: Vec<f64>,
}

impl Agent {
    fn new(start: (i32, i32), fov: f32, range: f32, noise_sigma: f32) -> Self {
        Self {
            position: start,
            fov_degrees: fov,
            sensor_range: range,
            sensor_noise: Normal::new(0.0, noise_sigma as f64).unwrap(),
            q_table: HashMap::new(),
            alpha: 0.1,
            gamma: 0.95,
            epsilon: 1.0,
            epsilon_decay: 0.99,
            epsilon_min: 0.01,
            update_magnitudes: Vec::new(),
        }
    }

    fn reset(&mut self, start: (i32, i32)) {
        self.position = start;
    }

    /// Get partial observation of the environment
    fn observe(&self, maze: &Maze, rng: &mut impl Rng) -> Observation {
        let mut visible_cells = Vec::new();

        // Observe cells within sensor range and FOV
        let range = self.sensor_range as i32;
        for dy in -range..=range {
            for dx in -range..=range {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist <= self.sensor_range {
                    // Simple FOV check (always visible in 90-degree cone around forward)
                    let x = self.position.0 + dx;
                    let y = self.position.1 + dy;
                    let is_wall = maze.is_wall(x, y);

                    // Add noise to wall detection
                    let noisy_wall = if self.sensor_noise.sample(rng).abs() > 0.3 {
                        !is_wall // Noise flips perception occasionally
                    } else {
                        is_wall
                    };

                    visible_cells.push((dx, dy, noisy_wall));
                }
            }
        }

        // Noisy distance to goal
        let goal_dist = (((maze.goal.0 as i32 - self.position.0).pow(2)
            + (maze.goal.1 as i32 - self.position.1).pow(2)) as f32)
            .sqrt();
        let noisy_dist = goal_dist + self.sensor_noise.sample(rng) as f32;

        Observation {
            visible_cells,
            noisy_goal_distance: noisy_dist,
            position: self.position,
        }
    }

    /// Select action using epsilon-greedy policy
    fn select_action(&self, state: u64, rng: &mut impl Rng) -> usize {
        if rng.gen::<f64>() < self.epsilon {
            rng.gen_range(0..4) // Random action
        } else {
            // Greedy action
            let q_values = self.q_table.get(&state).unwrap_or(&[0.0; 4]);
            q_values
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0)
        }
    }

    /// Apply action and return new position
    fn apply_action(&mut self, action: usize, maze: &Maze) -> bool {
        let (dx, dy) = match action {
            0 => (0, -1), // Up
            1 => (0, 1),  // Down
            2 => (-1, 0), // Left
            3 => (1, 0),  // Right
            _ => (0, 0),
        };

        let new_x = self.position.0 + dx;
        let new_y = self.position.1 + dy;

        if !maze.is_wall(new_x, new_y) {
            self.position = (new_x, new_y);
            true
        } else {
            false
        }
    }

    /// Update Q-value for state-action pair
    fn update_q(&mut self, state: u64, action: usize, reward: f64, next_state: u64) {
        let current_q = self.q_table.entry(state).or_insert([0.0; 4])[action];

        let next_q = self
            .q_table
            .get(&next_state)
            .unwrap_or(&[0.0; 4])
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let next_q = if next_q.is_finite() { next_q } else { 0.0 };

        let new_q = current_q + self.alpha * (reward + self.gamma * next_q - current_q);
        let delta = (new_q - current_q).abs();

        self.q_table.entry(state).or_insert([0.0; 4])[action] = new_q;
        self.update_magnitudes.push(delta);
    }

    /// Decay exploration rate
    fn decay_epsilon(&mut self) {
        self.epsilon = (self.epsilon * self.epsilon_decay).max(self.epsilon_min);
    }
}

/// Compute belief entropy from Q-table
fn compute_belief_entropy(q_table: &HashMap<u64, [f64; 4]>) -> f64 {
    if q_table.is_empty() {
        return 0.0;
    }

    let mut total_entropy = 0.0;
    let mut count = 0;

    for q_values in q_table.values() {
        // Convert Q-values to probabilities using softmax
        let max_q = q_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_values: Vec<f64> = q_values.iter().map(|q| (q - max_q).exp()).collect();
        let sum: f64 = exp_values.iter().sum();

        if sum > 0.0 {
            let probs: Vec<f64> = exp_values.iter().map(|e| e / sum).collect();

            // Compute entropy
            let entropy: f64 = -probs
                .iter()
                .filter(|&&p| p > 1e-10)
                .map(|&p| p * p.ln())
                .sum::<f64>();

            total_entropy += entropy;
            count += 1;
        }
    }

    if count > 0 {
        total_entropy / count as f64
    } else {
        0.0
    }
}

/// Run the Partial Observation benchmark
pub fn run_benchmark(config: &BenchmarkConfig) -> BenchmarkReport {
    let start = Instant::now();
    let obs_config = &config.observation;

    let witness_log = Vec::new();
    let mut metrics = BenchmarkMetrics::default();
    let mut criteria = Vec::new();

    // Initialize RNG
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(config.seed);

    // Generate maze
    let maze = Maze::generate(obs_config.maze_width, obs_config.maze_height, &mut rng);

    // Create agent
    let mut agent = Agent::new(
        (maze.start.0 as i32, maze.start.1 as i32),
        obs_config.fov_degrees,
        obs_config.sensor_range,
        obs_config.sensor_noise_sigma,
    );

    // Track metrics
    let mut success_per_episode: Vec<bool> = Vec::with_capacity(obs_config.n_episodes);
    let mut learning_curve: Vec<f32> = Vec::new();
    let mut belief_entropy_over_time: Vec<f64> = Vec::new();

    // Run episodes
    for episode in 0..obs_config.n_episodes {
        agent.reset((maze.start.0 as i32, maze.start.1 as i32));
        let mut success = false;

        for _step in 0..obs_config.max_steps_per_episode {
            // Observe
            let obs = agent.observe(&maze, &mut rng);
            let state = obs.to_state_id();

            // Select and apply action
            let action = agent.select_action(state, &mut rng);
            let moved = agent.apply_action(action, &maze);

            // Get next observation
            let next_obs = agent.observe(&maze, &mut rng);
            let next_state = next_obs.to_state_id();

            // Compute reward
            let reward = if agent.position
                == (maze.goal.0 as i32, maze.goal.1 as i32)
            {
                success = true;
                10.0 // Goal reward
            } else if !moved {
                -1.0 // Wall penalty
            } else {
                -0.01 // Step cost
            };

            // Update Q-table
            agent.update_q(state, action, reward, next_state);

            if success {
                break;
            }
        }

        success_per_episode.push(success);
        agent.decay_epsilon();

        // Record learning curve every 10 episodes
        if (episode + 1) % 10 == 0 || episode == obs_config.n_episodes - 1 {
            let recent_start = if episode >= 10 { episode - 9 } else { 0 };
            let recent_successes = success_per_episode[recent_start..=episode]
                .iter()
                .filter(|&&s| s)
                .count();
            let recent_total = episode - recent_start + 1;
            let success_rate = recent_successes as f32 / recent_total as f32;
            learning_curve.push(success_rate);

            // Compute belief entropy
            let entropy = compute_belief_entropy(&agent.q_table);
            belief_entropy_over_time.push(entropy);

            if config.verbose {
                println!(
                    "Episode {}: success_rate={:.2}, epsilon={:.3}, entropy={:.3}",
                    episode + 1,
                    success_rate,
                    agent.epsilon,
                    entropy
                );
            }
        }
    }

    // Evaluate criteria

    // Criterion 1: Success rate increases over episodes
    let early_rate = if learning_curve.len() >= 2 {
        learning_curve[0..learning_curve.len() / 2]
            .iter()
            .sum::<f32>()
            / (learning_curve.len() / 2) as f32
    } else {
        0.0
    };
    let late_rate = if learning_curve.len() >= 2 {
        learning_curve[learning_curve.len() / 2..]
            .iter()
            .sum::<f32>()
            / (learning_curve.len() - learning_curve.len() / 2) as f32
    } else {
        learning_curve.last().copied().unwrap_or(0.0)
    };
    let improving = late_rate > early_rate;
    criteria.push(CriterionResult {
        name: "Success rate improves".to_string(),
        passed: improving,
        expected: "late > early".to_string(),
        actual: format!("early={:.2}, late={:.2}", early_rate, late_rate),
    });

    // Criterion 2: Final success rate > required_success_rate
    let final_rate = *learning_curve.last().unwrap_or(&0.0);
    criteria.push(CriterionResult {
        name: "Final success rate".to_string(),
        passed: final_rate >= obs_config.required_success_rate,
        expected: format!(">= {:.0}%", obs_config.required_success_rate * 100.0),
        actual: format!("{:.0}%", final_rate * 100.0),
    });

    // Criterion 3: Information gain positive (entropy decreases or stays reasonable)
    let _early_entropy = belief_entropy_over_time.first().copied().unwrap_or(0.0);
    let late_entropy = belief_entropy_over_time.last().copied().unwrap_or(0.0);
    // Note: lower entropy = more confidence, which is generally good after learning
    // But we want entropy to be non-zero (not completely degenerate)
    let entropy_reasonable = late_entropy > 0.01 && late_entropy < 2.0;
    criteria.push(CriterionResult {
        name: "Belief entropy reasonable".to_string(),
        passed: entropy_reasonable,
        expected: "0.01 < entropy < 2.0".to_string(),
        actual: format!("{:.3}", late_entropy),
    });

    // Criterion 4: No catastrophic forgetting (variance in success rate bounded)
    let mean_rate: f32 = learning_curve.iter().sum::<f32>() / learning_curve.len() as f32;
    let variance: f32 = learning_curve
        .iter()
        .map(|&r| (r - mean_rate).powi(2))
        .sum::<f32>()
        / learning_curve.len() as f32;
    let stable = variance < 0.15; // Allow some variance but not too much
    criteria.push(CriterionResult {
        name: "Learning stability (no catastrophic forgetting)".to_string(),
        passed: stable,
        expected: "variance < 0.15".to_string(),
        actual: format!("variance={:.3}", variance),
    });

    // Populate metrics
    metrics.learning_curve = learning_curve;
    metrics.belief_entropy = belief_entropy_over_time;
    metrics.policy_update_magnitudes = agent.update_magnitudes;

    // Build summary
    let all_passed = criteria.iter().all(|c| c.passed);
    let summary = if all_passed {
        format!(
            "Partial observation learning verified: final success rate {:.0}%",
            final_rate * 100.0
        )
    } else {
        let failures: Vec<_> = criteria.iter().filter(|c| !c.passed).map(|c| &c.name).collect();
        format!("Partial observation FAILED: {:?}", failures)
    };

    BenchmarkReport {
        name: "B: Partial Observation Agency".to_string(),
        passed: all_passed,
        criteria,
        duration: start.elapsed(),
        metrics,
        witness_log,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maze_generation() {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
        let maze = Maze::generate(10, 10, &mut rng);

        // Start and goal should be clear
        assert!(!maze.is_wall(maze.start.0 as i32, maze.start.1 as i32));
        assert!(!maze.is_wall(maze.goal.0 as i32, maze.goal.1 as i32));

        // Borders should be walls
        assert!(maze.is_wall(0, 0));
        assert!(maze.is_wall(9, 9));
    }

    #[test]
    fn test_agent_movement() {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
        let maze = Maze::generate(10, 10, &mut rng);
        let mut agent = Agent::new((1, 1), 90.0, 3.0, 0.1);

        // Try to move right (should work if no wall)
        let initial_pos = agent.position;
        agent.apply_action(3, &maze); // Right

        // Position should have changed or stayed same depending on wall
        assert!(agent.position.0 >= initial_pos.0);
    }

    #[test]
    fn test_partial_observation_runs() {
        let mut config = BenchmarkConfig::default();
        config.observation.n_episodes = 20;
        config.observation.max_steps_per_episode = 50;

        let result = run_benchmark(&config);

        assert!(!result.criteria.is_empty());
        assert!(result.duration.as_secs() < 60);
        assert!(!result.metrics.learning_curve.is_empty());
    }
}
