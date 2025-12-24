//! Simple SNN Router demonstration
//!
//! Run with: cargo run --example snn_router_demo

use cognitum::ruvector::*;
use std::path::Path;

fn main() -> Result<(), RouterError> {
    println!("=== SNN Router Demonstration ===\n");

    // 1. Create router
    println!("1. Creating SNN Router (16 tiles, 256D embeddings)...");
    let mut router = SnnRouter::new(16, 256);

    // 2. Test basic inference
    println!("\n2. Testing basic inference...");
    let task1 = TaskEmbedding::from_description("vector_add", &[1024, 1024]);
    let task2 = TaskEmbedding::from_description("matrix_multiply", &[512, 512]);
    let task3 = TaskEmbedding::from_description("convolution", &[256, 256, 3]);

    let tile1 = router.predict_tile(&task1);
    let tile2 = router.predict_tile(&task2);
    let tile3 = router.predict_tile(&task3);

    println!("  vector_add       → Tile {} (confidence: {:.1}%)",
             tile1.0, router.confidence(&task1) * 100.0);
    println!("  matrix_multiply  → Tile {} (confidence: {:.1}%)",
             tile2.0, router.confidence(&task2) * 100.0);
    println!("  convolution      → Tile {} (confidence: {:.1}%)",
             tile3.0, router.confidence(&task3) * 100.0);

    // 3. Generate training data
    println!("\n3. Generating training data...");
    let traces = generate_training_data(200, 16);
    println!("  Generated {} execution traces", traces.len());

    // 4. Train the router
    println!("\n4. Training SNN Router...");
    let metrics = router.train(&traces)?;
    println!("  ✓ Training complete!");
    println!("    Epochs:   {}", metrics.epochs);
    println!("    Accuracy: {:.2}%", metrics.accuracy * 100.0);
    println!("    Loss:     {:.4}", metrics.final_loss);

    // 5. Test after training
    println!("\n5. Testing after training...");
    let test_traces = generate_training_data(40, 16);
    let mut correct = 0;

    for trace in &test_traces {
        if router.predict_tile(&trace.task_embedding) == trace.actual_tile {
            correct += 1;
        }
    }

    let test_accuracy = correct as f32 / test_traces.len() as f32;
    println!("  Test accuracy: {:.2}%", test_accuracy * 100.0);

    // 6. Save model
    println!("\n6. Saving trained model...");
    let model_path = Path::new("/tmp/snn_router_demo.json");
    router.save_model(model_path)?;
    println!("  ✓ Model saved to {:?}", model_path);

    // 7. Load model
    println!("\n7. Loading model into new router...");
    let mut new_router = SnnRouter::new(16, 256);
    new_router.load_model(model_path)?;
    println!("  ✓ Model loaded successfully");

    // Verify loaded model works
    let tile_before = router.predict_tile(&task1);
    let tile_after = new_router.predict_tile(&task1);
    assert_eq!(tile_before, tile_after);
    println!("  ✓ Loaded model produces same predictions");

    // 8. Demonstrate neuron dynamics
    println!("\n8. Demonstrating LIF neuron dynamics...");
    demonstrate_neuron();

    // 9. Demonstrate STDP learning
    println!("\n9. Demonstrating STDP learning...");
    demonstrate_stdp();

    // 10. Demonstrate lateral inhibition
    println!("\n10. Demonstrating lateral inhibition...");
    demonstrate_lateral_inhibition();

    println!("\n=== Demo Complete ===");
    Ok(())
}

fn generate_training_data(num_samples: usize, num_tiles: usize) -> Vec<ExecutionTrace> {
    (0..num_samples)
        .map(|i| {
            let mut task = TaskEmbedding::random();
            let tile_id = (i % num_tiles) as u32;

            // Create tile-specific patterns
            for j in 0..16 {
                task.data[tile_id as usize * 16 + j] = 0.85 + rand::random::<f32>() * 0.15;
            }

            ExecutionTrace {
                task_embedding: task,
                actual_tile: TileId(tile_id),
                execution_time_us: 1000 + (i % 500) as u64,
                success: true,
            }
        })
        .collect()
}

fn demonstrate_neuron() {
    let mut neuron = LifNeuron::new(1.0, 0.1, 2);

    println!("  LIF Neuron (threshold=1.0, leak=0.1, refractory=2)");
    println!("  Time | Input | Potential | Spike");
    println!("  -----|-------|-----------|------");

    let inputs = vec![0.3, 0.4, 0.5, 0.2, 0.8, 0.3, 0.3, 0.5];

    for (t, &input) in inputs.iter().enumerate() {
        let spike = neuron.integrate(input, t as f32);
        println!("  {:4} | {:5.2} | {:9.3} | {}",
                 t, input, neuron.membrane_potential(),
                 if spike { "🔥 SPIKE!" } else { "-" });
    }
}

fn demonstrate_stdp() {
    let stdp = StdpRule::default();

    println!("  STDP Learning Rule");
    println!("  Δt (ms) | Weight Change | Type");
    println!("  --------|---------------|------");

    let timings = vec![-20.0, -10.0, -5.0, 0.0, 5.0, 10.0, 20.0];

    for &dt in &timings {
        let pre_time = 0.0;
        let post_time = dt;
        let dw = stdp.compute_weight_change(pre_time, post_time);

        let change_type = if dw > 0.0 {
            "LTP ↑"
        } else if dw < 0.0 {
            "LTD ↓"
        } else {
            "None"
        };

        println!("  {:7.1} | {:13.6} | {}", dt, dw, change_type);
    }
}

fn demonstrate_lateral_inhibition() {
    println!("  Testing different lateral inhibition strengths:");
    println!();

    for &inhibition in &[0.0, 0.5, 0.8, 0.95] {
        let mut layer = SpikingLayer::new(100, 20, inhibition);
        let input = vec![0.8; 100]; // Strong input

        let mut total_spikes = 0;
        for _ in 0..50 {
            let spikes = layer.forward(&input);
            total_spikes += spikes.iter().filter(|&&s| s).count();
        }

        let avg_spikes = total_spikes as f32 / 50.0;
        let sparsity = 1.0 - (avg_spikes / 20.0);

        println!("  Inhibition: {:.2} → Avg spikes: {:.1}/20 (sparsity: {:.0}%)",
                 inhibition, avg_spikes, sparsity * 100.0);
    }

    println!();
    println!("  ✓ Higher inhibition → More sparse activation");
}
