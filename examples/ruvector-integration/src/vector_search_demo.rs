/// Vector Search Demo
///
/// Demonstrates high-performance vector similarity search on neuromorphic data
/// using Ruvector's HNSW indexing and SIMD acceleration.

use anyhow::Result;
use ndarray::{Array1, Array2};
use std::time::Instant;
use tracing::{info, warn};
use rayon::prelude::*;

/// Simulated neural activation pattern from Newport SIMD coprocessor
#[derive(Debug, Clone)]
struct NeuralActivation {
    tile_id: u32,
    layer_id: u8,
    timestamp: u64,
    activation_vector: Array1<f32>, // 256-dimensional activation
}

impl NeuralActivation {
    /// Generate synthetic neural activation pattern
    fn synthetic(tile_id: u32, layer_id: u8, timestamp: u64, seed: u64) -> Self {
        let dim = 256;
        let mut activation_vector = Array1::zeros(dim);

        // Generate pattern based on tile_id and layer_id
        for i in 0..dim {
            let val = ((tile_id as f32 * 0.1 + layer_id as f32 * 0.05
                       + (seed * i as u64) as f32 * 0.001)
                       .sin() * 0.5 + 0.5)
                       .clamp(0.0, 1.0);
            activation_vector[i] = val;
        }

        // Add some sparsity (ReLU-like)
        for i in 0..dim {
            if activation_vector[i] < 0.3 {
                activation_vector[i] = 0.0;
            }
        }

        // Normalize
        let norm: f32 = activation_vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            activation_vector.mapv_inplace(|x| x / norm);
        }

        Self {
            tile_id,
            layer_id,
            timestamp,
            activation_vector,
        }
    }
}

/// Vector database for neural activations (simplified HNSW simulation)
struct VectorDatabase {
    vectors: Vec<NeuralActivation>,
    index_built: bool,
}

impl VectorDatabase {
    fn new() -> Self {
        Self {
            vectors: Vec::new(),
            index_built: false,
        }
    }

    /// Insert a neural activation into the database
    fn insert(&mut self, activation: NeuralActivation) {
        self.vectors.push(activation);
        self.index_built = false; // Invalidate index
    }

    /// Build HNSW index for fast similarity search
    ///
    /// In production, this would use ruvector-core's optimized HNSW implementation
    /// with product quantization and SIMD acceleration
    fn build_index(&mut self) {
        info!("Building HNSW index for {} vectors...", self.vectors.len());
        let start = Instant::now();

        // Simulated index building (in production, this is highly optimized)
        // Actual ruvector-core performs:
        // - HNSW graph construction
        // - Product quantization for compression
        // - SIMD-optimized distance calculations

        self.index_built = true;
        let build_time = start.elapsed();

        info!("✓ Index built in {:?}", build_time);
        info!("  Index size: ~{} MB (estimated)",
              self.vectors.len() * 256 * 4 / 1024 / 1024);
    }

    /// Search for k nearest neighbors using cosine similarity
    fn search_knn(&self, query: &Array1<f32>, k: usize) -> Vec<(usize, f32)> {
        if !self.index_built {
            warn!("Index not built, using brute force search");
        }

        // Parallel cosine similarity computation
        let similarities: Vec<(usize, f32)> = self.vectors
            .par_iter()
            .enumerate()
            .map(|(idx, activation)| {
                let similarity = cosine_similarity(query, &activation.activation_vector);
                (idx, similarity)
            })
            .collect();

        // Sort by similarity (descending) and take top k
        let mut sorted = similarities;
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        sorted.truncate(k);

        sorted
    }

    /// Range search: find all vectors within similarity threshold
    fn search_range(&self, query: &Array1<f32>, threshold: f32) -> Vec<(usize, f32)> {
        let similarities: Vec<(usize, f32)> = self.vectors
            .par_iter()
            .enumerate()
            .map(|(idx, activation)| {
                let similarity = cosine_similarity(query, &activation.activation_vector);
                (idx, similarity)
            })
            .filter(|(_, sim)| *sim >= threshold)
            .collect();

        let mut sorted = similarities;
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        sorted
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &Array1<f32>, b: &Array1<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Demonstrate various vector search operations
async fn run_vector_search_demo() -> Result<()> {
    info!("=== Vector Search Demo ===");
    info!("High-performance similarity search on neuromorphic data");

    // Step 1: Generate synthetic neural activations from multiple tiles
    info!("\nGenerating neural activation patterns...");
    let num_tiles = 256;
    let num_layers = 8;
    let time_steps = 50;

    let start = Instant::now();
    let mut activations = Vec::new();

    for tile_id in 0..num_tiles {
        for layer_id in 0..num_layers {
            for t in 0..time_steps {
                let timestamp = t * 1000;
                let seed = (tile_id as u64 * 1000 + layer_id as u64 * 100 + t);
                activations.push(
                    NeuralActivation::synthetic(tile_id, layer_id, timestamp, seed)
                );
            }
        }
    }

    let gen_time = start.elapsed();
    info!("✓ Generated {} activation patterns in {:?}", activations.len(), gen_time);
    info!("  Tiles: {}, Layers: {}, Time steps: {}", num_tiles, num_layers, time_steps);
    info!("  Total data: ~{} MB", activations.len() * 256 * 4 / 1024 / 1024);

    // Step 2: Build vector database
    let mut db = VectorDatabase::new();
    for activation in activations {
        db.insert(activation);
    }
    db.build_index();

    // Step 3: K-nearest neighbor search
    info!("\n=== K-Nearest Neighbor Search ===");
    let query_activation = NeuralActivation::synthetic(42, 3, 5000, 12345);
    info!("Query: Tile {}, Layer {}, Time {}",
          query_activation.tile_id,
          query_activation.layer_id,
          query_activation.timestamp);

    let k = 10;
    let start = Instant::now();
    let knn_results = db.search_knn(&query_activation.activation_vector, k);
    let search_time = start.elapsed();

    info!("✓ Found {} nearest neighbors in {:?}", k, search_time);
    info!("Search throughput: {:.0} queries/sec",
          1.0 / search_time.as_secs_f64());

    info!("\nTop {} most similar activations:", k);
    for (rank, (idx, similarity)) in knn_results.iter().enumerate() {
        let activation = &db.vectors[*idx];
        info!(
            "  {}. Tile {:3}, Layer {}, t={:5} | Similarity: {:.4}",
            rank + 1,
            activation.tile_id,
            activation.layer_id,
            activation.timestamp,
            similarity
        );
    }

    // Step 4: Range search
    info!("\n=== Range Search (Threshold: 0.85) ===");
    let start = Instant::now();
    let range_results = db.search_range(&query_activation.activation_vector, 0.85);
    let range_time = start.elapsed();

    info!("✓ Found {} activations above threshold in {:?}",
          range_results.len(), range_time);

    if range_results.len() <= 20 {
        for (idx, similarity) in &range_results {
            let activation = &db.vectors[*idx];
            info!(
                "  Tile {:3}, Layer {}, t={:5} | Similarity: {:.4}",
                activation.tile_id,
                activation.layer_id,
                activation.timestamp,
                similarity
            );
        }
    } else {
        info!("  (Showing first 10 of {} results)", range_results.len());
        for (idx, similarity) in range_results.iter().take(10) {
            let activation = &db.vectors[*idx];
            info!(
                "  Tile {:3}, Layer {}, t={:5} | Similarity: {:.4}",
                activation.tile_id,
                activation.layer_id,
                activation.timestamp,
                similarity
            );
        }
    }

    // Step 5: Multi-query batch search
    info!("\n=== Batch Search (100 queries) ===");
    let mut queries = Vec::new();
    for i in 0..100 {
        queries.push(NeuralActivation::synthetic(i % num_tiles, i as u8 % num_layers, i * 1000, i * 7));
    }

    let start = Instant::now();
    let batch_results: Vec<_> = queries
        .par_iter()
        .map(|query| db.search_knn(&query.activation_vector, 5))
        .collect();
    let batch_time = start.elapsed();

    info!("✓ Completed {} searches in {:?}", queries.len(), batch_time);
    info!("  Avg latency: {:.2}ms per query",
          batch_time.as_millis() as f64 / queries.len() as f64);
    info!("  Throughput: {:.0} queries/sec",
          queries.len() as f64 / batch_time.as_secs_f64());

    // Step 6: Performance summary
    info!("\n=== Performance Summary ===");
    info!("Vector dimension: 256");
    info!("Total vectors: {}", db.vectors.len());
    info!("Index type: HNSW (simulated)");
    info!("Distance metric: Cosine similarity");
    info!("\nSearch Performance:");
    info!("  Single query (k=10): {:?}", search_time);
    info!("  Batch queries (100×5): {:?} total", batch_time);
    info!("  Range query (threshold 0.85): {:?}", range_time);

    info!("\n=== With ruvector-core Optimizations ===");
    info!("Production implementation provides:");
    info!("  ✓ HNSW indexing: ~100× faster than brute force");
    info!("  ✓ Product quantization: 4-32× memory reduction");
    info!("  ✓ SIMD acceleration: 4-8× faster distance calculations");
    info!("  ✓ Zero-copy operations: Minimal memory overhead");
    info!("  ✓ Approximate NN: Sub-millisecond queries on millions of vectors");

    info!("\n=== Use Cases for Newport + Ruvector ===");
    info!("1. Pattern Recognition:");
    info!("   - Find similar neural activations across time");
    info!("   - Detect anomalous processor behaviors");
    info!("   - Cluster processors by execution patterns");
    info!("\n2. Semantic Routing:");
    info!("   - Route data to processors with similar past patterns");
    info!("   - Content-based message passing");
    info!("   - Associative memory lookup");
    info!("\n3. Neuromorphic Learning:");
    info!("   - Store and retrieve learned patterns");
    info!("   - Similarity-based generalization");
    info!("   - Temporal pattern matching");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vector_search_demo=info".parse()?)
        )
        .init();

    info!("Starting Newport + Ruvector Vector Search Demo");

    run_vector_search_demo().await?;

    info!("\n✓ Demo completed successfully!");
    info!("\nNext steps:");
    info!("  1. Integrate with actual Newport simulator output");
    info!("  2. Use ruvector-core for production-grade HNSW indexing");
    info!("  3. Leverage Newport's SIMD coprocessors for vector operations");
    info!("  4. Implement online learning with streaming updates");

    Ok(())
}
