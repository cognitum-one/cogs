//! Tile simulator implementation
//!
//! Each tile contains an A2S processor, local memory, and RaceWay interface

use crate::{Result, SimulationError, SimulationEvent};
use cognitum_core::TileId;
use tokio::sync::mpsc;

/// Execution status returned after each cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Continue,
    Halted,
    Blocked,
}

impl ExecutionStatus {
    pub fn is_halted(&self) -> bool {
        matches!(self, ExecutionStatus::Halted)
    }
}

/// Simulates a single Cognitum tile (processor + memory + interfaces)
pub struct TileSimulator {
    tile_id: TileId,

    // Processor state
    pc: u32,
    instruction_count: u64,
    halted: bool,

    // Memory
    memory: Vec<u32>,

    // Data stack
    d_stack: Vec<u32>,

    // RaceWay interface
    raceway_rx: Option<mpsc::Receiver<SimulationEvent>>,
    raceway_tx: Option<mpsc::Sender<SimulationEvent>>,
    packets_received: u64,
    packets_sent: u64,
}

impl TileSimulator {
    pub fn new(tile_id: TileId) -> Result<Self> {
        Ok(Self {
            tile_id,
            pc: 0,
            instruction_count: 0,
            halted: false,
            memory: vec![0; 65536], // 64K words of memory
            d_stack: Vec::with_capacity(32),
            raceway_rx: None,
            raceway_tx: None,
            packets_received: 0,
            packets_sent: 0,
        })
    }

    pub fn tile_id(&self) -> TileId {
        self.tile_id
    }

    pub fn instruction_count(&self) -> u64 {
        self.instruction_count
    }

    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub fn peek_stack(&self) -> u32 {
        self.d_stack.last().copied().unwrap_or(0)
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn packets_received(&self) -> u64 {
        self.packets_received
    }

    pub fn load_program(&mut self, program: &[u32]) -> Result<()> {
        if program.len() > self.memory.len() {
            return Err(SimulationError::MemoryError(
                "Program too large".to_string(),
            ));
        }

        self.memory[..program.len()].copy_from_slice(program);
        self.pc = 0;
        self.halted = false;
        self.instruction_count = 0;

        Ok(())
    }

    pub fn attach_raceway(&mut self, rx: mpsc::Receiver<SimulationEvent>) {
        self.raceway_rx = Some(rx);
    }

    pub async fn run_one_cycle(&mut self) -> Result<ExecutionStatus> {
        // Check for incoming RaceWay packets
        if let Some(rx) = &mut self.raceway_rx {
            if let Ok(event) = rx.try_recv() {
                self.handle_raceway_event(event).await?;
            }
        }

        // If halted, don't execute
        if self.halted {
            return Ok(ExecutionStatus::Halted);
        }

        // Fetch instruction
        if self.pc as usize >= self.memory.len() {
            self.halted = true;
            return Ok(ExecutionStatus::Halted);
        }

        let instruction = self.memory[self.pc as usize];
        self.pc += 1;
        self.instruction_count += 1;

        // Decode and execute
        self.execute_instruction(instruction)?;

        if self.halted {
            Ok(ExecutionStatus::Halted)
        } else {
            Ok(ExecutionStatus::Continue)
        }
    }

    async fn handle_raceway_event(&mut self, event: SimulationEvent) -> Result<()> {
        match event {
            SimulationEvent::PacketArrival { .. } => {
                self.packets_received += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_instruction(&mut self, instr: u32) -> Result<()> {
        let opcode = (instr >> 26) & 0x3F;

        match opcode {
            0x00 => { /* NOP */ }

            0x01 => {
                // DUP
                if let Some(&top) = self.d_stack.last() {
                    self.d_stack.push(top);
                }
            }

            0x02 => {
                // DROP
                self.d_stack.pop();
            }

            0x03 => {
                // SWAP
                let len = self.d_stack.len();
                if len >= 2 {
                    self.d_stack.swap(len - 1, len - 2);
                }
            }

            0x04 => {
                // OVER
                if self.d_stack.len() >= 2 {
                    let second = self.d_stack[self.d_stack.len() - 2];
                    self.d_stack.push(second);
                }
            }

            0x10 => {
                // ADD
                if self.d_stack.len() >= 2 {
                    let b = self.d_stack.pop().unwrap();
                    let a = self.d_stack.pop().unwrap();
                    self.d_stack.push(a.wrapping_add(b));
                }
            }

            0x3F => {
                // HALT
                self.halted = true;
            }

            0x08..=0x0F => {
                // LITERAL
                let value = instr & 0x03FFFFFF;
                self.d_stack.push(value);
            }

            _ => {
                // Unknown instruction - treat as NOP for now
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_creation() {
        let tile = TileSimulator::new(TileId::new(0).unwrap()).unwrap();
        assert_eq!(tile.tile_id(), TileId::new(0).unwrap());
        assert_eq!(tile.pc(), 0);
    }

    #[test]
    fn test_load_program() {
        let mut tile = TileSimulator::new(TileId::new(0).unwrap()).unwrap();
        let program = vec![0x00000000, 0x01000000, 0x02000000];

        tile.load_program(&program).unwrap();

        assert_eq!(tile.pc(), 0);
        assert_eq!(tile.instruction_count(), 0);
    }
}
