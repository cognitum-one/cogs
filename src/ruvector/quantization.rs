//! Vector Quantization Module for Ruvector
//!
//! Provides scalar and product quantization for memory-efficient vector storage
//! with SIMD-accelerated distance computation and quantized HNSW indexing.
//!
//! # Features
//!
//! - **Scalar Quantization (SQ8)**: 4x memory compression using 8-bit quantization
//! - **Product Quantization (PQ)**: 16-32x compression using learned codebooks
//! - **SIMD Distance**: Fast asymmetric distance computation
//! - **Quantized HNSW**: Memory-efficient nearest neighbor search
//! - **Memory Estimation**: Calculate memory usage for different quantization types

use crate::ruvector::types::*;
use crate::ruvector::index::VectorIndex;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// ============================================================================
// Scalar Quantization (SQ8) - 4x Compression
// ============================================================================

/// Scalar quantizer using 8-bit quantization (4x memory compression)
///
/// Maps floating-point vectors to 8-bit integers using linear scaling:
/// `quantized = (value - min) / (max - min) * 255`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarQuantizer {
    /// Minimum value across all dimensions
    pub min_val: f32,
    /// Maximum value across all dimensions
    pub max_val: f32,
    /// Scale factor for quantization
    pub scale: f32,
    /// Dimension of vectors
    dimension: usize,
}

impl ScalarQuantizer {
    /// Create a new scalar quantizer
    pub fn new(dimension: usize) -> Self {
        Self {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 255.0,
            dimension,
        }
    }

    /// Fit the quantizer to a set of vectors by computing min/max values
    ///
    /// # Arguments
    ///
    /// * `vectors` - Training vectors to fit the quantizer
    pub fn fit(&mut self, vectors: &[Embedding]) {
        if vectors.is_empty() {
            return;
        }

        // Find global min and max
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;

        for vec in vectors {
            for &val in &vec.data {
                min = min.min(val);
                max = max.max(val);
            }
        }

        self.min_val = min;
        self.max_val = max;

        // Compute scale factor with epsilon to avoid division by zero
        let range = (max - min).max(1e-8);
        self.scale = range / 255.0;
    }

    /// Quantize a vector to 8-bit representation
    ///
    /// # Arguments
    ///
    /// * `vector` - Input vector to quantize
    ///
    /// # Returns
    ///
    /// Quantized vector with 8-bit values
    pub fn quantize(&self, vector: &Embedding) -> QuantizedVector {
        let data: Vec<u8> = vector
            .data
            .iter()
            .map(|&val| {
                let normalized = ((val - self.min_val) / self.scale).clamp(0.0, 255.0);
                normalized as u8
            })
            .collect();

        QuantizedVector {
            data,
            original_dim: vector.dimension(),
        }
    }

    /// Dequantize a vector back to floating-point representation
    ///
    /// # Arguments
    ///
    /// * `quantized` - Quantized vector to dequantize
    ///
    /// # Returns
    ///
    /// Approximate reconstruction of the original vector
    pub fn dequantize(&self, quantized: &QuantizedVector) -> Embedding {
        let data: Vec<f32> = quantized
            .data
            .iter()
            .map(|&val| (val as f32) * self.scale + self.min_val)
            .collect();

        Embedding::new(data)
    }

    /// Get the dimension of vectors this quantizer handles
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Quantized vector using 8-bit values (SQ8)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedVector {
    /// 8-bit quantized values
    pub data: Vec<u8>,
    /// Original dimension before quantization
    pub original_dim: usize,
}

impl QuantizedVector {
    /// Compute approximate L2 distance between quantized vectors
    ///
    /// This is less accurate than dequantizing first, but much faster
    pub fn l2_distance(&self, other: &QuantizedVector) -> f32 {
        self.data
            .iter()
            .zip(&other.data)
            .map(|(&a, &b)| {
                let diff = (a as i16 - b as i16) as f32;
                diff * diff
            })
            .sum::<f32>()
            .sqrt()
    }

    /// Get memory size in bytes
    pub fn memory_bytes(&self) -> usize {
        self.data.len() + std::mem::size_of::<usize>()
    }
}

// ============================================================================
// Product Quantization (PQ) - 16-32x Compression
// ============================================================================

/// Product quantizer using learned codebooks (16-32x memory compression)
///
/// Divides vector space into subspaces and learns a codebook for each.
/// Uses k-means clustering to learn optimal centroids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductQuantizer {
    /// Number of subspaces to divide vector into
    pub num_subspaces: usize,
    /// Number of centroids per subspace (typically 256 for 8-bit codes)
    pub num_centroids: usize,
    /// Learned codebooks: [subspace][centroid][dimension]
    pub codebooks: Vec<Vec<Vec<f32>>>,
    /// Dimension per subspace
    subspace_dim: usize,
    /// Total vector dimension
    total_dim: usize,
}

impl ProductQuantizer {
    /// Create a new product quantizer
    ///
    /// # Arguments
    ///
    /// * `dimension` - Total vector dimension
    /// * `num_subspaces` - Number of subspaces (typically 8-16)
    /// * `num_centroids` - Number of centroids per subspace (typically 256)
    pub fn new(dimension: usize, num_subspaces: usize, num_centroids: usize) -> Self {
        assert!(
            dimension % num_subspaces == 0,
            "Dimension must be divisible by num_subspaces"
        );

        let subspace_dim = dimension / num_subspaces;

        Self {
            num_subspaces,
            num_centroids,
            codebooks: vec![vec![vec![0.0; subspace_dim]; num_centroids]; num_subspaces],
            subspace_dim,
            total_dim: dimension,
        }
    }

    /// Fit the quantizer using k-means clustering
    ///
    /// # Arguments
    ///
    /// * `vectors` - Training vectors to fit the quantizer
    /// * `iterations` - Number of k-means iterations (typically 20-50)
    pub fn fit(&mut self, vectors: &[Embedding], iterations: usize) {
        if vectors.is_empty() {
            return;
        }

        // Train each subspace independently
        for subspace_idx in 0..self.num_subspaces {
            let start_dim = subspace_idx * self.subspace_dim;
            let end_dim = start_dim + self.subspace_dim;

            // Extract subspace vectors
            let subspace_vectors: Vec<Vec<f32>> = vectors
                .iter()
                .map(|v| v.data[start_dim..end_dim].to_vec())
                .collect();

            // Run k-means
            let centroids = self.kmeans(&subspace_vectors, iterations);
            self.codebooks[subspace_idx] = centroids;
        }
    }

    /// K-means clustering for a single subspace
    fn kmeans(&self, vectors: &[Vec<f32>], iterations: usize) -> Vec<Vec<f32>> {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        if vectors.is_empty() {
            return vec![vec![0.0; self.subspace_dim]; self.num_centroids];
        }

        // Initialize centroids randomly from data points
        let mut rng = thread_rng();
        let mut centroids: Vec<Vec<f32>> = vectors
            .choose_multiple(&mut rng, self.num_centroids.min(vectors.len()))
            .cloned()
            .collect();

        // Pad if we don't have enough vectors
        while centroids.len() < self.num_centroids {
            centroids.push(vec![0.0; self.subspace_dim]);
        }

        // K-means iterations
        for _ in 0..iterations {
            // Assignment step
            let mut assignments = vec![Vec::new(); self.num_centroids];

            for vector in vectors {
                let nearest_idx = self.find_nearest_centroid(vector, &centroids);
                assignments[nearest_idx].push(vector.clone());
            }

            // Update step
            for (centroid_idx, assigned) in assignments.iter().enumerate() {
                if !assigned.is_empty() {
                    // Compute mean of assigned vectors
                    let mut new_centroid = vec![0.0; self.subspace_dim];
                    for vector in assigned {
                        for (i, &val) in vector.iter().enumerate() {
                            new_centroid[i] += val;
                        }
                    }
                    for val in &mut new_centroid {
                        *val /= assigned.len() as f32;
                    }
                    centroids[centroid_idx] = new_centroid;
                }
            }
        }

        centroids
    }

    /// Find the nearest centroid to a vector
    fn find_nearest_centroid(&self, vector: &[f32], centroids: &[Vec<f32>]) -> usize {
        centroids
            .iter()
            .enumerate()
            .map(|(idx, centroid)| {
                let dist: f32 = vector
                    .iter()
                    .zip(centroid)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum();
                (idx, dist)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    /// Quantize a vector to a PQ code
    ///
    /// # Arguments
    ///
    /// * `vector` - Input vector to quantize
    ///
    /// # Returns
    ///
    /// PQ code containing centroid indices for each subspace
    pub fn quantize(&self, vector: &Embedding) -> PQCode {
        let mut codes = Vec::with_capacity(self.num_subspaces);

        for subspace_idx in 0..self.num_subspaces {
            let start_dim = subspace_idx * self.subspace_dim;
            let end_dim = start_dim + self.subspace_dim;

            let subvector = &vector.data[start_dim..end_dim];
            let nearest_idx =
                self.find_nearest_centroid(subvector, &self.codebooks[subspace_idx]);

            codes.push(nearest_idx as u8);
        }

        PQCode { codes }
    }

    /// Compute asymmetric distance between query vector and PQ code
    ///
    /// This is faster than symmetric distance and provides better accuracy
    /// for nearest neighbor search.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector (not quantized)
    /// * `code` - PQ code of database vector
    ///
    /// # Returns
    ///
    /// Approximate L2 distance
    pub fn asymmetric_distance(&self, query: &Embedding, code: &PQCode) -> f32 {
        let mut distance = 0.0;

        for subspace_idx in 0..self.num_subspaces {
            let start_dim = subspace_idx * self.subspace_dim;
            let end_dim = start_dim + self.subspace_dim;

            let query_subvector = &query.data[start_dim..end_dim];
            let centroid_idx = code.codes[subspace_idx] as usize;
            let centroid = &self.codebooks[subspace_idx][centroid_idx];

            // Compute subspace distance
            let subspace_dist: f32 = query_subvector
                .iter()
                .zip(centroid)
                .map(|(q, c)| (q - c) * (q - c))
                .sum();

            distance += subspace_dist;
        }

        distance.sqrt()
    }

    /// Precompute distance tables for fast asymmetric search
    ///
    /// For each subspace and centroid, compute the distance from the query
    /// subvector to that centroid. This enables O(num_subspaces) distance
    /// computation instead of O(dimension).
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    ///
    /// # Returns
    ///
    /// Precomputed distance tables
    pub fn precompute_tables(&self, query: &Embedding) -> DistanceTables {
        let mut tables = vec![vec![0.0; self.num_centroids]; self.num_subspaces];

        for subspace_idx in 0..self.num_subspaces {
            let start_dim = subspace_idx * self.subspace_dim;
            let end_dim = start_dim + self.subspace_dim;

            let query_subvector = &query.data[start_dim..end_dim];

            for centroid_idx in 0..self.num_centroids {
                let centroid = &self.codebooks[subspace_idx][centroid_idx];

                // Compute squared distance
                let dist: f32 = query_subvector
                    .iter()
                    .zip(centroid)
                    .map(|(q, c)| (q - c) * (q - c))
                    .sum();

                tables[subspace_idx][centroid_idx] = dist;
            }
        }

        DistanceTables { tables }
    }

    /// Compute distance using precomputed tables (O(num_subspaces))
    ///
    /// # Arguments
    ///
    /// * `tables` - Precomputed distance tables from query
    /// * `code` - PQ code of database vector
    ///
    /// # Returns
    ///
    /// Approximate L2 distance
    #[inline]
    pub fn fast_distance(&self, tables: &DistanceTables, code: &PQCode) -> f32 {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { self.fast_distance_simd(tables, code) };
            }
        }

        self.fast_distance_scalar(tables, code)
    }

    /// Scalar implementation of fast distance
    #[inline]
    fn fast_distance_scalar(&self, tables: &DistanceTables, code: &PQCode) -> f32 {
        code.codes
            .iter()
            .enumerate()
            .map(|(subspace_idx, &centroid_idx)| {
                tables.tables[subspace_idx][centroid_idx as usize]
            })
            .sum::<f32>()
            .sqrt()
    }

    /// SIMD-accelerated distance computation (x86_64 AVX2)
    #[cfg(target_arch = "x86_64")]
    #[inline]
    unsafe fn fast_distance_simd(&self, tables: &DistanceTables, code: &PQCode) -> f32 {
        let mut sum = _mm256_setzero_ps();

        // Process 8 subspaces at a time
        let chunks = code.codes.chunks(8);
        for (chunk_idx, chunk) in chunks.enumerate() {
            let base_idx = chunk_idx * 8;

            // Gather distances for this chunk
            let mut distances = [0.0f32; 8];
            for (i, &centroid_idx) in chunk.iter().enumerate() {
                let subspace_idx = base_idx + i;
                if subspace_idx < self.num_subspaces {
                    distances[i] = tables.tables[subspace_idx][centroid_idx as usize];
                }
            }

            // Load and accumulate
            let vec = _mm256_loadu_ps(distances.as_ptr());
            sum = _mm256_add_ps(sum, vec);
        }

        // Horizontal sum
        let sum_high = _mm256_extractf128_ps(sum, 1);
        let sum_low = _mm256_castps256_ps128(sum);
        let sum_128 = _mm_add_ps(sum_low, sum_high);

        let mut result = [0.0f32; 4];
        _mm_storeu_ps(result.as_mut_ptr(), sum_128);

        let total: f32 = result.iter().sum();
        total.sqrt()
    }

    /// Get the total dimension
    pub fn dimension(&self) -> usize {
        self.total_dim
    }
}

/// Product quantization code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQCode {
    /// Centroid indices for each subspace
    pub codes: Vec<u8>,
}

impl PQCode {
    /// Get memory size in bytes
    pub fn memory_bytes(&self) -> usize {
        self.codes.len()
    }
}

/// Precomputed distance tables for fast asymmetric search
#[derive(Debug, Clone)]
pub struct DistanceTables {
    /// Distance tables: [subspace][centroid]
    tables: Vec<Vec<f32>>,
}

// ============================================================================
// Quantized HNSW Index
// ============================================================================

/// HNSW index with product quantization for memory efficiency
///
/// Combines HNSW graph structure with PQ codes for storage.
/// Uses asymmetric distance computation during search.
pub struct QuantizedHnswIndex {
    /// Product quantizer
    quantizer: ProductQuantizer,
    /// PQ codes for stored vectors
    codes: Arc<RwLock<HashMap<EmbeddingId, PQCode>>>,
    /// Metadata storage
    metadata: Arc<RwLock<HashMap<EmbeddingId, Metadata>>>,
    /// HNSW graph structure (simplified for now)
    neighbors: Arc<RwLock<HashMap<EmbeddingId, Vec<EmbeddingId>>>>,
    /// Configuration (retained for runtime inspection)
    #[allow(dead_code)]
    config: QuantizedHnswConfig,
}

/// Configuration for quantized HNSW index
#[derive(Debug, Clone)]
pub struct QuantizedHnswConfig {
    /// Number of subspaces for PQ
    pub num_subspaces: usize,
    /// Number of centroids per subspace
    pub num_centroids: usize,
    /// HNSW parameter M (number of neighbors)
    pub m: usize,
    /// Search parameter ef
    pub ef_search: usize,
}

impl Default for QuantizedHnswConfig {
    fn default() -> Self {
        Self {
            num_subspaces: 8,
            num_centroids: 256,
            m: 16,
            ef_search: 50,
        }
    }
}

impl QuantizedHnswIndex {
    /// Create a new quantized HNSW index
    ///
    /// # Arguments
    ///
    /// * `dimension` - Vector dimension
    /// * `config` - Index configuration
    pub fn new(dimension: usize, config: QuantizedHnswConfig) -> Self {
        let quantizer = ProductQuantizer::new(
            dimension,
            config.num_subspaces,
            config.num_centroids,
        );

        Self {
            quantizer,
            codes: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            neighbors: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Train the quantizer on a set of vectors
    ///
    /// # Arguments
    ///
    /// * `vectors` - Training vectors
    /// * `iterations` - Number of k-means iterations
    pub fn train(&mut self, vectors: &[Embedding], iterations: usize) {
        self.quantizer.fit(vectors, iterations);
    }

    /// Get the quantizer (for external use)
    pub fn quantizer(&self) -> &ProductQuantizer {
        &self.quantizer
    }
}

impl VectorIndex for QuantizedHnswIndex {
    fn insert(
        &mut self,
        id: EmbeddingId,
        embedding: &Embedding,
        metadata: &Metadata,
    ) -> Result<(), IndexError> {
        if embedding.dimension() != self.quantizer.dimension() {
            return Err(IndexError::InvalidDimension {
                expected: self.quantizer.dimension(),
                actual: embedding.dimension(),
            });
        }

        // Quantize the vector
        let code = self.quantizer.quantize(embedding);

        // Store code and metadata
        self.codes.write().insert(id, code);
        self.metadata.write().insert(id, metadata.clone());

        // Initialize neighbor list (simplified HNSW)
        self.neighbors.write().insert(id, Vec::new());

        Ok(())
    }

    fn search(&self, query: &Embedding, k: usize) -> Result<Vec<SearchResult>, IndexError> {
        if query.dimension() != self.quantizer.dimension() {
            return Err(IndexError::InvalidDimension {
                expected: self.quantizer.dimension(),
                actual: query.dimension(),
            });
        }

        // Precompute distance tables for fast search
        let tables = self.quantizer.precompute_tables(query);

        let codes = self.codes.read();
        let metadata = self.metadata.read();

        // Compute distances for all vectors
        let mut results: Vec<(EmbeddingId, f32)> = codes
            .iter()
            .map(|(id, code)| {
                let distance = self.quantizer.fast_distance(&tables, code);
                (*id, distance)
            })
            .collect();

        // Sort by distance (ascending)
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Convert to search results (convert distance to similarity)
        Ok(results
            .into_iter()
            .take(k)
            .map(|(id, distance)| SearchResult {
                id,
                similarity: 1.0 / (1.0 + distance), // Convert distance to similarity
                metadata: metadata.get(&id).cloned().unwrap_or_default(),
            })
            .collect())
    }

    fn delete(&mut self, id: EmbeddingId) -> Result<(), IndexError> {
        self.codes
            .write()
            .remove(&id)
            .ok_or_else(|| IndexError::NotFound(format!("Embedding {:?} not found", id)))?;
        self.metadata.write().remove(&id);
        self.neighbors.write().remove(&id);
        Ok(())
    }

    fn stats(&self) -> IndexStats {
        let codes = self.codes.read();
        let num_vectors = codes.len();

        // Calculate memory usage
        let code_bytes: usize = codes.values().map(|c| c.memory_bytes()).sum();
        let metadata_bytes = num_vectors * std::mem::size_of::<Metadata>();
        let codebook_bytes = self.quantizer.num_subspaces
            * self.quantizer.num_centroids
            * self.quantizer.subspace_dim
            * std::mem::size_of::<f32>();

        IndexStats {
            num_vectors,
            dimension: self.quantizer.dimension(),
            memory_bytes: code_bytes + metadata_bytes + codebook_bytes,
        }
    }

    fn optimize(&mut self) -> Result<(), IndexError> {
        // In a full implementation, this would rebuild the HNSW graph
        Ok(())
    }
}

// ============================================================================
// Memory Estimation
// ============================================================================

/// Quantization type for memory estimation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationType {
    /// No quantization (4 bytes per dimension)
    None,
    /// Scalar quantization (1 byte per dimension) - 4x compression
    SQ8,
    /// Product quantization with 8 subspaces (8 bytes total) - 16-32x compression
    PQ8x256,
    /// Product quantization with 16 subspaces (16 bytes total) - 8-16x compression
    PQ16x256,
}

/// Memory usage estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEstimate {
    /// Total memory in bytes
    pub total_bytes: usize,
    /// Memory per vector in bytes
    pub bytes_per_vector: usize,
    /// Compression ratio compared to uncompressed
    pub compression_ratio: f32,
    /// Codebook size (for PQ)
    pub codebook_bytes: usize,
}

impl MemoryEstimate {
    /// Format as human-readable string
    pub fn format_size(&self) -> String {
        let (size, unit) = if self.total_bytes < 1024 {
            (self.total_bytes as f64, "B")
        } else if self.total_bytes < 1024 * 1024 {
            (self.total_bytes as f64 / 1024.0, "KB")
        } else if self.total_bytes < 1024 * 1024 * 1024 {
            (self.total_bytes as f64 / (1024.0 * 1024.0), "MB")
        } else {
            (self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0), "GB")
        };

        format!("{:.2} {}", size, unit)
    }
}

/// Estimate memory usage for different quantization types
///
/// # Arguments
///
/// * `num_vectors` - Number of vectors to store
/// * `dimension` - Vector dimension
/// * `quantization` - Quantization type
///
/// # Returns
///
/// Memory usage estimate
pub fn estimate_memory(
    num_vectors: usize,
    dimension: usize,
    quantization: QuantizationType,
) -> MemoryEstimate {
    let uncompressed_bytes = num_vectors * dimension * std::mem::size_of::<f32>();

    let (bytes_per_vector, codebook_bytes) = match quantization {
        QuantizationType::None => (dimension * std::mem::size_of::<f32>(), 0),

        QuantizationType::SQ8 => {
            // 1 byte per dimension + quantizer overhead
            let vec_bytes = dimension;
            let quantizer_bytes = 3 * std::mem::size_of::<f32>(); // min, max, scale
            (vec_bytes, quantizer_bytes)
        }

        QuantizationType::PQ8x256 => {
            // 8 bytes for codes + codebook
            let vec_bytes = 8;
            let subspace_dim = dimension / 8;
            let codebook_size = 8 * 256 * subspace_dim * std::mem::size_of::<f32>();
            (vec_bytes, codebook_size)
        }

        QuantizationType::PQ16x256 => {
            // 16 bytes for codes + codebook
            let vec_bytes = 16;
            let subspace_dim = dimension / 16;
            let codebook_size = 16 * 256 * subspace_dim * std::mem::size_of::<f32>();
            (vec_bytes, codebook_size)
        }
    };

    let total_bytes = num_vectors * bytes_per_vector + codebook_bytes;
    let compression_ratio = uncompressed_bytes as f32 / total_bytes as f32;

    MemoryEstimate {
        total_bytes,
        bytes_per_vector,
        compression_ratio,
        codebook_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_quantizer_fit() {
        let vectors = vec![
            Embedding::new(vec![0.0, 1.0, 2.0, 3.0]),
            Embedding::new(vec![-1.0, 0.5, 1.5, 2.5]),
            Embedding::new(vec![0.5, 1.5, 2.5, 3.5]),
        ];

        let mut quantizer = ScalarQuantizer::new(4);
        quantizer.fit(&vectors);

        assert_eq!(quantizer.min_val, -1.0);
        assert_eq!(quantizer.max_val, 3.5);
    }

    #[test]
    fn test_scalar_quantizer_roundtrip() {
        let vector = Embedding::new(vec![0.5, 1.0, 1.5, 2.0]);

        let mut quantizer = ScalarQuantizer::new(4);
        quantizer.fit(&[vector.clone()]);

        let quantized = quantizer.quantize(&vector);
        let dequantized = quantizer.dequantize(&quantized);

        // Check approximate equality (quantization introduces error)
        for (orig, deq) in vector.data.iter().zip(&dequantized.data) {
            assert!((orig - deq).abs() < 0.1);
        }
    }

    #[test]
    fn test_quantized_vector_distance() {
        let v1 = QuantizedVector {
            data: vec![0, 128, 255],
            original_dim: 3,
        };
        let v2 = QuantizedVector {
            data: vec![10, 120, 250],
            original_dim: 3,
        };

        let distance = v1.l2_distance(&v2);
        assert!(distance > 0.0);
        assert!(distance < 300.0); // Reasonable bound
    }

    #[test]
    fn test_product_quantizer_new() {
        let pq = ProductQuantizer::new(256, 8, 256);

        assert_eq!(pq.num_subspaces, 8);
        assert_eq!(pq.num_centroids, 256);
        assert_eq!(pq.subspace_dim, 32);
        assert_eq!(pq.total_dim, 256);
        assert_eq!(pq.codebooks.len(), 8);
        assert_eq!(pq.codebooks[0].len(), 256);
        assert_eq!(pq.codebooks[0][0].len(), 32);
    }

    #[test]
    fn test_product_quantizer_fit() {
        let vectors: Vec<Embedding> = (0..100)
            .map(|_| Embedding::random(64))
            .collect();

        let mut pq = ProductQuantizer::new(64, 4, 16);
        pq.fit(&vectors, 10);

        // Check that codebooks were updated
        for subspace in &pq.codebooks {
            for centroid in subspace {
                // At least one value should be non-zero
                assert!(centroid.iter().any(|&x| x != 0.0));
            }
        }
    }

    #[test]
    fn test_product_quantizer_quantize() {
        let vector = Embedding::random(128);
        let mut pq = ProductQuantizer::new(128, 8, 256);

        // Fit on a small dataset
        let vectors: Vec<Embedding> = (0..50)
            .map(|_| Embedding::random(128))
            .collect();
        pq.fit(&vectors, 5);

        let code = pq.quantize(&vector);

        assert_eq!(code.codes.len(), 8);
        for &c in &code.codes {
            assert!((c as usize) < 256);
        }
    }

    #[test]
    fn test_asymmetric_distance() {
        let mut pq = ProductQuantizer::new(64, 4, 16);

        // Fit on some data
        let vectors: Vec<Embedding> = (0..50)
            .map(|_| Embedding::random(64))
            .collect();
        pq.fit(&vectors, 5);

        let query = Embedding::random(64);
        let vector = Embedding::random(64);
        let code = pq.quantize(&vector);

        let distance = pq.asymmetric_distance(&query, &code);
        assert!(distance >= 0.0);
        assert!(distance.is_finite());
    }

    #[test]
    fn test_precompute_tables() {
        let mut pq = ProductQuantizer::new(64, 4, 16);

        let vectors: Vec<Embedding> = (0..50)
            .map(|_| Embedding::random(64))
            .collect();
        pq.fit(&vectors, 5);

        let query = Embedding::random(64);
        let tables = pq.precompute_tables(&query);

        assert_eq!(tables.tables.len(), 4);
        assert_eq!(tables.tables[0].len(), 16);
    }

    #[test]
    fn test_fast_distance_matches_asymmetric() {
        let mut pq = ProductQuantizer::new(64, 4, 16);

        let vectors: Vec<Embedding> = (0..50)
            .map(|_| Embedding::random(64))
            .collect();
        pq.fit(&vectors, 5);

        let query = Embedding::random(64);
        let vector = Embedding::random(64);
        let code = pq.quantize(&vector);

        let tables = pq.precompute_tables(&query);

        let dist1 = pq.asymmetric_distance(&query, &code);
        let dist2 = pq.fast_distance(&tables, &code);

        // Should be very close (within floating point error)
        assert!((dist1 - dist2).abs() < 0.001);
    }

    #[test]
    fn test_quantized_hnsw_insert_search() {
        let config = QuantizedHnswConfig {
            num_subspaces: 4,
            num_centroids: 16,
            m: 8,
            ef_search: 20,
        };

        let mut index = QuantizedHnswIndex::new(64, config);

        // Train the quantizer
        let training_vectors: Vec<Embedding> = (0..100)
            .map(|_| Embedding::random(64))
            .collect();
        index.train(&training_vectors, 10);

        // Insert vectors
        for i in 0..20 {
            let embedding = Embedding::random(64);
            let metadata = Metadata::default();
            index
                .insert(EmbeddingId(i), &embedding, &metadata)
                .unwrap();
        }

        assert_eq!(index.stats().num_vectors, 20);

        // Search
        let query = Embedding::random(64);
        let results = index.search(&query, 5).unwrap();

        assert_eq!(results.len(), 5);

        // Check that results are sorted by similarity (descending)
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn test_quantized_hnsw_delete() {
        let config = QuantizedHnswConfig::default();
        let mut index = QuantizedHnswIndex::new(64, config);

        // Train
        let training: Vec<Embedding> = (0..50).map(|_| Embedding::random(64)).collect();
        index.train(&training, 5);

        // Insert
        let embedding = Embedding::random(64);
        index
            .insert(EmbeddingId(1), &embedding, &Metadata::default())
            .unwrap();

        assert_eq!(index.stats().num_vectors, 1);

        // Delete
        index.delete(EmbeddingId(1)).unwrap();
        assert_eq!(index.stats().num_vectors, 0);
    }

    #[test]
    fn test_memory_estimation_none() {
        let estimate = estimate_memory(10000, 256, QuantizationType::None);

        assert_eq!(estimate.bytes_per_vector, 256 * 4);
        assert_eq!(estimate.total_bytes, 10000 * 256 * 4);
        assert_eq!(estimate.compression_ratio, 1.0);
        assert_eq!(estimate.codebook_bytes, 0);
    }

    #[test]
    fn test_memory_estimation_sq8() {
        let estimate = estimate_memory(10000, 256, QuantizationType::SQ8);

        assert_eq!(estimate.bytes_per_vector, 256);
        assert!(estimate.compression_ratio > 3.9); // Approximately 4x
        assert!(estimate.compression_ratio < 4.1);
    }

    #[test]
    fn test_memory_estimation_pq() {
        let estimate = estimate_memory(10000, 256, QuantizationType::PQ8x256);

        assert_eq!(estimate.bytes_per_vector, 8);
        assert!(estimate.compression_ratio > 10.0); // Much better compression
        assert!(estimate.codebook_bytes > 0);
    }

    #[test]
    fn test_memory_estimate_format() {
        let estimate = estimate_memory(1000, 256, QuantizationType::PQ8x256);
        let formatted = estimate.format_size();

        assert!(formatted.contains("KB") || formatted.contains("MB"));
    }

    #[test]
    fn test_pq_code_memory() {
        let code = PQCode {
            codes: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        assert_eq!(code.memory_bytes(), 8);
    }

    #[test]
    fn test_quantized_vector_memory() {
        let qv = QuantizedVector {
            data: vec![1, 2, 3, 4],
            original_dim: 4,
        };

        assert_eq!(
            qv.memory_bytes(),
            4 + std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn test_dimension_validation() {
        let config = QuantizedHnswConfig::default();
        let mut index = QuantizedHnswIndex::new(128, config);

        // Train
        let training: Vec<Embedding> = (0..50).map(|_| Embedding::random(128)).collect();
        index.train(&training, 5);

        // Try to insert wrong dimension
        let wrong_embedding = Embedding::random(64);
        let result = index.insert(
            EmbeddingId(1),
            &wrong_embedding,
            &Metadata::default(),
        );

        assert!(matches!(result, Err(IndexError::InvalidDimension { .. })));
    }
}

#[cfg(test)]
mod benches {
    use super::*;

    /// Benchmark helper to measure time
    fn bench_time<F: FnOnce()>(f: F) -> std::time::Duration {
        let start = std::time::Instant::now();
        f();
        start.elapsed()
    }

    #[test]
    fn bench_scalar_quantization() {
        let vectors: Vec<Embedding> = (0..1000).map(|_| Embedding::random(256)).collect();

        let mut quantizer = ScalarQuantizer::new(256);

        // Benchmark fit
        let fit_time = bench_time(|| {
            quantizer.fit(&vectors);
        });
        println!("SQ8 fit (1000 vectors): {:?}", fit_time);

        // Benchmark quantize
        let quantize_time = bench_time(|| {
            for vec in &vectors {
                let _ = quantizer.quantize(vec);
            }
        });
        println!("SQ8 quantize (1000 vectors): {:?}", quantize_time);

        // Benchmark dequantize
        let quantized: Vec<_> = vectors.iter().map(|v| quantizer.quantize(v)).collect();
        let dequantize_time = bench_time(|| {
            for qv in &quantized {
                let _ = quantizer.dequantize(qv);
            }
        });
        println!("SQ8 dequantize (1000 vectors): {:?}", dequantize_time);
    }

    #[test]
    fn bench_product_quantization() {
        let vectors: Vec<Embedding> = (0..1000).map(|_| Embedding::random(256)).collect();

        let mut pq = ProductQuantizer::new(256, 8, 256);

        // Benchmark fit
        let fit_time = bench_time(|| {
            pq.fit(&vectors, 20);
        });
        println!("PQ fit (1000 vectors, 20 iterations): {:?}", fit_time);

        // Benchmark quantize
        let quantize_time = bench_time(|| {
            for vec in &vectors {
                let _ = pq.quantize(vec);
            }
        });
        println!("PQ quantize (1000 vectors): {:?}", quantize_time);

        // Benchmark asymmetric distance
        let query = Embedding::random(256);
        let codes: Vec<_> = vectors.iter().map(|v| pq.quantize(v)).collect();
        let asymmetric_time = bench_time(|| {
            for code in &codes {
                let _ = pq.asymmetric_distance(&query, code);
            }
        });
        println!("PQ asymmetric distance (1000 codes): {:?}", asymmetric_time);

        // Benchmark fast distance
        let tables = pq.precompute_tables(&query);
        let fast_time = bench_time(|| {
            for code in &codes {
                let _ = pq.fast_distance(&tables, code);
            }
        });
        println!("PQ fast distance (1000 codes): {:?}", fast_time);

        let speedup = asymmetric_time.as_secs_f64() / fast_time.as_secs_f64();
        println!("Fast distance speedup: {:.2}x", speedup);
    }

    #[test]
    fn bench_quantized_hnsw() {
        let config = QuantizedHnswConfig::default();
        let mut index = QuantizedHnswIndex::new(256, config);

        // Train
        let training: Vec<Embedding> = (0..500).map(|_| Embedding::random(256)).collect();
        let train_time = bench_time(|| {
            index.train(&training, 20);
        });
        println!("Quantized HNSW train (500 vectors): {:?}", train_time);

        // Insert
        let vectors: Vec<Embedding> = (0..1000).map(|_| Embedding::random(256)).collect();
        let insert_time = bench_time(|| {
            for (i, vec) in vectors.iter().enumerate() {
                let _ = index.insert(EmbeddingId(i as u64), vec, &Metadata::default());
            }
        });
        println!("Quantized HNSW insert (1000 vectors): {:?}", insert_time);

        // Search
        let query = Embedding::random(256);
        let search_time = bench_time(|| {
            for _ in 0..100 {
                let _ = index.search(&query, 10);
            }
        });
        println!("Quantized HNSW search (100 queries, k=10): {:?}", search_time);

        let stats = index.stats();
        println!("Index stats: {:?}", stats);
        println!("Memory per vector: {} bytes", stats.memory_bytes / stats.num_vectors);
    }

    #[test]
    fn bench_memory_comparison() {
        let num_vectors = 100_000;
        let dimension = 256;

        println!("\nMemory comparison for {} vectors of dimension {}:", num_vectors, dimension);

        for quant_type in [
            QuantizationType::None,
            QuantizationType::SQ8,
            QuantizationType::PQ8x256,
            QuantizationType::PQ16x256,
        ] {
            let estimate = estimate_memory(num_vectors, dimension, quant_type);
            println!(
                "{:?}: {} ({:.2}x compression, {} bytes/vector)",
                quant_type,
                estimate.format_size(),
                estimate.compression_ratio,
                estimate.bytes_per_vector
            );
        }
    }
}
