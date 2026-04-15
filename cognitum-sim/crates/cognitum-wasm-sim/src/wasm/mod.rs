//! WASM execution engine simulation
//!
//! Implements the 8KB WASM system with:
//! - Bytecode decoder
//! - Instruction executor
//! - Stack with spill/fill
//! - Linear memory with bounds checking
//! - SIMD v128 operations

pub mod decoder;
pub mod executor;
pub mod stack;
pub mod memory;
pub mod simd;
pub mod opcodes;

pub use decoder::WasmDecoder;
pub use executor::WasmExecutor;
pub use stack::WasmStack;
pub use memory::WasmMemory;
pub use simd::SimdUnit;
pub use opcodes::{Opcode, OpType};

use crate::error::{Result, WasmSimError, WasmTrap};
use crate::network::Packet;

/// WASM engine configuration
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Code memory size in bytes (default: 8KB)
    pub code_memory_size: usize,

    /// Data memory size in bytes (default: 8KB)
    pub data_memory_size: usize,

    /// Work memory size in bytes (default: 64KB)
    pub work_memory_size: usize,

    /// Maximum WASM pages (64KB each)
    pub max_wasm_pages: u32,

    /// Initial WASM pages
    pub initial_wasm_pages: u32,

    /// Stack depth (register file size)
    pub stack_register_depth: usize,

    /// Shadow stack depth (call returns)
    pub shadow_stack_depth: usize,

    /// Enable SIMD operations
    pub enable_simd: bool,

    /// Enable neural extensions
    pub enable_neural: bool,

    /// Clock frequency in MHz
    pub clock_mhz: u32,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            code_memory_size: 8 * 1024,      // 8KB
            data_memory_size: 8 * 1024,      // 8KB
            work_memory_size: 64 * 1024,     // 64KB
            max_wasm_pages: 256,             // 16MB max
            initial_wasm_pages: 1,           // 64KB initial
            stack_register_depth: 16,        // 16 hardware registers
            shadow_stack_depth: 32,          // 32-entry call stack
            enable_simd: true,
            enable_neural: false,
            clock_mhz: 1000,                 // 1 GHz
        }
    }
}

/// WASM tile - single processor with WASM engine
pub struct WasmTile {
    /// Tile ID
    tile_id: u16,

    /// Configuration
    config: WasmConfig,

    /// Bytecode decoder
    decoder: WasmDecoder,

    /// Instruction executor
    executor: WasmExecutor,

    /// Value stack
    stack: WasmStack,

    /// Linear memory
    memory: WasmMemory,

    /// SIMD unit
    simd: SimdUnit,

    /// Program counter
    pc: u32,

    /// Halted state
    halted: bool,

    /// Trap state
    trap: Option<WasmTrap>,

    /// Instructions executed this cycle
    instructions_this_cycle: u32,

    /// Total instructions executed
    total_instructions: u64,

    /// Pending outbound packet
    pending_packet: Option<Packet>,
}

impl WasmTile {
    /// Create a new WASM tile
    pub fn new(tile_id: u16, config: WasmConfig) -> Result<Self> {
        let stack = WasmStack::new(
            config.stack_register_depth,
            config.shadow_stack_depth,
        );

        let memory = WasmMemory::new(
            config.code_memory_size,
            config.data_memory_size,
            config.work_memory_size,
            config.initial_wasm_pages,
            config.max_wasm_pages,
        )?;

        Ok(Self {
            tile_id,
            config: config.clone(),
            decoder: WasmDecoder::new(),
            executor: WasmExecutor::new(config.enable_simd, config.enable_neural),
            stack,
            memory,
            simd: SimdUnit::new(),
            pc: 0,
            halted: false,
            trap: None,
            instructions_this_cycle: 0,
            total_instructions: 0,
            pending_packet: None,
        })
    }

    /// Load WASM bytecode into code memory
    pub fn load_bytecode(&mut self, bytecode: &[u8]) -> Result<()> {
        if bytecode.len() > self.config.code_memory_size {
            return Err(WasmSimError::InvalidBytecode(format!(
                "Bytecode size {} exceeds code memory size {}",
                bytecode.len(),
                self.config.code_memory_size
            )));
        }

        self.memory.load_code(bytecode)?;
        self.pc = 0;
        self.halted = false;
        self.trap = None;
        self.total_instructions = 0;

        Ok(())
    }

    /// Execute one simulation cycle
    pub fn step(&mut self) -> Result<Option<Packet>> {
        self.instructions_this_cycle = 0;
        self.pending_packet = None;

        if self.halted || self.trap.is_some() {
            return Ok(None);
        }

        // Fetch bytecode at PC
        let bytecode = match self.memory.read_code(self.pc) {
            Ok(b) => b,
            Err(_) => {
                self.trap = Some(WasmTrap::MemoryAccessError);
                return Ok(None);
            }
        };

        // Decode instruction
        let decoded = self.decoder.decode(bytecode, &self.memory, self.pc)?;

        // Update PC based on instruction length
        self.pc += decoded.length as u32;

        // Execute instruction
        match self.executor.execute(
            &decoded,
            &mut self.stack,
            &mut self.memory,
            &mut self.simd,
            &mut self.pc,
        ) {
            Ok(effect) => {
                self.instructions_this_cycle = 1;
                self.total_instructions += 1;

                match effect {
                    ExecutionEffect::None => {}
                    ExecutionEffect::Halt => {
                        self.halted = true;
                    }
                    ExecutionEffect::Branch(target) => {
                        self.pc = target;
                    }
                    ExecutionEffect::SendPacket(packet) => {
                        self.pending_packet = Some(packet);
                    }
                    ExecutionEffect::Trap(trap) => {
                        self.trap = Some(trap);
                    }
                }
            }
            Err(e) => {
                if let WasmSimError::Trap(trap) = e {
                    self.trap = Some(trap);
                } else {
                    return Err(e);
                }
            }
        }

        Ok(self.pending_packet.take())
    }

    /// Get instructions executed this cycle
    pub fn instructions_this_cycle(&self) -> u32 {
        self.instructions_this_cycle
    }

    /// Get total instructions executed
    pub fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    /// Check if halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Get trap state
    pub fn trap(&self) -> Option<WasmTrap> {
        self.trap
    }

    /// Get current PC
    pub fn pc(&self) -> u32 {
        self.pc
    }

    /// Get tile ID
    pub fn tile_id(&self) -> u16 {
        self.tile_id
    }

    /// Peek at top of stack
    pub fn peek_stack(&self) -> Option<i32> {
        self.stack.peek()
    }

    /// Get stack depth
    pub fn stack_depth(&self) -> usize {
        self.stack.depth()
    }

    /// Read memory
    pub fn read_memory(&self, addr: u32) -> Result<i32> {
        self.memory.read_data(addr)
    }

    /// Write memory
    pub fn write_memory(&mut self, addr: u32, value: i32) -> Result<()> {
        self.memory.write_data(addr, value)
    }

    /// Reset tile
    pub fn reset(&mut self) {
        self.pc = 0;
        self.halted = false;
        self.trap = None;
        self.instructions_this_cycle = 0;
        self.total_instructions = 0;
        self.stack.clear();
        self.decoder.reset();
    }
}

/// Execution side effects
#[derive(Debug, Clone)]
pub enum ExecutionEffect {
    /// No side effect
    None,

    /// Halt execution
    Halt,

    /// Branch to address
    Branch(u32),

    /// Send packet over network
    SendPacket(Packet),

    /// Trap occurred
    Trap(WasmTrap),
}

/// WASM engine aggregate (for multi-tile coordination)
pub struct WasmEngine {
    /// All tiles
    tiles: Vec<WasmTile>,

    /// Global configuration
    config: WasmConfig,
}

impl WasmEngine {
    /// Create a new WASM engine with specified number of tiles
    pub fn new(num_tiles: usize, config: WasmConfig) -> Result<Self> {
        let mut tiles = Vec::with_capacity(num_tiles);
        for i in 0..num_tiles {
            tiles.push(WasmTile::new(i as u16, config.clone())?);
        }

        Ok(Self { tiles, config })
    }

    /// Get number of tiles
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get tile by ID
    pub fn tile(&self, id: u16) -> Option<&WasmTile> {
        self.tiles.get(id as usize)
    }

    /// Get mutable tile by ID
    pub fn tile_mut(&mut self, id: u16) -> Option<&mut WasmTile> {
        self.tiles.get_mut(id as usize)
    }

    /// Load bytecode into specific tile
    pub fn load(&mut self, tile_id: u16, bytecode: &[u8]) -> Result<()> {
        self.tile_mut(tile_id)
            .ok_or(WasmSimError::InvalidTileId(tile_id))?
            .load_bytecode(bytecode)
    }

    /// Step all tiles
    pub fn step_all(&mut self) -> Result<Vec<(u16, Option<Packet>)>> {
        let mut results = Vec::with_capacity(self.tiles.len());
        for tile in &mut self.tiles {
            let packet = tile.step()?;
            results.push((tile.tile_id(), packet));
        }
        Ok(results)
    }

    /// Get aggregate statistics
    pub fn stats(&self) -> WasmEngineStats {
        let mut stats = WasmEngineStats::default();
        for tile in &self.tiles {
            stats.total_instructions += tile.total_instructions();
            if tile.is_halted() {
                stats.halted_tiles += 1;
            }
            if tile.trap().is_some() {
                stats.trapped_tiles += 1;
            }
        }
        stats.active_tiles = self.tiles.len() - stats.halted_tiles - stats.trapped_tiles;
        stats
    }
}

/// WASM engine statistics
#[derive(Debug, Clone, Default)]
pub struct WasmEngineStats {
    pub total_instructions: u64,
    pub active_tiles: usize,
    pub halted_tiles: usize,
    pub trapped_tiles: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_creation() {
        let tile = WasmTile::new(0, WasmConfig::default()).unwrap();
        assert_eq!(tile.tile_id(), 0);
        assert!(!tile.is_halted());
    }

    #[test]
    fn test_engine_creation() {
        let engine = WasmEngine::new(16, WasmConfig::default()).unwrap();
        assert_eq!(engine.tile_count(), 16);
    }

    #[test]
    fn test_load_bytecode() {
        let mut tile = WasmTile::new(0, WasmConfig::default()).unwrap();
        let bytecode = vec![0x00, 0x0B]; // nop, end
        tile.load_bytecode(&bytecode).unwrap();
        assert_eq!(tile.pc(), 0);
    }
}
