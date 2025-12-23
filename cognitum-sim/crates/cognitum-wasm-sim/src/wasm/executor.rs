//! WASM instruction executor
//!
//! Executes decoded WASM instructions with full i32 support

use super::decoder::{DecodedInstruction, MemSize};
use super::opcodes::{Opcode, SimdOpcode, NeuralOpcode};
use super::stack::WasmStack;
use super::memory::WasmMemory;
use super::simd::SimdUnit;
use super::ExecutionEffect;
use crate::error::{Result, WasmSimError, WasmTrap};
use crate::network::Packet;

/// WASM instruction executor
pub struct WasmExecutor {
    /// Enable SIMD operations
    enable_simd: bool,

    /// Enable neural operations
    enable_neural: bool,

    /// Global variables
    globals: Vec<i32>,

    /// Block stack (for control flow)
    block_stack: Vec<BlockFrame>,
}

/// Block frame for control flow
#[derive(Debug, Clone)]
struct BlockFrame {
    /// Block type (block, loop, if)
    kind: BlockKind,

    /// Label index
    label: u32,

    /// Stack depth at block start
    stack_depth: usize,

    /// Target PC for branch
    target_pc: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Block,
    Loop,
    If,
}

impl WasmExecutor {
    /// Create new executor
    pub fn new(enable_simd: bool, enable_neural: bool) -> Self {
        Self {
            enable_simd,
            enable_neural,
            globals: vec![0; 64], // 64 global slots
            block_stack: Vec::with_capacity(32),
        }
    }

    /// Execute a decoded instruction
    pub fn execute(
        &mut self,
        instr: &DecodedInstruction,
        stack: &mut WasmStack,
        memory: &mut WasmMemory,
        simd: &mut SimdUnit,
        pc: &mut u32,
    ) -> Result<ExecutionEffect> {
        match instr.opcode {
            // ===== Control Flow =====
            Opcode::Unreachable => {
                return Ok(ExecutionEffect::Trap(WasmTrap::Unreachable));
            }

            Opcode::Nop => {}

            Opcode::Block => {
                self.block_stack.push(BlockFrame {
                    kind: BlockKind::Block,
                    label: self.block_stack.len() as u32,
                    stack_depth: stack.depth(),
                    target_pc: *pc, // Will be updated on End
                });
            }

            Opcode::Loop => {
                self.block_stack.push(BlockFrame {
                    kind: BlockKind::Loop,
                    label: self.block_stack.len() as u32,
                    stack_depth: stack.depth(),
                    target_pc: *pc, // Loop jumps back here
                });
            }

            Opcode::If => {
                let condition = stack.pop()?;
                self.block_stack.push(BlockFrame {
                    kind: BlockKind::If,
                    label: self.block_stack.len() as u32,
                    stack_depth: stack.depth(),
                    target_pc: *pc,
                });

                if condition == 0 {
                    // Skip to else or end - simplified for now
                    // In full implementation, would scan for matching else/end
                }
            }

            Opcode::Else => {
                // Skip to end if we came from the if branch
                // Simplified implementation
            }

            Opcode::End => {
                self.block_stack.pop();
            }

            Opcode::Br => {
                let depth = instr.immediate as u32;
                return self.branch(depth, stack);
            }

            Opcode::BrIf => {
                let condition = stack.pop()?;
                if condition != 0 {
                    let depth = instr.immediate as u32;
                    return self.branch(depth, stack);
                }
            }

            Opcode::Return => {
                let ret_addr = stack.pop_return()?;
                return Ok(ExecutionEffect::Branch(ret_addr));
            }

            Opcode::Call => {
                let func_idx = instr.immediate as u32;
                stack.push_return(*pc)?;
                // Simplified: treat func_idx as PC offset
                return Ok(ExecutionEffect::Branch(func_idx));
            }

            // ===== Parametric =====
            Opcode::Drop => {
                stack.pop()?;
            }

            Opcode::Select => {
                let condition = stack.pop()?;
                let val2 = stack.pop()?;
                let val1 = stack.pop()?;
                stack.push(if condition != 0 { val1 } else { val2 })?;
            }

            // ===== Variable Access =====
            Opcode::LocalGet => {
                let idx = instr.immediate as u32;
                let value = stack.get_local(idx)?;
                stack.push(value)?;
            }

            Opcode::LocalSet => {
                let idx = instr.immediate as u32;
                let value = stack.pop()?;
                stack.set_local(idx, value)?;
            }

            Opcode::LocalTee => {
                let idx = instr.immediate as u32;
                let value = stack.peek().ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))?;
                stack.set_local(idx, value)?;
            }

            Opcode::GlobalGet => {
                let idx = instr.immediate as usize;
                if idx >= self.globals.len() {
                    return Err(WasmSimError::MemoryOutOfBounds {
                        address: idx as u32,
                        size: 4,
                    });
                }
                stack.push(self.globals[idx])?;
            }

            Opcode::GlobalSet => {
                let idx = instr.immediate as usize;
                let value = stack.pop()?;
                if idx >= self.globals.len() {
                    return Err(WasmSimError::MemoryOutOfBounds {
                        address: idx as u32,
                        size: 4,
                    });
                }
                self.globals[idx] = value;
            }

            // ===== Memory Operations =====
            Opcode::I32Load => {
                let addr = stack.pop()? as u32;
                let value = memory.load_i32(addr, instr.immediate2)?;
                stack.push(value)?;
            }

            Opcode::I32Store => {
                let value = stack.pop()?;
                let addr = stack.pop()? as u32;
                memory.store_i32(addr, instr.immediate2, value)?;
            }

            Opcode::I32Load8S => {
                let addr = stack.pop()? as u32;
                let value = memory.load_i8_s(addr, instr.immediate2)?;
                stack.push(value)?;
            }

            Opcode::I32Load8U => {
                let addr = stack.pop()? as u32;
                let value = memory.load_i8_u(addr, instr.immediate2)?;
                stack.push(value)?;
            }

            Opcode::I32Load16S => {
                let addr = stack.pop()? as u32;
                let value = memory.load_i16_s(addr, instr.immediate2)?;
                stack.push(value)?;
            }

            Opcode::I32Load16U => {
                let addr = stack.pop()? as u32;
                let value = memory.load_i16_u(addr, instr.immediate2)?;
                stack.push(value)?;
            }

            Opcode::I32Store8 => {
                let value = stack.pop()?;
                let addr = stack.pop()? as u32;
                memory.store_i8(addr, instr.immediate2, value)?;
            }

            Opcode::I32Store16 => {
                let value = stack.pop()?;
                let addr = stack.pop()? as u32;
                memory.store_i16(addr, instr.immediate2, value)?;
            }

            Opcode::MemorySize => {
                stack.push(memory.size() as i32)?;
            }

            Opcode::MemoryGrow => {
                let delta = stack.pop()? as u32;
                let result = memory.grow(delta);
                stack.push(result)?;
            }

            // ===== Constants =====
            Opcode::I32Const => {
                stack.push(instr.immediate as i32)?;
            }

            // ===== i32 Comparison =====
            Opcode::I32Eqz => {
                let val = stack.pop()?;
                stack.push(if val == 0 { 1 } else { 0 })?;
            }

            Opcode::I32Eq => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a == b { 1 } else { 0 })?;
            }

            Opcode::I32Ne => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a != b { 1 } else { 0 })?;
            }

            Opcode::I32LtS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a < b { 1 } else { 0 })?;
            }

            Opcode::I32LtU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                stack.push(if a < b { 1 } else { 0 })?;
            }

            Opcode::I32GtS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a > b { 1 } else { 0 })?;
            }

            Opcode::I32GtU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                stack.push(if a > b { 1 } else { 0 })?;
            }

            Opcode::I32LeS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a <= b { 1 } else { 0 })?;
            }

            Opcode::I32LeU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                stack.push(if a <= b { 1 } else { 0 })?;
            }

            Opcode::I32GeS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(if a >= b { 1 } else { 0 })?;
            }

            Opcode::I32GeU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                stack.push(if a >= b { 1 } else { 0 })?;
            }

            // ===== i32 Arithmetic =====
            Opcode::I32Clz => {
                let val = stack.pop()? as u32;
                stack.push(val.leading_zeros() as i32)?;
            }

            Opcode::I32Ctz => {
                let val = stack.pop()? as u32;
                stack.push(val.trailing_zeros() as i32)?;
            }

            Opcode::I32Popcnt => {
                let val = stack.pop()? as u32;
                stack.push(val.count_ones() as i32)?;
            }

            Opcode::I32Add => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.wrapping_add(b))?;
            }

            Opcode::I32Sub => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.wrapping_sub(b))?;
            }

            Opcode::I32Mul => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.wrapping_mul(b))?;
            }

            Opcode::I32DivS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                if b == 0 {
                    return Ok(ExecutionEffect::Trap(WasmTrap::IntegerDivideByZero));
                }
                if a == i32::MIN && b == -1 {
                    return Ok(ExecutionEffect::Trap(WasmTrap::IntegerOverflow));
                }
                stack.push(a.wrapping_div(b))?;
            }

            Opcode::I32DivU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                if b == 0 {
                    return Ok(ExecutionEffect::Trap(WasmTrap::IntegerDivideByZero));
                }
                stack.push((a / b) as i32)?;
            }

            Opcode::I32RemS => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                if b == 0 {
                    return Ok(ExecutionEffect::Trap(WasmTrap::IntegerDivideByZero));
                }
                stack.push(a.wrapping_rem(b))?;
            }

            Opcode::I32RemU => {
                let b = stack.pop()? as u32;
                let a = stack.pop()? as u32;
                if b == 0 {
                    return Ok(ExecutionEffect::Trap(WasmTrap::IntegerDivideByZero));
                }
                stack.push((a % b) as i32)?;
            }

            Opcode::I32And => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a & b)?;
            }

            Opcode::I32Or => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a | b)?;
            }

            Opcode::I32Xor => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a ^ b)?;
            }

            Opcode::I32Shl => {
                let b = (stack.pop()? as u32) & 0x1F;
                let a = stack.pop()? as u32;
                stack.push((a << b) as i32)?;
            }

            Opcode::I32ShrS => {
                let b = (stack.pop()? as u32) & 0x1F;
                let a = stack.pop()?;
                stack.push(a >> b)?;
            }

            Opcode::I32ShrU => {
                let b = (stack.pop()? as u32) & 0x1F;
                let a = stack.pop()? as u32;
                stack.push((a >> b) as i32)?;
            }

            Opcode::I32Rotl => {
                let b = (stack.pop()? as u32) & 0x1F;
                let a = stack.pop()? as u32;
                stack.push(a.rotate_left(b) as i32)?;
            }

            Opcode::I32Rotr => {
                let b = (stack.pop()? as u32) & 0x1F;
                let a = stack.pop()? as u32;
                stack.push(a.rotate_right(b) as i32)?;
            }

            // ===== SIMD (via prefix) =====
            Opcode::SimdPrefix => {
                if !self.enable_simd {
                    return Err(WasmSimError::Unimplemented("SIMD disabled".into()));
                }
                if let Some(simd_op) = instr.simd_opcode {
                    self.execute_simd(simd_op, stack, memory, simd)?;
                }
            }

            // ===== Neural (via prefix) =====
            Opcode::NeuralPrefix => {
                if !self.enable_neural {
                    return Err(WasmSimError::Unimplemented("Neural extensions disabled".into()));
                }
                if let Some(neural_op) = instr.neural_opcode {
                    self.execute_neural(neural_op, stack, memory)?;
                }
            }

            _ => {
                return Err(WasmSimError::Unimplemented(format!(
                    "Opcode {:?} not implemented",
                    instr.opcode
                )));
            }
        }

        Ok(ExecutionEffect::None)
    }

    /// Execute branch
    fn branch(&mut self, depth: u32, stack: &mut WasmStack) -> Result<ExecutionEffect> {
        if depth as usize >= self.block_stack.len() {
            return Err(WasmSimError::InvalidBytecode("Branch depth exceeds block stack".into()));
        }

        let target_idx = self.block_stack.len() - 1 - depth as usize;
        let frame = &self.block_stack[target_idx];

        let target_pc = if frame.kind == BlockKind::Loop {
            frame.target_pc // Loop branches back to start
        } else {
            // Block/If branches to end - simplified
            frame.target_pc
        };

        // Pop blocks up to target
        self.block_stack.truncate(target_idx + 1);

        Ok(ExecutionEffect::Branch(target_pc))
    }

    /// Execute SIMD instruction
    fn execute_simd(
        &mut self,
        opcode: SimdOpcode,
        stack: &mut WasmStack,
        memory: &mut WasmMemory,
        simd: &mut SimdUnit,
    ) -> Result<()> {
        match opcode {
            SimdOpcode::V128Load => {
                let addr = stack.pop()? as u32;
                let value = memory.load_v128(addr, 0)?;
                simd.push(value)?;
            }

            SimdOpcode::V128Store => {
                let value = simd.pop()?;
                let addr = stack.pop()? as u32;
                memory.store_v128(addr, 0, value)?;
            }

            SimdOpcode::I32x4Add => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i].wrapping_add(b[i]);
                }
                simd.push(result)?;
            }

            SimdOpcode::I32x4Sub => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i].wrapping_sub(b[i]);
                }
                simd.push(result)?;
            }

            SimdOpcode::I32x4Mul => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i].wrapping_mul(b[i]);
                }
                simd.push(result)?;
            }

            SimdOpcode::I32x4Splat => {
                let val = stack.pop()?;
                simd.push([val, val, val, val])?;
            }

            SimdOpcode::V128And => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i] & b[i];
                }
                simd.push(result)?;
            }

            SimdOpcode::V128Or => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i] | b[i];
                }
                simd.push(result)?;
            }

            SimdOpcode::V128Xor => {
                let b = simd.pop()?;
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = a[i] ^ b[i];
                }
                simd.push(result)?;
            }

            SimdOpcode::V128Not => {
                let a = simd.pop()?;
                let mut result = [0i32; 4];
                for i in 0..4 {
                    result[i] = !a[i];
                }
                simd.push(result)?;
            }

            _ => {
                return Err(WasmSimError::Unimplemented(format!(
                    "SIMD opcode {:?} not implemented",
                    opcode
                )));
            }
        }

        Ok(())
    }

    /// Execute neural instruction
    fn execute_neural(
        &mut self,
        opcode: NeuralOpcode,
        stack: &mut WasmStack,
        memory: &mut WasmMemory,
    ) -> Result<()> {
        match opcode {
            NeuralOpcode::NeuralMac => {
                // Multiply-accumulate: acc = acc + a * b
                let b = stack.pop()?;
                let a = stack.pop()?;
                let acc = stack.pop()?;
                stack.push(acc.wrapping_add(a.wrapping_mul(b)))?;
            }

            NeuralOpcode::NeuralRelu => {
                let val = stack.pop()?;
                stack.push(if val > 0 { val } else { 0 })?;
            }

            _ => {
                return Err(WasmSimError::Unimplemented(format!(
                    "Neural opcode {:?} not implemented",
                    opcode
                )));
            }
        }

        Ok(())
    }

    /// Get global variable
    pub fn get_global(&self, idx: usize) -> Option<i32> {
        self.globals.get(idx).copied()
    }

    /// Set global variable
    pub fn set_global(&mut self, idx: usize, value: i32) -> Result<()> {
        if idx >= self.globals.len() {
            return Err(WasmSimError::MemoryOutOfBounds {
                address: idx as u32,
                size: 4,
            });
        }
        self.globals[idx] = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_environment() -> (WasmExecutor, WasmStack, WasmMemory, SimdUnit) {
        (
            WasmExecutor::new(true, true),
            WasmStack::new(16, 32),
            WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap(),
            SimdUnit::new(),
        )
    }

    #[test]
    fn test_i32_add() {
        let (mut exec, mut stack, mut mem, mut simd) = create_test_environment();
        let mut pc = 0u32;

        stack.push(10).unwrap();
        stack.push(20).unwrap();

        let instr = DecodedInstruction {
            opcode: Opcode::I32Add,
            op_type: super::super::opcodes::OpType::I32,
            immediate: 0,
            immediate2: 0,
            simd_opcode: None,
            neural_opcode: None,
            length: 1,
            mem_size: MemSize::Word,
            is_signed: false,
        };

        exec.execute(&instr, &mut stack, &mut mem, &mut simd, &mut pc).unwrap();
        assert_eq!(stack.pop().unwrap(), 30);
    }

    #[test]
    fn test_i32_div_by_zero() {
        let (mut exec, mut stack, mut mem, mut simd) = create_test_environment();
        let mut pc = 0u32;

        stack.push(10).unwrap();
        stack.push(0).unwrap();

        let instr = DecodedInstruction {
            opcode: Opcode::I32DivS,
            op_type: super::super::opcodes::OpType::I32,
            immediate: 0,
            immediate2: 0,
            simd_opcode: None,
            neural_opcode: None,
            length: 1,
            mem_size: MemSize::Word,
            is_signed: false,
        };

        let result = exec.execute(&instr, &mut stack, &mut mem, &mut simd, &mut pc);
        assert!(matches!(result, Ok(ExecutionEffect::Trap(WasmTrap::IntegerDivideByZero))));
    }
}
