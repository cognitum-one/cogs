//! Error types for Cognitum SDK

use thiserror::Error;

/// Result type alias for Cognitum operations
pub type Result<T> = std::result::Result<T, CognitumError>;

/// Comprehensive error type for Cognitum SDK
#[derive(Error, Debug)]
pub enum CognitumError {
    /// Simulation execution error
    #[error("Simulation error: {0}")]
    Simulation(String),

    /// Program loading error
    #[error("Failed to load program: {0}")]
    LoadError(String),

    /// Configuration error
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Tile access error
    #[error("Tile {0} error: {1}")]
    TileError(u8, String),

    /// Memory operation error
    #[error("Memory error at 0x{0:08X}: {1}")]
    MemoryError(u32, String),

    /// RaceWay communication error
    #[error("RaceWay error: {0}")]
    RaceWayError(String),

    /// Timeout error
    #[error("Operation timed out after {0} cycles")]
    Timeout(u64),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// General error with context
    #[error("Cognitum error: {0}")]
    General(#[from] anyhow::Error),
}

impl CognitumError {
    /// Create a simulation error
    pub fn simulation(msg: impl Into<String>) -> Self {
        Self::Simulation(msg.into())
    }

    /// Create a load error
    pub fn load(msg: impl Into<String>) -> Self {
        Self::LoadError(msg.into())
    }

    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }

    /// Create a tile error
    pub fn tile(id: u8, msg: impl Into<String>) -> Self {
        Self::TileError(id, msg.into())
    }

    /// Create a memory error
    pub fn memory(addr: u32, msg: impl Into<String>) -> Self {
        Self::MemoryError(addr, msg.into())
    }

    /// Create a RaceWay error
    pub fn raceway(msg: impl Into<String>) -> Self {
        Self::RaceWayError(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(cycles: u64) -> Self {
        Self::Timeout(cycles)
    }
}
