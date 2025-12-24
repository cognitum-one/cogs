//! Example using CognitumConfig builder for custom configuration

use cognitum::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Build custom configuration
    let config = CognitumConfig::builder()
        .tiles(64) // Use 64 tiles instead of 256
        .trace(true) // Enable execution tracing
        .trace_file("execution.log") // Save trace to file
        .max_cycles(1_000_000) // Limit to 1M cycles
        .worker_threads(16) // Use 16 worker threads
        .packet_timeout(Duration::from_millis(200)) // 200ms packet timeout
        .parallel_execution(true) // Enable parallel execution
        .build()?;

    println!("Configuration:");
    println!("  Tiles: {}", config.tiles);
    println!("  Worker threads: {}", config.worker_threads);
    println!("  Max cycles: {:?}", config.max_cycles);
    println!("  Trace enabled: {}", config.trace_enabled);
    println!("  Parallel execution: {}\n", config.parallel_execution);

    // Create simulator with custom config
    let mut cognitum = CognitumSDK::with_config(config)?;

    // Load a simple program
    let program = vec![0x30, 0x31, 0x28, 0x34];
    cognitum.load_program(TileId::new(0).unwrap(), &program)?;

    // Run simulation
    let results = cognitum.run().await?;

    println!("Results:");
    println!("  Cycles: {}", results.cycles);
    println!("  IPC: {:.2}", results.ipc());
    println!("  Cycles/sec: {:.2}", results.cycles_per_second());

    Ok(())
}
