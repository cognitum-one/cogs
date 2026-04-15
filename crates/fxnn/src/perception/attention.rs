//! Attention mechanisms for perception filtering.
//!
//! This module provides attention-based filtering to prioritize the most
//! relevant observations from a potentially large set of sensor readings.
//!
//! # Overview
//!
//! The attention system helps agents focus computational resources on the
//! most important observations by:
//!
//! - Computing salience scores for each observation
//! - Filtering observations based on attention priorities
//! - Selecting top-k most relevant observations
//!
//! # Examples
//!
//! ```rust,no_run
//! use fxnn::perception::{SalienceMap, TopKAttention, AttentionFilter};
//!
//! // Create salience map that prioritizes nearby, fast-moving atoms
//! let mut salience = SalienceMap::new();
//! salience.add_distance_salience(1.0, true);  // Closer = more salient
//! salience.add_velocity_salience(0.5);        // Faster = more salient
//!
//! // Apply top-10 attention filter
//! let attention = TopKAttention::new(10);
//! ```

use super::observer::{ObservationData, SensorReading};
use serde::{Deserialize, Serialize};

/// Result of applying attention filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionResult {
    /// Indices of selected readings in the original observation.
    pub selected_indices: Vec<usize>,

    /// Salience scores for selected readings.
    pub salience_scores: Vec<f32>,

    /// Total attention capacity used.
    pub capacity_used: f32,

    /// Number of readings that were filtered out.
    pub filtered_count: usize,
}

impl AttentionResult {
    /// Check if a reading index was selected.
    pub fn contains(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// Get the salience score for a selected reading by its index.
    pub fn salience_for(&self, index: usize) -> Option<f32> {
        self.selected_indices
            .iter()
            .position(|&i| i == index)
            .map(|pos| self.salience_scores[pos])
    }

    /// Get the number of selected readings.
    pub fn selected_count(&self) -> usize {
        self.selected_indices.len()
    }
}

/// Trait for attention filtering mechanisms.
///
/// Implementations decide which observations to prioritize based on
/// various criteria like salience, relevance, or novelty.
pub trait AttentionFilter {
    /// Filter observations based on attention priorities.
    ///
    /// # Arguments
    ///
    /// * `data` - The observation data to filter
    /// * `salience` - Optional salience map for computing priorities
    ///
    /// # Returns
    ///
    /// AttentionResult containing selected reading indices and scores.
    fn filter(&self, data: &ObservationData, salience: Option<&SalienceMap>) -> AttentionResult;

    /// Get the maximum number of observations this filter can return.
    fn capacity(&self) -> usize;
}

/// Source of salience for computing attention priorities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SalienceSource {
    /// Distance-based salience (closer or farther = more salient).
    Distance {
        /// Weight for this source.
        weight: f32,
        /// If true, closer is more salient; if false, farther is more salient.
        closer_is_salient: bool,
    },

    /// Velocity-based salience (faster = more salient).
    Velocity {
        /// Weight for this source.
        weight: f32,
    },

    /// Confidence-based salience (higher confidence = more salient).
    Confidence {
        /// Weight for this source.
        weight: f32,
    },

    /// Angle-based salience (more centered in FOV = more salient).
    Angle {
        /// Weight for this source.
        weight: f32,
    },

    /// Atom type based salience (specific types are more salient).
    AtomType {
        /// Weight for this source.
        weight: f32,
        /// Atom types that are considered salient (max 8).
        salient_types: [u16; 8],
        /// Number of valid entries in salient_types.
        count: usize,
    },

    /// Custom salience from external function.
    Custom {
        /// Weight for this source.
        weight: f32,
    },
}

/// Salience map for computing observation priorities.
///
/// Combines multiple salience sources to produce a composite score
/// for each observation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SalienceMap {
    /// Salience sources with their configurations.
    sources: Vec<SalienceSource>,

    /// Normalization mode for combining sources.
    normalize: bool,

    /// Custom salience values (indexed by atom_id).
    custom_values: Vec<(u32, f32)>,
}

impl SalienceMap {
    /// Create an empty salience map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add distance-based salience.
    ///
    /// # Arguments
    ///
    /// * `weight` - Weight for this salience source
    /// * `closer_is_salient` - If true, closer objects are more salient
    pub fn add_distance_salience(&mut self, weight: f32, closer_is_salient: bool) -> &mut Self {
        self.sources.push(SalienceSource::Distance {
            weight,
            closer_is_salient,
        });
        self
    }

    /// Add velocity-based salience (faster = more salient).
    pub fn add_velocity_salience(&mut self, weight: f32) -> &mut Self {
        self.sources.push(SalienceSource::Velocity { weight });
        self
    }

    /// Add confidence-based salience.
    pub fn add_confidence_salience(&mut self, weight: f32) -> &mut Self {
        self.sources.push(SalienceSource::Confidence { weight });
        self
    }

    /// Add angle-based salience (centered in FOV = more salient).
    pub fn add_angle_salience(&mut self, weight: f32) -> &mut Self {
        self.sources.push(SalienceSource::Angle { weight });
        self
    }

    /// Add atom type based salience.
    pub fn add_type_salience(&mut self, weight: f32, types: &[u16]) -> &mut Self {
        let mut salient_types = [0u16; 8];
        let count = types.len().min(8);
        salient_types[..count].copy_from_slice(&types[..count]);
        self.sources.push(SalienceSource::AtomType {
            weight,
            salient_types,
            count,
        });
        self
    }

    /// Set custom salience value for a specific atom.
    pub fn set_custom_salience(&mut self, atom_id: u32, salience: f32) -> &mut Self {
        // Remove existing entry if present
        self.custom_values.retain(|(id, _)| *id != atom_id);
        self.custom_values.push((atom_id, salience));
        self
    }

    /// Enable normalization of combined salience scores.
    pub fn with_normalization(&mut self, normalize: bool) -> &mut Self {
        self.normalize = normalize;
        self
    }

    /// Compute salience score for a single reading.
    pub fn compute_salience(&self, reading: &SensorReading, config: &super::observer::ObserverConfig) -> f32 {
        if self.sources.is_empty() {
            return 1.0; // Default salience if no sources defined
        }

        let mut total_salience = 0.0;
        let mut total_weight = 0.0;

        for source in &self.sources {
            let (score, weight) = match source {
                SalienceSource::Distance {
                    weight,
                    closer_is_salient,
                } => {
                    let normalized_dist = reading.distance / config.max_range;
                    let score = if *closer_is_salient {
                        1.0 - normalized_dist
                    } else {
                        normalized_dist
                    };
                    (score, *weight)
                }

                SalienceSource::Velocity { weight } => {
                    let speed = reading.velocity.map_or(0.0, |v| {
                        (v[0].powi(2) + v[1].powi(2) + v[2].powi(2)).sqrt()
                    });
                    // Normalize velocity (assume max speed of 10 units)
                    let score = (speed / 10.0).min(1.0);
                    (score, *weight)
                }

                SalienceSource::Confidence { weight } => (reading.confidence, *weight),

                SalienceSource::Angle { weight } => {
                    let normalized_angle = reading.angle / config.fov_angle.max(0.001);
                    let score = 1.0 - normalized_angle.min(1.0);
                    (score, *weight)
                }

                SalienceSource::AtomType {
                    weight,
                    salient_types,
                    count,
                } => {
                    let score = if salient_types[..*count].contains(&reading.atom_type) {
                        1.0
                    } else {
                        0.0
                    };
                    (score, *weight)
                }

                SalienceSource::Custom { weight } => {
                    let score = self
                        .custom_values
                        .iter()
                        .find(|(id, _)| *id == reading.atom_id)
                        .map(|(_, s)| *s)
                        .unwrap_or(0.0);
                    (score, *weight)
                }
            };

            total_salience += score * weight;
            total_weight += weight;
        }

        if self.normalize && total_weight > 0.0 {
            total_salience / total_weight
        } else {
            total_salience
        }
    }

    /// Compute salience scores for all readings.
    pub fn compute_all_salience(&self, data: &ObservationData) -> Vec<f32> {
        data.readings
            .iter()
            .map(|r| self.compute_salience(r, &data.config))
            .collect()
    }
}

/// Top-K attention filter that selects the K most salient observations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TopKAttention {
    /// Maximum number of observations to select.
    k: usize,

    /// Minimum salience threshold (readings below this are excluded).
    min_salience: f32,
}

impl TopKAttention {
    /// Create a new top-K attention filter.
    ///
    /// # Arguments
    ///
    /// * `k` - Maximum number of observations to select
    pub fn new(k: usize) -> Self {
        Self {
            k,
            min_salience: 0.0,
        }
    }

    /// Set minimum salience threshold.
    pub fn with_min_salience(mut self, threshold: f32) -> Self {
        self.min_salience = threshold;
        self
    }
}

impl AttentionFilter for TopKAttention {
    fn filter(&self, data: &ObservationData, salience: Option<&SalienceMap>) -> AttentionResult {
        // Compute salience scores
        let scores: Vec<f32> = match salience {
            Some(map) => map.compute_all_salience(data),
            None => data.readings.iter().map(|r| r.confidence).collect(),
        };

        // Create index-score pairs and filter by minimum salience
        let mut indexed_scores: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score >= self.min_salience)
            .collect();

        // Sort by salience (highest first)
        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Take top K
        indexed_scores.truncate(self.k);

        let filtered_count = data.readings.len() - indexed_scores.len();
        let capacity_used = indexed_scores.len() as f32 / self.k as f32;

        AttentionResult {
            selected_indices: indexed_scores.iter().map(|(i, _)| *i).collect(),
            salience_scores: indexed_scores.iter().map(|(_, s)| *s).collect(),
            capacity_used,
            filtered_count,
        }
    }

    fn capacity(&self) -> usize {
        self.k
    }
}

/// Threshold-based attention filter that selects all observations above a salience threshold.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThresholdAttention {
    /// Minimum salience threshold.
    threshold: f32,

    /// Optional maximum number of observations.
    max_count: Option<usize>,
}

impl ThresholdAttention {
    /// Create a new threshold-based attention filter.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum salience score to include
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            max_count: None,
        }
    }

    /// Set maximum number of observations.
    pub fn with_max_count(mut self, max: usize) -> Self {
        self.max_count = Some(max);
        self
    }
}

impl AttentionFilter for ThresholdAttention {
    fn filter(&self, data: &ObservationData, salience: Option<&SalienceMap>) -> AttentionResult {
        let scores: Vec<f32> = match salience {
            Some(map) => map.compute_all_salience(data),
            None => data.readings.iter().map(|r| r.confidence).collect(),
        };

        let mut indexed_scores: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score >= self.threshold)
            .collect();

        // Sort by salience for consistent ordering
        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Apply max count if specified
        if let Some(max) = self.max_count {
            indexed_scores.truncate(max);
        }

        let filtered_count = data.readings.len() - indexed_scores.len();
        let capacity_used = if let Some(max) = self.max_count {
            indexed_scores.len() as f32 / max as f32
        } else {
            1.0
        };

        AttentionResult {
            selected_indices: indexed_scores.iter().map(|(i, _)| *i).collect(),
            salience_scores: indexed_scores.iter().map(|(_, s)| *s).collect(),
            capacity_used,
            filtered_count,
        }
    }

    fn capacity(&self) -> usize {
        self.max_count.unwrap_or(usize::MAX)
    }
}

/// Weighted random attention that samples observations proportional to salience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StochasticAttention {
    /// Number of samples to draw.
    sample_count: usize,

    /// Random seed for reproducibility.
    seed: u64,
}

impl StochasticAttention {
    /// Create a new stochastic attention filter.
    ///
    /// # Arguments
    ///
    /// * `sample_count` - Number of observations to sample
    /// * `seed` - Random seed for reproducibility
    pub fn new(sample_count: usize, seed: u64) -> Self {
        Self { sample_count, seed }
    }
}

impl AttentionFilter for StochasticAttention {
    fn filter(&self, data: &ObservationData, salience: Option<&SalienceMap>) -> AttentionResult {
        use rand::{Rng, SeedableRng};
        use rand_xoshiro::Xoshiro256PlusPlus;

        let scores: Vec<f32> = match salience {
            Some(map) => map.compute_all_salience(data),
            None => data.readings.iter().map(|r| r.confidence).collect(),
        };

        if scores.is_empty() {
            return AttentionResult {
                selected_indices: vec![],
                salience_scores: vec![],
                capacity_used: 0.0,
                filtered_count: 0,
            };
        }

        let total: f32 = scores.iter().sum();
        if total <= 0.0 {
            return AttentionResult {
                selected_indices: vec![],
                salience_scores: vec![],
                capacity_used: 0.0,
                filtered_count: data.readings.len(),
            };
        }

        // Normalize scores to probabilities
        let probs: Vec<f32> = scores.iter().map(|s| s / total).collect();

        // Compute cumulative distribution
        let cdf: Vec<f32> = probs
            .iter()
            .scan(0.0, |acc, &p| {
                *acc += p;
                Some(*acc)
            })
            .collect();

        let mut rng = Xoshiro256PlusPlus::seed_from_u64(self.seed);
        let mut selected = Vec::new();
        let mut selected_scores = Vec::new();

        for _ in 0..self.sample_count.min(data.readings.len()) {
            let r: f32 = rng.gen();
            let idx = cdf.iter().position(|&c| c >= r).unwrap_or(cdf.len() - 1);

            if !selected.contains(&idx) {
                selected.push(idx);
                selected_scores.push(scores[idx]);
            }
        }

        let filtered_count = data.readings.len() - selected.len();
        let capacity_used = selected.len() as f32 / self.sample_count as f32;

        AttentionResult {
            selected_indices: selected,
            salience_scores: selected_scores,
            capacity_used,
            filtered_count,
        }
    }

    fn capacity(&self) -> usize {
        self.sample_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::observer::ObserverConfig;

    fn create_test_data() -> ObservationData {
        let readings = vec![
            SensorReading {
                atom_id: 0,
                position: [1.0, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([1.0, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: 1.0,
                angle: 0.0,
                confidence: 0.9,
                atom_type: 0,
            },
            SensorReading {
                atom_id: 1,
                position: [5.0, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([0.1, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: 5.0,
                angle: 0.0,
                confidence: 0.5,
                atom_type: 1,
            },
            SensorReading {
                atom_id: 2,
                position: [3.0, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([5.0, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: 3.0,
                angle: 0.2,
                confidence: 0.7,
                atom_type: 0,
            },
        ];

        ObservationData {
            readings,
            observer_position: [0.0; 3],
            observer_direction: [1.0, 0.0, 0.0],
            config: ObserverConfig::default(),
        }
    }

    #[test]
    fn test_salience_map_distance() {
        let mut salience = SalienceMap::new();
        salience.add_distance_salience(1.0, true);

        let data = create_test_data();
        let scores = salience.compute_all_salience(&data);

        // Closer atoms should have higher salience
        assert!(scores[0] > scores[1]);
    }

    #[test]
    fn test_salience_map_velocity() {
        let mut salience = SalienceMap::new();
        salience.add_velocity_salience(1.0);

        let data = create_test_data();
        let scores = salience.compute_all_salience(&data);

        // Faster atoms should have higher salience
        assert!(scores[2] > scores[1]); // atom 2 is moving at speed 5
    }

    #[test]
    fn test_top_k_attention() {
        let data = create_test_data();
        let attention = TopKAttention::new(2);

        let result = attention.filter(&data, None);

        assert_eq!(result.selected_count(), 2);
        // Should select the two highest confidence readings
        assert!(result.contains(0)); // confidence 0.9
        assert!(result.contains(2)); // confidence 0.7
    }

    #[test]
    fn test_threshold_attention() {
        let data = create_test_data();
        let attention = ThresholdAttention::new(0.6);

        let result = attention.filter(&data, None);

        // Should select readings with confidence >= 0.6
        assert!(result.contains(0)); // confidence 0.9
        assert!(result.contains(2)); // confidence 0.7
        assert!(!result.contains(1)); // confidence 0.5
    }

    #[test]
    fn test_combined_salience() {
        let mut salience = SalienceMap::new();
        salience
            .add_distance_salience(0.5, true)
            .add_velocity_salience(0.5)
            .with_normalization(true);

        let data = create_test_data();
        let scores = salience.compute_all_salience(&data);

        // All scores should be between 0 and 1 with normalization
        for score in &scores {
            assert!(*score >= 0.0 && *score <= 1.0);
        }
    }

    #[test]
    fn test_stochastic_attention_reproducibility() {
        let data = create_test_data();
        let attention = StochasticAttention::new(2, 42);

        let result1 = attention.filter(&data, None);
        let result2 = attention.filter(&data, None);

        // Same seed should produce same results
        assert_eq!(result1.selected_indices, result2.selected_indices);
    }
}
