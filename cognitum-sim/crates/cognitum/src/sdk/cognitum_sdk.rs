//! Main SDK implementation with dependency injection

use super::config::CognitumConfig;
use super::errors::{Error, Result};
use super::events::HandlerId;
use super::results::{CycleResult, SimulationResults};
use super::state::{CycleState, SimulatorState};
use super::traits::{EventHandler, MetricsCollector, Simulator};
use std::time::Instant;

/// High-level Cognitum SDK with dependency injection
pub struct CognitumSDK {
    config: CognitumConfig,
    simulator: Box<dyn Simulator>,
    handlers: Vec<Box<dyn EventHandler>>,
    metrics: Option<Box<dyn MetricsCollector>>,
    program_loaded: bool,
    program_size: usize,
    cycle_count: u64,
}

impl std::fmt::Debug for CognitumSDK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CognitumSDK")
            .field("config", &self.config)
            .field("program_loaded", &self.program_loaded)
            .field("program_size", &self.program_size)
            .field("cycle_count", &self.cycle_count)
            .field("handlers_count", &self.handlers.len())
            .field("has_metrics", &self.metrics.is_some())
            .finish()
    }
}

impl CognitumSDK {
    /// Create new SDK with default configuration and real simulator
    pub fn new(config: CognitumConfig) -> Result<Self> {
        config.validate()?;
        // In production, this would create a real simulator
        // For now, we require using with_simulator for testing
        Err(Error::Simulator(
            "Use with_simulator() for testing or new_with_real_simulator() for production".into(),
        ))
    }

    /// Create SDK with injected simulator (for testing)
    pub fn with_simulator(simulator: Box<dyn Simulator>) -> Self {
        Self {
            config: CognitumConfig::default(),
            simulator,
            handlers: Vec::new(),
            metrics: None,
            program_loaded: false,
            program_size: 0,
            cycle_count: 0,
        }
    }

    /// Set configuration (builder pattern)
    pub fn with_config(mut self, config: CognitumConfig) -> Result<Self> {
        config.validate()?;
        self.config = config;
        Ok(self)
    }

    /// Add metrics collector
    pub fn with_metrics_collector(mut self, metrics: Box<dyn MetricsCollector>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Add event handler
    pub fn add_handler(&mut self, handler: Box<dyn EventHandler>) -> HandlerId {
        self.handlers.push(handler);
        HandlerId(self.handlers.len() - 1)
    }

    /// Remove event handler
    pub fn remove_handler(&mut self, id: HandlerId) {
        if id.0 < self.handlers.len() {
            self.handlers.remove(id.0);
        }
    }

    /// Load program into simulator
    pub fn load_program(&mut self, program: &[u8]) -> Result<()> {
        // Validate program is not empty
        if program.is_empty() {
            return Err(Error::EmptyProgram);
        }

        // Reset simulator before loading new program
        self.simulator.reset();

        // Reset metrics if present
        if let Some(metrics) = &mut self.metrics {
            metrics.reset();
        }

        // Load program into simulator
        self.simulator
            .load_program(program)
            .map_err(|e| Error::Simulator(e.to_string()))?;

        // Update internal state
        self.program_loaded = true;
        self.program_size = program.len();
        self.cycle_count = 0;

        Ok(())
    }

    /// Run simulation until completion
    pub fn run(&mut self) -> Result<SimulationResults> {
        self.check_program_loaded()?;

        let start = Instant::now();

        // Execute until completion
        let exec_result = self
            .simulator
            .execute(u64::MAX)
            .map_err(|e| Error::Simulator(e.to_string()))?;

        let execution_time_ns = start.elapsed().as_nanos() as u64;

        // Get metrics from simulator
        let sim_metrics = self.simulator.get_metrics();

        // Get summary from metrics collector if present
        let metrics_summary = self.metrics.as_ref().map(|m| m.get_summary());

        // Build results
        let results = SimulationResults {
            cycles_executed: exec_result.cycles,
            instructions_executed: metrics_summary
                .as_ref()
                .map(|m| m.total_instructions)
                .unwrap_or(sim_metrics.instructions),
            execution_time_ns,
            exit_reason: exec_result.exit_reason,
            memory_reads: metrics_summary
                .as_ref()
                .map(|m| m.memory_reads)
                .unwrap_or(sim_metrics.memory_reads),
            memory_writes: metrics_summary
                .as_ref()
                .map(|m| m.memory_writes)
                .unwrap_or(sim_metrics.memory_writes),
        };

        // Notify handlers
        for handler in &mut self.handlers {
            handler.on_complete(&results);
        }

        Ok(results)
    }

    /// Run simulation for specified number of cycles
    pub fn run_for(&mut self, cycles: u64) -> Result<SimulationResults> {
        self.check_program_loaded()?;

        if cycles == 0 {
            return Err(Error::InvalidCycleCount);
        }

        let start = Instant::now();

        // Execute for specified cycles
        let exec_result = self
            .simulator
            .execute(cycles)
            .map_err(|e| Error::Simulator(e.to_string()))?;

        let execution_time_ns = start.elapsed().as_nanos() as u64;

        // Get metrics
        let sim_metrics = self.simulator.get_metrics();
        let metrics_summary = self.metrics.as_ref().map(|m| m.get_summary());

        // Build results
        let results = SimulationResults {
            cycles_executed: exec_result.cycles,
            instructions_executed: metrics_summary
                .as_ref()
                .map(|m| m.total_instructions)
                .unwrap_or(sim_metrics.instructions),
            execution_time_ns,
            exit_reason: exec_result.exit_reason,
            memory_reads: metrics_summary
                .as_ref()
                .map(|m| m.memory_reads)
                .unwrap_or(sim_metrics.memory_reads),
            memory_writes: metrics_summary
                .as_ref()
                .map(|m| m.memory_writes)
                .unwrap_or(sim_metrics.memory_writes),
        };

        Ok(results)
    }

    /// Execute a single cycle
    pub fn step(&mut self) -> Result<CycleResult> {
        self.check_program_loaded()?;

        // Execute single step
        let step_result = self
            .simulator
            .step()
            .map_err(|e| Error::Simulator(e.to_string()))?;

        // Increment internal cycle counter
        self.cycle_count += 1;

        // Create cycle state for event handlers
        let cycle_state = CycleState {
            cycle: self.cycle_count,
            active_tiles: step_result.active_tiles.clone(),
            instructions: step_result.instructions_executed,
        };

        // Notify handlers
        for handler in &mut self.handlers {
            handler.on_cycle(self.cycle_count, &cycle_state);
        }

        // Return cycle result
        Ok(CycleResult {
            cycle: self.cycle_count,
            instructions_executed: step_result.instructions_executed,
            active_tiles: step_result.active_tiles,
        })
    }

    /// Get current simulator state
    pub fn get_state(&self) -> SimulatorState {
        let internal_state = self.simulator.get_state();

        let mut state = SimulatorState::new();
        state.program_loaded = self.program_loaded;
        state.program_size = self.program_size;
        state.current_cycle = internal_state.cycle;
        state.tiles = internal_state.tiles;
        state.set_memory(internal_state.memory);

        state
    }

    /// Reset simulator to initial state
    pub fn reset(&mut self) {
        self.simulator.reset();

        if let Some(metrics) = &mut self.metrics {
            metrics.reset();
        }

        self.program_loaded = false;
        self.program_size = 0;
        self.cycle_count = 0;
    }

    /// Check if program is loaded
    fn check_program_loaded(&self) -> Result<()> {
        if !self.program_loaded {
            return Err(Error::NoProgramLoaded);
        }
        Ok(())
    }

    /// Helper for tests to mark program as loaded without calling simulator
    /// This is pub so integration tests can use it too
    pub fn mark_program_loaded(&mut self) {
        self.program_loaded = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::traits::MockSimulator;

    #[test]
    fn test_sdk_creation_with_simulator() {
        let mock = MockSimulator::new();
        let sdk = CognitumSDK::with_simulator(Box::new(mock));
        assert!(!sdk.program_loaded);
    }

    #[test]
    fn test_config_validation() {
        let config = CognitumConfig::builder().tiles(300).build();
        assert!(config.is_err());
    }
}
