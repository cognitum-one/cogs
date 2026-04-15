//! Core data types for ThermalBrain

use heapless::Vec as HVec;

/// Maximum non-zero elements in sparse vector
pub const MAX_NNZ: usize = 32;

/// Feature vector dimensions
pub const FEATURE_DIMS: usize = 16;

/// Maximum label length
pub const MAX_LABEL_LEN: usize = 32;

/// Maximum patterns in HNSW
pub const MAX_PATTERNS: usize = 2000;

/// HNSW connections per node
pub const HNSW_M: usize = 8;

/// Thermal zone enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum ThermalZone {
    #[default]
    Cool = 0,
    Warm = 1,
    Hot = 2,
    Critical = 3,
    Emergency = 4,
}

impl ThermalZone {
    /// Get spike threshold for this zone
    pub fn spike_threshold(&self) -> f32 {
        match self {
            Self::Cool => 0.30,
            Self::Warm => 0.50,
            Self::Hot => 0.70,
            Self::Critical => 0.90,
            Self::Emergency => 0.99,
        }
    }

    /// Get refractory period for this zone (ms)
    pub fn refractory_ms(&self) -> u32 {
        match self {
            Self::Cool => 1,
            Self::Warm => 10,
            Self::Hot => 50,
            Self::Critical => 100,
            Self::Emergency => 500,
        }
    }

    /// Get sleep duration for this zone (ms)
    pub fn sleep_ms(&self) -> u32 {
        match self {
            Self::Cool => 1,
            Self::Warm => 10,
            Self::Hot => 50,
            Self::Critical => 100,
            Self::Emergency => 500,
        }
    }

    /// Get zone from temperature (without hysteresis)
    pub fn from_temp(temp_c: f32) -> Self {
        if temp_c >= 70.0 {
            Self::Emergency
        } else if temp_c >= 60.0 {
            Self::Critical
        } else if temp_c >= 50.0 {
            Self::Hot
        } else if temp_c >= 40.0 {
            Self::Warm
        } else {
            Self::Cool
        }
    }
}

/// Sparse vector representation
#[derive(Clone, Debug, Default)]
pub struct SparseVector {
    /// Non-zero indices
    pub indices: HVec<u8, MAX_NNZ>,
    /// Quantized values (int8)
    pub values: HVec<i8, MAX_NNZ>,
    /// Threshold used for sparsification
    pub threshold: f32,
}

impl SparseVector {
    /// Create empty sparse vector
    pub fn new(threshold: f32) -> Self {
        Self {
            indices: HVec::new(),
            values: HVec::new(),
            threshold,
        }
    }

    /// Number of non-zero elements
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Sparsity ratio (0.0 = dense, 1.0 = all zeros)
    pub fn sparsity(&self, total_dims: usize) -> f32 {
        1.0 - (self.nnz() as f32 / total_dims as f32)
    }

    /// Add a non-zero element
    pub fn push(&mut self, index: u8, value: i8) -> Result<(), ()> {
        self.indices.push(index).map_err(|_| ())?;
        self.values.push(value).map_err(|_| ())?;
        Ok(())
    }

    /// Clear all elements
    pub fn clear(&mut self) {
        self.indices.clear();
        self.values.clear();
    }

    /// Compute dot product with another sparse vector
    pub fn dot(&self, other: &SparseVector) -> i32 {
        let mut sum: i32 = 0;
        for (&idx_a, &val_a) in self.indices.iter().zip(self.values.iter()) {
            for (&idx_b, &val_b) in other.indices.iter().zip(other.values.iter()) {
                if idx_a == idx_b {
                    sum += (val_a as i32) * (val_b as i32);
                }
            }
        }
        sum
    }

    /// Compute dot product with dense i8 vector
    pub fn dot_dense(&self, dense: &[i8]) -> i32 {
        let mut sum: i32 = 0;
        for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
            if (idx as usize) < dense.len() {
                sum += (val as i32) * (dense[idx as usize] as i32);
            }
        }
        sum
    }
}

/// Dense feature vector (fixed size)
pub type FeatureVector = [f32; FEATURE_DIMS];

/// Quantized pattern vector (for storage)
pub type PatternVector = [i8; FEATURE_DIMS];

/// Pattern match result
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// Pattern label
    pub label: heapless::String<MAX_LABEL_LEN>,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Pattern ID
    pub pattern_id: u32,
    /// Neuron index that fired
    pub neuron_idx: usize,
}

impl MatchResult {
    /// Create a new match result
    pub fn new(label: &str, confidence: f32, pattern_id: u32, neuron_idx: usize) -> Self {
        let mut label_str = heapless::String::new();
        let _ = label_str.push_str(&label[..label.len().min(MAX_LABEL_LEN - 1)]);
        Self {
            label: label_str,
            confidence,
            pattern_id,
            neuron_idx,
        }
    }
}

/// System status
#[derive(Clone, Debug, Default)]
pub struct SystemStatus {
    /// Current thermal zone
    pub zone: ThermalZone,
    /// Current temperature (°C)
    pub temperature_c: f32,
    /// Current spike threshold
    pub spike_threshold: f32,
    /// Patterns stored
    pub pattern_count: usize,
    /// Total inferences
    pub inference_count: u64,
    /// Total spikes
    pub spike_count: u64,
}

/// HNSW index statistics
#[derive(Clone, Debug, Default)]
pub struct HnswStats {
    /// Number of vectors stored
    pub num_vectors: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Approximate memory usage in bytes
    pub memory_bytes: usize,
    /// Average search hops
    pub avg_search_hops: f32,
}

/// Pattern information
#[derive(Clone, Debug)]
pub struct PatternInfo {
    /// Pattern ID
    pub id: u32,
    /// Pattern label
    pub label: heapless::String<MAX_LABEL_LEN>,
    /// Match count
    pub match_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_zone() {
        assert_eq!(ThermalZone::Cool.spike_threshold(), 0.30);
        assert_eq!(ThermalZone::Emergency.refractory_ms(), 500);
        assert_eq!(ThermalZone::from_temp(35.0), ThermalZone::Cool);
        assert_eq!(ThermalZone::from_temp(45.0), ThermalZone::Warm);
        assert_eq!(ThermalZone::from_temp(75.0), ThermalZone::Emergency);
    }

    #[test]
    fn test_sparse_vector() {
        let mut sv = SparseVector::new(0.5);
        sv.push(0, 100).unwrap();
        sv.push(3, -50).unwrap();
        sv.push(7, 75).unwrap();

        assert_eq!(sv.nnz(), 3);
        assert!(sv.sparsity(16) > 0.8);
    }

    #[test]
    fn test_sparse_dot_dense() {
        let mut sv = SparseVector::new(0.5);
        sv.push(0, 2).unwrap();
        sv.push(1, 3).unwrap();

        let dense = [4i8, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // 2*4 + 3*5 = 8 + 15 = 23
        assert_eq!(sv.dot_dense(&dense), 23);
    }
}
