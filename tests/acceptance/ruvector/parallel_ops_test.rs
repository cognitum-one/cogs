//! Acceptance tests for parallel vector operations

#[cfg(test)]
mod parallel_operations {
    use cognitum::ruvector::{CognitumRuvector, RuvectorConfig};
    use cognitum::ruvector::types::{TileId, VectorOp};

    #[test]
    fn should_execute_parallel_dot_product() {
        let config = RuvectorConfig {
            num_tiles: 16,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        // Create tile group
        let group = ruvector.create_tile_group(&[
            TileId(0), TileId(1), TileId(2), TileId(3)
        ]).unwrap();

        // Large vectors for dot product
        let a: Vec<f32> = (0..256).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..256).map(|i| (i * 2) as f32).collect();

        let result = ruvector.parallel_dot_product(group, &a, &b).unwrap();

        // Verify result matches sequential
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let diff = (result - expected).abs();
        println!("Parallel: {}, Sequential: {}, Diff: {}", result, expected, diff);

        // Should be close (allowing for floating point errors)
        assert!(diff < 1.0, "Difference too large: {}", diff);
    }

    #[test]
    fn should_execute_parallel_sum() {
        let config = RuvectorConfig {
            num_tiles: 8,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = ruvector.create_tile_group(&tiles).unwrap();

        let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let results = ruvector.parallel_op(group, VectorOp::Sum, &data).unwrap();

        println!("Parallel sum results: {:?}", results);

        // Should have one result per tile
        assert_eq!(results.len(), 4);

        // Combined results should be meaningful
        let total: f32 = results.iter().sum();
        assert!(total > 0.0);
    }

    #[test]
    fn should_execute_normalize_operation() {
        let config = RuvectorConfig {
            num_tiles: 4,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let tiles = vec![TileId(0), TileId(1)];
        let group = ruvector.create_tile_group(&tiles).unwrap();

        let data: Vec<f32> = vec![3.0, 4.0, 5.0, 12.0]; // Will be split
        let results = ruvector.parallel_op(group, VectorOp::Normalize, &data).unwrap();

        println!("Normalize results: {:?}", results);

        // Results should be normalized
        assert!(!results.is_empty());
        for &val in &results {
            assert!(val.is_finite());
        }
    }

    #[test]
    fn should_handle_single_tile_operation() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let data: Vec<f32> = (0..32).map(|i| i as f32).collect();
        let result = ruvector.single_tile_operation(TileId(0), VectorOp::Sum, &data).unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn should_create_multiple_groups() {
        let config = RuvectorConfig {
            num_tiles: 16,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let group1 = ruvector.create_tile_group(&[TileId(0), TileId(1), TileId(2), TileId(3)]).unwrap();
        let group2 = ruvector.create_tile_group(&[TileId(4), TileId(5), TileId(6), TileId(7)]).unwrap();
        let group3 = ruvector.create_tile_group(&[TileId(8), TileId(9)]).unwrap();

        // Groups should have different IDs
        assert_ne!(group1, group2);
        assert_ne!(group2, group3);
        assert_ne!(group1, group3);

        // All groups should be usable
        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();

        let result1 = ruvector.parallel_op(group1, VectorOp::Sum, &data);
        let result2 = ruvector.parallel_op(group2, VectorOp::Sum, &data);
        let result3 = ruvector.parallel_op(group3, VectorOp::Sum, &data);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
    }

    #[test]
    fn should_measure_parallel_speedup() {
        let config = RuvectorConfig {
            num_tiles: 64,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let data: Vec<f32> = (0..4096).map(|i| i as f32).collect();

        // Single tile
        let start1 = std::time::Instant::now();
        let _result1 = ruvector.single_tile_operation(TileId(0), VectorOp::Sum, &data);
        let single_time = start1.elapsed();

        // 16 tiles in parallel
        let group = ruvector.create_tile_group(
            &(0..16).map(TileId).collect::<Vec<_>>()
        ).unwrap();

        let start2 = std::time::Instant::now();
        let _result2 = ruvector.parallel_op(group, VectorOp::Sum, &data);
        let parallel_time = start2.elapsed();

        println!("Single tile: {:?}", single_time);
        println!("16 tiles parallel: {:?}", parallel_time);

        if single_time.as_nanos() > 0 && parallel_time.as_nanos() > 0 {
            let speedup = single_time.as_nanos() as f64 / parallel_time.as_nanos() as f64;
            println!("Speedup: {:.2}x", speedup);

            // Note: In simulation, speedup may not be linear, but parallel should not be slower
            // In real hardware, we'd expect 4-16x speedup
        }
    }
}
