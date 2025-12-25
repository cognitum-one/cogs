//! Error types for SDK Core

use thiserror::Error;

/// Result type alias for SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// SDK Error types
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Empty program provided
    #[error("Program is empty")]
    EmptyProgram,

    /// Invalid program format
    #[error("Invalid program format: {0}")]
    InvalidProgram(String),

    /// No program loaded
    #[error("No program loaded")]
    NoProgramLoaded,

    /// Invalid cycle count
    #[error("Invalid cycle count: must be > 0")]
    InvalidCycleCount,

    /// Simulator error
    #[error("Simulator error: {0}")]
    Simulator(String),

    /// Event handler error
    #[error("Event handler error: {0}")]
    EventHandler(String),

    /// Metrics collector error
    #[error("Metrics error: {0}")]
    Metrics(String),
}

/// Simulator-specific errors
#[derive(Error, Debug, Clone)]
pub enum SimulatorError {
    /// Invalid program
    #[error("Invalid program: {0}")]
    InvalidProgram(String),

    /// Execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Step failed
    #[error("Step failed: {0}")]
    StepFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}
