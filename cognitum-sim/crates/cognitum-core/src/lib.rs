// Cognitum Core Library
// Core types and memory system for Cognitum ASIC simulator

pub mod error;
pub mod memory;
pub mod types;

pub use error::CognitumError;
pub use memory::{Memory, RAM};
pub use types::{Instruction, MemoryAddress, PhysAddr, Register, TileId, VirtAddr};

pub type Result<T> = std::result::Result<T, CognitumError>;
