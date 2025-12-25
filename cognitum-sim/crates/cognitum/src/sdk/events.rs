//! Event handling for SDK

use super::errors::SimulatorError;
use super::results::SimulationResults;
use super::state::Breakpoint;

/// Event types emitted during simulation
#[derive(Debug, Clone)]
pub enum Event {
    /// Cycle started
    Cycle(u64),

    /// Breakpoint hit
    Breakpoint(Breakpoint),

    /// Error occurred
    Error(SimulatorError),

    /// Simulation complete
    Complete(SimulationResults),
}

/// Handler ID for removing handlers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(pub usize);
