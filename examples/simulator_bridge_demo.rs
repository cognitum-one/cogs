//! Simulator Bridge Integration Example
//!
//! This example demonstrates how to use the SimulatorBridge to connect
//! the ruvector partitioning system to the real Cognitum chip simulator.

use cognitum::ruvector::{SimulatorBridge, TaskEmbedding, PerformanceMetrics};
use cognitum::sdk::{CognitumSimulator, SimulatorConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cognitum Simulator Bridge Demo ===\n");

    // 1. Create the simulator
    println!("1. Creating Cognitum simulator with 256 tiles...");
    let config = SimulatorConfig::default();
    let mut simulator = CognitumSimulator::new(config)?;

    // 2. Enable ruvector routing
    println!("2. Enabling ruvector-based intelligent routing...");
    simulator = simulator.with_ruvector_routing();

    // 3. Get the bridge
    let bridge = simulator
        .get_routing_bridge()
        .expect("Routing bridge should be available");

    println!("   ✓ Bridge initialized with 256 tiles\n");

    // 4. Capture current tile states
    println!("3. Capturing tile states from simulator...");
    let states = bridge.capture_tile_states().await?;
    println!("   ✓ Captured {} tile states", states.len());

    // Show sample state
    if let Some((tile_id, state)) = states.first() {
        println!("   Sample tile {}: {} messages, {} cycles",
            tile_id.0, state.message_count, state.cycle_count);
    }
    println!();

    // 5. Route some tasks
    println!("4. Routing tasks to optimal tiles...");
    let mut routing_results = Vec::new();

    for i in 0..10 {
        let task = TaskEmbedding::random();
        let tile = bridge.route_task(&task).await?;
        routing_results.push(tile);

        if i < 3 {
            println!("   Task {} -> Tile {}", i, tile.0);
        }
    }
    println!("   ... (7 more tasks routed)\n");

    // 6. Get partition information
    println!("5. Checking partition assignments...");
    let mut partition_counts = std::collections::HashMap::new();

    for i in 0..256 {
        let tile = cognitum::ruvector::TileId(i);
        let partition = bridge.get_partition(tile);
        *partition_counts.entry(partition.0).or_insert(0) += 1;
    }

    println!("   Partition distribution:");
    for (partition_id, count) in partition_counts.iter() {
        println!("     Partition {}: {} tiles", partition_id, count);
    }
    println!();

    // 7. Update communication graph
    println!("6. Updating communication graph based on traffic...");
    let bridge_mut = simulator
        .get_routing_bridge_mut()
        .expect("Routing bridge should be available");

    bridge_mut.update_communication_graph().await?;
    println!("   ✓ Graph updated with current traffic patterns\n");

    // 8. Collect performance metrics
    println!("7. Collecting performance metrics...");
    let metrics = bridge.collect_metrics();
    print_metrics(&metrics);

    // 9. Rebalance partitions
    println!("\n8. Rebalancing partitions for optimal performance...");
    let rebalance_result = bridge_mut.rebalance().await?;

    println!("   Rebalancing completed:");
    println!("     Tiles moved: {}", rebalance_result.tiles_moved);
    println!("     Cut improvement: {}", rebalance_result.cut_improvement);
    println!("     New imbalance: {:.4}", rebalance_result.new_imbalance);
    println!("     Duration: {} ms", rebalance_result.duration_ms);

    // 10. Final metrics
    println!("\n9. Final performance metrics:");
    let final_metrics = bridge.collect_metrics();
    print_metrics(&final_metrics);

    println!("\n=== Demo Complete ===");
    Ok(())
}

fn print_metrics(metrics: &PerformanceMetrics) {
    println!("   Performance Metrics:");
    println!("     Active tiles: {}", metrics.active_tiles);
    println!("     Inter-partition traffic: {}", metrics.inter_partition_traffic);
    println!("     Intra-partition traffic: {}", metrics.intra_partition_traffic);
    println!("     Inter-partition ratio: {:.2}%", metrics.inter_partition_ratio() * 100.0);
    println!("     Partition imbalance: {:.4}", metrics.partition_imbalance);
    println!("     Min cut value: {:.0}", metrics.min_cut_value);
    println!("     Avg routing latency: {:.2} μs", metrics.routing_latency_us);
}
