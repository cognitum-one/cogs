//! Error types for the Cognitum simulator

use cognitum_core::TileId;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimulationError {
    #[error("Tile {0} execution fault: {1}")]
    TileExecutionFault(TileId, String),

    #[error("Channel closed for tile {0}")]
    ChannelClosed(TileId),

    #[error("Event scheduling error: {0}")]
    SchedulingError(String),

    #[error("Time synchronization error: {0}")]
    TimeSyncError(String),

    #[error("Invalid tile ID: {0}")]
    InvalidTileId(u8),

    #[error("Processor error: {0}")]
    ProcessorError(String),

    #[error("Memory error: {0}")]
    MemoryError(String),

    #[error("RaceWay error: {0}")]
    RaceWayError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SimulationError>;
