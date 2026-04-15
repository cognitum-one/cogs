//! Prelude module for convenient imports
//!
//! Import everything you need for basic Cognitum usage:
//!
//! ```
//! use cognitum::prelude::*;
//! ```

pub use crate::config::{CognitumConfig, CognitumConfigBuilder};
pub use crate::error::{CognitumError, Result};
pub use crate::results::SimulationResults;
pub use crate::sdk::CognitumSDK;

// Re-export common types from core modules
pub use cognitum_core::{MemoryAddress, TileId};
// Note: Some types may not be exported from upstream crates yet
// pub use cognitum_memory::MemoryError;
// pub use cognitum_processor::{ProcessorState, RegisterId};
pub use cognitum_raceway::RaceWayPacket;
