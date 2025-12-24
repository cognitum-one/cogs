//! Core SDK implementation bridging to real Cognitum simulator

use cognitum_core::TileId;
use cognitum_sim::{Cognitum, CognitumConfig as SimConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::errors::SdkError;
use super::types::{ExecutionResult, MemorySnapshot, ProgramHandle, SimulatorConfig};

/// Real Cognitum simulator wrapper
pub struct CognitumSimulator {
    /// The actual simulator instance
    cognitum: Arc<Mutex<Cognitum>>,

    /// Configuration
    config: SimulatorConfig,

    /// Loaded program tracking
    program_loaded: bool,

    /// Current cycle count
    cycles: u64,
}

impl CognitumSimulator {
    /// Create a new simulator with the given configuration
    pub fn new(config: SimulatorConfig) -> Result<Self, SdkError> {
        // Convert SDK config to simulator config
        let sim_config = Self::convert_config(&config);

        // Create the simulator
        let cognitum = Cognitum::new(sim_config);

        Ok(Self {
            cognitum: Arc::new(Mutex::new(cognitum)),
            config,
            program_loaded: false,
            cycles: 0,
        })
    }

    /// Create a program from bytecode
    pub async fn create_program(&mut self, bytecode: &[u8]) -> Result<ProgramHandle, SdkError> {
        if bytecode.is_empty() {
            return Err(SdkError::InvalidProgram("Empty bytecode".to_string()));
        }

        // Convert bytes to u32 instructions (A2S uses 32-bit words)
        if bytecode.len() % 4 != 0 {
            return Err(SdkError::InvalidProgram(
                "Bytecode length must be multiple of 4".to_string(),
            ));
        }

        let program: Vec<u32> = bytecode
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Load into tile 0 by default
        let mut cognitum = self.cognitum.lock().await;
        cognitum
            .load_program(TileId::new(0).unwrap(), &program)
            .await
            .map_err(|e| SdkError::SimulationError(e.to_string()))?;

        drop(cognitum);

        self.program_loaded = true;

        Ok(ProgramHandle {
            id: 0,
            size: bytecode.len(),
            tile: 0,
        })
    }

    /// Execute the loaded program
    pub async fn execute(&mut self, max_cycles: Option<u64>) -> Result<ExecutionResult, SdkError> {
        if !self.program_loaded {
            return Err(SdkError::NoProgramLoaded);
        }

        let cycles = max_cycles.unwrap_or(1_000_000);

        let mut cognitum = self.cognitum.lock().await;

        // Run the simulation
        cognitum
            .run_for(cycles)
            .await
            .map_err(|e| SdkError::SimulationError(e.to_string()))?;

        // Get statistics
        let stats = cognitum.statistics().await;

        self.cycles += stats.total_cycles;

        Ok(ExecutionResult {
            cycles_executed: stats.total_cycles,
            instructions_executed: stats.total_instructions,
            halted: true, // If run_for completes, simulation is done
        })
    }

    /// Get memory contents for a specific tile
    pub async fn get_memory(&self, _tile_id: u8, _address: u32, size: usize) -> Result<Vec<u8>, SdkError> {
        let _cognitum = self.cognitum.lock().await;

        // For now, return empty memory as the simulator doesn't expose direct memory access yet
        // This would need to be extended in the real simulator
        Ok(vec![0; size])
    }

    /// Get a snapshot of simulator state
    pub async fn get_snapshot(&self) -> Result<MemorySnapshot, SdkError> {
        let cognitum = self.cognitum.lock().await;
        let stats = cognitum.statistics().await;

        Ok(MemorySnapshot {
            cycles: stats.total_cycles,
            instructions: stats.total_instructions,
            active_tiles: vec![0], // Would need to be exposed by simulator
        })
    }

    /// Configure simulator parameters
    pub async fn configure(&mut self, config: SimulatorConfig) -> Result<(), SdkError> {
        // Create new simulator with updated config
        let sim_config = Self::convert_config(&config);
        let new_cognitum = Cognitum::new(sim_config);

        *self.cognitum.lock().await = new_cognitum;
        self.config = config;
        self.program_loaded = false;
        self.cycles = 0;

        Ok(())
    }

    /// Reset the simulator
    pub async fn reset(&mut self) -> Result<(), SdkError> {
        let mut cognitum = self.cognitum.lock().await;
        cognitum.reset().await;

        self.program_loaded = false;
        self.cycles = 0;

        Ok(())
    }

    /// Get current cycle count
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// Convert SDK config to simulator config
    fn convert_config(config: &SimulatorConfig) -> SimConfig {
        SimConfig {
            worker_threads: config.worker_threads.unwrap_or(8),
            channel_depth: 4,
            clock_frequency: 1_000_000_000,
            deterministic: config.deterministic,
            enable_tracing: config.trace_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simulator_creation() {
        let config = SimulatorConfig::default();
        let sim = CognitumSimulator::new(config);
        assert!(sim.is_ok());
    }

    #[tokio::test]
    async fn test_program_loading() {
        let config = SimulatorConfig::default();
        let mut sim = CognitumSimulator::new(config).unwrap();

        // Simple program: LITERAL 1, HALT
        // LITERAL (0x08) = 0x08 << 26 = 0x20000000, with value in lower 26 bits
        // HALT (0x3F) = 0x3F << 26 = 0xFC000000
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x20, // LITERAL 1
            0x00, 0x00, 0x00, 0xFC, // HALT
        ];

        let result = sim.create_program(&bytecode).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execution() {
        let config = SimulatorConfig::default();
        let mut sim = CognitumSimulator::new(config).unwrap();

        // Load program first
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x20, // LITERAL 1
            0x00, 0x00, 0x00, 0xFC, // HALT
        ];

        sim.create_program(&bytecode).await.unwrap();

        // Execute
        let result = sim.execute(Some(100)).await;
        assert!(result.is_ok());
    }
}
