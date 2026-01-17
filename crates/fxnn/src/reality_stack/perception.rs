//! # Layer 3: PERCEPTION
//!
//! The perception layer provides the information bottleneck between the physical
//! world and agent decision-making. Key features:
//!
//! - **Partial Observability**: Agents cannot see the full world state
//! - **Attention Mechanisms**: Focus computational resources on relevant information
//! - **Bandwidth Limits**: Constrain information flow to realistic levels
//! - **Noise Models**: Add realistic sensor noise
//!
//! ## Design Philosophy
//!
//! The perception layer enforces that agents must learn to deal with incomplete
//! and noisy information, leading to more robust and realistic emergent behaviors.

use crate::error::{FxnnError, Result};
use crate::types::{Atom, SimulationBox};
use super::agency::{AgentId, SensorReading};
use super::physics::WorldState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Core Types
// ============================================================================

/// An observation for a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Agent this observation is for
    pub agent_id: AgentId,
    /// Sensor readings
    pub readings: Vec<SensorReading>,
    /// Timestamp
    pub timestamp: u64,
    /// Total bandwidth used
    pub bandwidth_used: usize,
    /// Attention weights applied
    pub attention_weights: Vec<f32>,
}

impl Observation {
    /// Create a new observation
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            readings: Vec::new(),
            timestamp: 0,
            bandwidth_used: 0,
            attention_weights: Vec::new(),
        }
    }

    /// Add a sensor reading
    pub fn add_reading(&mut self, reading: SensorReading) {
        self.bandwidth_used += reading.values.len();
        self.readings.push(reading);
    }

    /// Get total number of values
    pub fn n_values(&self) -> usize {
        self.readings.iter().map(|r| r.values.len()).sum()
    }
}

// ============================================================================
// Attention Mechanisms
// ============================================================================

/// Attention mask for focusing perception
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttentionMask {
    /// Uniform attention (no focus)
    Uniform,

    /// Spatial attention (focus on nearby regions)
    Spatial {
        center: [f32; 3],
        radius: f32,
        falloff: f32,
    },

    /// Type-based attention (focus on specific atom types)
    TypeBased {
        target_types: Vec<u16>,
        weight: f32,
    },

    /// Velocity-based attention (focus on moving objects)
    VelocityBased {
        min_velocity: f32,
        weight: f32,
    },

    /// Combined attention (multiple masks)
    Combined {
        masks: Vec<AttentionMask>,
        weights: Vec<f32>,
    },

    /// Learned attention (from neural network)
    Learned {
        weights: Vec<f32>,
        bias: f32,
    },
}

impl AttentionMask {
    /// Create spatial attention
    pub fn spatial(radius: f32) -> Self {
        Self::Spatial {
            center: [0.0; 3],
            radius,
            falloff: 2.0,
        }
    }

    /// Create type-based attention
    pub fn type_based(types: Vec<u16>) -> Self {
        Self::TypeBased {
            target_types: types,
            weight: 2.0,
        }
    }

    /// Compute attention weight for an atom
    pub fn weight(&self, atom: &Atom, agent_position: [f32; 3], box_: &SimulationBox) -> f32 {
        match self {
            Self::Uniform => 1.0,

            Self::Spatial { center, radius, falloff } => {
                let actual_center = [
                    agent_position[0] + center[0],
                    agent_position[1] + center[1],
                    agent_position[2] + center[2],
                ];
                let dr = box_.minimum_image(
                    atom.position[0] - actual_center[0],
                    atom.position[1] - actual_center[1],
                    atom.position[2] - actual_center[2],
                );
                let dist = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
                if dist < *radius {
                    1.0
                } else {
                    (-falloff * (dist - radius) / radius).exp()
                }
            }

            Self::TypeBased { target_types, weight } => {
                if target_types.contains(&atom.atom_type) {
                    *weight
                } else {
                    1.0
                }
            }

            Self::VelocityBased { min_velocity, weight } => {
                let speed = (atom.velocity[0].powi(2) + atom.velocity[1].powi(2) + atom.velocity[2].powi(2)).sqrt();
                if speed > *min_velocity {
                    *weight
                } else {
                    1.0
                }
            }

            Self::Combined { masks, weights } => {
                let mut total = 0.0;
                for (mask, w) in masks.iter().zip(weights.iter()) {
                    total += w * mask.weight(atom, agent_position, box_);
                }
                total / weights.iter().sum::<f32>().max(1.0)
            }

            Self::Learned { weights, bias } => {
                // Simple linear model: w0*x + w1*y + w2*z + w3*vx + ... + bias
                let features = [
                    atom.position[0], atom.position[1], atom.position[2],
                    atom.velocity[0], atom.velocity[1], atom.velocity[2],
                    atom.charge,
                ];
                let mut score = *bias;
                for (i, &w) in weights.iter().enumerate() {
                    if i < features.len() {
                        score += w * features[i];
                    }
                }
                score.sigmoid()
            }
        }
    }
}

/// Sigmoid helper
trait Sigmoid {
    fn sigmoid(self) -> Self;
}

impl Sigmoid for f32 {
    fn sigmoid(self) -> Self {
        1.0 / (1.0 + (-self).exp())
    }
}

// ============================================================================
// Bandwidth Limiting
// ============================================================================

/// Bandwidth limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthLimit {
    /// Maximum floats per observation
    pub max_values: usize,
    /// Maximum sensors per observation
    pub max_sensors: usize,
    /// Strategy for handling overflow
    pub overflow_strategy: OverflowStrategy,
}

/// Strategy for handling bandwidth overflow
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OverflowStrategy {
    /// Truncate to max
    Truncate,
    /// Sample randomly
    Sample,
    /// Use attention to prioritize
    AttentionBased,
    /// Compress with quantization
    Quantize,
}

impl Default for BandwidthLimit {
    fn default() -> Self {
        Self {
            max_values: 1024,
            max_sensors: 16,
            overflow_strategy: OverflowStrategy::AttentionBased,
        }
    }
}

impl BandwidthLimit {
    /// Create a new bandwidth limit
    pub fn new(max_values: usize) -> Self {
        Self {
            max_values,
            ..Default::default()
        }
    }

    /// Check if observation exceeds limit
    pub fn exceeds(&self, observation: &Observation) -> bool {
        observation.bandwidth_used > self.max_values ||
        observation.readings.len() > self.max_sensors
    }

    /// Apply limit to observation
    pub fn apply(&self, observation: &mut Observation) {
        match self.overflow_strategy {
            OverflowStrategy::Truncate => {
                // Truncate readings to fit
                while observation.bandwidth_used > self.max_values && !observation.readings.is_empty() {
                    if let Some(removed) = observation.readings.pop() {
                        observation.bandwidth_used -= removed.values.len();
                    }
                }
            }

            OverflowStrategy::Sample => {
                // Randomly sample readings to fit
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                observation.readings.shuffle(&mut rng);

                while observation.bandwidth_used > self.max_values && !observation.readings.is_empty() {
                    if let Some(removed) = observation.readings.pop() {
                        observation.bandwidth_used -= removed.values.len();
                    }
                }
            }

            OverflowStrategy::AttentionBased => {
                // Sort by attention weight and keep top
                observation.readings.sort_by(|a, b| {
                    // Higher noise = lower quality = sort to end
                    b.noise_level.partial_cmp(&a.noise_level).unwrap_or(std::cmp::Ordering::Equal)
                });

                while observation.bandwidth_used > self.max_values && !observation.readings.is_empty() {
                    if let Some(removed) = observation.readings.pop() {
                        observation.bandwidth_used -= removed.values.len();
                    }
                }
            }

            OverflowStrategy::Quantize => {
                // Quantize values to reduce effective bandwidth
                for reading in &mut observation.readings {
                    for value in &mut reading.values {
                        // Quantize to 256 levels
                        *value = ((*value * 128.0).round() / 128.0).clamp(-2.0, 2.0);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Noise Models
// ============================================================================

/// Noise model for perception
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoiseModel {
    /// No noise
    None,

    /// Gaussian noise
    Gaussian {
        mean: f32,
        std_dev: f32,
    },

    /// Uniform noise
    Uniform {
        min: f32,
        max: f32,
    },

    /// Quantization noise
    Quantization {
        levels: u32,
    },

    /// Distance-dependent noise (farther = noisier)
    DistanceDependent {
        base_noise: f32,
        distance_factor: f32,
    },

    /// Combined noise models
    Combined {
        models: Vec<NoiseModel>,
    },
}

impl Default for NoiseModel {
    fn default() -> Self {
        Self::Gaussian {
            mean: 0.0,
            std_dev: 0.01,
        }
    }
}

impl NoiseModel {
    /// Apply noise to a value
    pub fn apply(&self, value: f32, distance: f32) -> f32 {
        match self {
            Self::None => value,

            Self::Gaussian { mean, std_dev } => {
                use rand_distr::{Distribution, Normal};
                let normal = Normal::new(*mean as f64, *std_dev as f64).unwrap();
                let mut rng = rand::thread_rng();
                value + normal.sample(&mut rng) as f32
            }

            Self::Uniform { min, max } => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                value + rng.gen_range(*min..*max)
            }

            Self::Quantization { levels } => {
                let scale = *levels as f32;
                (value * scale).round() / scale
            }

            Self::DistanceDependent { base_noise, distance_factor } => {
                let effective_noise = base_noise + distance * distance_factor;
                use rand_distr::{Distribution, Normal};
                let normal = Normal::new(0.0, effective_noise as f64).unwrap();
                let mut rng = rand::thread_rng();
                value + normal.sample(&mut rng) as f32
            }

            Self::Combined { models } => {
                let mut result = value;
                for model in models {
                    result = model.apply(result, distance);
                }
                result
            }
        }
    }

    /// Apply noise to a reading
    pub fn apply_to_reading(&self, reading: &mut SensorReading, distances: &[f32]) {
        for (i, value) in reading.values.iter_mut().enumerate() {
            let dist = distances.get(i).copied().unwrap_or(1.0);
            *value = self.apply(*value, dist);
        }
    }
}

// ============================================================================
// Observer
// ============================================================================

/// Main observer struct for generating observations
pub struct Observer {
    /// Attention mask
    attention: AttentionMask,
    /// Bandwidth limit
    bandwidth_limit: BandwidthLimit,
    /// Noise model
    noise_model: NoiseModel,
    /// Agent positions (for computing observations)
    agent_positions: HashMap<AgentId, [f32; 3]>,
    /// Registered sensors per agent
    agent_sensors: HashMap<AgentId, Vec<Box<dyn super::agency::Sensor>>>,
}

impl Observer {
    /// Create a new observer
    pub fn new() -> Self {
        Self {
            attention: AttentionMask::Uniform,
            bandwidth_limit: BandwidthLimit::default(),
            noise_model: NoiseModel::default(),
            agent_positions: HashMap::new(),
            agent_sensors: HashMap::new(),
        }
    }

    /// Set attention mask
    pub fn with_attention(mut self, attention: AttentionMask) -> Self {
        self.attention = attention;
        self
    }

    /// Set bandwidth limit
    pub fn with_bandwidth(mut self, max_values: usize) -> Self {
        self.bandwidth_limit = BandwidthLimit::new(max_values);
        self
    }

    /// Set noise model
    pub fn with_noise(mut self, noise_model: NoiseModel) -> Self {
        self.noise_model = noise_model;
        self
    }

    /// Register an agent
    pub fn register_agent(
        &mut self,
        agent_id: AgentId,
        position: [f32; 3],
        sensors: Vec<Box<dyn super::agency::Sensor>>,
    ) {
        self.agent_positions.insert(agent_id, position);
        self.agent_sensors.insert(agent_id, sensors);
    }

    /// Update agent position
    pub fn update_position(&mut self, agent_id: AgentId, position: [f32; 3]) {
        self.agent_positions.insert(agent_id, position);
    }

    /// Generate observations for all agents
    pub fn observe(&self, world_state: &WorldState) -> Result<Vec<Observation>> {
        let mut observations = Vec::new();

        for (&agent_id, &position) in &self.agent_positions {
            let mut obs = Observation::new(agent_id);

            if let Some(sensors) = self.agent_sensors.get(&agent_id) {
                for sensor in sensors {
                    let mut reading = sensor.read(
                        position,
                        &world_state.atoms,
                        &world_state.box_,
                    );

                    // Apply attention-weighted noise
                    let distances: Vec<f32> = reading.values.iter().map(|_| 1.0).collect();
                    self.noise_model.apply_to_reading(&mut reading, &distances);

                    obs.add_reading(reading);
                }
            }

            // Apply bandwidth limit
            let mut obs_copy = obs.clone();
            self.bandwidth_limit.apply(&mut obs_copy);

            observations.push(obs_copy);
        }

        Ok(observations)
    }

    /// Get bandwidth limit
    pub fn bandwidth(&self) -> usize {
        self.bandwidth_limit.max_values
    }

    /// Get noise model
    pub fn noise_model(&self) -> &NoiseModel {
        &self.noise_model
    }
}

impl Default for Observer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Occlusion
// ============================================================================

/// Occlusion model for line-of-sight blocking
pub struct OcclusionModel {
    /// Atoms that can block vision
    blocking_types: Vec<u16>,
    /// Blocking radius
    blocking_radius: f32,
}

impl OcclusionModel {
    /// Create a new occlusion model
    pub fn new() -> Self {
        Self {
            blocking_types: Vec::new(),
            blocking_radius: 0.5,
        }
    }

    /// Add blocking atom type
    pub fn with_blocking_type(mut self, atom_type: u16) -> Self {
        self.blocking_types.push(atom_type);
        self
    }

    /// Check if line of sight is blocked
    pub fn is_blocked(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        atoms: &[Atom],
        box_: &SimulationBox,
    ) -> bool {
        let dr = box_.minimum_image(
            to[0] - from[0],
            to[1] - from[1],
            to[2] - from[2],
        );
        let dist = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();

        for atom in atoms {
            if !self.blocking_types.contains(&atom.atom_type) {
                continue;
            }

            // Check if atom is between from and to
            let da = box_.minimum_image(
                atom.position[0] - from[0],
                atom.position[1] - from[1],
                atom.position[2] - from[2],
            );

            // Project onto line
            let t = (da[0] * dr[0] + da[1] * dr[1] + da[2] * dr[2]) / (dist * dist);

            if t > 0.0 && t < 1.0 {
                // Closest point on line
                let closest = [
                    from[0] + t * dr[0],
                    from[1] + t * dr[1],
                    from[2] + t * dr[2],
                ];

                let dc = box_.minimum_image(
                    atom.position[0] - closest[0],
                    atom.position[1] - closest[1],
                    atom.position[2] - closest[2],
                );
                let dist_to_line = (dc[0] * dc[0] + dc[1] * dc[1] + dc[2] * dc[2]).sqrt();

                if dist_to_line < self.blocking_radius {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for OcclusionModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_spatial() {
        let mask = AttentionMask::spatial(5.0);
        // Use larger box to avoid PBC wrapping making "far" atoms appear close
        let box_ = SimulationBox::cubic(30.0);

        // atom_near at distance 1.0 (within radius, weight = 1.0)
        let atom_near = Atom::new(0, 0, 1.0).with_position(1.0, 0.0, 0.0);
        // atom_far at distance 8.0 (outside radius, weight = exponential falloff)
        let atom_far = Atom::new(1, 0, 1.0).with_position(8.0, 0.0, 0.0);

        let w_near = mask.weight(&atom_near, [0.0, 0.0, 0.0], &box_);
        let w_far = mask.weight(&atom_far, [0.0, 0.0, 0.0], &box_);

        assert!(w_near > w_far, "Near atom (dist=1) should have higher weight than far atom (dist=8)");
    }

    #[test]
    fn test_bandwidth_limit() {
        let limit = BandwidthLimit::new(100);
        let mut obs = Observation::new(AgentId(0));

        // Add readings that exceed limit
        for _ in 0..20 {
            obs.add_reading(SensorReading {
                sensor_id: super::super::agency::sensor::SensorId(0),
                kind: super::super::agency::sensor::SensorKind::Proprioceptive,
                values: vec![0.0; 10],
                timestamp: 0,
                noise_level: 0.0,
            });
        }

        assert!(limit.exceeds(&obs));

        limit.apply(&mut obs);
        assert!(!limit.exceeds(&obs));
    }
}
