//! Unit tests for CognitumRuvector facade

#[cfg(test)]
mod cognitum_ruvector {
    use cognitum::ruvector::{CognitumRuvector, RuvectorConfig};
    use cognitum::ruvector::types::{
        TileState, Embedding, TaskEmbedding, TileId, VectorOp, ExecutionTrace, EmbeddingId, Metadata
    };

    #[test]
    fn should_capture_state() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();
        let embeddings = ruvector.capture_state(&states);

        assert_eq!(embeddings.len(), 4);
        for emb in &embeddings {
            assert_eq!(emb.dimension(), 256);
        }
    }

    #[test]
    fn should_store_embeddings() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let embeddings: Vec<Embedding> = (0..10).map(|_| Embedding::random(256)).collect();
        let result = ruvector.store_embeddings(&embeddings);

        assert!(result.is_ok());

        let stats = ruvector.index_stats();
        assert_eq!(stats.num_vectors, 10);
    }

    #[test]
    fn should_search_similar() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Store embeddings
        let embeddings: Vec<Embedding> = (0..20).map(|_| Embedding::random(256)).collect();
        ruvector.store_embeddings(&embeddings).unwrap();

        // Search
        let query = Embedding::random(256);
        let results = ruvector.search_similar(&query, 5).unwrap();

        assert_eq!(results.len(), 5);

        // Verify sorted
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn should_route_task() {
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
    fn should_train_router() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let traces: Vec<ExecutionTrace> = (0..50).map(|_| ExecutionTrace::random()).collect();
        let result = ruvector.train_router(&traces);

        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics.accuracy >= 0.0 && metrics.accuracy <= 1.0);
    }

    #[test]
    fn should_create_tile_group() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1), TileId(2)];
        let result = ruvector.create_tile_group(&tiles);

        assert!(result.is_ok());
    }

    #[test]
    fn should_execute_parallel_operations() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = ruvector.create_tile_group(&tiles).unwrap();

        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let result = ruvector.parallel_op(group, VectorOp::Sum, &data);

        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn should_execute_parallel_dot_product() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = ruvector.create_tile_group(&tiles).unwrap();

        let a: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..64).map(|i| (i * 2) as f32).collect();

        let result = ruvector.parallel_dot_product(group, &a, &b);

        assert!(result.is_ok());
    }

    #[test]
    fn should_save_and_load_router_model() {
        use std::fs;

        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let temp_path = "/tmp/test_facade_router.json";

        // Save
        let save_result = ruvector.save_router_model(temp_path);
        assert!(save_result.is_ok());

        // Load
        let load_result = ruvector.load_router_model(temp_path);
        assert!(load_result.is_ok());

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn should_store_embedding_with_metadata() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let embedding = Embedding::random(256);
        let mut metadata = Metadata::default();
        metadata.tile_id = Some(TileId(5));
        metadata.timestamp = Some(1234567890);

        let result = ruvector.store_embedding(EmbeddingId(1), &embedding, &metadata);
        assert!(result.is_ok());

        let stats = ruvector.index_stats();
        assert_eq!(stats.num_vectors, 1);
    }
}
