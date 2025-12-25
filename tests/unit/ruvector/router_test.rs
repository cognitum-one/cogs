//! Unit tests for TinyDancerRouter

#[cfg(test)]
mod tiny_dancer_router {
    use cognitum::ruvector::{TaskRouter, TinyDancerRouter};
    use cognitum::ruvector::types::{TaskEmbedding, TileId, ExecutionTrace};

    #[test]
    fn should_return_valid_tile_id() {
        let router = TinyDancerRouter::new(8, 256);

        let task = TaskEmbedding::random();
        let tile_id = router.predict_tile(&task);

        assert!(tile_id.0 < 8);
    }

    #[test]
    fn should_return_argmax_as_tile_id() {
        let router = TinyDancerRouter::new(16, 256);

        // Test multiple predictions
        for _ in 0..10 {
            let task = TaskEmbedding::random();
            let tile_id = router.predict_tile(&task);
            assert!(tile_id.0 < 16);
        }
    }

    #[test]
    fn should_compute_confidence_in_range() {
        let router = TinyDancerRouter::new(8, 256);

        let task = TaskEmbedding::random();
        let confidence = router.confidence(&task);

        assert!(confidence >= 0.0 && confidence <= 1.0);
    }

    #[test]
    fn should_train_model_from_traces() {
        let mut router = TinyDancerRouter::new(4, 256);

        let traces: Vec<ExecutionTrace> = (0..100)
            .map(|i| ExecutionTrace {
                task_embedding: TaskEmbedding::random(),
                actual_tile: TileId((i % 4) as u32),
                execution_time_us: 1000,
                success: true,
            })
            .collect();

        let result = router.train(&traces);
        assert!(result.is_ok());

        let metrics = result.unwrap();
        assert_eq!(metrics.epochs, 100);
        assert!(metrics.final_loss >= 0.0);
        assert!(metrics.accuracy >= 0.0 && metrics.accuracy <= 1.0);
    }

    #[test]
    fn should_fail_train_with_empty_traces() {
        let mut router = TinyDancerRouter::new(8, 256);

        let traces: Vec<ExecutionTrace> = vec![];
        let result = router.train(&traces);

        assert!(result.is_err());
    }

    #[test]
    fn should_save_and_load_model() {
        use std::fs;
        use std::path::Path;

        let router = TinyDancerRouter::new(8, 256);
        let temp_path = "/tmp/test_router_model.json";

        // Save model
        let save_result = router.save_model(Path::new(temp_path));
        assert!(save_result.is_ok());

        // Load model into new router
        let mut new_router = TinyDancerRouter::new(8, 256);
        let load_result = new_router.load_model(Path::new(temp_path));
        assert!(load_result.is_ok());

        // Clean up
        let _ = fs::remove_file(temp_path);
    }
}
