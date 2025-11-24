//! Error types for RaceWay operations

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum RaceWayError {
    #[error("Invalid packet format: {0}")]
    InvalidPacket(String),

    #[error("Invalid tile ID: {0}")]
    InvalidTileId(u8),

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("Channel full, cannot send packet")]
    ChannelFull,

    #[error("Invalid command: {0:#x}")]
    InvalidCommand(u8),
}

pub type Result<T> = std::result::Result<T, RaceWayError>;
