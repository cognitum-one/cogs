//! Sparse vector encoding

use crate::types::{FeatureVector, PatternVector, SparseVector, FEATURE_DIMS};
use libm::fabsf;

/// Sparse encoder - converts dense vectors to sparse representation
pub struct SparseEncoder {
    last_sparse: SparseVector,
}

impl Default for SparseEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SparseEncoder {
    /// Create a new sparse encoder
    pub fn new() -> Self {
        Self {
            last_sparse: SparseVector::new(0.5),
        }
    }

    /// Encode a dense feature vector to sparse representation
    ///
    /// # Arguments
    /// * `dense` - Dense feature vector
    /// * `threshold` - Sparsification threshold (values below this are zeroed)
    ///
    /// # Returns
    /// Sparse vector with only significant elements
    pub fn encode(&mut self, dense: &FeatureVector, threshold: f32) -> SparseVector {
        let mut sparse = SparseVector::new(threshold);

        for (i, &val) in dense.iter().enumerate() {
            if fabsf(val) > threshold {
                // Quantize to int8 (-127 to +127)
                let quantized = (val.clamp(-1.0, 1.0) * 127.0) as i8;
                if sparse.push(i as u8, quantized).is_err() {
                    // Max capacity reached
                    break;
                }
            }
        }

        self.last_sparse = sparse.clone();
        sparse
    }

    /// Quantize a dense feature vector to pattern vector (i8)
    pub fn quantize(&self, dense: &FeatureVector) -> PatternVector {
        let mut pattern = [0i8; FEATURE_DIMS];
        for (i, &val) in dense.iter().enumerate() {
            pattern[i] = (val.clamp(-1.0, 1.0) * 127.0) as i8;
        }
        pattern
    }

    /// Dequantize a pattern vector back to f32
    pub fn dequantize(&self, pattern: &PatternVector) -> FeatureVector {
        let mut dense = [0.0f32; FEATURE_DIMS];
        for (i, &val) in pattern.iter().enumerate() {
            dense[i] = val as f32 / 127.0;
        }
        dense
    }

    /// Get the last sparse encoding
    pub fn last_sparse(&self) -> &SparseVector {
        &self.last_sparse
    }

    /// Calculate sparsity of a dense vector at given threshold
    pub fn calculate_sparsity(dense: &FeatureVector, threshold: f32) -> f32 {
        let non_zero = dense.iter().filter(|&&x| fabsf(x) > threshold).count();
        1.0 - (non_zero as f32 / FEATURE_DIMS as f32)
    }

    /// Sparse dot product between two sparse vectors
    pub fn sparse_dot(a: &SparseVector, b: &SparseVector) -> i32 {
        a.dot(b)
    }

    /// Sparse-dense dot product
    pub fn sparse_dense_dot(sparse: &SparseVector, dense: &PatternVector) -> i32 {
        sparse.dot_dense(dense)
    }

    /// Cosine similarity between sparse vectors (approximate)
    pub fn sparse_cosine(a: &SparseVector, b: &SparseVector) -> f32 {
        let dot = Self::sparse_dot(a, b) as f32;

        // Calculate magnitudes
        let mag_a: f32 = a.values.iter().map(|&x| (x as f32) * (x as f32)).sum();
        let mag_b: f32 = b.values.iter().map(|&x| (x as f32) * (x as f32)).sum();

        let denom = libm::sqrtf(mag_a) * libm::sqrtf(mag_b);
        if denom < 1e-10 {
            return 0.0;
        }

        dot / denom
    }
}

/// Sparse vector utilities for batch operations
pub struct SparseBatch;

impl SparseBatch {
    /// Find top-k most similar patterns to query
    pub fn top_k(
        query: &SparseVector,
        patterns: &[PatternVector],
        k: usize,
    ) -> heapless::Vec<(usize, f32), 16> {
        let mut scores: heapless::Vec<(usize, f32), 64> = heapless::Vec::new();

        for (i, pattern) in patterns.iter().enumerate() {
            let sim = query.dot_dense(pattern) as f32;
            let _ = scores.push((i, sim));
        }

        // Sort by similarity (descending)
        // Simple bubble sort for small k (heapless doesn't have sort)
        for i in 0..scores.len() {
            for j in i + 1..scores.len() {
                if scores[j].1 > scores[i].1 {
                    scores.swap(i, j);
                }
            }
        }

        // Take top k
        let mut result = heapless::Vec::new();
        for &item in scores.iter().take(k) {
            let _ = result.push(item);
        }
        result
    }

    /// Calculate similarity matrix for a set of sparse vectors
    pub fn similarity_matrix(
        vectors: &[SparseVector],
        max_size: usize,
    ) -> heapless::Vec<heapless::Vec<f32, 64>, 64> {
        let n = vectors.len().min(max_size);
        let mut matrix = heapless::Vec::new();

        for i in 0..n {
            let mut row = heapless::Vec::new();
            for j in 0..n {
                let sim = SparseEncoder::sparse_cosine(&vectors[i], &vectors[j]);
                let _ = row.push(sim);
            }
            let _ = matrix.push(row);
        }

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_encode() {
        let encoder = SparseEncoder::new();
        let dense: FeatureVector = [0.8, 0.1, -0.6, 0.3, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let mut enc = SparseEncoder::new();
        let sparse = enc.encode(&dense, 0.5);

        // Should only include values > 0.5 or < -0.5
        assert_eq!(sparse.nnz(), 3); // 0.8, -0.6, 0.9

        // Check indices
        assert!(sparse.indices.contains(&0)); // 0.8
        assert!(sparse.indices.contains(&2)); // -0.6
        assert!(sparse.indices.contains(&4)); // 0.9
    }

    #[test]
    fn test_quantize_dequantize() {
        let encoder = SparseEncoder::new();
        let dense: FeatureVector = [0.5, -0.3, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let pattern = encoder.quantize(&dense);
        let recovered = encoder.dequantize(&pattern);

        // Should be approximately equal (within quantization error)
        for (a, b) in dense.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 0.02, "Quantization error too large: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_sparse_dot() {
        let mut a = SparseVector::new(0.5);
        a.push(0, 10).unwrap();
        a.push(2, 20).unwrap();

        let mut b = SparseVector::new(0.5);
        b.push(0, 5).unwrap();
        b.push(2, 3).unwrap();
        b.push(3, 7).unwrap(); // No match

        // dot = 10*5 + 20*3 = 50 + 60 = 110
        assert_eq!(SparseEncoder::sparse_dot(&a, &b), 110);
    }

    #[test]
    fn test_sparsity() {
        let dense: FeatureVector = [0.8, 0.1, 0.6, 0.3, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let sparsity_05 = SparseEncoder::calculate_sparsity(&dense, 0.5);
        // 3 values > 0.5 (0.8, 0.6, 0.9), so 13 zeros -> sparsity = 13/16 = 0.8125
        assert!((sparsity_05 - 0.8125).abs() < 0.01);
    }

    #[test]
    fn test_top_k() {
        let mut query = SparseVector::new(0.5);
        query.push(0, 100).unwrap();
        query.push(1, 50).unwrap();

        let patterns: [PatternVector; 3] = [
            [100, 50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // Perfect match
            [50, 25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],  // Half match
            [-100, -50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // Anti-correlated
        ];

        let top = SparseBatch::top_k(&query, &patterns, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, 0); // Best match is pattern 0
    }
}
