//! # ThermalBrain
//!
//! A neuromorphic-inspired embedded system combining sparse vector computation,
//! spiking neural networks, and thermal self-regulation.
//!
//! ## Features
//!
//! - **Sparse Encoding**: Efficient representation with >90% sparsity
//! - **Spiking Neural Network**: LIF neurons with adaptive thresholds
//! - **Thermal Governor**: Temperature-based adaptive control
//! - **Mini-HNSW**: Approximate nearest neighbor search
//! - **Multi-Platform**: ESP32-S3, Cognitum V1/V2, WASM
//!
//! ## Example
//!
//! ```rust,no_run
//! use thermal_brain::{ThermalBrain, ThermalBrainConfig};
//!
//! let config = ThermalBrainConfig::default();
//! let mut brain = ThermalBrain::new(config);
//!
//! // Process sensor data
//! brain.push_sample(25.5);
//!
//! // Run inference
//! if let Some(result) = brain.process() {
//!     println!("Matched: {} ({:.1}%)", result.label, result.confidence * 100.0);
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Core modules
pub mod config;
pub mod error;
pub mod types;

// Processing modules
pub mod encoding;
pub mod governor;
pub mod neural;

// Optimization modules (SOTA techniques)
pub mod optimization;

// Platform abstractions
pub mod platform;

// WASM bindings
#[cfg(feature = "wasm")]
pub mod wasm;

// Re-exports
pub use config::*;
pub use error::*;
pub use types::*;

use encoding::{FeatureExtractor, SparseEncoder};
use governor::ThermalGovernor;
use neural::{MiniHnsw, SpikingMatcher};

/// Main ThermalBrain system controller
pub struct ThermalBrain {
    config: ThermalBrainConfig,
    governor: ThermalGovernor,
    extractor: FeatureExtractor,
    encoder: SparseEncoder,
    matcher: SpikingMatcher,
    hnsw: MiniHnsw,
    status: SystemStatus,
    last_process_ms: u64,
}

impl ThermalBrain {
    /// Create new ThermalBrain instance with configuration
    pub fn new(config: ThermalBrainConfig) -> Self {
        let governor = ThermalGovernor::new(config.thermal.clone());
        let extractor = FeatureExtractor::new(config.encoding.clone());
        let encoder = SparseEncoder::new();
        let matcher = SpikingMatcher::new(config.neural.clone());
        let hnsw = MiniHnsw::new(config.neural.hnsw_m, config.neural.hnsw_ef_construction);

        Self {
            config,
            governor,
            extractor,
            encoder,
            matcher,
            hnsw,
            status: SystemStatus::default(),
            last_process_ms: 0,
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(ThermalBrainConfig::default())
    }

    /// Push a temperature sample
    pub fn push_sample(&mut self, temperature_c: f32) {
        self.extractor.push(temperature_c);
        self.governor.update(temperature_c);
        self.status.temperature_c = temperature_c;
        self.status.zone = self.governor.zone();
    }

    /// Run one processing cycle
    ///
    /// Returns a match result if a pattern was detected
    pub fn process(&mut self) -> Option<MatchResult> {
        // Check if we can process
        if !self.governor.can_process() {
            return None;
        }

        // Extract features
        let features = self.extractor.compute();

        // Encode to sparse representation
        let sparse = self.encoder.encode(&features, self.governor.spike_threshold());

        // Skip if too sparse (no content)
        if sparse.nnz() == 0 {
            return None;
        }

        // Update status
        self.status.inference_count += 1;
        self.status.spike_threshold = self.governor.spike_threshold();

        // Run spiking matcher
        let dt_ms = 10; // TODO: Calculate actual dt
        let result = self.matcher.process(
            &sparse,
            self.governor.refractory_ms(),
            dt_ms,
            &self.hnsw,
        );

        if result.is_some() {
            self.status.spike_count += 1;
        }

        result
    }

    /// Learn a new pattern from current state
    pub fn learn(&mut self, label: &str) -> core::result::Result<u32, ThermalBrainError> {
        // Validate label
        if label.is_empty() || label.len() > 31 {
            return Err(ThermalBrainError::InvalidLabel);
        }

        // Check capacity
        if self.hnsw.len() >= self.config.storage.max_patterns {
            return Err(ThermalBrainError::PatternLimitReached);
        }

        // Extract and encode current state
        let features = self.extractor.compute();
        let pattern = self.encoder.quantize(&features);

        // Add to HNSW index
        let id = self.hnsw.insert(pattern)?;

        // Add to matcher
        self.matcher.add_pattern(id, pattern, label)?;

        self.status.pattern_count = self.hnsw.len();

        Ok(id)
    }

    /// Delete a learned pattern
    pub fn forget(&mut self, pattern_id: u32) -> core::result::Result<(), ThermalBrainError> {
        self.matcher.remove_pattern(pattern_id)?;
        // Note: HNSW doesn't support deletion, we mark as deleted
        Ok(())
    }

    /// Get current system status
    pub fn status(&self) -> &SystemStatus {
        &self.status
    }

    /// Get current thermal zone
    pub fn thermal_zone(&self) -> ThermalZone {
        self.governor.zone()
    }

    /// Get recommended sleep duration in milliseconds
    pub fn recommended_sleep_ms(&self) -> u32 {
        self.governor.sleep_ms()
    }

    /// Get the sparse vector from last encoding
    pub fn last_sparse(&self) -> &SparseVector {
        self.encoder.last_sparse()
    }

    /// Get the feature vector from last extraction
    pub fn last_features(&self) -> &FeatureVector {
        self.extractor.last_features()
    }

    /// Get number of stored patterns
    pub fn pattern_count(&self) -> usize {
        self.hnsw.len()
    }

    /// Reset all neurons to initial state
    pub fn reset_neurons(&mut self) {
        self.matcher.reset_all();
    }

    /// Get HNSW index statistics
    pub fn hnsw_stats(&self) -> HnswStats {
        self.hnsw.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThermalConfig;

    #[test]
    fn test_create_brain() {
        let brain = ThermalBrain::default_config();
        assert_eq!(brain.pattern_count(), 0);
        assert_eq!(brain.thermal_zone(), ThermalZone::Cool);
    }

    #[test]
    fn test_push_sample() {
        let mut brain = ThermalBrain::default_config();
        brain.push_sample(25.0);
        assert_eq!(brain.status().temperature_c, 25.0);
        assert_eq!(brain.thermal_zone(), ThermalZone::Cool);
    }

    #[test]
    fn test_thermal_zones() {
        // Use instant EMA for testing
        let config = ThermalBrainConfig {
            thermal: ThermalConfig {
                ema_alpha: 1.0, // Instant response for testing
                ..ThermalConfig::default()
            },
            ..Default::default()
        };
        let mut brain = ThermalBrain::new(config);

        brain.push_sample(30.0);
        assert_eq!(brain.thermal_zone(), ThermalZone::Cool);

        brain.push_sample(45.0);
        assert_eq!(brain.thermal_zone(), ThermalZone::Warm);

        brain.push_sample(55.0);
        assert_eq!(brain.thermal_zone(), ThermalZone::Hot);
    }

    #[test]
    fn test_learn_pattern() {
        let mut brain = ThermalBrain::default_config();

        // Push some samples to create a pattern
        for i in 0..100 {
            brain.push_sample(25.0 + (i as f32 * 0.1));
        }

        let result = brain.learn("test_pattern");
        assert!(result.is_ok());
        assert_eq!(brain.pattern_count(), 1);
    }
}
