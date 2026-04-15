//! Error types for WASM simulation

use thiserror::Error;

/// Result type for WASM simulation operations
pub type Result<T> = std::result::Result<T, WasmSimError>;

/// WASM simulation errors
#[derive(Error, Debug)]
pub enum WasmSimError {
    #[error("Invalid tile ID: {0}")]
    InvalidTileId(u16),

    #[error("Invalid WASM bytecode: {0}")]
    InvalidBytecode(String),

    #[error("WASM trap: {0}")]
    Trap(WasmTrap),

    #[error("Memory out of bounds: address {address:#x}, size {size}")]
    MemoryOutOfBounds { address: u32, size: u32 },

    #[error("Stack overflow: depth {0}")]
    StackOverflow(usize),

    #[error("Stack underflow")]
    StackUnderflow,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Topology error: {0}")]
    TopologyError(String),

    #[error("Scale configuration error: {0}")]
    ScaleError(String),

    #[error("Invalid opcode: {0:#x}")]
    InvalidOpcode(u8),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Integer overflow")]
    IntegerOverflow,

    #[error("Unimplemented instruction: {0}")]
    Unimplemented(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// WASM trap codes matching hardware specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmTrap {
    /// Unreachable instruction executed
    Unreachable = 0x00,

    /// Unknown/invalid opcode
    UnknownOpcode = 0x01,

    /// Memory access error
    MemoryAccessError = 0x02,

    /// Unimplemented i32 operation
    UnimplementedI32 = 0x03,

    /// FPU not implemented
    FpuNotImplemented = 0x04,

    /// Stack overflow
    StackOverflow = 0x05,

    /// Stack underflow
    StackUnderflow = 0x06,

    /// Call stack overflow
    CallStackOverflow = 0x07,

    /// Integer divide by zero
    IntegerDivideByZero = 0x08,

    /// Integer overflow
    IntegerOverflow = 0x09,

    /// Invalid conversion
    InvalidConversion = 0x0A,

    /// Indirect call type mismatch
    IndirectCallTypeMismatch = 0x0B,

    /// Table out of bounds
    TableOutOfBounds = 0x0C,

    /// Element out of bounds
    ElementOutOfBounds = 0x0D,

    /// Data segment out of bounds
    DataSegmentOutOfBounds = 0x0E,

    /// Memory limit exceeded
    MemoryLimitExceeded = 0x0F,
}

impl std::fmt::Display for WasmTrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmTrap::Unreachable => write!(f, "unreachable executed"),
            WasmTrap::UnknownOpcode => write!(f, "unknown opcode"),
            WasmTrap::MemoryAccessError => write!(f, "memory access error"),
            WasmTrap::UnimplementedI32 => write!(f, "unimplemented i32 operation"),
            WasmTrap::FpuNotImplemented => write!(f, "FPU not implemented"),
            WasmTrap::StackOverflow => write!(f, "stack overflow"),
            WasmTrap::StackUnderflow => write!(f, "stack underflow"),
            WasmTrap::CallStackOverflow => write!(f, "call stack overflow"),
            WasmTrap::IntegerDivideByZero => write!(f, "integer divide by zero"),
            WasmTrap::IntegerOverflow => write!(f, "integer overflow"),
            WasmTrap::InvalidConversion => write!(f, "invalid conversion"),
            WasmTrap::IndirectCallTypeMismatch => write!(f, "indirect call type mismatch"),
            WasmTrap::TableOutOfBounds => write!(f, "table out of bounds"),
            WasmTrap::ElementOutOfBounds => write!(f, "element out of bounds"),
            WasmTrap::DataSegmentOutOfBounds => write!(f, "data segment out of bounds"),
            WasmTrap::MemoryLimitExceeded => write!(f, "memory limit exceeded"),
        }
    }
}
