//! Acceptance tests for intelligent task routing

#[cfg(test)]
mod intelligent_routing {
    use cognitum::ruvector::{CognitumRuvector, RuvectorConfig};
    use cognitum::ruvector::types::{TaskEmbedding, ExecutionTrace, TileId};

    #[test]
    fn should_route_task_to_valid_tile() {
        let config = RuvectorConfig {
            num_tiles: 16,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let task = TaskEmbedding::random();
        let tile_id = ruvector.route_task(&task);
        let confidence = ruvector.route_confidence(&task);

        assert!(tile_id.0 < 16, "Invalid tile ID: {}", tile_id.0);
        assert!(confidence >= 0.0 && confidence <= 1.0, "Invalid confidence: {}", confidence);
    }

    #[test]
    fn should_improve_routing_with_training() {
        let config = RuvectorConfig {
            num_tiles: 8,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        // Generate training traces with pattern:
        // Tasks with high first feature go to tile 0
        // Tasks with high second feature go to tile 1, etc.
        let mut traces: Vec<ExecutionTrace> = Vec::new();

        for i in 0..800 {
            let tile_idx = i % 8;
            let mut task_data = vec![0.1; 256];
            task_data[tile_idx] = 0.9; // Strong signal for this tile

            traces.push(ExecutionTrace {
                task_embedding: TaskEmbedding::new(task_data),
                actual_tile: TileId(tile_idx as u32),
                execution_time_us: 1000,
                success: true,
            });
        }

        // Train router
        let metrics = ruvector.train_router(&traces).unwrap();

        println!("Training metrics: accuracy={:.2}, loss={:.4}", metrics.accuracy, metrics.final_loss);

        // Should achieve reasonable accuracy
        assert!(metrics.accuracy > 0.3, "Accuracy too low: {}", metrics.accuracy);
        assert!(metrics.final_loss < 5.0, "Loss too high: {}", metrics.final_loss);
    }

    #[test]
    fn should_provide_consistent_routing() {
        let config = RuvectorConfig {
            num_tiles: 8,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let task = TaskEmbedding::random();

        // Route same task multiple times
        let tile1 = ruvector.route_task(&task);
        let tile2 = ruvector.route_task(&task);
        let tile3 = ruvector.route_task(&task);

        // Should be consistent
        assert_eq!(tile1, tile2);
        assert_eq!(tile2, tile3);
    }

    #[test]
    fn should_save_and_load_router_model() {
        use std::fs;

        let config = RuvectorConfig {
            num_tiles: 4,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        // Train model
        let traces: Vec<ExecutionTrace> = (0..100).map(|_| ExecutionTrace::random()).collect();
        ruvector.train_router(&traces).unwrap();

        let temp_path = "/tmp/test_routing_model.json";

        // Save model
        ruvector.save_router_model(temp_path).unwrap();

        // Create new instance and load
        let ruvector2 = CognitumRuvector::new(config);
        ruvector2.load_router_model(temp_path).unwrap();

        // Should produce same routing
        let task = TaskEmbedding::random();
        let tile1 = ruvector.route_task(&task);
        let tile2 = ruvector2.route_task(&task);

        assert_eq!(tile1, tile2);

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn should_handle_multiple_tasks() {
        let config = RuvectorConfig {
            num_tiles: 16,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let mut tile_counts = vec![0u32; 16];

        // Route 1000 random tasks
        for _ in 0..1000 {
            let task = TaskEmbedding::random();
            let tile = ruvector.route_task(&task);
            tile_counts[tile.0 as usize] += 1;
        }

        // All tiles should be used at least once (with high probability)
        let tiles_used = tile_counts.iter().filter(|&&count| count > 0).count();
        println!("Tiles used: {}/16", tiles_used);
        println!("Distribution: {:?}", tile_counts);

        // With random tasks, most tiles should get some work
        assert!(tiles_used >= 8, "Not enough tiles used: {}", tiles_used);
    }
}
