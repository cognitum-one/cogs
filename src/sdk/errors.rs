//! SDK error types

use thiserror::Error;

/// SDK error types
#[derive(Error, Debug)]
pub enum SdkError {
    /// Invalid program
    #[error("Invalid program: {0}")]
    InvalidProgram(String),

    /// No program loaded
    #[error("No program loaded")]
    NoProgramLoaded,

    /// Simulation error
    #[error("Simulation error: {0}")]
    SimulationError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Memory error
    #[error("Memory error: {0}")]
    MemoryError(String),

    /// Tile error
    #[error("Tile error: {0}")]
    TileError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

/// Result type for SDK operations
pub type Result<T> = std::result::Result<T, SdkError>;
