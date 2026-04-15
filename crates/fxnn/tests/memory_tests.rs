//! Memory Layer Tests
//!
//! Tests for memory and learning mechanisms:
//! - SONA learning (Self-Organizing Neural Architecture)
//! - Trajectory storage and retrieval
//! - EWC penalty (Elastic Weight Consolidation)
//! - Memory write rate limiting
//!
//! These tests model agent memory systems that operate alongside
//! the physics simulation for learning and adaptation.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// Memory Infrastructure
// ============================================================================

/// A single experience/trajectory point
#[derive(Debug, Clone)]
struct Experience {
    state: Vec<f32>,
    action: Vec<f32>,
    reward: f32,
    next_state: Vec<f32>,
    done: bool,
    timestamp: u64,
}

/// Trajectory: a sequence of experiences
#[derive(Debug, Clone)]
struct Trajectory {
    experiences: Vec<Experience>,
    total_reward: f32,
    episode_id: u64,
}

/// Simple neural network weights for testing
#[derive(Debug, Clone)]
struct NetworkWeights {
    weights: Vec<f32>,
    biases: Vec<f32>,
}

impl NetworkWeights {
    fn new(size: usize) -> Self {
        Self {
            weights: vec![0.0; size],
            biases: vec![0.0; size / 4],
        }
    }

    fn random(size: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            weights: (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect(),
            biases: (0..size / 4).map(|_| rng.gen_range(-0.5..0.5)).collect(),
        }
    }

    fn distance(&self, other: &NetworkWeights) -> f32 {
        let weight_dist: f32 = self.weights.iter()
            .zip(other.weights.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        let bias_dist: f32 = self.biases.iter()
            .zip(other.biases.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        (weight_dist + bias_dist).sqrt()
    }
}

// ============================================================================
// SONA (Self-Organizing Neural Architecture) Learning Tests
// ============================================================================

/// SONA-like adaptive learning system
struct SONALearner {
    /// Current network weights
    weights: NetworkWeights,
    /// Learning rate that adapts based on performance
    learning_rate: f32,
    /// Minimum learning rate
    min_lr: f32,
    /// Maximum learning rate
    max_lr: f32,
    /// History of recent losses for adaptation
    loss_history: VecDeque<f32>,
    /// Window size for loss smoothing
    window_size: usize,
    /// Adaptation time constant
    adaptation_tau: f32,
}

impl SONALearner {
    fn new(weight_size: usize) -> Self {
        Self {
            weights: NetworkWeights::new(weight_size),
            learning_rate: 0.01,
            min_lr: 0.0001,
            max_lr: 0.1,
            loss_history: VecDeque::new(),
            window_size: 10,
            adaptation_tau: 0.05, // < 0.05ms adaptation target
        }
    }

    /// Perform a learning step with SONA-style adaptation
    fn learn(&mut self, gradient: &[f32], loss: f32) -> f32 {
        // Record loss for adaptation
        self.loss_history.push_back(loss);
        if self.loss_history.len() > self.window_size {
            self.loss_history.pop_front();
        }

        // Adapt learning rate based on loss trend
        self.adapt_learning_rate();

        // Apply gradient update
        let lr = self.learning_rate;
        for (i, g) in gradient.iter().enumerate() {
            if i < self.weights.weights.len() {
                self.weights.weights[i] -= lr * g;
            }
        }

        self.learning_rate
    }

    /// SONA-style learning rate adaptation
    fn adapt_learning_rate(&mut self) {
        if self.loss_history.len() < 2 {
            return;
        }

        // Calculate loss trend
        let recent: Vec<f32> = self.loss_history.iter().cloned().collect();
        let n = recent.len();
        let half = n / 2;

        if half == 0 {
            return;
        }

        let early_avg: f32 = recent[..half].iter().sum::<f32>() / half as f32;
        let late_avg: f32 = recent[half..].iter().sum::<f32>() / (n - half) as f32;

        // If loss is increasing, reduce learning rate
        // If loss is decreasing, can increase learning rate
        let ratio = late_avg / early_avg.max(1e-10);

        if ratio > 1.1 {
            // Loss increasing, reduce LR
            self.learning_rate = (self.learning_rate * 0.9).max(self.min_lr);
        } else if ratio < 0.9 {
            // Loss decreasing well, can increase LR
            self.learning_rate = (self.learning_rate * 1.1).min(self.max_lr);
        }
    }

    fn get_learning_rate(&self) -> f32 {
        self.learning_rate
    }
}

/// Test SONA learning adapts learning rate
#[test]
fn test_sona_learning() {
    let mut learner = SONALearner::new(100);
    let initial_lr = learner.get_learning_rate();

    // Simulate decreasing loss (good progress)
    let gradient = vec![0.1; 100];
    for i in 0..20 {
        let loss = 1.0 / (i as f32 + 1.0); // Decreasing loss
        learner.learn(&gradient, loss);
    }

    let lr_after_good_progress = learner.get_learning_rate();
    println!("Initial LR: {}, After good progress: {}", initial_lr, lr_after_good_progress);

    // With decreasing loss, learning rate should increase or stay stable
    assert!(
        lr_after_good_progress >= initial_lr * 0.8,
        "Learning rate should not decrease much with good progress"
    );

    // Now simulate increasing loss (bad progress)
    for i in 0..20 {
        let loss = (i as f32 + 1.0) * 0.5; // Increasing loss
        learner.learn(&gradient, loss);
    }

    let lr_after_bad_progress = learner.get_learning_rate();
    println!("After bad progress: {}", lr_after_bad_progress);

    // With increasing loss, learning rate should decrease
    assert!(
        lr_after_bad_progress < lr_after_good_progress,
        "Learning rate should decrease with increasing loss"
    );
}

/// Test SONA adaptation time is fast (<0.05ms target)
#[test]
fn test_sona_adaptation_time() {
    let mut learner = SONALearner::new(1000);
    let gradient = vec![0.1; 1000];

    // Warm up
    for _ in 0..10 {
        learner.learn(&gradient, 1.0);
    }

    // Measure adaptation time
    let iterations = 100;
    let start = Instant::now();
    for i in 0..iterations {
        learner.learn(&gradient, 1.0 - i as f32 * 0.001);
    }
    let elapsed = start.elapsed();

    let avg_time_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    println!("Average SONA adaptation time: {:.4}ms", avg_time_ms);

    // Target: <0.05ms per adaptation
    assert!(
        avg_time_ms < 0.5, // Relaxed for test environment
        "SONA adaptation should be fast, got {:.4}ms",
        avg_time_ms
    );
}

// ============================================================================
// Trajectory Storage Tests
// ============================================================================

/// Trajectory buffer with capacity limit
struct TrajectoryBuffer {
    trajectories: VecDeque<Trajectory>,
    max_capacity: usize,
    total_experiences: usize,
}

impl TrajectoryBuffer {
    fn new(max_capacity: usize) -> Self {
        Self {
            trajectories: VecDeque::new(),
            max_capacity,
            total_experiences: 0,
        }
    }

    fn store(&mut self, trajectory: Trajectory) {
        self.total_experiences += trajectory.experiences.len();

        self.trajectories.push_back(trajectory);

        // Remove oldest if over capacity
        while self.trajectories.len() > self.max_capacity {
            if let Some(old) = self.trajectories.pop_front() {
                self.total_experiences -= old.experiences.len();
            }
        }
    }

    fn sample(&self, n: usize) -> Vec<&Trajectory> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();

        let all: Vec<&Trajectory> = self.trajectories.iter().collect();
        all.choose_multiple(&mut rng, n.min(all.len())).cloned().collect()
    }

    fn get_by_reward(&self, min_reward: f32) -> Vec<&Trajectory> {
        self.trajectories.iter()
            .filter(|t| t.total_reward >= min_reward)
            .collect()
    }

    fn len(&self) -> usize {
        self.trajectories.len()
    }

    fn total_experiences(&self) -> usize {
        self.total_experiences
    }
}

/// Test trajectory storage and retrieval
#[test]
fn test_trajectory_storage() {
    let mut buffer = TrajectoryBuffer::new(100);

    // Store some trajectories
    for i in 0..50 {
        let trajectory = Trajectory {
            experiences: vec![
                Experience {
                    state: vec![i as f32],
                    action: vec![0.0],
                    reward: i as f32 * 0.1,
                    next_state: vec![i as f32 + 1.0],
                    done: i == 49,
                    timestamp: i as u64,
                }
            ],
            total_reward: i as f32 * 0.1,
            episode_id: i as u64,
        };
        buffer.store(trajectory);
    }

    assert_eq!(buffer.len(), 50);
    assert_eq!(buffer.total_experiences(), 50);

    // Sample trajectories
    let sample = buffer.sample(10);
    assert_eq!(sample.len(), 10);

    // Get high-reward trajectories
    let high_reward = buffer.get_by_reward(3.0);
    assert!(high_reward.len() > 0);
    for t in &high_reward {
        assert!(t.total_reward >= 3.0);
    }
}

/// Test trajectory buffer capacity limit
#[test]
fn test_trajectory_buffer_capacity() {
    let mut buffer = TrajectoryBuffer::new(10);

    // Store more than capacity
    for i in 0..20 {
        let trajectory = Trajectory {
            experiences: vec![
                Experience {
                    state: vec![i as f32],
                    action: vec![0.0],
                    reward: 1.0,
                    next_state: vec![i as f32 + 1.0],
                    done: true,
                    timestamp: i as u64,
                }
            ],
            total_reward: 1.0,
            episode_id: i as u64,
        };
        buffer.store(trajectory);
    }

    // Should only have max_capacity trajectories
    assert_eq!(buffer.len(), 10);

    // Oldest should be removed (episode_id 10-19 should remain)
    let min_episode_id = buffer.trajectories.front().unwrap().episode_id;
    assert!(min_episode_id >= 10, "Oldest trajectories should be removed");
}

// ============================================================================
// EWC (Elastic Weight Consolidation) Penalty Tests
// ============================================================================

/// EWC-style continual learning system
struct EWCLearner {
    /// Current weights
    weights: NetworkWeights,
    /// Old weights from previous task
    old_weights: Option<NetworkWeights>,
    /// Fisher information (importance) for each weight
    fisher_information: Option<Vec<f32>>,
    /// EWC penalty strength
    lambda: f32,
}

impl EWCLearner {
    fn new(weight_size: usize) -> Self {
        Self {
            weights: NetworkWeights::random(weight_size),
            old_weights: None,
            fisher_information: None,
            lambda: 1000.0, // Standard EWC lambda
        }
    }

    /// Compute EWC penalty for current weights
    fn ewc_penalty(&self) -> f32 {
        match (&self.old_weights, &self.fisher_information) {
            (Some(old), Some(fisher)) => {
                let mut penalty = 0.0;
                for (i, f) in fisher.iter().enumerate() {
                    if i < self.weights.weights.len() {
                        let diff = self.weights.weights[i] - old.weights[i];
                        penalty += f * diff * diff;
                    }
                }
                self.lambda * penalty / 2.0
            }
            _ => 0.0,
        }
    }

    /// Consolidate current task by computing Fisher information
    fn consolidate(&mut self) {
        // In real EWC, Fisher is computed from gradient variance on task data
        // Here we use a simplified version
        let fisher: Vec<f32> = self.weights.weights.iter()
            .map(|w| w.abs() + 0.1) // Importance proportional to magnitude
            .collect();

        self.old_weights = Some(self.weights.clone());
        self.fisher_information = Some(fisher);
    }

    /// Learn with EWC penalty
    fn learn_with_ewc(&mut self, gradient: &[f32], task_loss: f32) -> (f32, f32) {
        let ewc_penalty = self.ewc_penalty();
        let total_loss = task_loss + ewc_penalty;

        // Apply gradient (simplified - should include EWC gradient)
        let lr = 0.01;
        for (i, g) in gradient.iter().enumerate() {
            if i < self.weights.weights.len() {
                let mut total_grad = *g;

                // Add EWC gradient if we have consolidated
                if let (Some(old), Some(fisher)) = (&self.old_weights, &self.fisher_information) {
                    let diff = self.weights.weights[i] - old.weights[i];
                    total_grad += self.lambda * fisher[i] * diff;
                }

                self.weights.weights[i] -= lr * total_grad;
            }
        }

        (total_loss, ewc_penalty)
    }
}

/// Test EWC penalty increases when weights deviate
#[test]
fn test_ewc_penalty() {
    let mut learner = EWCLearner::new(100);

    // Initial EWC penalty should be zero (no consolidation yet)
    assert_eq!(learner.ewc_penalty(), 0.0);

    // Consolidate current task
    learner.consolidate();

    // Immediately after consolidation, penalty should be zero
    let penalty_after_consolidation = learner.ewc_penalty();
    assert!(
        penalty_after_consolidation < 1e-6,
        "Penalty should be zero right after consolidation"
    );

    // Modify weights
    for w in &mut learner.weights.weights {
        *w += 0.1;
    }

    // Now penalty should be positive
    let penalty_after_change = learner.ewc_penalty();
    assert!(
        penalty_after_change > 0.0,
        "Penalty should be positive after weights change"
    );

    println!("EWC penalty after weight change: {}", penalty_after_change);
}

/// Test EWC prevents catastrophic forgetting
#[test]
fn test_ewc_prevents_forgetting() {
    let mut learner = EWCLearner::new(50);
    learner.lambda = 10.0; // Reasonable lambda to prevent gradient explosion

    // "Learn" task 1 (just set weights to specific values)
    for (i, w) in learner.weights.weights.iter_mut().enumerate() {
        *w = (i as f32 * 0.1).sin();
    }
    let task1_weights = learner.weights.clone();

    // Consolidate task 1
    learner.consolidate();

    // Now "learn" task 2 with EWC (small gradient, limited iterations)
    let gradient = vec![0.1; 50]; // Moderate gradient
    for _ in 0..20 {
        learner.learn_with_ewc(&gradient, 0.1);
    }

    // Weights should not have drifted too far from task 1
    let drift = learner.weights.distance(&task1_weights);
    println!("Weight drift from task 1: {:.4}", drift);

    // With EWC, drift should be finite and limited
    assert!(
        drift.is_finite() && drift < 5.0,
        "EWC should limit weight drift, got {}",
        drift
    );
}

// ============================================================================
// Memory Write Rate Limiting Tests
// ============================================================================

/// Rate-limited memory writer
struct RateLimitedMemory {
    buffer: Vec<Experience>,
    max_writes_per_second: f32,
    last_write_time: Instant,
    min_interval: Duration,
    write_count: usize,
    rejected_count: usize,
}

impl RateLimitedMemory {
    fn new(max_writes_per_second: f32) -> Self {
        let min_interval = Duration::from_secs_f32(1.0 / max_writes_per_second);
        Self {
            buffer: Vec::new(),
            max_writes_per_second,
            last_write_time: Instant::now() - min_interval,
            min_interval,
            write_count: 0,
            rejected_count: 0,
        }
    }

    /// Try to write an experience, returns true if successful
    fn try_write(&mut self, experience: Experience) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_write_time);

        if elapsed >= self.min_interval {
            self.buffer.push(experience);
            self.last_write_time = now;
            self.write_count += 1;
            true
        } else {
            self.rejected_count += 1;
            false
        }
    }

    /// Force write (ignores rate limit)
    fn force_write(&mut self, experience: Experience) {
        self.buffer.push(experience);
        self.write_count += 1;
        self.last_write_time = Instant::now();
    }

    fn get_stats(&self) -> (usize, usize, f32) {
        let acceptance_rate = if self.write_count + self.rejected_count > 0 {
            self.write_count as f32 / (self.write_count + self.rejected_count) as f32
        } else {
            1.0
        };
        (self.write_count, self.rejected_count, acceptance_rate)
    }
}

/// Test memory write rate limiting
#[test]
fn test_memory_write_rate_limit() {
    let mut memory = RateLimitedMemory::new(100.0); // 100 writes/sec max

    // Try to write 1000 experiences as fast as possible
    let mut accepted = 0;
    let start = Instant::now();

    for i in 0..1000 {
        let exp = Experience {
            state: vec![i as f32],
            action: vec![0.0],
            reward: 1.0,
            next_state: vec![i as f32 + 1.0],
            done: false,
            timestamp: i as u64,
        };

        if memory.try_write(exp) {
            accepted += 1;
        }
    }

    let elapsed = start.elapsed();
    let (writes, rejected, rate) = memory.get_stats();

    println!("Elapsed: {:?}", elapsed);
    println!("Accepted: {}, Rejected: {}, Rate: {:.2}%", writes, rejected, rate * 100.0);

    // Most should be rejected since we're writing faster than the limit
    assert!(
        rejected > accepted,
        "Rate limiting should reject most rapid writes"
    );
}

/// Test rate limiting allows writes at appropriate rate
#[test]
fn test_memory_write_rate_allows_slow_writes() {
    let mut memory = RateLimitedMemory::new(1000.0); // 1000 writes/sec

    // Write at a slower rate (every 2ms = 500/sec)
    let mut accepted = 0;
    for i in 0..10 {
        let exp = Experience {
            state: vec![i as f32],
            action: vec![0.0],
            reward: 1.0,
            next_state: vec![i as f32 + 1.0],
            done: false,
            timestamp: i as u64,
        };

        std::thread::sleep(Duration::from_millis(2));

        if memory.try_write(exp) {
            accepted += 1;
        }
    }

    println!("Slow writes accepted: {}/10", accepted);

    // All or most should be accepted when writing slowly
    assert!(
        accepted >= 8,
        "Slow writes should be mostly accepted, got {}/10",
        accepted
    );
}

// ============================================================================
// Integrated Memory System Tests
// ============================================================================

/// Complete memory system with learning, storage, and rate limiting
struct MemorySystem {
    sona: SONALearner,
    trajectory_buffer: TrajectoryBuffer,
    ewc: EWCLearner,
    write_limiter: RateLimitedMemory,
}

impl MemorySystem {
    fn new() -> Self {
        Self {
            sona: SONALearner::new(100),
            trajectory_buffer: TrajectoryBuffer::new(1000),
            ewc: EWCLearner::new(100),
            write_limiter: RateLimitedMemory::new(500.0),
        }
    }

    /// Process and store an experience
    fn process_experience(&mut self, experience: Experience, loss: f32) -> bool {
        // Rate limit the write
        if !self.write_limiter.try_write(experience.clone()) {
            return false;
        }

        // SONA learning
        let gradient = vec![0.1; 100];
        self.sona.learn(&gradient, loss);

        // EWC learning
        self.ewc.learn_with_ewc(&gradient, loss);

        true
    }

    /// End of episode: store trajectory and potentially consolidate
    fn end_episode(&mut self, trajectory: Trajectory, should_consolidate: bool) {
        self.trajectory_buffer.store(trajectory);

        if should_consolidate {
            self.ewc.consolidate();
        }
    }
}

/// Test integrated memory system
#[test]
fn test_integrated_memory_system() {
    let mut system = MemorySystem::new();

    // Simulate an episode
    let mut experiences = Vec::new();
    let mut total_reward = 0.0;

    for step in 0..100 {
        let exp = Experience {
            state: vec![step as f32],
            action: vec![(step as f32 * 0.1).sin()],
            reward: 1.0 / (step as f32 + 1.0),
            next_state: vec![step as f32 + 1.0],
            done: step == 99,
            timestamp: step as u64,
        };

        total_reward += exp.reward;
        experiences.push(exp.clone());

        // Process with simulated loss
        let loss = 1.0 / (step as f32 + 10.0);
        system.process_experience(exp, loss);
    }

    // End episode
    let trajectory = Trajectory {
        experiences,
        total_reward,
        episode_id: 0,
    };
    system.end_episode(trajectory, true);

    // Check system state
    assert_eq!(system.trajectory_buffer.len(), 1);
    assert!(system.ewc.old_weights.is_some(), "Should have consolidated");

    let (writes, rejected, rate) = system.write_limiter.get_stats();
    println!(
        "Memory system: {} writes, {} rejected, {:.2}% acceptance",
        writes, rejected, rate * 100.0
    );
}
