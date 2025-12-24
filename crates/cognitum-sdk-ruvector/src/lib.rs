//! # Cognitum Ruvector SDK
//!
//! Production-ready SDK for vector search and neural routing on Cognitum chip architecture.
//!
//! ## Features
//!
//! - **Vector Search**: HNSW-based vector indexing with 150x faster search
//! - **Neural Routing**: TinyDancer (FastGRNN) for intelligent task routing
//! - **Embedding Generation**: Convert chip states to vector embeddings
//! - **Async API**: Fully asynchronous with Tokio runtime
//! - **Production Ready**: Comprehensive error handling and validation
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use cognitum_sdk_ruvector::{RuvectorClient, RuvectorConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client with default configuration
//!     let client = RuvectorClient::builder()
//!         .embedding_dimension(256)
//!         .num_tiles(16)
//!         .build()?;
//!
//!     // Insert embeddings
//!     let embedding = vec![0.1; 256];
//!     client.insert(1, &embedding, Default::default()).await?;
//!
//!     // Search for similar vectors
//!     let query = vec![0.1; 256];
//!     let results = client.search(&query, 10).await?;
//!
//!     println!("Found {} similar vectors", results.len());
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The SDK wraps three core Ruvector components:
//!
//! 1. **HNSW Vector Index** (`index.rs`) - Hierarchical Navigable Small World graphs
//! 2. **TinyDancer Router** (`router.rs`) - Neural routing with FastGRNN
//! 3. **Embedding Generator** (`embedding.rs`) - Chip state to vector conversion
//!
//! ## Performance
//!
//! - Vector search: O(log n) with HNSW
//! - Neural routing: <100μs inference time
//! - Embedding generation: Batch processing for 16 tiles
//! - Memory efficient: Supports quantization for 4-32x reduction

pub mod client;
pub mod config;
pub mod error;
pub mod types;

// Re-export main types from cognitum parent crate
pub use cognitum::ruvector::{
    // Core types
    Embedding, EmbeddingId, Metadata, SearchResult, TileId, GroupId,
    TaskEmbedding, ExecutionTrace, TrainingMetrics, IndexStats,
    TileState, ProcessorState, VectorOp,

    // Traits
    VectorIndex, TaskRouter, EmbeddingGenerator,

    // Implementations
    HnswVectorIndex, TinyDancerRouter, DefaultEmbeddingGenerator,

    // Helper functions
    cosine_similarity,
};

// Re-export SDK-specific types
pub use client::{RuvectorClient, RuvectorClientBuilder};
pub use config::RuvectorConfig;
pub use error::{RuvectorError, Result};

/// SDK version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::client::{RuvectorClient, RuvectorClientBuilder};
    pub use crate::config::RuvectorConfig;
    pub use crate::error::{RuvectorError, Result};
    pub use crate::types::*;

    // Re-export commonly used types
    pub use cognitum::ruvector::{
        Embedding, EmbeddingId, Metadata, SearchResult,
        TileId, TaskEmbedding, VectorIndex, TaskRouter,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
        assert_eq!(VERSION, "1.0.0");
    }

    #[test]
    fn test_prelude_imports() {
        use prelude::*;

        // Verify key types are available
        let _config: RuvectorConfig = RuvectorConfig::default();
        let _embedding = Embedding::zeros(256);
    }
}
