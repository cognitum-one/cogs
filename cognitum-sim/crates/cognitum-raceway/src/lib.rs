//! Cognitum RaceWay Interconnect
//!
//! High-performance message-passing network for the Cognitum ASIC.
//! Implements a 97-bit packet format with dimension-order routing,
//! broadcast support, and crossbar switching.
//!
//! # Packet Format
//!
//! The RaceWay uses 97-bit packets (96 data bits + 1 PUSH bit):
//! - Bit 96: PUSH (valid)
//! - Bits 95:88: COMMAND
//! - Bits 87:80: TAG
//! - Bits 79:72: DEST
//! - Bits 71:64: SOURCE
//! - Bits 63:32: WRITE_DATA / READ_DATA0
//! - Bits 31:0: ADDRESS / READ_DATA1
//!
//! # Architecture
//!
//! - 256 tiles in 16x16 array
//! - 16 columns of 8 tiles each
//! - 2 hubs connecting quadrants
//! - Dimension-order routing
//! - Broadcast support (column, quadrant, global)
//!
//! # Performance
//!
//! - Local (same column): 2-5 cycles
//! - Cross-hub: 15-25 cycles
//! - Broadcast: 20-30 cycles
//! - Throughput: ~500 GB/s (realistic with 50% utilization)

pub mod broadcast;
pub mod column;
pub mod error;
pub mod hub;
pub mod network;
pub mod packet;
pub mod tile;

pub use error::{RaceWayError, Result};
pub use network::RaceWayNetwork;
pub use packet::{Command, RaceWayPacket, TileId};
