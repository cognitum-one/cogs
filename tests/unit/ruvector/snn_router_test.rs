//! Comprehensive tests for SNN Router

use cognitum::ruvector::*;
use std::time::Instant;

#[test]
fn test_lif_neuron_basic() {
    let mut neuron = LifNeuron::new(1.0, 0.1, 2);

    // Small input shouldn't spike
    assert!(!neuron.integrate(0.1, 0.0));
    assert!(neuron.membrane_potential() > 0.0);

    // Large input should spike
    assert!(neuron.integrate(1.5, 1.0));
    assert_eq!(neuron.membrane_potential(), 0.0);
}

#[test]
fn test_lif_neuron_leak() {
    let mut neuron = LifNeuron::new(1.0, 0.2, 2);

    neuron.integrate(0.5, 0.0);
    let potential1 = neuron.membrane_potential();

    neuron.integrate(0.0, 1.0); // No input, should leak
    let potential2 = neuron.membrane_potential();

    assert!(potential2 < potential1);
}

#[test]
fn test_stdp_timing_window() {
    let stdp = StdpRule::default();

    // Pre before post = LTP (positive)
    let ltp = stdp.compute_weight_change(0.0, 10.0);
    assert!(ltp > 0.0);

    // Post before pre = LTD (negative)
    let ltd = stdp.compute_weight_change(10.0, 0.0);
    assert!(ltd < 0.0);

    // Simultaneous = no change
    let no_change = stdp.compute_weight_change(5.0, 5.0);
    assert_eq!(no_change, 0.0);
}

#[test]
fn test_spiking_layer_creation() {
    let layer = SpikingLayer::new(128, 32, 0.8);
    let input = vec![0.5; 128];

    // Should not panic
    let mut layer_mut = layer;
    let spikes = layer_mut.forward(&input);
    assert_eq!(spikes.len(), 32);
}

#[test]
fn test_spiking_layer_sparsity() {
    let mut layer = SpikingLayer::new(100, 50, 0.85);
    let input = vec![0.8; 100]; // Strong input

    let mut total_spikes = 0;
    for _ in 0..50 {
        let spikes = layer.forward(&input);
        total_spikes += spikes.iter().filter(|&&s| s).count();
    }

    let avg_spikes = total_spikes as f32 / 50.0;
    let sparsity = 1.0 - (avg_spikes / 50.0);

    println!("Average spikes: {:.2}, Sparsity: {:.2}%", avg_spikes, sparsity * 100.0);
    assert!(sparsity > 0.5); // At least 50% sparse
}

#[test]
fn test_snn_router_creation() {
    let router = SnnRouter::new(16, 256);
    let task = TaskEmbedding::random();

    let tile = router.predict_tile(&task);
    assert!(tile.0 < 16);
}

#[test]
fn test_snn_router_consistency() {
    let router = SnnRouter::new(8, 256);
    let task = TaskEmbedding::random();

    // Same input should give same output
    let tile1 = router.predict_tile(&task);
    let tile2 = router.predict_tile(&task);

    assert_eq!(tile1, tile2);
}

#[test]
fn test_snn_router_confidence() {
    let router = SnnRouter::new(8, 256);
    let task = TaskEmbedding::random();

    let conf = router.confidence(&task);
    assert!(conf >= 0.0 && conf <= 1.0);
}

#[test]
fn test_snn_router_training() {
    let mut router = SnnRouter::new(4, 256);

    // Generate training data with clear patterns
    let traces: Vec<ExecutionTrace> = (0..100)
        .map(|i| {
            let mut task = TaskEmbedding::random();
            let tile_id = (i % 4) as u32;

            // Create strong pattern for each tile
            for j in 0..64 {
                task.data[tile_id as usize * 64 + j] = 0.9;
            }

            ExecutionTrace {
                task_embedding: task,
                actual_tile: TileId(tile_id),
                execution_time_us: 1000,
                success: true,
            }
        })
        .collect();

    let metrics = router.train(&traces).unwrap();

    println!("SNN Training Results:");
    println!("  Epochs: {}", metrics.epochs);
    println!("  Accuracy: {:.2}%", metrics.accuracy * 100.0);
    println!("  Final Loss: {:.4}", metrics.final_loss);

    // Should learn something
    assert!(metrics.accuracy > 0.2);
}

#[test]
fn test_snn_router_save_load() {
    let router = SnnRouter::new(4, 256);
    let test_task = TaskEmbedding::random();

    let pred_before = router.predict_tile(&test_task);

    let temp_path = std::env::temp_dir().join("snn_test_model.json");
    router.save_model(&temp_path).unwrap();

    let mut router2 = SnnRouter::new(4, 256);
    router2.load_model(&temp_path).unwrap();

    let pred_after = router2.predict_tile(&test_task);
    assert_eq!(pred_before, pred_after);

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_simd_batch_integration() {
    let mut neurons: Vec<LifNeuron> = (0..64)
        .map(|_| LifNeuron::new(1.0, 0.1, 2))
        .collect();

    let inputs = vec![0.3; 64];

    simd_integrate_batch(&mut neurons, &inputs);

    for neuron in &neurons {
        assert!(neuron.membrane_potential() > 0.25);
        assert!(neuron.membrane_potential() < 0.35);
    }
}

#[test]
fn test_snn_vs_tinydancer_comparison() {
    let num_tiles = 16;
    let input_dim = 256;

    let snn_router = SnnRouter::new(num_tiles, input_dim);
    let td_router = TinyDancerRouter::new(num_tiles, input_dim);

    let task = TaskEmbedding::random();

    let snn_tile = snn_router.predict_tile(&task);
    let td_tile = td_router.predict_tile(&task);

    // Both should return valid tiles
    assert!(snn_tile.0 < num_tiles as u32);
    assert!(td_tile.0 < num_tiles as u32);

    println!("SNN predicted: {:?}", snn_tile);
    println!("TinyDancer predicted: {:?}", td_tile);
}

#[test]
fn bench_snn_inference_speed() {
    let router = SnnRouter::new(16, 256);
    let tasks: Vec<TaskEmbedding> = (0..100)
        .map(|_| TaskEmbedding::random())
        .collect();

    let start = Instant::now();
    for task in &tasks {
        router.predict_tile(task);
    }
    let elapsed = start.elapsed();

    let avg_time = elapsed.as_micros() as f32 / tasks.len() as f32;
    println!("Average SNN inference time: {:.2} μs", avg_time);

    // Should be reasonably fast
    assert!(avg_time < 10000.0); // Less than 10ms per inference
}

#[test]
fn bench_simd_vs_scalar() {
    let size = 256;
    let iterations = 1000;

    let mut neurons: Vec<LifNeuron> = (0..size)
        .map(|_| LifNeuron::new(1.0, 0.1, 2))
        .collect();
    let inputs = vec![0.5; size];

    let start = Instant::now();
    for _ in 0..iterations {
        simd_integrate_batch(&mut neurons, &inputs);
    }
    let elapsed = start.elapsed();

    let avg_time = elapsed.as_nanos() as f32 / (size * iterations) as f32;
    println!("Average SIMD integration time: {:.2} ns/neuron", avg_time);

    assert!(avg_time < 1000.0); // Less than 1 μs per neuron
}

#[test]
fn test_lateral_inhibition_winner_take_all() {
    let mut layer = SpikingLayer::new(10, 10, 0.95); // Very high inhibition

    // Create input that would normally activate all neurons
    let input = vec![1.0; 10];

    let spikes = layer.forward(&input);
    let spike_count = spikes.iter().filter(|&&s| s).count();

    println!("Spikes with 95% lateral inhibition: {}", spike_count);

    // Should have very few spikes (winner-take-all)
    assert!(spike_count <= 2);
}

#[test]
fn test_temporal_coding() {
    let mut layer = SpikingLayer::new(5, 3, 0.0);

    // Early strong input should cause early spikes
    let strong_input = vec![0.9, 0.8, 0.7, 0.6, 0.5];
    let weak_input = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    let spikes_strong = layer.forward(&strong_input);
    let spikes_weak = layer.forward(&weak_input);

    let strong_count = spikes_strong.iter().filter(|&&s| s).count();
    let weak_count = spikes_weak.iter().filter(|&&s| s).count();

    // Strong input should produce more spikes
    assert!(strong_count >= weak_count);
}

#[test]
fn test_training_improves_performance() {
    let mut router = SnnRouter::new(4, 256);

    // Generate consistent training data
    let traces: Vec<ExecutionTrace> = (0..200)
        .map(|i| {
            let mut task = TaskEmbedding::random();
            let tile_id = (i % 4) as u32;

            // Tile-specific features
            for j in (tile_id * 64) as usize..((tile_id + 1) * 64) as usize {
                task.data[j] = 0.85;
            }

            ExecutionTrace {
                task_embedding: task,
                actual_tile: TileId(tile_id),
                execution_time_us: 1000,
                success: true,
            }
        })
        .collect();

    // Train
    let metrics = router.train(&traces).unwrap();

    // Validate on test set
    let test_traces: Vec<ExecutionTrace> = (0..40)
        .map(|i| {
            let mut task = TaskEmbedding::random();
            let tile_id = (i % 4) as u32;

            for j in (tile_id * 64) as usize..((tile_id + 1) * 64) as usize {
                task.data[j] = 0.85;
            }

            ExecutionTrace {
                task_embedding: task,
                actual_tile: TileId(tile_id),
                execution_time_us: 1000,
                success: true,
            }
        })
        .collect();

    let mut correct = 0;
    for trace in &test_traces {
        if router.predict_tile(&trace.task_embedding) == trace.actual_tile {
            correct += 1;
        }
    }
    let test_accuracy = correct as f32 / test_traces.len() as f32;

    println!("\nTraining Performance:");
    println!("  Training Accuracy: {:.2}%", metrics.accuracy * 100.0);
    println!("  Test Accuracy: {:.2}%", test_accuracy * 100.0);

    assert!(test_accuracy > 0.4); // Should generalize reasonably
}
