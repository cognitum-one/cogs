//! Cognitum ASIC Simulator SDK
//!
//! High-level Rust SDK for the Cognitum 256-tile stack processor simulator.
//!
//! # Quick Start
//!
//! ```no_run
//! use cognitum::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), CognitumError> {
//!     // Create simulator with default configuration
//!     let mut cognitum = CognitumSDK::new()?;
//!
//!     // Load a program
//!     let program = vec![0x30, 0x31, 0x28, 0x34]; // ZERO, ONE, ADD, HALT
//!     cognitum.load_program(TileId(0), &program)?;
//!
//!     // Run simulation
//!     let results = cognitum.run().await?;
//!
//!     println!("Completed in {} cycles", results.cycles);
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

// Re-export core types
pub use cognitum_core::*;
pub use cognitum_memory::*;
pub use cognitum_processor::*;
pub use cognitum_raceway::*;
pub use cognitum_sim::*;

#[cfg(feature = "coprocessor")]
pub use cognitum_coprocessor::*;

#[cfg(feature = "io")]
pub use cognitum_io::*;

#[cfg(feature = "debug")]
pub use cognitum_debug::*;

pub mod config;
pub mod error;
pub mod prelude;
pub mod results;

// New SDK module with dependency injection
pub mod sdk;

// Legacy exports for backward compatibility
pub use config::{CognitumConfig, CognitumConfigBuilder};
pub use error::{CognitumError, Result};
pub use results::SimulationResults;

// Re-export new SDK
pub use sdk::CognitumSDK;
