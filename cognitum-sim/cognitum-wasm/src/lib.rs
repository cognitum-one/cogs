use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Newport ASIC configuration
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewportConfig {
    num_tiles: u8,
    memory_size: u32,
    clock_freq_mhz: u32,
    enable_debug: bool,
}

#[wasm_bindgen]
impl NewportConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(num_tiles: u8, memory_size: u32, clock_freq_mhz: u32) -> NewportConfig {
        NewportConfig {
            num_tiles,
            memory_size,
            clock_freq_mhz,
            enable_debug: false,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn num_tiles(&self) -> u8 {
        self.num_tiles
    }

    #[wasm_bindgen(getter)]
    pub fn memory_size(&self) -> u32 {
        self.memory_size
    }

    #[wasm_bindgen(getter)]
    pub fn clock_freq_mhz(&self) -> u32 {
        self.clock_freq_mhz
    }

    #[wasm_bindgen(js_name = enableDebug)]
    pub fn enable_debug(&mut self) {
        self.enable_debug = true;
    }
}

impl Default for NewportConfig {
    fn default() -> Self {
        NewportConfig {
            num_tiles: 16,
            memory_size: 1024 * 1024, // 1MB per tile
            clock_freq_mhz: 1000,
            enable_debug: false,
        }
    }
}

/// Tile state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileState {
    pub tile_id: u8,
    pub pc: u32,
    pub registers: Vec<u32>,
    pub cycle_count: u64,
    pub status: String,
}

/// Main Newport WASM simulator
#[wasm_bindgen]
pub struct NewportWasm {
    config: NewportConfig,
    tiles: Vec<TileState>,
    total_cycles: u64,
    running: bool,
}

#[wasm_bindgen]
impl NewportWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(config: Option<NewportConfig>) -> Result<NewportWasm, JsValue> {
        let cfg = config.unwrap_or_default();

        // Initialize tiles
        let mut tiles = Vec::new();
        for i in 0..cfg.num_tiles {
            tiles.push(TileState {
                tile_id: i,
                pc: 0,
                registers: vec![0; 32],
                cycle_count: 0,
                status: "idle".to_string(),
            });
        }

        Ok(NewportWasm {
            config: cfg,
            tiles,
            total_cycles: 0,
            running: false,
        })
    }

    /// Load a program into a specific tile
    #[wasm_bindgen(js_name = loadProgram)]
    pub fn load_program(&mut self, tile_id: u8, program: &[u8]) -> Result<(), JsValue> {
        if tile_id >= self.config.num_tiles {
            return Err(JsValue::from_str(&format!(
                "Invalid tile ID: {} (max: {})",
                tile_id,
                self.config.num_tiles - 1
            )));
        }

        if program.is_empty() {
            return Err(JsValue::from_str("Program cannot be empty"));
        }

        // Reset tile state
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "ready".to_string();

            if self.config.enable_debug {
                web_sys::console::log_1(&format!(
                    "Loaded {} bytes into tile {}",
                    program.len(),
                    tile_id
                ).into());
            }
        }

        Ok(())
    }

    /// Run simulation for N cycles
    #[wasm_bindgen(js_name = runCycles)]
    pub async fn run_cycles(&mut self, cycles: u64) -> Result<(), JsValue> {
        if cycles == 0 {
            return Err(JsValue::from_str("Cycles must be greater than 0"));
        }

        self.running = true;

        for cycle in 0..cycles {
            // Simulate each tile
            for tile in &mut self.tiles {
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

            self.total_cycles += 1;

            // Yield to browser event loop every 1000 cycles
            if cycle % 1000 == 0 {
                let promise = js_sys::Promise::resolve(&JsValue::NULL);
                wasm_bindgen_futures::JsFuture::from(promise).await?;
            }
        }

        self.running = false;
        Ok(())
    }

    /// Get state of a specific tile
    #[wasm_bindgen(js_name = getState)]
    pub fn get_state(&self, tile_id: u8) -> Result<JsValue, JsValue> {
        if tile_id >= self.config.num_tiles {
            return Err(JsValue::from_str(&format!(
                "Invalid tile ID: {}",
                tile_id
            )));
        }

        if let Some(tile) = self.tiles.get(tile_id as usize) {
            serde_wasm_bindgen::to_value(tile)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        } else {
            Err(JsValue::from_str("Tile not found"))
        }
    }

    /// Get all tile states
    #[wasm_bindgen(js_name = getAllStates)]
    pub fn get_all_states(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.tiles)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reset a specific tile
    #[wasm_bindgen(js_name = resetTile)]
    pub fn reset_tile(&mut self, tile_id: u8) -> Result<(), JsValue> {
        if tile_id >= self.config.num_tiles {
            return Err(JsValue::from_str(&format!(
                "Invalid tile ID: {}",
                tile_id
            )));
        }

        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "idle".to_string();
        }

        Ok(())
    }

    /// Reset entire simulator
    #[wasm_bindgen(js_name = resetAll)]
    pub fn reset_all(&mut self) {
        for tile in &mut self.tiles {
            tile.pc = 0;
            tile.registers = vec![0; 32];
            tile.cycle_count = 0;
            tile.status = "idle".to_string();
        }
        self.total_cycles = 0;
        self.running = false;
    }

    /// Get total cycle count
    #[wasm_bindgen(js_name = getTotalCycles)]
    pub fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }

    /// Check if simulator is running
    #[wasm_bindgen(js_name = isRunning)]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get register value from a specific tile
    #[wasm_bindgen(js_name = getRegister)]
    pub fn get_register(&self, tile_id: u8, reg_num: usize) -> Result<u32, JsValue> {
        if tile_id >= self.config.num_tiles {
            return Err(JsValue::from_str("Invalid tile ID"));
        }

        if let Some(tile) = self.tiles.get(tile_id as usize) {
            if reg_num >= tile.registers.len() {
                return Err(JsValue::from_str("Invalid register number"));
            }
            Ok(tile.registers[reg_num])
        } else {
            Err(JsValue::from_str("Tile not found"))
        }
    }

    /// Set register value for a specific tile
    #[wasm_bindgen(js_name = setRegister)]
    pub fn set_register(&mut self, tile_id: u8, reg_num: usize, value: u32) -> Result<(), JsValue> {
        if tile_id >= self.config.num_tiles {
            return Err(JsValue::from_str("Invalid tile ID"));
        }

        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            if reg_num >= tile.registers.len() {
                return Err(JsValue::from_str("Invalid register number"));
            }
            tile.registers[reg_num] = value;
            Ok(())
        } else {
            Err(JsValue::from_str("Tile not found"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_config_creation() {
        let config = NewportConfig::new(8, 512 * 1024, 800);
        assert_eq!(config.num_tiles(), 8);
        assert_eq!(config.memory_size(), 512 * 1024);
        assert_eq!(config.clock_freq_mhz(), 800);
    }

    #[wasm_bindgen_test]
    fn test_newport_creation() {
        let newport = NewportWasm::new(None).unwrap();
        assert_eq!(newport.get_total_cycles(), 0);
        assert!(!newport.is_running());
    }

    #[wasm_bindgen_test]
    fn test_load_program() {
        let mut newport = NewportWasm::new(None).unwrap();
        let program = vec![0x01, 0x02, 0x03, 0x04];
        assert!(newport.load_program(0, &program).is_ok());
    }

    #[wasm_bindgen_test]
    fn test_reset_tile() {
        let mut newport = NewportWasm::new(None).unwrap();
        assert!(newport.reset_tile(0).is_ok());
        assert!(newport.reset_tile(255).is_err());
    }
}
