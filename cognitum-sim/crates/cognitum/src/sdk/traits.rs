//! Core traits for SDK with mockall support

use super::config::TileConfig;
use super::errors::SimulatorError;
use super::results::{ExecutionResult, ProgramId, StepResult};
use super::state::{Breakpoint, CycleState, InternalState};
use super::SimulationResults;
use mockall::automock;

/// Simulator trait - abstracts the underlying simulator
#[automock]
pub trait Simulator: Send + Sync {
    /// Load a compiled program into simulator memory
    fn load_program(&mut self, program: &[u8]) -> std::result::Result<ProgramId, SimulatorError>;

    /// Execute for specified cycles
    fn execute(&mut self, cycles: u64) -> std::result::Result<ExecutionResult, SimulatorError>;

    /// Execute single cycle
    fn step(&mut self) -> std::result::Result<StepResult, SimulatorError>;

    /// Get current simulator state
    fn get_state(&self) -> InternalState;

    /// Reset simulator to initial state
    fn reset(&mut self);

    /// Get accumulated metrics
    fn get_metrics(&self) -> Metrics;

    /// Set tile configuration
    fn configure_tiles(&mut self, config: TileConfig) -> std::result::Result<(), SimulatorError>;
}

/// Event handler trait - handles simulation events
#[automock]
pub trait EventHandler: Send + Sync {
    /// Called on each cycle
    fn on_cycle(&mut self, cycle: u64, state: &CycleState);

    /// Called when breakpoint is hit
    fn on_breakpoint(&mut self, bp: &Breakpoint);

    /// Called when error occurs
    fn on_error(&mut self, error: &SimulatorError);

    /// Called when simulation completes
    fn on_complete(&mut self, results: &SimulationResults);
}

/// Metrics collector trait - collects simulation metrics
#[automock]
pub trait MetricsCollector: Send + Sync {
    /// Record cycle data
    fn record_cycle(&mut self, cycle_data: &CycleData);

    /// Record memory access
    fn record_memory_access(&mut self, access: &MemoryAccess);

    /// Record RaceWay message
    fn record_message(&mut self, message: &RaceWayMessage);

    /// Get metrics summary
    fn get_summary(&self) -> MetricsSummary;

    /// Reset metrics
    fn reset(&mut self);
}

/// Metrics from simulator
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    /// Total instructions executed
    pub instructions: u64,

    /// Memory reads
    pub memory_reads: u64,

    /// Memory writes
    pub memory_writes: u64,

    /// RaceWay messages sent
    pub messages_sent: u64,
}

/// Cycle data for metrics
#[derive(Debug, Clone)]
pub struct CycleData {
    /// Cycle number
    pub cycle: u64,

    /// Instructions in this cycle
    pub instructions: u64,

    /// Active tiles
    pub active_tiles: usize,
}

/// Memory access record
#[derive(Debug, Clone)]
pub struct MemoryAccess {
    /// Address accessed
    pub address: u64,

    /// Whether this was a read (true) or write (false)
    pub is_read: bool,

    /// Size of access
    pub size: usize,

    /// Cycle when accessed
    pub cycle: u64,
}

/// RaceWay message record
#[derive(Debug, Clone)]
pub struct RaceWayMessage {
    /// Source tile
    pub source: u8,

    /// Destination tile
    pub destination: u8,

    /// Payload size
    pub payload_size: usize,

    /// Cycle when sent
    pub cycle: u64,
}

/// Metrics summary
#[derive(Debug, Clone, Default)]
pub struct MetricsSummary {
    /// Total instructions executed
    pub total_instructions: u64,

    /// Total memory reads
    pub memory_reads: u64,

    /// Total memory writes
    pub memory_writes: u64,

    /// Total messages sent
    pub messages_sent: u64,

    /// Average instructions per cycle
    pub avg_ipc: f64,
}
