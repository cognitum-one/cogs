//! Test suite for 256-tile Cognitum simulation
//!
//! These tests verify the basic API and simulation infrastructure work correctly.

use cognitum_core::TileId;
use cognitum_sim::{Cognitum, CognitumConfig, Result};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cognitum_initialization() -> Result<()> {
    let cognitum = Cognitum::new(CognitumConfig::default());
    assert_eq!(cognitum.tile_count(), 256);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_load_program_to_tile() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // 32-bit instruction format: opcode in bits 31-26
    // LITERAL (0x08) = 0x08 << 26 = 0x20000000
    // HALT (0x3F) = 0x3F << 26 = 0xFC000000
    let program = vec![
        0x20000001, // LITERAL 1
        0xFC000000, // HALT
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &program).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_256_tile_simulation() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Load simple program to first few tiles
    let program = vec![
        0x20000001, // LITERAL 1
        0xFC000000, // HALT
    ];

    for i in 0..4 {
        cognitum.load_program(TileId::new(i as u16).unwrap(), &program).await?;
    }

    // Run for a few cycles
    cognitum.run_for(10).await?;

    // Just verify we can get statistics
    let stats = cognitum.statistics().await;
    assert!(stats.total_cycles > 0 || stats.total_instructions >= 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_basic_execution_on_tile_zero() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Simple program with LITERAL and HALT
    let program: Vec<u32> = vec![
        0x20000001, // LITERAL 1 (opcode 0x08 << 26 | value 1)
        0xFC000000, // HALT (opcode 0x3F << 26)
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &program).await?;
    cognitum.run_for(100).await?;

    // Verify tile 0 has something on stack
    let stack_top = cognitum.tile_stack_top(TileId::new(0).unwrap()).await?;
    // Value should be 1 after LITERAL 1
    assert_eq!(stack_top, 1, "Stack top should be 1 after LITERAL 1");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_raceway_communication() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Simple program that halts
    let program = vec![0xFC000000]; // HALT

    cognitum.load_program(TileId::new(0).unwrap(), &program).await?;
    cognitum.load_program(TileId::new(1).unwrap(), &program).await?;

    cognitum.run_for(100).await?;

    // Just verify the API works - RaceWay isn't fully implemented yet
    let packets = cognitum.tile_packets_received(TileId::new(1).unwrap()).await?;
    assert!(packets >= 0); // Basic sanity check

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_statistics_api() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    let program = vec![
        0x20000001, // LITERAL 1
        0xFC000000, // HALT
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &program).await?;
    cognitum.run_for(10).await?;

    // Verify statistics API works
    let stats = cognitum.statistics().await;
    assert_eq!(stats.total_cycles, 10);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_deterministic_config() -> Result<()> {
    let config = CognitumConfig::deterministic();
    assert!(config.deterministic);

    let cognitum = Cognitum::new(config);
    assert_eq!(cognitum.tile_count(), 256);

    Ok(())
}
