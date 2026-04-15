//! SDK Core Module
//!
//! This module provides the high-level SDK for Cognitum with dependency injection
//! and comprehensive testing support via mockall.

pub mod cognitum_sdk;
pub mod config;
pub mod errors;
pub mod events;
pub mod results;
pub mod state;
pub mod traits;

// Re-export main types
pub use cognitum_sdk::CognitumSDK;
pub use config::{CognitumConfig, CognitumConfigBuilder, TileConfig};
pub use errors::{Error, Result, SimulatorError};
pub use events::{Event, HandlerId};
pub use results::{
    CycleResult, ExitReason, ExecutionResult, ProgramId, SimulationResults, StepResult, TileId,
};
pub use state::{Breakpoint, CycleState, InternalState, SimulatorState, TileState};
pub use traits::{
    CycleData, EventHandler, MemoryAccess, Metrics, MetricsCollector, MetricsSummary,
    RaceWayMessage, Simulator,
};

// Re-export mocks for testing (both unit and integration tests)
pub use traits::{MockEventHandler, MockMetricsCollector, MockSimulator};
