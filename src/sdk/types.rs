//! SDK type definitions

use serde::{Deserialize, Serialize};

/// Simulator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    /// Number of worker threads
    pub worker_threads: Option<usize>,

    /// Enable deterministic execution
    pub deterministic: bool,

    /// Enable execution tracing
    pub trace_enabled: bool,

    /// Maximum cycles before timeout
    pub max_cycles: Option<u64>,
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self {
            worker_threads: Some(8),
            deterministic: false,
            trace_enabled: false,
            max_cycles: None,
        }
    }
}

/// Handle to a loaded program
#[derive(Debug, Clone)]
pub struct ProgramHandle {
    /// Internal program ID
    pub id: u64,

    /// Program size in bytes
    pub size: usize,

    /// Tile the program is loaded on
    pub tile: u8,
}

/// Execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Number of cycles executed
    pub cycles_executed: u64,

    /// Number of instructions executed
    pub instructions_executed: u64,

    /// Whether execution halted normally
    pub halted: bool,
}

/// Memory snapshot
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    /// Current cycle count
    pub cycles: u64,

    /// Total instructions executed
    pub instructions: u64,

    /// Active tiles
    pub active_tiles: Vec<u8>,
}

/// Processor state for a single tile
#[derive(Debug, Clone)]
pub struct ProcessorState {
    /// Tile ID
    pub tile_id: u8,

    /// Program counter
    pub pc: u32,

    /// Stack depth
    pub stack_depth: usize,

    /// Is halted
    pub halted: bool,
}
