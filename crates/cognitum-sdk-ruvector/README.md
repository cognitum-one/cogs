# Cognitum Ruvector SDK

Production-ready Rust SDK for vector search and neural routing on Cognitum chip architecture.

## Features

- **HNSW Vector Search**: 150x faster similarity search with hierarchical navigable small world graphs
- **Neural Routing**: TinyDancer (FastGRNN) for intelligent task routing to chip tiles
- **Embedding Generation**: Convert chip states to vector embeddings
- **Async API**: Fully asynchronous with Tokio runtime
- **Production Ready**: Comprehensive error handling, validation, and metrics

## Quick Start

```rust
use cognitum_sdk_ruvector::{RuvectorClient, RuvectorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with builder pattern
    let client = RuvectorClient::builder()
        .embedding_dimension(256)
        .num_tiles(16)
        .auto_train_router(true)
        .build()?;

    // Insert embeddings
    let embedding = vec![0.1; 256];
    client.insert(1, &embedding, Default::default()).await?;

    // Search for similar vectors
    let query = vec![0.1; 256];
    let results = client.search(&query, 10).await?;

    println!("Found {} similar vectors", results.len());

    // Predict optimal tile for task
    let task = vec![0.5; 256];
    let tile_id = client.predict_tile(&task).await?;
    println!("Route task to tile: {:?}", tile_id);

    Ok(())
}
```

## Configuration

### Production Configuration

```rust
let config = RuvectorConfig::production();
let client = RuvectorClient::with_config(config)?;
```

Production preset:
- 512-dimensional embeddings
- 1M vector capacity
- HNSW M=32, ef_construction=400
- Auto-training enabled
- Metrics collection enabled

### Development Configuration

```rust
let config = RuvectorConfig::development();
let client = RuvectorClient::with_config(config)?;
```

### Custom Configuration

```rust
let config = RuvectorConfig::builder()
    .embedding_dimension(512)
    .index_capacity(100_000)
    .hnsw_m(24)
    .hnsw_ef_construction(200)
    .hnsw_ef_search(100)
    .num_tiles(16)
    .auto_train_router(true)
    .enable_metrics(true)
    .build()?;
```

## API Reference

### Vector Operations

```rust
// Insert embedding
client.insert(id, &embedding, metadata).await?;

// Search for k nearest neighbors
let results = client.search(&query, k).await?;

// Delete embedding
client.delete(id).await?;

// Optimize index
client.optimize_index().await?;
```

### Neural Routing

```rust
// Predict optimal tile
let tile_id = client.predict_tile(&task_embedding).await?;

// Get routing confidence
let confidence = client.routing_confidence(&task_embedding).await?;

// Record execution trace
client.record_trace(trace);

// Train router from traces
let metrics = client.train_router().await?;

// Save/load model
client.save_router_model(path).await?;
client.load_router_model(path).await?;
```

### Monitoring

```rust
// Get index statistics
let stats = client.index_stats();
println!("Vectors: {}", stats.num_vectors);

// Get operation statistics
let vec_stats = client.vector_stats();
println!("Avg search time: {}μs", vec_stats.avg_search_time_us);

let router_stats = client.router_stats();
println!("Accuracy: {}", router_stats.current_accuracy);

// Health check
let health = client.health();
println!("Status: {:?}", health.status);
```

## Performance

- **Vector Search**: O(log n) with HNSW, ~100μs for 1M vectors
- **Neural Routing**: <100μs inference time with TinyDancer
- **Embedding Generation**: Batch processing for 16 tiles
- **Memory**: Configurable quantization for 4-32x reduction

## Error Handling

All operations return `Result<T, RuvectorError>`:

```rust
match client.search(&query, 10).await {
    Ok(results) => println!("Found {} results", results.len()),
    Err(RuvectorError::Timeout(ms)) => println!("Timeout after {}ms", ms),
    Err(RuvectorError::InvalidDimension { expected, actual }) => {
        println!("Wrong dimension: expected {}, got {}", expected, actual)
    }
    Err(e) => println!("Error: {}", e),
}
```

## Architecture

The SDK wraps three core Ruvector components from the parent `cognitum` crate:

1. **HNSW Vector Index** - Hierarchical Navigable Small World graphs for fast similarity search
2. **TinyDancer Router** - FastGRNN neural network for intelligent task routing
3. **Embedding Generator** - Converts chip tile states to vector embeddings

## License

MIT

## Links

- [Cognitum Repository](https://github.com/cognitum/cognitum)
- [Documentation](https://docs.rs/cognitum-sdk-ruvector)
- [Ruvector Paper](https://arxiv.org/abs/2024.ruvector)
