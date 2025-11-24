//! Test suite for 256-tile Cognitum simulation (RED PHASE - TDD)

use cognitum_core::TileId;
use cognitum_sim::{Cognitum, CognitumConfig, Result};

#[tokio::test]
async fn test_cognitum_initialization() -> Result<()> {
    let cognitum = Cognitum::new(CognitumConfig::default());

    assert_eq!(cognitum.tile_count(), 256);

    Ok(())
}

#[tokio::test]
async fn test_load_program_to_tile() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    let program = vec![
        0x08000001, // LITERAL 1
        0x08000001, // LITERAL 1
        0x10000000, // ADD
        0x3F000000, // HALT
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &program)?;

    Ok(())
}

#[tokio::test]
async fn test_256_tile_simulation() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Load simple program to all tiles
    let program = vec![
        0x08000001, // LITERAL 1
        0x00000000, // NOP
        0x00000000, // NOP
        0x3F000000, // HALT
    ];

    for i in 0..256 {
        cognitum.load_program(TileId::new(i as u16).unwrap(), &program)?;
    }

    // Run for 10 cycles
    cognitum.run_for(10).await?;

    // All tiles should have executed some instructions
    let stats = cognitum.statistics();
    assert!(stats.total_instructions > 0);

    Ok(())
}

#[tokio::test]
async fn test_fibonacci_on_tile_zero() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Fibonacci program
    let fibonacci_program = vec![
        0x08000001, // LITERAL 1  (a)
        0x08000001, // LITERAL 1  (b)
        // Loop: (10 iterations)
        0x01000000, // DUP        (a b b)
        0x04000000, // OVER       (a b b a)
        0x10000000, // ADD        (a b c)
        0x03000000, // SWAP       (a c b)
        0x02000000, // DROP       (a c)
        // Repeat...
        0x3F000000, // HALT
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &fibonacci_program)?;

    // Run for 1000 cycles
    cognitum.run_for(1000).await?;

    // Tile 0 should have non-zero stack
    let stack_top = cognitum.tile_stack_top(TileId::new(0).unwrap())?;
    assert!(stack_top > 0);

    Ok(())
}

#[tokio::test]
async fn test_parallel_execution_256_tiles() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Load different programs to different tiles
    for i in 0..256 {
        let program = vec![
            0x08000000 | i, // LITERAL i
            0x08000000 | i, // LITERAL i
            0x10000000,     // ADD (result = 2*i)
            0x3F000000,     // HALT
        ];

        cognitum.load_program(TileId::new(i as u16).unwrap(), &program)?;
    }

    // Run all tiles in parallel
    cognitum.run_for(100).await?;

    // Check each tile has correct result
    for i in 0..256 {
        let stack_top = cognitum.tile_stack_top(TileId::new(i as u16).unwrap())?;
        assert_eq!(stack_top, 2 * i);
    }

    Ok(())
}

#[tokio::test]
async fn test_raceway_communication() -> Result<()> {
    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Program for tile 0: send message
    let sender_program = vec![
        0x08000042, // LITERAL 0x42
        // RaceWay send instruction (pseudo-code)
        0x3F000000, // HALT
    ];

    // Program for tile 1: receive message
    let receiver_program = vec![
        // RaceWay receive instruction
        0x3F000000, // HALT
    ];

    cognitum.load_program(TileId::new(0).unwrap(), &sender_program)?;
    cognitum.load_program(TileId::new(1).unwrap(), &receiver_program)?;

    cognitum.run_for(1000).await?;

    // Tile 1 should have received packet
    let packets = cognitum.tile_packets_received(TileId::new(1).unwrap())?;
    assert!(packets > 0);

    Ok(())
}

#[tokio::test]
async fn test_performance_target_1mips_per_tile() -> Result<()> {
    use std::time::Instant;

    let mut cognitum = Cognitum::new(CognitumConfig::default());

    // Load compute-intensive program
    let program = vec![0x00000000; 10000]; // 10K NOPs

    for i in 0..256 {
        cognitum.load_program(TileId::new(i as u16).unwrap(), &program)?;
    }

    let start = Instant::now();
    cognitum.run_for(10000).await?;
    let elapsed = start.elapsed();

    let stats = cognitum.statistics();
    let mips = (stats.total_instructions as f64) / elapsed.as_secs_f64() / 1_000_000.0;

    println!(
        "Achieved: {:.2} MIPS total ({:.2} MIPS/tile)",
        mips,
        mips / 256.0
    );

    // Target: > 1 MIPS per tile = > 256 MIPS total
    assert!(mips > 100.0, "Performance too low: {:.2} MIPS", mips);

    Ok(())
}

#[tokio::test]
async fn test_deterministic_replay() -> Result<()> {
    // Pause Tokio time for deterministic execution
    tokio::time::pause();

    let mut cognitum1 = Cognitum::new(CognitumConfig::deterministic());
    let mut cognitum2 = Cognitum::new(CognitumConfig::deterministic());

    let program = vec![
        0x08000001, // LITERAL 1
        0x08000002, // LITERAL 2
        0x10000000, // ADD
        0x3F000000, // HALT
    ];

    cognitum1.load_program(TileId::new(0).unwrap(), &program)?;
    cognitum2.load_program(TileId::new(0).unwrap(), &program)?;

    cognitum1.run_for(100).await?;
    cognitum2.run_for(100).await?;

    // Both should produce identical results
    assert_eq!(
        cognitum1.tile_stack_top(TileId::new(0).unwrap())?,
        cognitum2.tile_stack_top(TileId::new(0).unwrap())?
    );

    tokio::time::resume();
    Ok(())
}
