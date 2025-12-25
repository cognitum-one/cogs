//! Simulation results and related types

/// Simulation execution results
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationResults {
    /// Number of cycles executed
    pub cycles_executed: u64,

    /// Number of instructions executed
    pub instructions_executed: u64,

    /// Wall-clock execution time in nanoseconds
    pub execution_time_ns: u64,

    /// Exit reason
    pub exit_reason: ExitReason,

    /// Memory reads
    pub memory_reads: u64,

    /// Memory writes
    pub memory_writes: u64,
}

impl Default for SimulationResults {
    fn default() -> Self {
        Self {
            cycles_executed: 0,
            instructions_executed: 0,
            execution_time_ns: 0,
            exit_reason: ExitReason::ProgramComplete,
            memory_reads: 0,
            memory_writes: 0,
        }
    }
}

/// Result from a single cycle step
#[derive(Debug, Clone, PartialEq)]
pub struct CycleResult {
    /// Current cycle number
    pub cycle: u64,

    /// Instructions executed in this cycle
    pub instructions_executed: u64,

    /// Active tiles in this cycle
    pub active_tiles: Vec<TileId>,
}

impl Default for CycleResult {
    fn default() -> Self {
        Self {
            cycle: 0,
            instructions_executed: 0,
            active_tiles: Vec::new(),
        }
    }
}

/// Exit reason for simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Program completed successfully
    ProgramComplete,

    /// Cycle limit reached
    CycleLimit,

    /// Error occurred
    Error,

    /// Manual halt
    Halt,
}

/// Tile identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId(pub u8);

/// Program identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramId(pub u64);

/// Execution result from simulator
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    /// Cycles executed
    pub cycles: u64,

    /// Exit reason
    pub exit_reason: ExitReason,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            cycles: 0,
            exit_reason: ExitReason::ProgramComplete,
        }
    }
}

/// Step result from simulator
#[derive(Debug, Clone, PartialEq)]
pub struct StepResult {
    /// Current cycle
    pub cycle: u64,

    /// Active tiles
    pub active_tiles: Vec<TileId>,

    /// Instructions executed in this step
    pub instructions_executed: u64,
}

impl Default for StepResult {
    fn default() -> Self {
        Self {
            cycle: 0,
            active_tiles: Vec::new(),
            instructions_executed: 0,
        }
    }
}
