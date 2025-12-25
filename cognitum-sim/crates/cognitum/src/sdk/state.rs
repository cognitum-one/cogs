//! Simulator state types

use super::results::TileId;

/// Complete simulator state
#[derive(Debug, Clone)]
pub struct SimulatorState {
    /// Whether a program is loaded
    pub program_loaded: bool,

    /// Size of loaded program in bytes
    pub program_size: usize,

    /// Current cycle count
    pub current_cycle: u64,

    /// State of each tile
    pub tiles: Vec<TileState>,

    /// Memory snapshot
    memory: Vec<u8>,
}

impl SimulatorState {
    /// Create a new simulator state
    pub fn new() -> Self {
        Self {
            program_loaded: false,
            program_size: 0,
            current_cycle: 0,
            tiles: Vec::new(),
            memory: Vec::new(),
        }
    }

    /// Get memory contents at specified range
    pub fn get_memory(&self, start: usize, len: usize) -> Vec<u8> {
        let end = (start + len).min(self.memory.len());
        if start >= self.memory.len() {
            Vec::new()
        } else {
            self.memory[start..end].to_vec()
        }
    }

    /// Set memory contents (for internal use)
    pub(crate) fn set_memory(&mut self, memory: Vec<u8>) {
        self.memory = memory;
    }
}

impl Default for SimulatorState {
    fn default() -> Self {
        Self::new()
    }
}

/// State of a single tile
#[derive(Debug, Clone)]
pub struct TileState {
    /// Tile identifier
    pub id: TileId,

    /// Program counter
    pub program_counter: u64,

    /// Stack pointer
    pub stack_pointer: u8,
}

/// Internal simulator state (from Simulator trait)
#[derive(Debug, Clone)]
pub struct InternalState {
    /// Tile states
    pub tiles: Vec<TileState>,

    /// Current cycle
    pub cycle: u64,

    /// Memory contents
    pub memory: Vec<u8>,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            tiles: Vec::new(),
            cycle: 0,
            memory: Vec::new(),
        }
    }
}

/// Cycle state for event handling
#[derive(Debug, Clone)]
pub struct CycleState {
    /// Current cycle number
    pub cycle: u64,

    /// Active tiles
    pub active_tiles: Vec<TileId>,

    /// Instructions executed in this cycle
    pub instructions: u64,
}

/// Breakpoint information
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Tile where breakpoint hit
    pub tile: TileId,

    /// Program counter at breakpoint
    pub pc: u64,

    /// Cycle when hit
    pub cycle: u64,
}
