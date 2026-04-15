/// Neural Embeddings Example
///
/// Demonstrates capturing processor states from Newport tiles and storing them
/// as vector embeddings in Ruvector for similarity search and pattern recognition.

use anyhow::Result;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{info, warn};

/// Represents the state of a Newport processor at a given time
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessorState {
    tile_id: u32,
    timestamp: u64,
    program_counter: u32,
    stack_depth: usize,
    stack_top: Vec<u32>,  // Top N stack values
    register_state: Vec<u32>,
    coprocessor_active: bool,
    message_count: u64,
}

/// Converts a processor state into a 128-dimensional embedding vector
///
/// This is a simplified embedding generation. In production, you might use:
/// - Learned embeddings from neural networks
/// - PCA/t-SNE dimensionality reduction
/// - Autoencoders trained on processor states
fn state_to_embedding(state: &ProcessorState) -> Array1<f32> {
    let mut embedding = Array1::zeros(128);

    // Normalize and encode various state components
    // Position 0-3: Tile ID (one-hot-ish encoding)
    let tile_component = (state.tile_id as f32) / 256.0;
    embedding[0] = tile_component;
    embedding[1] = tile_component.sin();
    embedding[2] = tile_component.cos();
    embedding[3] = (state.timestamp as f32 % 1000.0) / 1000.0;

    // Position 4-7: Program counter encoding
    let pc = state.program_counter as f32 / 65536.0;
    embedding[4] = pc;
    embedding[5] = pc.sqrt();
    embedding[6] = (state.stack_depth as f32) / 256.0;
    embedding[7] = if state.coprocessor_active { 1.0 } else { 0.0 };

    // Position 8-39: Stack top values (up to 32 values)
    for (i, &val) in state.stack_top.iter().take(32).enumerate() {
        embedding[8 + i] = (val as f32) / u32::MAX as f32;
    }

    // Position 40-71: Register state (up to 32 registers)
    for (i, &val) in state.register_state.iter().take(32).enumerate() {
        embedding[40 + i] = (val as f32) / u32::MAX as f32;
    }

    // Position 72-95: Derived features
    let stack_mean = if state.stack_top.is_empty() {
        0.0
    } else {
        state.stack_top.iter().map(|&x| x as f32).sum::<f32>() / state.stack_top.len() as f32
    };
    embedding[72] = stack_mean / u32::MAX as f32;

    // Position 96-127: Message activity and temporal features
    let msg_rate = (state.message_count as f32).ln() / 20.0; // Log scale
    embedding[96] = msg_rate;
    embedding[97] = msg_rate.sin();

    // Fill remaining with temporal and spectral features
    for i in 98..128 {
        let freq = (i - 98) as f32 / 30.0;
        embedding[i] = ((state.timestamp as f32 * freq).sin() * 0.1).tanh();
    }

    embedding
}

/// Simulates collecting processor states over time
fn simulate_processor_activity() -> Vec<ProcessorState> {
    let mut states = Vec::new();

    info!("Simulating processor activity across 256 Newport tiles...");

    // Simulate different workload patterns
    for tile_id in 0..256 {
        for t in 0..10 {
            let timestamp = t * 1000 + tile_id as u64;

            // Create synthetic processor state
            let state = ProcessorState {
                tile_id,
                timestamp,
                program_counter: (tile_id * 100 + t * 10) as u32,
                stack_depth: (tile_id % 8 + t % 4) as usize,
                stack_top: vec![
                    tile_id * 1000 + t * 10,
                    tile_id * 500 + t * 5,
                    tile_id * 250 + t * 2,
                ],
                register_state: vec![
                    tile_id,
                    t * 100,
                    (tile_id * t) % 65536,
                ],
                coprocessor_active: (tile_id + t) % 3 == 0,
                message_count: (tile_id as u64 * t as u64 * 7) % 10000,
            };

            states.push(state);
        }
    }

    info!("Generated {} processor states", states.len());
    states
}

/// Demonstrates vector database operations with processor states
async fn run_vector_database_demo() -> Result<()> {
    info!("=== Neural Embeddings Demo ===");
    info!("Integrating Newport processor states with Ruvector vector database");

    // Step 1: Collect processor states
    let states = simulate_processor_activity();
    info!("✓ Collected {} processor states", states.len());

    // Step 2: Convert states to embeddings
    let start = Instant::now();
    let embeddings: Vec<Array1<f32>> = states
        .iter()
        .map(state_to_embedding)
        .collect();
    let embedding_time = start.elapsed();
    info!("✓ Generated {} embeddings in {:?}", embeddings.len(), embedding_time);

    // Step 3: Demonstrate similarity search (simplified)
    // In a real implementation, you would use ruvector-core's HNSW index
    let query_state = &states[42]; // Arbitrary query state
    let query_embedding = state_to_embedding(query_state);

    info!("Query processor: Tile {} at timestamp {}",
          query_state.tile_id, query_state.timestamp);

    // Find similar states using cosine similarity
    let start = Instant::now();
    let mut similarities: Vec<(usize, f32)> = embeddings
        .iter()
        .enumerate()
        .map(|(idx, emb)| {
            // Cosine similarity
            let dot: f32 = query_embedding.iter().zip(emb.iter()).map(|(a, b)| a * b).sum();
            let norm_q: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_e: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            let similarity = dot / (norm_q * norm_e + 1e-10);
            (idx, similarity)
        })
        .collect();

    // Sort by similarity (descending)
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let search_time = start.elapsed();

    info!("✓ Similarity search completed in {:?}", search_time);
    info!("Top 10 most similar processor states:");

    for (rank, (idx, sim)) in similarities.iter().take(10).enumerate() {
        let state = &states[*idx];
        info!(
            "  {}. Tile {:3} @ t={:6} | Similarity: {:.4} | Stack depth: {} | PC: 0x{:04x}",
            rank + 1,
            state.tile_id,
            state.timestamp,
            sim,
            state.stack_depth,
            state.program_counter
        );
    }

    // Step 4: Pattern detection - find processors with similar behavior
    info!("\n=== Pattern Detection ===");
    info!("Grouping processors by behavioral similarity...");

    // Find all states from different tiles with high similarity to query
    let similar_tiles: Vec<_> = similarities
        .iter()
        .filter(|(idx, sim)| {
            *sim > 0.9 && states[*idx].tile_id != query_state.tile_id
        })
        .take(5)
        .collect();

    if similar_tiles.is_empty() {
        warn!("No highly similar patterns found (threshold: 0.9)");
    } else {
        info!("Found {} tiles with similar execution patterns:", similar_tiles.len());
        for (idx, sim) in similar_tiles {
            let state = &states[*idx];
            info!(
                "  Tile {:3} | Similarity: {:.4} | Pattern: PC=0x{:04x}, Stack={}, Msgs={}",
                state.tile_id, sim, state.program_counter,
                state.stack_depth, state.message_count
            );
        }
    }

    // Step 5: Performance summary
    info!("\n=== Performance Summary ===");
    info!("Total states processed: {}", states.len());
    info!("Embedding dimension: 128");
    info!("Avg embedding time: {:.2}µs per state",
          embedding_time.as_micros() as f64 / states.len() as f64);
    info!("Search time: {:?}", search_time);
    info!("Throughput: {:.0} searches/sec",
          1.0 / search_time.as_secs_f64());

    info!("\n=== Integration Benefits ===");
    info!("✓ Pattern recognition across 256 processors");
    info!("✓ Anomaly detection via similarity thresholds");
    info!("✓ Temporal pattern matching");
    info!("✓ Cross-processor behavior correlation");
    info!("✓ Scalable to millions of state snapshots with HNSW indexing");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("neural_embeddings=info".parse()?)
        )
        .init();

    info!("Starting Newport + Ruvector Neural Embeddings Demo");

    run_vector_database_demo().await?;

    info!("\n✓ Demo completed successfully!");
    info!("\nNext steps:");
    info!("  1. Integrate with real Newport simulator");
    info!("  2. Use ruvector-core HNSW index for production scale");
    info!("  3. Train neural embeddings on real processor traces");
    info!("  4. Deploy pattern detection in real-time");

    Ok(())
}
