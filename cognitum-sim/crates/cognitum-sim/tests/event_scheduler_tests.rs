//! Test suite for event scheduling system (RED PHASE - TDD)

use cognitum_core::TileId;
use cognitum_sim::{EventScheduler, Result, SimulationEngine, SimulationEvent};
use std::time::Duration;

#[tokio::test]
async fn test_event_scheduling() -> Result<()> {
    let mut scheduler = EventScheduler::new();

    // Schedule events at different times
    scheduler.schedule_at(10, SimulationEvent::ClockTick { cycle: 10 });
    scheduler.schedule_at(
        5,
        SimulationEvent::PacketArrival {
            tile_id: TileId::new(0).unwrap(),
            time: 5,
        },
    );
    scheduler.schedule_at(15, SimulationEvent::ClockTick { cycle: 15 });

    // Next event should be at time 5
    let next = scheduler.next_event().await?;
    assert_eq!(next.time(), 5);

    // Then time 10
    let next = scheduler.next_event().await?;
    assert_eq!(next.time(), 10);

    // Then time 15
    let next = scheduler.next_event().await?;
    assert_eq!(next.time(), 15);

    Ok(())
}

#[tokio::test]
async fn test_event_priority() -> Result<()> {
    let mut scheduler = EventScheduler::new();

    // Schedule events at same time with different priorities
    scheduler.schedule_at(10, SimulationEvent::ClockTick { cycle: 10 });
    scheduler.schedule_at(
        10,
        SimulationEvent::PacketArrival {
            tile_id: TileId::new(0).unwrap(),
            time: 10,
        },
    );
    scheduler.schedule_at(
        10,
        SimulationEvent::InstructionComplete {
            tile_id: TileId::new(0).unwrap(),
            pc: 0x100,
            cycles: 1,
        },
    );

    // Events at same time should be ordered by priority
    let next = scheduler.next_event().await?;
    assert!(matches!(next, SimulationEvent::PacketArrival { .. }));

    Ok(())
}

#[tokio::test]
async fn test_process_until() -> Result<()> {
    let mut scheduler = EventScheduler::new();

    // Schedule multiple events
    for i in 0..10 {
        scheduler.schedule_at(i, SimulationEvent::ClockTick { cycle: i });
    }

    // Process until time 5
    let processed = scheduler.process_until(5).await?;
    assert_eq!(processed.len(), 6); // Events at 0, 1, 2, 3, 4, 5

    // Process remaining
    let processed = scheduler.process_until(10).await?;
    assert_eq!(processed.len(), 4); // Events at 6, 7, 8, 9

    Ok(())
}

#[tokio::test]
async fn test_concurrent_event_scheduling() -> Result<()> {
    let mut engine = SimulationEngine::new();

    // Spawn multiple tasks that schedule events
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let mut eng = engine.clone();
            tokio::spawn(async move {
                eng.schedule_at(i * 10, SimulationEvent::ClockTick { cycle: i * 10 })
                    .await
            })
        })
        .collect();

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap()?;
    }

    // All events should be scheduled
    assert_eq!(engine.pending_events(), 10);

    Ok(())
}

#[tokio::test]
async fn test_deterministic_timing() -> Result<()> {
    // Pause Tokio time for deterministic testing
    tokio::time::pause();

    let mut engine = SimulationEngine::new();

    // Schedule event in the future
    engine.schedule_at(1000, SimulationEvent::ClockTick { cycle: 1000 });

    // Advance virtual time
    tokio::time::advance(Duration::from_nanos(1000)).await;

    // Event should be ready now
    let next = engine.next_event().await?;
    assert_eq!(next.time(), 1000);

    tokio::time::resume();
    Ok(())
}
