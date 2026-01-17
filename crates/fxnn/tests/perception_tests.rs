//! Perception Layer Tests
//!
//! Tests for perception mechanisms in agent-based modeling:
//! - Partial observability (occlusion)
//! - Bandwidth limiting (information rate constraints)
//! - Attention filtering (selective observation)
//! - Noise injection (sensor uncertainty)
//!
//! These tests model perception constraints that agents face
//! when observing their environment in a physics simulation.

use fxnn::{
    SimulationBox,
    types::Atom,
};
use rand::Rng;
use rand_distr::{Normal, Distribution};
use std::collections::HashSet;

// ============================================================================
// Perception Infrastructure
// ============================================================================

/// Raw sensor data before perception processing
#[derive(Debug, Clone)]
struct RawObservation {
    /// Positions of all atoms in sensing range
    neighbor_positions: Vec<[f32; 3]>,
    /// Velocities of all atoms in sensing range
    neighbor_velocities: Vec<[f32; 3]>,
    /// Self position
    self_position: [f32; 3],
    /// Self velocity
    self_velocity: [f32; 3],
    /// IDs of neighbors
    neighbor_ids: Vec<u32>,
}

/// Processed observation after perception pipeline
#[derive(Debug, Clone)]
struct ProcessedObservation {
    /// Visible neighbor positions (after occlusion)
    visible_positions: Vec<[f32; 3]>,
    /// Visible neighbor velocities
    visible_velocities: Vec<[f32; 3]>,
    /// Self position (potentially noisy)
    perceived_self_position: [f32; 3],
    /// Self velocity (potentially noisy)
    perceived_self_velocity: [f32; 3],
    /// Number of neighbors occluded
    occluded_count: usize,
    /// Attention weights for each visible neighbor
    attention_weights: Vec<f32>,
}

/// Perception system that processes raw observations
struct PerceptionSystem {
    /// Enable occlusion checking
    enable_occlusion: bool,
    /// Maximum number of items to perceive (bandwidth limit)
    bandwidth_limit: Option<usize>,
    /// Enable attention-based filtering
    enable_attention: bool,
    /// Attention radius (things further get less attention)
    attention_radius: f32,
    /// Enable noise injection
    enable_noise: bool,
    /// Position noise standard deviation
    position_noise_std: f32,
    /// Velocity noise standard deviation
    velocity_noise_std: f32,
}

impl PerceptionSystem {
    fn new() -> Self {
        Self {
            enable_occlusion: false,
            bandwidth_limit: None,
            enable_attention: false,
            attention_radius: 3.0,
            enable_noise: false,
            position_noise_std: 0.0,
            velocity_noise_std: 0.0,
        }
    }

    fn with_occlusion(mut self, enable: bool) -> Self {
        self.enable_occlusion = enable;
        self
    }

    fn with_bandwidth_limit(mut self, limit: usize) -> Self {
        self.bandwidth_limit = Some(limit);
        self
    }

    fn with_attention(mut self, enable: bool, radius: f32) -> Self {
        self.enable_attention = enable;
        self.attention_radius = radius;
        self
    }

    fn with_noise(mut self, position_std: f32, velocity_std: f32) -> Self {
        self.enable_noise = true;
        self.position_noise_std = position_std;
        self.velocity_noise_std = velocity_std;
        self
    }

    fn process(&self, raw: &RawObservation, box_: &SimulationBox) -> ProcessedObservation {
        let mut visible_positions = raw.neighbor_positions.clone();
        let mut visible_velocities = raw.neighbor_velocities.clone();
        let mut occluded_count = 0;

        // Step 1: Occlusion filtering
        if self.enable_occlusion {
            let (vis_pos, vis_vel, occ) = self.apply_occlusion(
                &raw.self_position,
                &raw.neighbor_positions,
                &raw.neighbor_velocities,
                box_,
            );
            visible_positions = vis_pos;
            visible_velocities = vis_vel;
            occluded_count = occ;
        }

        // Step 2: Bandwidth limiting (keep only N closest)
        if let Some(limit) = self.bandwidth_limit {
            if visible_positions.len() > limit {
                let (limited_pos, limited_vel) = self.apply_bandwidth_limit(
                    &raw.self_position,
                    &visible_positions,
                    &visible_velocities,
                    limit,
                    box_,
                );
                visible_positions = limited_pos;
                visible_velocities = limited_vel;
            }
        }

        // Step 3: Attention filtering (compute attention weights)
        let attention_weights = if self.enable_attention {
            self.compute_attention_weights(&raw.self_position, &visible_positions, box_)
        } else {
            vec![1.0; visible_positions.len()]
        };

        // Step 4: Noise injection
        let (perceived_self_position, perceived_self_velocity) = if self.enable_noise {
            self.add_noise(&raw.self_position, &raw.self_velocity)
        } else {
            (raw.self_position, raw.self_velocity)
        };

        ProcessedObservation {
            visible_positions,
            visible_velocities,
            perceived_self_position,
            perceived_self_velocity,
            occluded_count,
            attention_weights,
        }
    }

    /// Check if a neighbor is occluded by another neighbor
    fn is_occluded(
        &self,
        observer: &[f32; 3],
        target: &[f32; 3],
        blocker: &[f32; 3],
        box_: &SimulationBox,
        occlusion_radius: f32,
    ) -> bool {
        let [dx_t, dy_t, dz_t] = box_.displacement(observer, target);
        let dist_target = (dx_t*dx_t + dy_t*dy_t + dz_t*dz_t).sqrt();

        let [dx_b, dy_b, dz_b] = box_.displacement(observer, blocker);
        let dist_blocker = (dx_b*dx_b + dy_b*dy_b + dz_b*dz_b).sqrt();

        // Blocker must be between observer and target
        if dist_blocker >= dist_target || dist_blocker < 0.1 {
            return false;
        }

        // Check if blocker is roughly on the line to target
        // Using cross product magnitude / distance for approximate perpendicular distance
        let t = dist_blocker / dist_target;
        let closest_on_line = [
            dx_t * t,
            dy_t * t,
            dz_t * t,
        ];

        let perp_dist = (
            (dx_b - closest_on_line[0]).powi(2)
            + (dy_b - closest_on_line[1]).powi(2)
            + (dz_b - closest_on_line[2]).powi(2)
        ).sqrt();

        perp_dist < occlusion_radius
    }

    fn apply_occlusion(
        &self,
        observer: &[f32; 3],
        positions: &[[f32; 3]],
        velocities: &[[f32; 3]],
        box_: &SimulationBox,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, usize) {
        let occlusion_radius = 0.5; // How "thick" an occluding body is
        let mut visible_pos = Vec::new();
        let mut visible_vel = Vec::new();
        let mut occluded = 0;

        for (i, target) in positions.iter().enumerate() {
            let mut is_blocked = false;

            for (j, blocker) in positions.iter().enumerate() {
                if i == j {
                    continue;
                }

                if self.is_occluded(observer, target, blocker, box_, occlusion_radius) {
                    is_blocked = true;
                    break;
                }
            }

            if is_blocked {
                occluded += 1;
            } else {
                visible_pos.push(*target);
                visible_vel.push(velocities[i]);
            }
        }

        (visible_pos, visible_vel, occluded)
    }

    fn apply_bandwidth_limit(
        &self,
        observer: &[f32; 3],
        positions: &[[f32; 3]],
        velocities: &[[f32; 3]],
        limit: usize,
        box_: &SimulationBox,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
        // Sort by distance and keep closest
        let mut indexed: Vec<(usize, f32)> = positions
            .iter()
            .enumerate()
            .map(|(i, pos)| (i, box_.distance(observer, pos)))
            .collect();

        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let selected: Vec<usize> = indexed.iter().take(limit).map(|(i, _)| *i).collect();

        let limited_pos: Vec<[f32; 3]> = selected.iter().map(|&i| positions[i]).collect();
        let limited_vel: Vec<[f32; 3]> = selected.iter().map(|&i| velocities[i]).collect();

        (limited_pos, limited_vel)
    }

    fn compute_attention_weights(
        &self,
        observer: &[f32; 3],
        positions: &[[f32; 3]],
        box_: &SimulationBox,
    ) -> Vec<f32> {
        positions
            .iter()
            .map(|pos| {
                let dist = box_.distance(observer, pos);
                // Exponential decay attention
                (-dist / self.attention_radius).exp()
            })
            .collect()
    }

    fn add_noise(&self, position: &[f32; 3], velocity: &[f32; 3]) -> ([f32; 3], [f32; 3]) {
        let mut rng = rand::thread_rng();

        let pos_noise = Normal::new(0.0, self.position_noise_std as f64).unwrap();
        let vel_noise = Normal::new(0.0, self.velocity_noise_std as f64).unwrap();

        let noisy_pos = [
            position[0] + pos_noise.sample(&mut rng) as f32,
            position[1] + pos_noise.sample(&mut rng) as f32,
            position[2] + pos_noise.sample(&mut rng) as f32,
        ];

        let noisy_vel = [
            velocity[0] + vel_noise.sample(&mut rng) as f32,
            velocity[1] + vel_noise.sample(&mut rng) as f32,
            velocity[2] + vel_noise.sample(&mut rng) as f32,
        ];

        (noisy_pos, noisy_vel)
    }
}

// ============================================================================
// Partial Observability (Occlusion) Tests
// ============================================================================

/// Test that atoms behind other atoms are occluded
#[test]
fn test_partial_observability_occlusion() {
    let box_ = SimulationBox::cubic(20.0);

    // Observer at origin
    let observer = [5.0, 5.0, 5.0];

    // Blocker directly in front
    let blocker = [6.0, 5.0, 5.0];

    // Target behind blocker
    let target_behind = [8.0, 5.0, 5.0];

    // Target not behind blocker (to the side)
    let target_visible = [8.0, 8.0, 5.0];

    let raw = RawObservation {
        neighbor_positions: vec![blocker, target_behind, target_visible],
        neighbor_velocities: vec![[0.0; 3]; 3],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1, 2, 3],
    };

    let perception = PerceptionSystem::new().with_occlusion(true);
    let processed = perception.process(&raw, &box_);

    println!("Visible count: {}", processed.visible_positions.len());
    println!("Occluded count: {}", processed.occluded_count);

    // Should see blocker and target_visible, but not target_behind
    assert!(
        processed.occluded_count >= 1,
        "At least one target should be occluded"
    );
    assert!(
        processed.visible_positions.len() >= 2,
        "Should see at least blocker and visible target"
    );
}

/// Test occlusion with complex geometry
#[test]
fn test_occlusion_complex_geometry() {
    let box_ = SimulationBox::cubic(30.0);

    // Observer
    let observer = [5.0, 5.0, 5.0];

    // Ring of blockers around observer
    let mut neighbors = Vec::new();
    for angle in 0..8 {
        let theta = angle as f32 * std::f32::consts::PI / 4.0;
        neighbors.push([
            5.0 + 2.0 * theta.cos(),
            5.0 + 2.0 * theta.sin(),
            5.0,
        ]);
    }

    // Targets far away in each direction
    for angle in 0..8 {
        let theta = angle as f32 * std::f32::consts::PI / 4.0;
        neighbors.push([
            5.0 + 5.0 * theta.cos(),
            5.0 + 5.0 * theta.sin(),
            5.0,
        ]);
    }

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: vec![[0.0; 3]; neighbors.len()],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: (0..neighbors.len() as u32).collect(),
    };

    let perception = PerceptionSystem::new().with_occlusion(true);
    let processed = perception.process(&raw, &box_);

    println!(
        "Complex geometry: {} visible, {} occluded",
        processed.visible_positions.len(),
        processed.occluded_count
    );

    // Some far targets should be occluded by the ring of blockers
    assert!(
        processed.occluded_count > 0,
        "Some targets should be occluded by ring of blockers"
    );
}

// ============================================================================
// Bandwidth Limiting Tests
// ============================================================================

/// Test bandwidth limiting keeps only closest neighbors
#[test]
fn test_bandwidth_limiting() {
    let box_ = SimulationBox::cubic(20.0);

    let observer = [10.0, 10.0, 10.0];

    // Create neighbors at various distances
    let neighbors: Vec<[f32; 3]> = vec![
        [11.0, 10.0, 10.0], // dist = 1
        [12.0, 10.0, 10.0], // dist = 2
        [13.0, 10.0, 10.0], // dist = 3
        [14.0, 10.0, 10.0], // dist = 4
        [15.0, 10.0, 10.0], // dist = 5
    ];

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: vec![[0.0; 3]; 5],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1, 2, 3, 4, 5],
    };

    // Limit to 3 closest
    let perception = PerceptionSystem::new().with_bandwidth_limit(3);
    let processed = perception.process(&raw, &box_);

    assert_eq!(processed.visible_positions.len(), 3);

    // Check that the closest 3 are selected (distances 1, 2, 3)
    for pos in &processed.visible_positions {
        let dist = box_.distance(&observer, pos);
        assert!(
            dist <= 3.5,
            "Should only include closest neighbors, found dist={}",
            dist
        );
    }
}

/// Test bandwidth limit with all items within limit
#[test]
fn test_bandwidth_limit_not_applied_when_under_limit() {
    let box_ = SimulationBox::cubic(20.0);

    let observer = [10.0, 10.0, 10.0];
    let neighbors = vec![
        [11.0, 10.0, 10.0],
        [12.0, 10.0, 10.0],
    ];

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: vec![[0.0; 3]; 2],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1, 2],
    };

    // Limit to 5 (more than we have)
    let perception = PerceptionSystem::new().with_bandwidth_limit(5);
    let processed = perception.process(&raw, &box_);

    assert_eq!(processed.visible_positions.len(), 2);
}

// ============================================================================
// Attention Filtering Tests
// ============================================================================

/// Test attention weights decrease with distance
#[test]
fn test_attention_filtering() {
    let box_ = SimulationBox::cubic(20.0);

    let observer = [10.0, 10.0, 10.0];

    let neighbors = vec![
        [11.0, 10.0, 10.0], // dist = 1
        [13.0, 10.0, 10.0], // dist = 3
        [17.0, 10.0, 10.0], // dist = 7
    ];

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: vec![[0.0; 3]; 3],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1, 2, 3],
    };

    let perception = PerceptionSystem::new().with_attention(true, 3.0);
    let processed = perception.process(&raw, &box_);

    assert_eq!(processed.attention_weights.len(), 3);

    // Closer neighbors should have higher attention
    println!("Attention weights: {:?}", processed.attention_weights);

    assert!(
        processed.attention_weights[0] > processed.attention_weights[1],
        "Closer neighbor should have higher attention"
    );
    assert!(
        processed.attention_weights[1] > processed.attention_weights[2],
        "Middle neighbor should have higher attention than far"
    );
}

/// Test attention with different radii
#[test]
fn test_attention_radius_sensitivity() {
    let box_ = SimulationBox::cubic(20.0);

    let observer = [10.0, 10.0, 10.0];
    let neighbor = [13.0, 10.0, 10.0]; // dist = 3

    let raw = RawObservation {
        neighbor_positions: vec![neighbor],
        neighbor_velocities: vec![[0.0; 3]],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1],
    };

    // Small attention radius
    let perception_small = PerceptionSystem::new().with_attention(true, 1.0);
    let processed_small = perception_small.process(&raw, &box_);

    // Large attention radius
    let perception_large = PerceptionSystem::new().with_attention(true, 10.0);
    let processed_large = perception_large.process(&raw, &box_);

    println!(
        "Small radius attention: {}, Large radius attention: {}",
        processed_small.attention_weights[0], processed_large.attention_weights[0]
    );

    // Larger radius should give higher attention at same distance
    assert!(
        processed_large.attention_weights[0] > processed_small.attention_weights[0],
        "Larger attention radius should give higher weight at same distance"
    );
}

// ============================================================================
// Noise Injection Tests
// ============================================================================

/// Test that noise is injected into observations
#[test]
fn test_noise_injection() {
    let box_ = SimulationBox::cubic(20.0);

    let true_position = [10.0, 10.0, 10.0];
    let true_velocity = [1.0, 2.0, 3.0];

    let raw = RawObservation {
        neighbor_positions: vec![],
        neighbor_velocities: vec![],
        self_position: true_position,
        self_velocity: true_velocity,
        neighbor_ids: vec![],
    };

    let perception = PerceptionSystem::new().with_noise(0.5, 0.1);

    // Run multiple times to check statistical properties
    let mut position_errors = Vec::new();
    let mut velocity_errors = Vec::new();

    for _ in 0..100 {
        let processed = perception.process(&raw, &box_);

        let pos_error = (
            (processed.perceived_self_position[0] - true_position[0]).powi(2)
            + (processed.perceived_self_position[1] - true_position[1]).powi(2)
            + (processed.perceived_self_position[2] - true_position[2]).powi(2)
        ).sqrt();

        let vel_error = (
            (processed.perceived_self_velocity[0] - true_velocity[0]).powi(2)
            + (processed.perceived_self_velocity[1] - true_velocity[1]).powi(2)
            + (processed.perceived_self_velocity[2] - true_velocity[2]).powi(2)
        ).sqrt();

        position_errors.push(pos_error);
        velocity_errors.push(vel_error);
    }

    let avg_pos_error: f32 = position_errors.iter().sum::<f32>() / position_errors.len() as f32;
    let avg_vel_error: f32 = velocity_errors.iter().sum::<f32>() / velocity_errors.len() as f32;

    println!("Average position error: {}", avg_pos_error);
    println!("Average velocity error: {}", avg_vel_error);

    // Errors should be non-zero
    assert!(avg_pos_error > 0.1, "Position noise should be injected");
    assert!(avg_vel_error > 0.01, "Velocity noise should be injected");

    // Position noise should be larger than velocity noise (due to std settings)
    assert!(
        avg_pos_error > avg_vel_error * 2.0,
        "Position noise should be larger than velocity noise"
    );
}

/// Test noise with zero std (should be no noise)
#[test]
fn test_noise_zero_std() {
    let box_ = SimulationBox::cubic(20.0);

    let true_position = [10.0, 10.0, 10.0];
    let true_velocity = [1.0, 2.0, 3.0];

    let raw = RawObservation {
        neighbor_positions: vec![],
        neighbor_velocities: vec![],
        self_position: true_position,
        self_velocity: true_velocity,
        neighbor_ids: vec![],
    };

    // No noise (default)
    let perception = PerceptionSystem::new();
    let processed = perception.process(&raw, &box_);

    assert_eq!(processed.perceived_self_position, true_position);
    assert_eq!(processed.perceived_self_velocity, true_velocity);
}

// ============================================================================
// Combined Perception Pipeline Tests
// ============================================================================

/// Test full perception pipeline with all features
#[test]
fn test_full_perception_pipeline() {
    let box_ = SimulationBox::cubic(30.0);

    let observer = [15.0, 15.0, 15.0];

    // Create a complex scene
    let mut neighbors = Vec::new();
    let mut velocities = Vec::new();

    // Close neighbors
    for i in 0..5 {
        neighbors.push([16.0 + i as f32 * 0.3, 15.0, 15.0]);
        velocities.push([0.1, 0.0, 0.0]);
    }

    // Far neighbors
    for i in 0..10 {
        let angle = i as f32 * std::f32::consts::PI * 2.0 / 10.0;
        neighbors.push([
            15.0 + 10.0 * angle.cos(),
            15.0 + 10.0 * angle.sin(),
            15.0,
        ]);
        velocities.push([0.0, 0.1, 0.0]);
    }

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: velocities,
        self_position: observer,
        self_velocity: [0.5, 0.5, 0.0],
        neighbor_ids: (0..neighbors.len() as u32).collect(),
    };

    let perception = PerceptionSystem::new()
        .with_occlusion(true)
        .with_bandwidth_limit(8)
        .with_attention(true, 5.0)
        .with_noise(0.1, 0.05);

    let processed = perception.process(&raw, &box_);

    println!("Full pipeline results:");
    println!("  Input neighbors: {}", neighbors.len());
    println!("  Visible after occlusion: {}", neighbors.len() - processed.occluded_count);
    println!("  After bandwidth limit: {}", processed.visible_positions.len());
    println!("  Occluded count: {}", processed.occluded_count);

    // Bandwidth limit should be respected
    assert!(processed.visible_positions.len() <= 8);

    // Attention weights should be computed
    assert_eq!(processed.attention_weights.len(), processed.visible_positions.len());

    // Self perception should have noise
    // (statistical - might occasionally match by chance)
    let pos_diff = (
        (processed.perceived_self_position[0] - observer[0]).powi(2)
        + (processed.perceived_self_position[1] - observer[1]).powi(2)
        + (processed.perceived_self_position[2] - observer[2]).powi(2)
    ).sqrt();

    // With noise std 0.1, we expect some difference
    println!("  Self position noise magnitude: {}", pos_diff);
}

/// Test perception consistency across multiple calls
#[test]
fn test_perception_determinism_without_noise() {
    let box_ = SimulationBox::cubic(20.0);

    let observer = [10.0, 10.0, 10.0];
    let neighbors = vec![
        [12.0, 10.0, 10.0],
        [14.0, 10.0, 10.0],
    ];

    let raw = RawObservation {
        neighbor_positions: neighbors.clone(),
        neighbor_velocities: vec![[0.0; 3]; 2],
        self_position: observer,
        self_velocity: [0.0; 3],
        neighbor_ids: vec![1, 2],
    };

    // Without noise, should be deterministic
    let perception = PerceptionSystem::new()
        .with_occlusion(true)
        .with_bandwidth_limit(5)
        .with_attention(true, 3.0);

    let processed1 = perception.process(&raw, &box_);
    let processed2 = perception.process(&raw, &box_);

    assert_eq!(processed1.visible_positions, processed2.visible_positions);
    assert_eq!(processed1.attention_weights, processed2.attention_weights);
    assert_eq!(processed1.occluded_count, processed2.occluded_count);
}
