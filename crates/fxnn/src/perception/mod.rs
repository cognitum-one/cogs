//! # Perception Module for FXNN
//!
//! This module implements Layer 3 (PERCEPTION) of the FXNN cognitive architecture,
//! providing entropy-bounded observation systems with attention mechanisms and
//! bandwidth-limited information processing.
//!
//! ## Overview
//!
//! The perception layer sits between the raw simulation state and higher cognitive
//! layers, providing:
//!
//! - **Observation**: Partial, noisy sensing of simulation state
//! - **Attention**: Salience-based filtering of observations
//! - **Bandwidth**: Information budget management per tick
//! - **Noise**: Realistic sensor noise models
//!
//! ## Architecture
//!
//! ```text
//! +------------------+
//! |  Simulation State|
//! +--------+---------+
//!          |
//!          v
//! +--------+---------+     +------------------+
//! |   Observer       | --> |  Observation     |
//! | (partial FOV,    |     | (sensor data,    |
//! |  range limits)   |     |  uncertainty)    |
//! +--------+---------+     +--------+---------+
//!          |                        |
//!          v                        v
//! +--------+---------+     +--------+---------+
//! |   Noise Model    | --> | AttentionFilter  |
//! | (Gaussian, depth,|     | (salience map,   |
//! |  occlusion)      |     |  top-k selection)|
//! +--------+---------+     +--------+---------+
//!          |                        |
//!          v                        v
//! +--------+---------+     +--------+---------+
//! | BandwidthLimiter | --> |  Final Output    |
//! | (entropy budget, |     | (entropy-bounded |
//! |  downsampling)   |     |  observations)   |
//! +------------------+     +------------------+
//! ```
//!
//! ## Key ADR-001 Requirements
//!
//! - **Entropy bounded observations**: Shannon entropy limits on sensor data
//! - **Max bandwidth per tick**: Configurable bytes/second limits
//! - **Occlusion handling**: Ray-based visibility testing
//! - **Update rate limits**: Configurable observation frequency
//! - **Information gain calculation**: Mutual information metrics
//!
//! ## Examples
//!
//! ### Basic Observation
//!
//! ```rust,no_run
//! use fxnn::perception::{Observer, PartialObserver, ObserverConfig};
//! use fxnn::types::{Atom, SimulationBox};
//!
//! // Create observer with limited field of view
//! let config = ObserverConfig {
//!     fov_angle: std::f32::consts::FRAC_PI_3, // 60 degrees
//!     max_range: 10.0,
//!     update_rate: 30.0, // Hz
//!     ..Default::default()
//! };
//! let observer = PartialObserver::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], config);
//!
//! // Observe atoms within FOV
//! let atoms = vec![Atom::default()];
//! let sim_box = SimulationBox::cubic(20.0);
//! let observation = observer.observe(&atoms, &sim_box);
//! ```
//!
//! ### Attention-Based Filtering
//!
//! ```rust,no_run
//! use fxnn::perception::{SalienceMap, TopKAttention, AttentionFilter};
//!
//! // Create salience map prioritizing high-velocity atoms
//! let mut salience_map = SalienceMap::new();
//! salience_map.add_velocity_salience(1.0);
//!
//! // Select top 10 most salient observations
//! let attention = TopKAttention::new(10);
//! ```
//!
//! ### Bandwidth-Limited Processing
//!
//! ```rust,no_run
//! use fxnn::perception::{BandwidthLimiter, EntropyBudget};
//!
//! // Limit to 1KB/second with entropy constraint
//! let bandwidth = BandwidthLimiter::new(1024); // bytes/sec
//! let entropy_budget = EntropyBudget::new(8.0); // max Shannon entropy
//! ```

mod attention;
mod bandwidth;
mod noise;
mod observer;

pub use attention::{
    AttentionFilter, AttentionResult, SalienceMap, SalienceSource, TopKAttention,
};
pub use bandwidth::{
    BandwidthLimiter, BandwidthMetrics, Downsampler, DownsampleStrategy, EntropyBudget,
    // ADR-001 Agent Budget Enforcement
    ComputeBudget, ComputeBudgetStats,
    MemoryWriteBudget, MemoryWriteStats, WriteResult,
    AgentBudgetEnforcer, AgentBudgetStatus,
    InformationBudget,
};
pub use noise::{DepthNoise, GaussianNoise, NoiseModel, OcclusionModel, OcclusionResult};
pub use observer::{
    Observation, ObservationData, Observer, ObserverConfig, PartialObservation, PartialObserver,
    SensorReading,
};

/// Information-theoretic utilities for perception
pub mod info_theory {
    /// Calculate Shannon entropy of a discrete probability distribution
    ///
    /// # Arguments
    ///
    /// * `probabilities` - Slice of probability values (should sum to 1.0)
    ///
    /// # Returns
    ///
    /// Shannon entropy in bits: H = -sum(p * log2(p))
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fxnn::perception::info_theory::shannon_entropy;
    ///
    /// // Maximum entropy for uniform distribution over 4 outcomes
    /// let probs = [0.25, 0.25, 0.25, 0.25];
    /// let entropy = shannon_entropy(&probs);
    /// assert!((entropy - 2.0).abs() < 1e-6); // log2(4) = 2 bits
    /// ```
    pub fn shannon_entropy(probabilities: &[f32]) -> f32 {
        probabilities
            .iter()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum()
    }

    /// Calculate mutual information between two probability distributions
    ///
    /// # Arguments
    ///
    /// * `joint` - Joint probability distribution P(X,Y) as 2D array (flattened)
    /// * `marginal_x` - Marginal distribution P(X)
    /// * `marginal_y` - Marginal distribution P(Y)
    ///
    /// # Returns
    ///
    /// Mutual information I(X;Y) = H(X) + H(Y) - H(X,Y)
    pub fn mutual_information(joint: &[f32], marginal_x: &[f32], marginal_y: &[f32]) -> f32 {
        let h_x = shannon_entropy(marginal_x);
        let h_y = shannon_entropy(marginal_y);
        let h_xy = shannon_entropy(joint);
        (h_x + h_y - h_xy).max(0.0)
    }

    /// Calculate information gain (reduction in entropy)
    ///
    /// # Arguments
    ///
    /// * `prior_entropy` - Entropy before observation
    /// * `posterior_entropy` - Entropy after observation
    ///
    /// # Returns
    ///
    /// Information gain: IG = H_prior - H_posterior
    pub fn information_gain(prior_entropy: f32, posterior_entropy: f32) -> f32 {
        (prior_entropy - posterior_entropy).max(0.0)
    }

    /// Quantize continuous values to discrete bins for entropy calculation
    ///
    /// # Arguments
    ///
    /// * `values` - Continuous values to quantize
    /// * `min_val` - Minimum expected value
    /// * `max_val` - Maximum expected value
    /// * `num_bins` - Number of discrete bins
    ///
    /// # Returns
    ///
    /// Probability distribution over bins
    pub fn quantize_to_probabilities(
        values: &[f32],
        min_val: f32,
        max_val: f32,
        num_bins: usize,
    ) -> Vec<f32> {
        let mut counts = vec![0usize; num_bins];
        let range = max_val - min_val;

        for &value in values {
            let normalized = ((value - min_val) / range).clamp(0.0, 0.9999);
            let bin = (normalized * num_bins as f32) as usize;
            counts[bin] += 1;
        }

        let total = values.len() as f32;
        counts.into_iter().map(|c| c as f32 / total).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::info_theory::*;

    #[test]
    fn test_shannon_entropy_uniform() {
        // Maximum entropy for 4 equally likely outcomes
        let probs = [0.25, 0.25, 0.25, 0.25];
        let entropy = shannon_entropy(&probs);
        assert!((entropy - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_shannon_entropy_deterministic() {
        // Zero entropy when outcome is certain
        let probs = [1.0, 0.0, 0.0, 0.0];
        let entropy = shannon_entropy(&probs);
        assert!((entropy - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_information_gain() {
        let ig = information_gain(3.0, 1.0);
        assert!((ig - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_quantize_to_probabilities() {
        let values = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let probs = quantize_to_probabilities(&values, 0.0, 1.0, 5);
        assert_eq!(probs.len(), 5);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }
}
