//! Cognitum ASIC Event-Driven Simulation Engine
//!
//! This crate provides a high-performance, async-based simulation framework
//! for the 256-processor Cognitum ASIC using Tokio's event-driven runtime.

pub mod error;
pub mod event;
pub mod cognitum;
pub mod tile;
pub mod time;

pub use error::{Result, SimulationError};
pub use event::{EventScheduler, SimulationEngine, SimulationEvent};
pub use cognitum::{Cognitum, CognitumConfig};
pub use tile::TileSimulator;
pub use time::TimeManager;

/// Re-export common types
pub use cognitum_core::TileId;
