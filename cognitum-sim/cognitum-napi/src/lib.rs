use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewportConfigNode {
    pub num_tiles: u8,
    pub memory_size: u32,
    pub clock_freq_mhz: u32,
    pub enable_debug: bool,
}

impl Default for NewportConfigNode {
    fn default() -> Self {
        NewportConfigNode {
            num_tiles: 16,
            memory_size: 1024 * 1024,
            clock_freq_mhz: 1000,
            enable_debug: false,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileStateNode {
    pub tile_id: u8,
    pub pc: u32,
    pub registers: Vec<u32>,
    pub cycle_count: i64,
    pub status: String,
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_cycles: i64,
    pub active_tiles: u32,
    pub total_instructions: i64,
    pub avg_ipc: f64,
}

/// Internal Newport simulator state
struct NewportState {
    config: NewportConfigNode,
    tiles: Vec<TileStateNode>,
    total_cycles: u64,
    running: bool,
}

impl NewportState {
    fn new(config: NewportConfigNode) -> Self {
        let mut tiles = Vec::new();
        for i in 0..config.num_tiles {
            tiles.push(TileStateNode {
                tile_id: i,
                pc: 0,
                registers: vec![0; 32],
                cycle_count: 0,
                status: "idle".to_string(),
            });
        }

        NewportState {
            config,
            tiles,
            total_cycles: 0,
            running: false,
        }
    }
}

/// Main Newport Node.js native module
#[napi]
pub struct NewportNode {
    state: Arc<RwLock<NewportState>>,
}

#[napi]
impl NewportNode {
    #[napi(constructor)]
    pub fn new(config: Option<NewportConfigNode>) -> Result<Self> {
        let cfg = config.unwrap_or_default();
        let state = NewportState::new(cfg);

        Ok(NewportNode {
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// Load a program into a specific tile
    #[napi]
    pub async fn load_program(&self, tile_id: u8, program: Buffer) -> Result<()> {
        let mut state = self.state.write().await;

        if tile_id >= state.config.num_tiles {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "Invalid tile ID: {} (max: {})",
                    tile_id,
                    state.config.num_tiles - 1
                ),
            ));
        }

        if program.is_empty() {
            return Err(Error::new(Status::InvalidArg, "Program cannot be empty"));
        }

        // Reset tile state
        if let Some(tile) = state.tiles.get_mut(tile_id as usize) {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "ready".to_string();

            if state.config.enable_debug {
                println!("Loaded {} bytes into tile {}", program.len(), tile_id);
            }
        }

        Ok(())
    }

    /// Run simulation for N cycles (async)
    #[napi]
    pub async fn run_cycles(&self, cycles: u32) -> Result<()> {
        if cycles == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Cycles must be greater than 0",
            ));
        }

        let mut state = self.state.write().await;
        state.running = true;

        for _ in 0..cycles {
            // Simulate each tile
            for tile in &mut state.tiles {
                if tile.status == "ready" || tile.status == "running" {
                    tile.status = "running".to_string();
                    tile.cycle_count += 1;

                    // Simulate instruction execution (placeholder)
                    tile.pc += 4;

                    // Simple example: increment register 1
                    if let Some(reg) = tile.registers.get_mut(1) {
                        *reg = reg.wrapping_add(1);
                    }
                }
            }

            state.total_cycles += 1;
        }

        state.running = false;
        Ok(())
    }

    /// Run cycles synchronously (blocking)
    #[napi]
    pub fn run_cycles_sync(&self, cycles: u32) -> Result<()> {
        if cycles == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Cycles must be greater than 0",
            ));
        }

        // Use blocking runtime
        let runtime = tokio::runtime::Handle::current();
        runtime.block_on(async {
            let mut state = self.state.write().await;
            state.running = true;

            for _ in 0..cycles {
                for tile in &mut state.tiles {
                    if tile.status == "ready" || tile.status == "running" {
                        tile.status = "running".to_string();
                        tile.cycle_count += 1;
                        tile.pc += 4;

                        if let Some(reg) = tile.registers.get_mut(1) {
                            *reg = reg.wrapping_add(1);
                        }
                    }
                }
                state.total_cycles += 1;
            }

            state.running = false;
            Ok(())
        })
    }

    /// Get state of a specific tile
    #[napi]
    pub async fn get_state(&self, tile_id: u8) -> Result<TileStateNode> {
        let state = self.state.read().await;

        if tile_id >= state.config.num_tiles {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Invalid tile ID: {}", tile_id),
            ));
        }

        state
            .tiles
            .get(tile_id as usize)
            .cloned()
            .ok_or_else(|| Error::new(Status::GenericFailure, "Tile not found"))
    }

    /// Get all tile states
    #[napi]
    pub async fn get_all_states(&self) -> Result<Vec<TileStateNode>> {
        let state = self.state.read().await;
        Ok(state.tiles.clone())
    }

    /// Reset a specific tile
    #[napi]
    pub async fn reset_tile(&self, tile_id: u8) -> Result<()> {
        let mut state = self.state.write().await;

        if tile_id >= state.config.num_tiles {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Invalid tile ID: {}", tile_id),
            ));
        }

        if let Some(tile) = state.tiles.get_mut(tile_id as usize) {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "idle".to_string();
        }

        Ok(())
    }

    /// Reset entire simulator
    #[napi]
    pub async fn reset_all(&self) -> Result<()> {
        let mut state = self.state.write().await;

        for tile in &mut state.tiles {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "idle".to_string();
        }

        state.total_cycles = 0;
        state.running = false;
        Ok(())
    }

    /// Get total cycle count
    #[napi]
    pub async fn get_total_cycles(&self) -> Result<i64> {
        let state = self.state.read().await;
        Ok(state.total_cycles as i64)
    }

    /// Check if simulator is running
    #[napi]
    pub async fn is_running(&self) -> Result<bool> {
        let state = self.state.read().await;
        Ok(state.running)
    }

    /// Get register value from a specific tile
    #[napi]
    pub async fn get_register(&self, tile_id: u8, reg_num: u32) -> Result<u32> {
        let state = self.state.read().await;

        if tile_id >= state.config.num_tiles {
            return Err(Error::new(Status::InvalidArg, "Invalid tile ID"));
        }

        if let Some(tile) = state.tiles.get(tile_id as usize) {
            if reg_num as usize >= tile.registers.len() {
                return Err(Error::new(Status::InvalidArg, "Invalid register number"));
            }
            Ok(tile.registers[reg_num as usize])
        } else {
            Err(Error::new(Status::GenericFailure, "Tile not found"))
        }
    }

    /// Set register value for a specific tile
    #[napi]
    pub async fn set_register(&self, tile_id: u8, reg_num: u32, value: u32) -> Result<()> {
        let mut state = self.state.write().await;

        if tile_id >= state.config.num_tiles {
            return Err(Error::new(Status::InvalidArg, "Invalid tile ID"));
        }

        if let Some(tile) = state.tiles.get_mut(tile_id as usize) {
            if reg_num as usize >= tile.registers.len() {
                return Err(Error::new(Status::InvalidArg, "Invalid register number"));
            }
            tile.registers[reg_num as usize] = value;
            Ok(())
        } else {
            Err(Error::new(Status::GenericFailure, "Tile not found"))
        }
    }

    /// Get configuration
    #[napi]
    pub async fn get_config(&self) -> Result<NewportConfigNode> {
        let state = self.state.read().await;
        Ok(state.config.clone())
    }

    /// Get performance metrics
    #[napi]
    pub async fn get_metrics(&self) -> Result<PerformanceMetrics> {
        let state = self.state.read().await;

        let active_tiles = state
            .tiles
            .iter()
            .filter(|t| t.status != "idle")
            .count() as u32;

        let total_instructions: i64 = state.tiles.iter().map(|t| t.cycle_count).sum();

        let avg_ipc = if state.total_cycles > 0 {
            total_instructions as f64 / state.total_cycles as f64
        } else {
            0.0
        };

        Ok(PerformanceMetrics {
            total_cycles: state.total_cycles as i64,
            active_tiles,
            total_instructions,
            avg_ipc,
        })
    }
}
