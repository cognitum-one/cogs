//! SDK module for Cognitum chip v1
//!
//! This module provides SDK interfaces and client libraries for the Cognitum
//! ASIC simulator.
//!
//! # Quick Start
//!
//! ```no_run
//! use cognitum::sdk::{CognitumSimulator, SimulatorConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create simulator
//!     let config = SimulatorConfig::default();
//!     let mut simulator = CognitumSimulator::new(config)?;
//!
//!     // Load program (example bytecode)
//!     let bytecode = vec![0x01, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0xFC];
//!     let program = simulator.create_program(&bytecode).await?;
//!
//!     // Execute
//!     let result = simulator.execute(Some(1000)).await?;
//!     println!("Executed {} cycles", result.cycles_executed);
//!
//!     Ok(())
//! }
//! ```

pub mod core;
pub mod errors;
pub mod types;
pub mod validation;

// Re-exports
pub use core::CognitumSimulator;
pub use errors::{Result, SdkError};
pub use types::{
    ExecutionResult, MemorySnapshot, ProcessorState, ProgramHandle, SimulatorConfig,
};
pub use validation::{validate_bytecode, validate_config};
