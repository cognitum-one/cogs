//! Test suite for tile simulator (RED PHASE - TDD)

use cognitum_core::TileId;
use cognitum_sim::{Result, SimulationEvent, TileSimulator};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tile_initialization() -> Result<()> {
    let tile = TileSimulator::new(TileId::new(0).unwrap())?;

    assert_eq!(tile.tile_id(), TileId::new(0).unwrap());
    assert_eq!(tile.instruction_count(), 0);

    Ok(())
}

#[tokio::test]
async fn test_tile_runs_one_cycle() -> Result<()> {
    let mut tile = TileSimulator::new(TileId::new(0).unwrap())?;

    // Load simple program (NOP instruction)
    let program = vec![0x00000000]; // NOP
    tile.load_program(&program)?;

    // Run one cycle
    tile.run_one_cycle().await?;

    assert_eq!(tile.instruction_count(), 1);
    assert_eq!(tile.pc(), 1);

    Ok(())
}

#[tokio::test]
async fn test_tile_receives_raceway_packet() -> Result<()> {
    let mut tile = TileSimulator::new(TileId::new(0).unwrap())?;
    let (tx, rx) = mpsc::channel(4);

    tile.attach_raceway(rx);

    // Send a packet
    tx.send(SimulationEvent::PacketArrival {
        tile_id: TileId::new(0).unwrap(),
        time: 10,
    })
    .await
    .unwrap();

    // Process one cycle - should handle packet
    tile.run_one_cycle().await?;

    assert_eq!(tile.packets_received(), 1);

    Ok(())
}

#[tokio::test]
async fn test_fibonacci_execution() -> Result<()> {
    let mut tile = TileSimulator::new(TileId::new(0).unwrap())?;

    // Simple Fibonacci program (pseudo-code in A2S)
    let program = vec![
        0x08000001, // LITERAL 1
        0x08000001, // LITERAL 1
        // Loop:
        0x01000000, // DUP
        0x04000000, // OVER
        0x10000000, // ADD
                    // More instructions...
    ];

    tile.load_program(&program)?;

    // Run for 1000 cycles
    for _ in 0..1000 {
        tile.run_one_cycle().await?;
    }

    // Check stack has Fibonacci number
    assert!(tile.peek_stack() > 0);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_tile_execution() -> Result<()> {
    // Spawn 4 tiles concurrently
    let mut handles = Vec::new();

    for i in 0..4 {
        let handle = tokio::spawn(async move {
            let mut tile = TileSimulator::new(TileId::new(i).unwrap())?;

            let program = vec![0x00000000; 100]; // 100 NOPs
            tile.load_program(&program)?;

            for _ in 0..100 {
                tile.run_one_cycle().await?;
            }

            Ok::<_, cognitum_sim::SimulationError>(tile.instruction_count())
        });

        handles.push(handle);
    }

    // Wait for all tiles
    for handle in handles {
        let count = handle.await.unwrap()?;
        assert_eq!(count, 100);
    }

    Ok(())
}

#[tokio::test]
async fn test_tile_halts_correctly() -> Result<()> {
    let mut tile = TileSimulator::new(TileId::new(0).unwrap())?;

    let program = vec![
        0x00000000, // NOP
        0x00000000, // NOP
        0x3F000000, // HALT
    ];

    tile.load_program(&program)?;

    // Run until halt
    loop {
        match tile.run_one_cycle().await {
            Ok(status) if status.is_halted() => break,
            Ok(_) => continue,
            Err(e) => return Err(e),
        }
    }

    assert_eq!(tile.instruction_count(), 3);
    assert!(tile.is_halted());

    Ok(())
}
