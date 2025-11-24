//! Event-driven simulation engine
//!
//! Provides event scheduling, priority queues, and async event processing

use crate::{Result, SimulationError};
use cognitum_core::TileId;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Core simulation events
#[derive(Debug, Clone)]
pub enum SimulationEvent {
    /// Clock tick event
    ClockTick { cycle: u64 },

    /// Instruction execution completed
    InstructionComplete {
        tile_id: TileId,
        pc: u32,
        cycles: u64,
    },

    /// RaceWay packet arrives at tile
    PacketArrival { tile_id: TileId, time: u64 },

    /// Coprocessor operation completes
    CoprocessorComplete { tile_id: TileId, operation: String },

    /// Timer expiration
    TimerExpired { tile_id: TileId, timer_id: u32 },
}

impl SimulationEvent {
    fn time(&self) -> u64 {
        match self {
            SimulationEvent::ClockTick { cycle } => *cycle,
            SimulationEvent::InstructionComplete { cycles, .. } => *cycles,
            SimulationEvent::PacketArrival { time, .. } => *time,
            SimulationEvent::CoprocessorComplete { .. } => 0,
            SimulationEvent::TimerExpired { .. } => 0,
        }
    }

    fn priority(&self) -> EventPriority {
        match self {
            SimulationEvent::TimerExpired { .. } => EventPriority::High,
            SimulationEvent::CoprocessorComplete { .. } => EventPriority::High,
            SimulationEvent::PacketArrival { .. } => EventPriority::Normal,
            SimulationEvent::InstructionComplete { .. } => EventPriority::Low,
            SimulationEvent::ClockTick { .. } => EventPriority::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Scheduled event with timestamp and priority
#[derive(Clone)]
struct ScheduledEvent {
    time: u64,
    priority: EventPriority,
    event: SimulationEvent,
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Earlier times have higher priority (reverse order for min-heap)
        other
            .time
            .cmp(&self.time)
            .then(self.priority.cmp(&other.priority))
    }
}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.priority == other.priority
    }
}

impl Eq for ScheduledEvent {}

/// Event scheduler with priority queue
pub struct EventScheduler {
    /// Priority queue of scheduled events (sorted by time)
    events: BinaryHeap<ScheduledEvent>,

    /// Current simulation time (in nanoseconds)
    current_time: u64,

    /// Cycle count (for cycle-accurate mode)
    cycle_count: u64,
}

impl EventScheduler {
    pub fn new() -> Self {
        Self {
            events: BinaryHeap::new(),
            current_time: 0,
            cycle_count: 0,
        }
    }

    pub fn schedule_at(&mut self, time: u64, event: SimulationEvent) {
        let priority = event.priority();
        self.events.push(ScheduledEvent {
            time,
            priority,
            event,
        });
    }

    pub async fn next_event(&mut self) -> Result<SimulationEvent> {
        match self.events.pop() {
            Some(scheduled) => {
                self.current_time = scheduled.time;
                Ok(scheduled.event)
            }
            None => Err(SimulationError::SchedulingError(
                "No more events".to_string(),
            )),
        }
    }

    pub async fn process_until(&mut self, deadline: u64) -> Result<Vec<SimulationEvent>> {
        let mut processed = Vec::new();

        while let Some(scheduled) = self.events.peek() {
            if scheduled.time > deadline {
                break;
            }

            let scheduled = self.events.pop().unwrap();
            self.current_time = scheduled.time;
            processed.push(scheduled.event);
        }

        self.current_time = deadline;
        Ok(processed)
    }

    pub fn advance_cycle(&mut self) {
        self.cycle_count += 1;
        self.current_time += 1; // 1ns per cycle at 1 GHz
    }

    pub fn current_time(&self) -> u64 {
        self.current_time
    }

    pub fn pending_events(&self) -> usize {
        self.events.len()
    }
}

/// Thread-safe simulation engine
#[derive(Clone)]
pub struct SimulationEngine {
    inner: Arc<Mutex<EventScheduler>>,
}

impl SimulationEngine {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventScheduler::new())),
        }
    }

    pub async fn schedule_at(&mut self, time: u64, event: SimulationEvent) -> Result<()> {
        let mut scheduler = self.inner.lock().await;
        scheduler.schedule_at(time, event);
        Ok(())
    }

    pub async fn next_event(&mut self) -> Result<SimulationEvent> {
        let mut scheduler = self.inner.lock().await;
        scheduler.next_event().await
    }

    pub async fn pending_events(&self) -> usize {
        let scheduler = self.inner.lock().await;
        scheduler.pending_events()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_priority_ordering() {
        let mut scheduler = EventScheduler::new();

        scheduler.schedule_at(10, SimulationEvent::ClockTick { cycle: 10 });
        scheduler.schedule_at(
            5,
            SimulationEvent::PacketArrival {
                tile_id: TileId::new(0).unwrap(),
                time: 5,
            },
        );

        assert_eq!(scheduler.pending_events(), 2);
    }
}
