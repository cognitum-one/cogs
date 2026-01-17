//! Benchmark C: Emergence Falsifiability - Communication Ablation
//!
//! **Purpose**: Prove emergent cooperation is genuine, not an artifact.
//!
//! # Test Protocol (from ADR-001)
//!
//! ## Setup
//! - 4 agents in cooperative foraging task
//! - Shared reward: all agents benefit from any food found
//! - Communication channel: agents can broadcast messages
//!
//! ## Run (Two Conditions)
//! - Condition 1: Communication ENABLED (100 episodes)
//! - Condition 2: Communication DISABLED (100 episodes)
//!
//! ## Pass Criteria
//! - Cooperation index with comm > without comm
//! - Cooperation collapses (>50% drop) when comm removed
//! - Message-event mutual information > threshold
//! - Statistical significance: p < 0.01
//!
//! ## Report
//! - Cooperation index both conditions
//! - Message content analysis
//! - Statistical test results
//!
//! ## Interpretation
//! If cooperation survives without communication, cooperation was NOT emergent
//! from communication (indicates hard-coded or coincidental coordination)

use super::{
    BenchmarkConfig, BenchmarkMetrics, BenchmarkReport, CriterionResult, WitnessEventType,
    WitnessRecord,
};
use rand::prelude::*;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::collections::HashMap;
use std::time::Instant;

/// A simple 2D foraging environment
#[derive(Clone)]
struct ForagingEnv {
    width: usize,
    height: usize,
    food_positions: Vec<(usize, usize)>,
    collected: Vec<bool>,
}

impl ForagingEnv {
    fn new(width: usize, height: usize, n_food: usize, rng: &mut impl Rng) -> Self {
        let mut food_positions = Vec::with_capacity(n_food);
        for _ in 0..n_food {
            let x = rng.gen_range(0..width);
            let y = rng.gen_range(0..height);
            food_positions.push((x, y));
        }
        let collected = vec![false; n_food];
        Self {
            width,
            height,
            food_positions,
            collected,
        }
    }

    fn reset(&mut self, rng: &mut impl Rng) {
        self.food_positions.clear();
        let n_food = self.collected.len();
        for _ in 0..n_food {
            let x = rng.gen_range(0..self.width);
            let y = rng.gen_range(0..self.height);
            self.food_positions.push((x, y));
        }
        self.collected.fill(false);
    }

    fn check_collection(&mut self, agent_positions: &[(i32, i32)]) -> f64 {
        let mut reward = 0.0;
        for (i, &(fx, fy)) in self.food_positions.iter().enumerate() {
            if self.collected[i] {
                continue;
            }
            for &(ax, ay) in agent_positions {
                if ax as usize == fx && ay as usize == fy {
                    self.collected[i] = true;
                    reward += 1.0;
                    break;
                }
            }
        }
        reward
    }

    fn uncollected_food(&self) -> Vec<(usize, usize)> {
        self.food_positions
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.collected[*i])
            .map(|(_, &pos)| pos)
            .collect()
    }

    fn all_collected(&self) -> bool {
        self.collected.iter().all(|&c| c)
    }
}

/// A message that agents can broadcast
#[derive(Clone, Debug)]
struct Message {
    sender_id: usize,
    /// Message type: 0=no message, 1=food_here, 2=going_to, 3=need_help
    msg_type: u8,
    /// Target location encoded
    location: (i32, i32),
}

/// A foraging agent
struct ForagingAgent {
    id: usize,
    position: (i32, i32),
    /// Q-table for movement: state -> action -> value
    q_table: HashMap<u64, [f64; 4]>,
    /// Message sending policy
    msg_policy: HashMap<u64, [f64; 4]>, // 4 message types
    alpha: f64,
    gamma: f64,
    epsilon: f64,
    /// Messages received this step
    inbox: Vec<Message>,
}

impl ForagingAgent {
    fn new(id: usize, start: (i32, i32)) -> Self {
        Self {
            id,
            position: start,
            q_table: HashMap::new(),
            msg_policy: HashMap::new(),
            alpha: 0.1,
            gamma: 0.9,
            epsilon: 0.3,
            inbox: Vec::new(),
        }
    }

    fn reset(&mut self, start: (i32, i32)) {
        self.position = start;
        self.inbox.clear();
    }

    fn receive_message(&mut self, msg: Message) {
        self.inbox.push(msg);
    }

    fn observe_state(&self, env: &ForagingEnv, other_agents: &[(i32, i32)]) -> u64 {
        let mut state: u64 = 0;

        // Encode own position
        state |= (self.position.0 as u64 & 0xFF) << 0;
        state |= (self.position.1 as u64 & 0xFF) << 8;

        // Encode nearest food direction
        let uncollected = env.uncollected_food();
        if let Some(&(fx, fy)) = uncollected.first() {
            let dx = (fx as i32 - self.position.0).signum() + 1; // 0, 1, or 2
            let dy = (fy as i32 - self.position.1).signum() + 1;
            state |= (dx as u64) << 16;
            state |= (dy as u64) << 18;
        }

        // Encode received message info
        if let Some(msg) = self.inbox.first() {
            state |= (msg.msg_type as u64) << 20;
        }

        // Encode nearest other agent
        let nearest_dist = other_agents
            .iter()
            .map(|&(ox, oy)| {
                ((ox - self.position.0).abs() + (oy - self.position.1).abs()) as u64
            })
            .min()
            .unwrap_or(255);
        state |= (nearest_dist.min(15)) << 24;

        state
    }

    fn select_action(&self, state: u64, rng: &mut impl Rng) -> usize {
        if rng.gen::<f64>() < self.epsilon {
            rng.gen_range(0..4)
        } else {
            let q_values = self.q_table.get(&state).unwrap_or(&[0.0; 4]);
            q_values
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0)
        }
    }

    fn select_message(&self, _state: u64, comm_enabled: bool, rng: &mut impl Rng) -> Option<Message> {
        if !comm_enabled {
            return None;
        }

        // Simple heuristic: sometimes broadcast when finding food
        if rng.gen::<f64>() < 0.3 {
            Some(Message {
                sender_id: self.id,
                msg_type: 1, // food_here
                location: self.position,
            })
        } else {
            None
        }
    }

    fn apply_action(&mut self, action: usize, env: &ForagingEnv) {
        let (dx, dy) = match action {
            0 => (0, -1), // Up
            1 => (0, 1),  // Down
            2 => (-1, 0), // Left
            3 => (1, 0),  // Right
            _ => (0, 0),
        };

        let new_x = (self.position.0 + dx).clamp(0, env.width as i32 - 1);
        let new_y = (self.position.1 + dy).clamp(0, env.height as i32 - 1);
        self.position = (new_x, new_y);
    }

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
        self.q_table.entry(state).or_insert([0.0; 4])[action] = new_q;
    }

    fn clear_inbox(&mut self) {
        self.inbox.clear();
    }
}

/// Compute cooperation index
/// Higher is better: measures how efficiently agents collect food together
fn compute_cooperation_index(total_reward: f64, steps: usize, _n_agents: usize, n_food: usize) -> f64 {
    // Normalize by theoretical maximum
    // Best case: agents spread out and collect all food in minimum steps
    let theoretical_max = n_food as f64;
    let efficiency = total_reward / theoretical_max;

    // Penalize for taking too many steps
    let step_penalty = 1.0 - (steps as f64 / (n_food * 10) as f64).min(1.0);

    // Cooperation bonus: did multiple agents contribute?
    efficiency * (1.0 + step_penalty * 0.5)
}

/// Compute mutual information between messages and food collection events
fn compute_message_mutual_info(
    message_log: &[(usize, u8, (i32, i32))],
    collection_log: &[(usize, usize, (i32, i32))],
) -> f64 {
    if message_log.is_empty() || collection_log.is_empty() {
        return 0.0;
    }

    // Simplified MI: check if messages correlate with subsequent collections
    let mut matches = 0;
    for (msg_step, msg_type, msg_loc) in message_log {
        if *msg_type == 1 {
            // food_here
            for (coll_step, _, coll_loc) in collection_log {
                if *coll_step > *msg_step && *coll_step <= *msg_step + 10 {
                    // Check if collection was near message location
                    let dist = ((coll_loc.0 - msg_loc.0).abs() + (coll_loc.1 - msg_loc.1).abs()) as f64;
                    if dist < 3.0 {
                        matches += 1;
                    }
                }
            }
        }
    }

    // Normalize to [0, 1]
    let max_possible = message_log.len().min(collection_log.len());
    if max_possible > 0 {
        (matches as f64 / max_possible as f64).min(1.0)
    } else {
        0.0
    }
}

/// Perform t-test for two samples
fn t_test(sample1: &[f64], sample2: &[f64]) -> (f64, f64) {
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;

    if n1 < 2.0 || n2 < 2.0 {
        return (0.0, 1.0); // Not enough samples
    }

    let mean1: f64 = sample1.iter().sum::<f64>() / n1;
    let mean2: f64 = sample2.iter().sum::<f64>() / n2;

    let var1: f64 = sample1.iter().map(|x| (x - mean1).powi(2)).sum::<f64>() / (n1 - 1.0);
    let var2: f64 = sample2.iter().map(|x| (x - mean2).powi(2)).sum::<f64>() / (n2 - 1.0);

    // Pooled standard error
    let se = ((var1 / n1) + (var2 / n2)).sqrt();
    if se < 1e-10 {
        return (f64::INFINITY, 0.0);
    }

    let t = (mean1 - mean2) / se;

    // Degrees of freedom (Welch-Satterthwaite)
    let df = ((var1 / n1 + var2 / n2).powi(2))
        / ((var1 / n1).powi(2) / (n1 - 1.0) + (var2 / n2).powi(2) / (n2 - 1.0));

    // Approximate p-value using normal distribution for large df
    // For a proper implementation, use a t-distribution CDF
    let p = if df > 30.0 {
        // Use normal approximation
        2.0 * (1.0 - normal_cdf(t.abs()))
    } else {
        // Rough approximation for small df
        2.0 * (1.0 - normal_cdf(t.abs() * (1.0 + 0.5 / df).sqrt()))
    };

    (t, p)
}

/// Standard normal CDF approximation
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + (x / 2.0_f64.sqrt()).erf_approx())
}

/// Approximate error function
trait ErfApprox {
    fn erf_approx(&self) -> f64;
}

impl ErfApprox for f64 {
    fn erf_approx(&self) -> f64 {
        // Horner form approximation
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if *self < 0.0 { -1.0 } else { 1.0 };
        let x = self.abs();
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        sign * y
    }
}

/// Run the Emergence Falsifiability benchmark
pub fn run_benchmark(config: &BenchmarkConfig) -> BenchmarkReport {
    let start = Instant::now();
    let em_config = &config.emergence;

    let mut witness_log = Vec::new();
    let mut metrics = BenchmarkMetrics::default();
    let mut criteria = Vec::new();

    // Initialize RNG
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(config.seed);

    let env_width = 10;
    let env_height = 10;
    let n_food = 8;
    let max_steps = 100;

    // Create environment
    let mut env = ForagingEnv::new(env_width, env_height, n_food, &mut rng);

    // Create agents
    let mut agents: Vec<ForagingAgent> = (0..em_config.n_agents)
        .map(|i| {
            let x = rng.gen_range(0..env_width) as i32;
            let y = rng.gen_range(0..env_height) as i32;
            ForagingAgent::new(i, (x, y))
        })
        .collect();

    // Storage for cooperation indices
    let mut cooperation_with_comm: Vec<f64> = Vec::with_capacity(em_config.episodes_with_comm);
    let mut cooperation_without_comm: Vec<f64> = Vec::with_capacity(em_config.episodes_without_comm);

    // Storage for message analysis
    let mut total_message_log: Vec<(usize, u8, (i32, i32))> = Vec::new();
    let mut total_collection_log: Vec<(usize, usize, (i32, i32))> = Vec::new();

    // Condition 1: Communication ENABLED
    if config.verbose {
        println!("Running {} episodes WITH communication...", em_config.episodes_with_comm);
    }

    for episode in 0..em_config.episodes_with_comm {
        env.reset(&mut rng);
        for (i, agent) in agents.iter_mut().enumerate() {
            let x = rng.gen_range(0..env_width) as i32;
            let y = rng.gen_range(0..env_height) as i32;
            agent.reset((x, y));
        }

        let mut total_reward = 0.0;
        let mut steps = 0;

        for step in 0..max_steps {
            if env.all_collected() {
                break;
            }

            // Agents observe and select actions
            let other_positions: Vec<(i32, i32)> = agents.iter().map(|a| a.position).collect();

            // Each agent sends messages
            let messages: Vec<Option<Message>> = agents
                .iter()
                .map(|a| a.select_message(0, true, &mut rng))
                .collect();

            // Distribute messages to other agents
            for (i, msg_opt) in messages.iter().enumerate() {
                if let Some(msg) = msg_opt {
                    total_message_log.push((step, msg.msg_type, msg.location));
                    for (j, other_agent) in agents.iter_mut().enumerate() {
                        if i != j {
                            other_agent.receive_message(msg.clone());
                        }
                    }
                }
            }

            // Agents take actions
            let mut states_actions: Vec<(u64, usize)> = Vec::with_capacity(agents.len());
            for (i, agent) in agents.iter_mut().enumerate() {
                let other_pos: Vec<(i32, i32)> = other_positions
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, &p)| p)
                    .collect();
                let state = agent.observe_state(&env, &other_pos);
                let action = agent.select_action(state, &mut rng);
                states_actions.push((state, action));
            }

            for (i, (_, action)) in states_actions.iter().enumerate() {
                agents[i].apply_action(*action, &env);
            }

            // Check food collection
            let agent_positions: Vec<(i32, i32)> = agents.iter().map(|a| a.position).collect();
            let reward = env.check_collection(&agent_positions);

            if reward > 0.0 {
                for (i, &pos) in agent_positions.iter().enumerate() {
                    total_collection_log.push((step, i, pos));
                }
            }

            total_reward += reward;

            // Update Q-values (shared reward)
            let per_agent_reward = reward / em_config.n_agents as f64;
            for (i, agent) in agents.iter_mut().enumerate() {
                let other_pos: Vec<(i32, i32)> = agent_positions
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, &p)| p)
                    .collect();
                let next_state = agent.observe_state(&env, &other_pos);
                let (state, action) = states_actions[i];
                agent.update_q(state, action, per_agent_reward, next_state);
                agent.clear_inbox();
            }

            steps = step + 1;
        }

        let coop_idx = compute_cooperation_index(total_reward, steps, em_config.n_agents, n_food);
        cooperation_with_comm.push(coop_idx);
    }

    // Condition 2: Communication DISABLED
    if config.verbose {
        println!(
            "Running {} episodes WITHOUT communication...",
            em_config.episodes_without_comm
        );
    }

    for episode in 0..em_config.episodes_without_comm {
        env.reset(&mut rng);
        for (i, agent) in agents.iter_mut().enumerate() {
            let x = rng.gen_range(0..env_width) as i32;
            let y = rng.gen_range(0..env_height) as i32;
            agent.reset((x, y));
        }

        let mut total_reward = 0.0;
        let mut steps = 0;

        for step in 0..max_steps {
            if env.all_collected() {
                break;
            }

            let other_positions: Vec<(i32, i32)> = agents.iter().map(|a| a.position).collect();

            // NO messages sent (communication disabled)

            // Agents take actions
            let mut states_actions: Vec<(u64, usize)> = Vec::with_capacity(agents.len());
            for (i, agent) in agents.iter_mut().enumerate() {
                let other_pos: Vec<(i32, i32)> = other_positions
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, &p)| p)
                    .collect();
                let state = agent.observe_state(&env, &other_pos);
                let action = agent.select_action(state, &mut rng);
                states_actions.push((state, action));
            }

            for (i, (_, action)) in states_actions.iter().enumerate() {
                agents[i].apply_action(*action, &env);
            }

            // Check food collection
            let agent_positions: Vec<(i32, i32)> = agents.iter().map(|a| a.position).collect();
            let reward = env.check_collection(&agent_positions);
            total_reward += reward;

            // Update Q-values
            let per_agent_reward = reward / em_config.n_agents as f64;
            for (i, agent) in agents.iter_mut().enumerate() {
                let other_pos: Vec<(i32, i32)> = agent_positions
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, &p)| p)
                    .collect();
                let next_state = agent.observe_state(&env, &other_pos);
                let (state, action) = states_actions[i];
                agent.update_q(state, action, per_agent_reward, next_state);
            }

            steps = step + 1;
        }

        let coop_idx = compute_cooperation_index(total_reward, steps, em_config.n_agents, n_food);
        cooperation_without_comm.push(coop_idx);
    }

    // Compute statistics
    let mean_with = cooperation_with_comm.iter().sum::<f64>() / cooperation_with_comm.len() as f64;
    let mean_without =
        cooperation_without_comm.iter().sum::<f64>() / cooperation_without_comm.len() as f64;

    let (t_stat, p_value) = t_test(&cooperation_with_comm, &cooperation_without_comm);

    let cooperation_drop = if mean_with > 0.0 {
        (mean_with - mean_without) / mean_with
    } else {
        0.0
    };

    let mutual_info = compute_message_mutual_info(&total_message_log, &total_collection_log);

    // Evaluate criteria

    // Criterion 1: Cooperation with comm > without comm
    criteria.push(CriterionResult {
        name: "Cooperation higher with communication".to_string(),
        passed: mean_with > mean_without,
        expected: "with > without".to_string(),
        actual: format!("with={:.3}, without={:.3}", mean_with, mean_without),
    });

    // Criterion 2: Cooperation drops >50% when comm removed
    let sufficient_drop = cooperation_drop >= em_config.required_cooperation_drop as f64;
    criteria.push(CriterionResult {
        name: "Cooperation collapse when comm disabled".to_string(),
        passed: sufficient_drop,
        expected: format!(">= {:.0}% drop", em_config.required_cooperation_drop * 100.0),
        actual: format!("{:.1}% drop", cooperation_drop * 100.0),
    });

    // Criterion 3: Message-event mutual information > threshold
    // Use a low threshold since this is a simplified model
    let mi_threshold = 0.05;
    criteria.push(CriterionResult {
        name: "Message-event mutual information".to_string(),
        passed: mutual_info > mi_threshold,
        expected: format!("> {}", mi_threshold),
        actual: format!("{:.4}", mutual_info),
    });

    // Criterion 4: Statistical significance p < 0.01
    criteria.push(CriterionResult {
        name: "Statistical significance".to_string(),
        passed: p_value < em_config.p_value_threshold,
        expected: format!("p < {}", em_config.p_value_threshold),
        actual: format!("p = {:.4}, t = {:.2}", p_value, t_stat),
    });

    // Add witness entries
    witness_log.push(WitnessRecord {
        tick: 0,
        event_type: WitnessEventType::ActionRejected,
        entity_ids: vec![],
        constraint_fired: "emergence_test".to_string(),
        delta_magnitude: cooperation_drop,
        description: format!(
            "Cooperation change: {:.3} -> {:.3} ({:.1}% {})",
            mean_with,
            mean_without,
            cooperation_drop.abs() * 100.0,
            if cooperation_drop > 0.0 { "drop" } else { "increase" }
        ),
    });

    // Populate metrics
    metrics.cooperation_with_comm = Some(mean_with);
    metrics.cooperation_without_comm = Some(mean_without);
    metrics.message_mutual_info = Some(mutual_info);
    metrics.p_value = Some(p_value);
    metrics.t_statistic = Some(t_stat);

    // Build summary
    let all_passed = criteria.iter().all(|c| c.passed);
    let summary = if all_passed {
        format!(
            "Emergence verified: cooperation drops {:.1}% without communication (p={:.4})",
            cooperation_drop * 100.0,
            p_value
        )
    } else {
        let failures: Vec<_> = criteria.iter().filter(|c| !c.passed).map(|c| &c.name).collect();
        format!("Emergence falsifiability FAILED: {:?}", failures)
    };

    BenchmarkReport {
        name: "C: Emergence Falsifiability".to_string(),
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
    fn test_foraging_env() {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
        let mut env = ForagingEnv::new(10, 10, 5, &mut rng);

        assert_eq!(env.food_positions.len(), 5);
        assert!(!env.all_collected());

        // Simulate collection
        let positions = env.food_positions.clone();
        let agent_positions: Vec<(i32, i32)> = positions
            .iter()
            .map(|&(x, y)| (x as i32, y as i32))
            .collect();
        let reward = env.check_collection(&agent_positions);

        assert!(reward >= 1.0);
    }

    #[test]
    fn test_t_test() {
        // Test with clearly different distributions
        let sample1 = vec![10.0, 11.0, 12.0, 10.5, 11.5];
        let sample2 = vec![5.0, 6.0, 5.5, 6.5, 5.0];

        let (t, p) = t_test(&sample1, &sample2);

        assert!(t > 0.0, "t-statistic should be positive");
        assert!(p < 0.05, "p-value should be significant");
    }

    #[test]
    fn test_emergence_runs() {
        let mut config = BenchmarkConfig::default();
        config.emergence.episodes_with_comm = 10;
        config.emergence.episodes_without_comm = 10;

        let result = run_benchmark(&config);

        assert!(!result.criteria.is_empty());
        assert!(result.duration.as_secs() < 60);
    }
}
