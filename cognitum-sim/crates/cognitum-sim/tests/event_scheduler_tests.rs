//! Test suite for event scheduling system
//!
//! Tests the EventScheduler and SimulationEngine APIs.

use cognitum_core::TileId;
use cognitum_sim::{EventScheduler, Result, SimulationEngine, SimulationEvent};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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

    // Next event should be at time 5 (earliest)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
    // PacketArrival has Normal priority, ClockTick and InstructionComplete have Low
    let next = scheduler.next_event().await?;
    assert!(matches!(next, SimulationEvent::PacketArrival { .. }));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_simulation_engine_scheduling() -> Result<()> {
    let mut engine = SimulationEngine::new();

    // Schedule events sequentially
    for i in 0..10 {
        engine
            .schedule_at(i * 10, SimulationEvent::ClockTick { cycle: i * 10 })
            .await?;
    }

    // All events should be scheduled
    let pending = engine.pending_events().await;
    assert_eq!(pending, 10);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pending_events_count() -> Result<()> {
    let mut scheduler = EventScheduler::new();

    // Initially no events
    assert_eq!(scheduler.pending_events(), 0);

    // Add events
    scheduler.schedule_at(10, SimulationEvent::ClockTick { cycle: 10 });
    scheduler.schedule_at(20, SimulationEvent::ClockTick { cycle: 20 });

    assert_eq!(scheduler.pending_events(), 2);

    // Pop one event
    let _ = scheduler.next_event().await?;
    assert_eq!(scheduler.pending_events(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_current_time_tracking() -> Result<()> {
    let mut scheduler = EventScheduler::new();

    // Initial time is 0
    assert_eq!(scheduler.current_time(), 0);

    // Schedule and process event
    scheduler.schedule_at(100, SimulationEvent::ClockTick { cycle: 100 });
    let _ = scheduler.next_event().await?;

    // Time should advance to event time
    assert_eq!(scheduler.current_time(), 100);

    Ok(())
}
