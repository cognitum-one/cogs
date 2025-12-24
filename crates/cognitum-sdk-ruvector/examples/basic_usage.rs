//! Basic usage example for Cognitum Ruvector SDK

use cognitum_sdk_ruvector::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Cognitum Ruvector SDK - Basic Usage Example\n");

    // Create client with default configuration
    let client = RuvectorClient::builder()
        .embedding_dimension(256)
        .num_tiles(16)
        .index_capacity(10_000)
        .build()?;

    println!("✓ Client created");
    println!("  Dimension: {}", client.config.embedding_dimension);
    println!("  Tiles: {}\n", client.config.num_tiles);

    // Insert some embeddings
    println!("Inserting embeddings...");
    for i in 0..100 {
        let mut embedding = vec![0.0; 256];
        // Create simple patterns
        embedding[i % 256] = 1.0;
        embedding[(i + 1) % 256] = 0.5;

        let mut metadata = Metadata::default();
        metadata.custom.insert("category".to_string(), format!("cat_{}", i % 10));

        client.insert(i, &embedding, metadata).await?;
    }
    println!("✓ Inserted 100 embeddings\n");

    // Search for similar vectors
    println!("Searching for similar vectors...");
    let mut query = vec![0.0; 256];
    query[10] = 1.0;
    query[11] = 0.5;

    let results = client.search(&query, 5).await?;
    println!("✓ Found {} results:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. ID: {:?}, Similarity: {:.4}",
            i + 1,
            result.id,
            result.similarity
        );
    }
    println!();

    // Test neural routing
    println!("Testing neural routing...");
    let task_embedding = vec![0.3; 256];
    let tile_id = client.predict_tile(&task_embedding).await?;
    let confidence = client.routing_confidence(&task_embedding).await?;

    println!("✓ Routing prediction:");
    println!("  Tile ID: {:?}", tile_id);
    println!("  Confidence: {:.2}%\n", confidence * 100.0);

    // Get statistics
    println!("Statistics:");
    let index_stats = client.index_stats();
    println!("  Index vectors: {}", index_stats.num_vectors);
    println!("  Index memory: {} bytes", index_stats.memory_bytes);

    let vec_stats = client.vector_stats();
    println!("  Total searches: {}", vec_stats.total_searches);
    println!("  Avg search time: {:.2}μs", vec_stats.avg_search_time_us);

    let router_stats = client.router_stats();
    println!("  Total predictions: {}", router_stats.total_predictions);
    println!("  Avg prediction time: {:.2}μs", router_stats.avg_prediction_time_us);
    println!();

    // Health check
    let health = client.health();
    println!("Health Status: {:?}", health.status);
    println!("  Index healthy: {}", health.index_healthy);
    println!("  Router healthy: {}", health.router_healthy);
    println!("  Router trained: {}", health.router_trained);
    println!("  Uptime: {}s", health.uptime_seconds);

    println!("\n✓ Example completed successfully!");

    Ok(())
}
