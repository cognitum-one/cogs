//! Cognitum ASIC top-level simulator
//!
//! Coordinates 256 concurrent tile simulations using Tokio

use crate::{Result, SimulationError, TileSimulator};
use cognitum_core::TileId;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for Cognitum simulation
#[derive(Debug, Clone)]
pub struct CognitumConfig {
    /// Number of worker threads for Tokio runtime
    pub worker_threads: usize,

    /// Channel depth for RaceWay interfaces
    pub channel_depth: usize,

    /// Clock frequency in Hz
    pub clock_frequency: u64,

    /// Enable deterministic mode
    pub deterministic: bool,

    /// Enable detailed tracing
    pub enable_tracing: bool,
}

impl Default for CognitumConfig {
    fn default() -> Self {
        Self {
            worker_threads: 8,
            channel_depth: 4,
            clock_frequency: 1_000_000_000, // 1 GHz
            deterministic: false,
            enable_tracing: false,
        }
    }
}

impl CognitumConfig {
    pub fn deterministic() -> Self {
        Self {
            deterministic: true,
            ..Default::default()
        }
    }
}

/// Statistics from simulation run
#[derive(Debug, Clone, Default)]
pub struct SimulationStatistics {
    pub total_instructions: u64,
    pub total_cycles: u64,
    pub total_packets: u64,
    pub simulation_time_ns: u64,
}

/// Main Cognitum simulator containing 256 tiles
pub struct Cognitum {
    config: CognitumConfig,

    /// All 256 tiles
    tiles: Vec<Arc<Mutex<TileSimulator>>>,

    /// Simulation statistics
    stats: Arc<Mutex<SimulationStatistics>>,

    /// Running state
    running: Arc<Mutex<bool>>,
}

impl Cognitum {
    pub fn new(config: CognitumConfig) -> Self {
        // Create 256 tiles
        let mut tiles = Vec::with_capacity(256);
        for i in 0..256 {
            let tile = TileSimulator::new(TileId::new(i as u16).unwrap()).unwrap();
            tiles.push(Arc::new(Mutex::new(tile)));
        }

        Self {
            config,
            tiles,
            stats: Arc::new(Mutex::new(SimulationStatistics::default())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn load_program(&mut self, tile_id: TileId, program: &[u32]) -> Result<()> {
        let tile_idx = tile_id.value() as usize;
        if tile_idx >= 256 {
            return Err(SimulationError::InvalidTileId(tile_id.value()));
        }

        // Load program synchronously (we're not in async context)
        let tile = Arc::clone(&self.tiles[tile_idx]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut tile = tile.lock().await;
            tile.load_program(program)
        })
    }

    pub async fn run_for(&mut self, cycles: u64) -> Result<()> {
        *self.running.lock().await = true;

        // Spawn all 256 tiles as concurrent tasks
        let mut handles = Vec::new();

        for tile in &self.tiles {
            let tile = Arc::clone(tile);
            let running = Arc::clone(&self.running);

            let handle = tokio::spawn(async move {
                let mut cycle_count = 0u64;

                while *running.lock().await && cycle_count < cycles {
                    let mut tile = tile.lock().await;
                    match tile.run_one_cycle().await {
                        Ok(status) if status.is_halted() => break,
                        Ok(_) => {
                            cycle_count += 1;
                        }
                        Err(e) => return Err(e),
                    }
                    drop(tile); // Release lock between cycles
                    tokio::task::yield_now().await; // Allow other tasks to run
                }

                Ok::<u64, SimulationError>(cycle_count)
            });

            handles.push(handle);
        }

        // Wait for all tiles to complete or reach cycle limit
        let mut total_instructions = 0u64;
        for handle in handles {
            match handle.await {
                Ok(Ok(cycles)) => {
                    total_instructions += cycles;
                }
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(SimulationError::SchedulingError(e.to_string())),
            }
        }

        // Update statistics
        let mut stats = self.stats.lock().await;
        stats.total_instructions = total_instructions;
        stats.total_cycles = cycles;

        *self.running.lock().await = false;

        Ok(())
    }

    pub fn statistics(&self) -> SimulationStatistics {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let stats = self.stats.lock().await;
            stats.clone()
        })
    }

    pub fn tile_stack_top(&self, tile_id: TileId) -> Result<u32> {
        let tile_idx = tile_id.value() as usize;
        if tile_idx >= 256 {
            return Err(SimulationError::InvalidTileId(tile_id.value()));
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tile = self.tiles[tile_idx].lock().await;
            Ok(tile.peek_stack())
        })
    }

    pub fn tile_packets_received(&self, tile_id: TileId) -> Result<u64> {
        let tile_idx = tile_id.value() as usize;
        if tile_idx >= 256 {
            return Err(SimulationError::InvalidTileId(tile_id.value()));
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tile = self.tiles[tile_idx].lock().await;
            Ok(tile.packets_received())
        })
    }

    pub async fn reset(&mut self) {
        for tile in &self.tiles {
            let mut tile = tile.lock().await;
            // Reset would reload programs and reset state
            // For now, just clear running flag
        }
        *self.running.lock().await = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitum_creation() {
        let cognitum = Cognitum::new(CognitumConfig::default());
        assert_eq!(cognitum.tile_count(), 256);
    }

    #[tokio::test]
    async fn test_load_and_run() {
        let mut cognitum = Cognitum::new(CognitumConfig::default());

        let program = vec![
            0x08000001, // LITERAL 1
            0x3F000000, // HALT
        ];

        cognitum
            .load_program(TileId::new(0).unwrap(), &program)
            .unwrap();

        cognitum.run_for(10).await.unwrap();

        let stack = cognitum.tile_stack_top(TileId::new(0).unwrap()).unwrap();
        assert_eq!(stack, 1);
    }
}
