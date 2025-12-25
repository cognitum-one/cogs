//! CognitumRuvector facade - Main API for Ruvector integration

use crate::ruvector::types::*;
use crate::ruvector::embedding::{EmbeddingGenerator, DefaultEmbeddingGenerator};
use crate::ruvector::index::{VectorIndex, HnswVectorIndex};
use crate::ruvector::router::{TaskRouter, TinyDancerRouter};
use crate::ruvector::bridge::{RaceWayBridge, DefaultRaceWayBridge};
use std::path::Path;
use std::sync::Arc;
use parking_lot::RwLock;

/// Main facade for Cognitum-Ruvector integration
pub struct CognitumRuvector {
    generator: Arc<dyn EmbeddingGenerator>,
    index: Arc<RwLock<Box<dyn VectorIndex>>>,
    router: Arc<RwLock<Box<dyn TaskRouter>>>,
    bridge: Arc<RwLock<Box<dyn RaceWayBridge>>>,
    config: RuvectorConfig,
    next_embedding_id: Arc<RwLock<u64>>,
}

impl CognitumRuvector {
    /// Create new CognitumRuvector with default configuration
    pub fn new(config: RuvectorConfig) -> Self {
        let generator = Arc::new(DefaultEmbeddingGenerator::new(config.embedding_dimension));
        let index: Arc<RwLock<Box<dyn VectorIndex>>> = Arc::new(RwLock::new(
            Box::new(HnswVectorIndex::new(config.embedding_dimension))
        ));
        let router: Arc<RwLock<Box<dyn TaskRouter>>> = Arc::new(RwLock::new(
            Box::new(TinyDancerRouter::new(config.num_tiles, config.embedding_dimension))
        ));
        let bridge: Arc<RwLock<Box<dyn RaceWayBridge>>> = Arc::new(RwLock::new(
            Box::new(DefaultRaceWayBridge::new(config.num_tiles))
        ));

        Self {
            generator,
            index,
            router,
            bridge,
            config,
            next_embedding_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Capture state embeddings from tile states
    pub fn capture_state(&self, states: &[TileState]) -> Vec<Embedding> {
        self.generator.batch_generate(states)
    }

    /// Store embeddings in vector index
    pub fn store_embeddings(&self, embeddings: &[Embedding]) -> Result<(), IndexError> {
        let mut index = self.index.write();
        let mut next_id = self.next_embedding_id.write();

        for embedding in embeddings {
            let id = EmbeddingId(*next_id);
            *next_id += 1;

            let metadata = Metadata::default();
            index.insert(id, embedding, &metadata)?;
        }

        Ok(())
    }

    /// Store single embedding with metadata
    pub fn store_embedding(&self, id: EmbeddingId, embedding: &Embedding, metadata: &Metadata)
        -> Result<(), IndexError> {
        let mut index = self.index.write();
        index.insert(id, embedding, metadata)
    }

    /// Search for similar embeddings
    pub fn search_similar(&self, query: &Embedding, k: usize) -> Result<Vec<SearchResult>, IndexError> {
        let index = self.index.read();
        index.search(query, k)
    }

    /// Route task to optimal tile
    pub fn route_task(&self, task: &TaskEmbedding) -> TileId {
        let router = self.router.read();
        router.predict_tile(task)
    }

    /// Get routing confidence
    pub fn route_confidence(&self, task: &TaskEmbedding) -> f32 {
        let router = self.router.read();
        router.confidence(task)
    }

    /// Train routing model from execution traces
    pub fn train_router(&self, traces: &[ExecutionTrace]) -> Result<TrainingMetrics, RouterError> {
        let mut router = self.router.write();
        router.train(traces)
    }

    /// Load pre-trained router model
    pub fn load_router_model(&self, path: &str) -> Result<(), RouterError> {
        let mut router = self.router.write();
        router.load_model(Path::new(path))
    }

    /// Save router model
    pub fn save_router_model(&self, path: &str) -> Result<(), RouterError> {
        let router = self.router.read();
        router.save_model(Path::new(path))
    }

    /// Create tile group for parallel operations
    pub fn create_tile_group(&self, tiles: &[TileId]) -> Result<GroupId, RaceWayError> {
        let mut bridge = self.bridge.write();
        bridge.create_group(tiles)
    }

    /// Execute parallel vector operation
    pub fn parallel_op(&self, group: GroupId, op: VectorOp, data: &[f32])
        -> Result<Vec<f32>, RaceWayError> {
        let bridge = self.bridge.read();
        bridge.parallel_op(group, op, data)
    }

    /// Execute parallel dot product
    pub fn parallel_dot_product(&self, group: GroupId, a: &[f32], b: &[f32])
        -> Result<f32, RaceWayError> {
        // Concatenate vectors for dot product operation
        let mut data = Vec::with_capacity(a.len() + b.len());
        data.extend_from_slice(a);
        data.extend_from_slice(b);

        let results = self.parallel_op(group, VectorOp::DotProduct, &data)?;
        Ok(results.iter().sum())
    }

    /// Execute operation on single tile
    pub fn single_tile_operation(&self, tile: TileId, op: VectorOp, data: &[f32])
        -> Result<Vec<f32>, RaceWayError> {
        let group = {
            let mut bridge = self.bridge.write();
            bridge.create_group(&[tile])?
        };
        self.parallel_op(group, op, data)
    }

    /// Get index statistics
    pub fn index_stats(&self) -> IndexStats {
        let index = self.index.read();
        index.stats()
    }

    /// Get configuration
    pub fn config(&self) -> &RuvectorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_and_store_state() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Generate some tile states
        let states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();

        // Capture embeddings
        let embeddings = ruvector.capture_state(&states);
        assert_eq!(embeddings.len(), 4);

        // Store embeddings
        let result = ruvector.store_embeddings(&embeddings);
        assert!(result.is_ok());

        // Verify stored
        let stats = ruvector.index_stats();
        assert_eq!(stats.num_vectors, 4);
    }

    #[test]
    fn test_search_similar() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Store some embeddings
        let embeddings: Vec<Embedding> = (0..10).map(|_| Embedding::random(256)).collect();
        ruvector.store_embeddings(&embeddings).unwrap();

        // Search
        let query = Embedding::random(256);
        let results = ruvector.search_similar(&query, 5).unwrap();

        assert_eq!(results.len(), 5);
        // Verify sorted by similarity
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn test_route_task() {
        let config = RuvectorConfig {
            num_tiles: 8,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let task = TaskEmbedding::random();
        let tile = ruvector.route_task(&task);

        assert!(tile.0 < 8);

        let confidence = ruvector.route_confidence(&task);
        assert!(confidence >= 0.0 && confidence <= 1.0);
    }

    #[test]
    fn test_parallel_operations() {
        let config = RuvectorConfig {
            num_tiles: 4,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = ruvector.create_tile_group(&tiles).unwrap();

        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let result = ruvector.parallel_op(group, VectorOp::Sum, &data).unwrap();

        assert!(!result.is_empty());
    }
}
