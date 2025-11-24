use crate::{
    error::{ProcessorError, Result},
    instruction::Instruction,
    memory::{Memory, SimpleMemory},
    stack::{DataStack, ReturnStack, Stack},
};

/// A2S v2r3 Processor Core
pub struct A2SProcessor {
    /// Data stack
    stack: DataStack,

    /// Return stack (for subroutine calls)
    rstack: ReturnStack,

    /// Memory subsystem
    memory: Box<dyn Memory>,

    /// Address registers
    reg_a: u32,
    reg_b: u32,
    reg_c: u32,

    /// Program counter
    pc: u32,

    /// Running state
    halted: bool,
}

impl A2SProcessor {
    /// Create new processor with custom memory
    pub fn new(memory: Box<dyn Memory>) -> Self {
        Self {
            stack: DataStack::with_default_size(),
            rstack: ReturnStack::with_default_size(),
            memory,
            reg_a: 0,
            reg_b: 0,
            reg_c: 0,
            pc: 0,
            halted: false,
        }
    }

    /// Create processor with default memory
    pub fn with_default_memory() -> Self {
        Self::new(Box::new(SimpleMemory::with_default_size()))
    }

    /// Execute a single instruction
    pub fn execute(&mut self, instr: Instruction) -> Result<()> {
        if self.halted {
            return Ok(());
        }

        match instr {
            // Stack operations
            Instruction::Push(val) => self.stack.push(val)?,
            Instruction::Pop => {
                self.stack.pop()?;
            }
            Instruction::Dup => {
                let val = self.stack.peek()?;
                self.stack.push(val)?;
            }
            Instruction::Swap => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x2)?;
                self.stack.push(x1)?;
            }
            Instruction::Over => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x1)?;
                self.stack.push(x2)?;
                self.stack.push(x1)?;
            }
            Instruction::Rot3 => {
                let x3 = self.stack.pop()?;
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x2)?;
                self.stack.push(x3)?;
                self.stack.push(x1)?;
            }
            Instruction::Rot4 => {
                let x4 = self.stack.pop()?;
                let x3 = self.stack.pop()?;
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x2)?;
                self.stack.push(x3)?;
                self.stack.push(x4)?;
                self.stack.push(x1)?;
            }
            Instruction::Drop => {
                self.stack.pop()?;
                // Discard second item, keep top
            }
            Instruction::Nip => {
                let x2 = self.stack.pop()?;
                self.stack.pop()?; // Remove second item
                self.stack.push(x2)?;
            }

            // Arithmetic
            Instruction::Add => {
                let n2 = self.stack.pop()?;
                let n1 = self.stack.pop()?;
                self.stack.push(n1.wrapping_add(n2))?;
            }
            Instruction::Sub => {
                let n2 = self.stack.pop()?;
                let n1 = self.stack.pop()?;
                self.stack.push(n1.wrapping_sub(n2))?;
            }
            Instruction::Multiply => {
                let n2 = self.stack.pop()?;
                let n1 = self.stack.pop()?;
                self.stack.push(n1.wrapping_mul(n2))?;
            }
            Instruction::Divide => {
                let n2 = self.stack.pop()?;
                let n1 = self.stack.pop()?;
                if n2 == 0 {
                    return Err(ProcessorError::DivisionByZero);
                }
                self.stack.push(n1 / n2)?;
            }

            // Extended Multiply Operations
            Instruction::MultiplySigned => {
                // MULS: signed 32x32 -> 64-bit result
                let b = self.stack.pop()? as i32;
                let a = self.stack.pop()? as i32;
                let result = (a as i64).wrapping_mul(b as i64);
                // Push low 32 bits first, then high 32 bits (stack grows up)
                self.stack.push(result as u32 as i32)?;
                self.stack.push((result >> 32) as u32 as i32)?;
            }
            Instruction::MultiplyUnsigned => {
                // MULU: unsigned 32x32 -> 64-bit result
                let b = self.stack.pop()? as u32;
                let a = self.stack.pop()? as u32;
                let result = (a as u64).wrapping_mul(b as u64);
                // Push low 32 bits first, then high 32 bits
                self.stack.push((result & 0xFFFFFFFF) as u32 as i32)?;
                self.stack.push((result >> 32) as u32 as i32)?;
            }
            Instruction::MultiplyHighSigned => {
                // MULH: signed multiply, return upper 32 bits only
                let b = self.stack.pop()? as i32;
                let a = self.stack.pop()? as i32;
                let result = (a as i64).wrapping_mul(b as i64);
                self.stack.push((result >> 32) as u32 as i32)?;
            }
            Instruction::MultiplyHighUnsigned => {
                // MULHU: unsigned multiply, return upper 32 bits only
                let b = self.stack.pop()? as u32;
                let a = self.stack.pop()? as u32;
                let result = (a as u64).wrapping_mul(b as u64);
                self.stack.push((result >> 32) as u32 as i32)?;
            }

            // Extended Divide Operations
            Instruction::DivideSigned => {
                // DIVS: signed division (quotient)
                let divisor = self.stack.pop()? as i32;
                let dividend = self.stack.pop()? as i32;
                if divisor == 0 {
                    return Err(ProcessorError::DivisionByZero);
                }
                let quotient = dividend.wrapping_div(divisor);
                self.stack.push(quotient)?;
            }
            Instruction::DivideUnsigned => {
                // DIVU: unsigned division (quotient)
                let divisor = self.stack.pop()? as u32;
                let dividend = self.stack.pop()? as u32;
                if divisor == 0 {
                    return Err(ProcessorError::DivisionByZero);
                }
                let quotient = dividend.wrapping_div(divisor);
                self.stack.push(quotient as i32)?;
            }
            Instruction::ModuloSigned => {
                // MODS: signed modulo (remainder)
                let divisor = self.stack.pop()? as i32;
                let dividend = self.stack.pop()? as i32;
                if divisor == 0 {
                    return Err(ProcessorError::DivisionByZero);
                }
                let remainder = dividend.wrapping_rem(divisor);
                self.stack.push(remainder)?;
            }
            Instruction::ModuloUnsigned => {
                // MODU: unsigned modulo (remainder)
                let divisor = self.stack.pop()? as u32;
                let dividend = self.stack.pop()? as u32;
                if divisor == 0 {
                    return Err(ProcessorError::DivisionByZero);
                }
                let remainder = dividend.wrapping_rem(divisor);
                self.stack.push(remainder as i32)?;
            }

            // Bitwise logic
            Instruction::And => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x1 & x2)?;
            }
            Instruction::Or => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x1 | x2)?;
            }
            Instruction::Xor => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(x1 ^ x2)?;
            }
            Instruction::Not => {
                let x = self.stack.pop()?;
                self.stack.push(!x)?;
            }

            // Comparison
            Instruction::Equal => {
                let x2 = self.stack.pop()?;
                let x1 = self.stack.pop()?;
                self.stack.push(if x1 == x2 { -1 } else { 0 })?;
            }
            Instruction::LessThan => {
                let n2 = self.stack.pop()?;
                let n1 = self.stack.pop()?;
                self.stack.push(if n1 < n2 { -1 } else { 0 })?;
            }
            Instruction::UnsignedLessThan => {
                let u2 = self.stack.pop()? as u32;
                let u1 = self.stack.pop()? as u32;
                self.stack.push(if u1 < u2 { -1 } else { 0 })?;
            }

            // Memory operations
            Instruction::Load => {
                let addr = self.stack.pop()? as u32;
                let value = self.memory.read(addr)?;
                self.stack.push(value)?;
            }
            Instruction::Store => {
                let addr = self.stack.pop()? as u32;
                let value = self.stack.pop()?;
                self.memory.write(addr, value)?;
            }
            Instruction::LoadA => {
                let value = self.memory.read(self.reg_a)?;
                self.stack.push(value)?;
            }
            Instruction::LoadB => {
                let value = self.memory.read(self.reg_b)?;
                self.stack.push(value)?;
            }
            Instruction::LoadC => {
                let value = self.memory.read(self.reg_c)?;
                self.stack.push(value)?;
            }
            Instruction::StoreA => {
                let value = self.stack.pop()?;
                self.memory.write(self.reg_a, value)?;
            }
            Instruction::StoreB => {
                let value = self.stack.pop()?;
                self.memory.write(self.reg_b, value)?;
            }
            Instruction::StoreC => {
                let value = self.stack.pop()?;
                self.memory.write(self.reg_c, value)?;
            }

            // Register operations
            Instruction::ToA => {
                self.reg_a = self.stack.pop()? as u32;
            }
            Instruction::ToB => {
                self.reg_b = self.stack.pop()? as u32;
            }
            Instruction::ToC => {
                self.reg_c = self.stack.pop()? as u32;
            }
            Instruction::ToR => {
                let value = self.stack.pop()? as u32;
                self.rstack.push(value)?;
            }
            Instruction::FromA => {
                self.stack.push(self.reg_a as i32)?;
            }
            Instruction::FromB => {
                self.stack.push(self.reg_b as i32)?;
            }
            Instruction::FromC => {
                self.stack.push(self.reg_c as i32)?;
            }
            Instruction::FromR => {
                let value = self.rstack.pop()?;
                self.stack.push(value as i32)?;
            }

            // Control flow
            Instruction::Jump(offset) => {
                self.pc = (self.pc as i32 + offset as i32) as u32;
            }
            Instruction::JumpZero(offset) => {
                let x = self.stack.pop()?;
                if x == 0 {
                    self.pc = (self.pc as i32 + offset as i32) as u32;
                }
            }
            Instruction::JumpNegative(offset) => {
                let n = self.stack.pop()?;
                if n < 0 {
                    self.pc = (self.pc as i32 + offset as i32) as u32;
                }
            }
            Instruction::Call(offset) => {
                self.rstack.push(self.pc)?;
                self.pc = (self.pc as i32 + offset as i32) as u32;
            }
            Instruction::Return => {
                self.pc = self.rstack.pop()?;
            }

            // Shift and Rotate operations
            Instruction::ShiftLeft => {
                let shift = (self.stack.pop()? as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value << shift;
                self.stack.push(result as i32)?;
            }
            Instruction::ShiftRight => {
                let shift = (self.stack.pop()? as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value >> shift;
                self.stack.push(result as i32)?;
            }
            Instruction::ShiftRightArith => {
                let shift = (self.stack.pop()? as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()?; // Keep as signed i32
                let result = value >> shift; // Arithmetic shift preserves sign
                self.stack.push(result)?;
            }
            Instruction::ShiftLeftImm(shift) => {
                let shift_amount = (shift as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value << shift_amount;
                self.stack.push(result as i32)?;
            }
            Instruction::ShiftRightImm(shift) => {
                let shift_amount = (shift as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value >> shift_amount;
                self.stack.push(result as i32)?;
            }
            Instruction::ShiftRightArithImm(shift) => {
                let shift_amount = (shift as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()?; // Keep as signed i32
                let result = value >> shift_amount; // Arithmetic shift preserves sign
                self.stack.push(result)?;
            }
            Instruction::RotateLeft => {
                let shift = (self.stack.pop()? as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value.rotate_left(shift);
                self.stack.push(result as i32)?;
            }
            Instruction::RotateRight => {
                let shift = (self.stack.pop()? as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value.rotate_right(shift);
                self.stack.push(result as i32)?;
            }
            Instruction::RotateLeftImm(shift) => {
                let shift_amount = (shift as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value.rotate_left(shift_amount);
                self.stack.push(result as i32)?;
            }
            Instruction::RotateRightImm(shift) => {
                let shift_amount = (shift as u32) & 0x1F; // Limit to 0-31
                let value = self.stack.pop()? as u32;
                let result = value.rotate_right(shift_amount);
                self.stack.push(result as i32)?;
            }

            Instruction::Nop => {}
            Instruction::Halt => {
                self.halted = true;
            }

            // Floating-point operations (not yet implemented)
            Instruction::FAdd
            | Instruction::FSub
            | Instruction::FMul
            | Instruction::FDiv
            | Instruction::FSqrt
            | Instruction::FCmp
            | Instruction::FClt
            | Instruction::FCeq
            | Instruction::FCle
            | Instruction::F2I
            | Instruction::I2F
            | Instruction::FAbs
            | Instruction::FChs
            | Instruction::FMax
            | Instruction::FMin
            | Instruction::FNan
            | Instruction::DAdd
            | Instruction::DSub
            | Instruction::DMul
            | Instruction::DDiv
            | Instruction::DSqrt
            | Instruction::DCmp
            | Instruction::DClt
            | Instruction::DCeq
            | Instruction::DCle
            | Instruction::D2I
            | Instruction::I2D
            | Instruction::DAbs
            | Instruction::DChs
            | Instruction::DMax
            | Instruction::DMin
            | Instruction::DNan
            | Instruction::F2D
            | Instruction::D2F => {
                unimplemented!("Floating-point operations not yet implemented")
            }
        }

        Ok(())
    }

    /// Peek at top of stack without popping
    pub fn peek_stack(&self) -> Result<i32> {
        self.stack.peek()
    }

    /// Get stack depth
    pub fn stack_depth(&self) -> usize {
        self.stack.depth()
    }

    /// Get register A
    pub fn get_reg_a(&self) -> u32 {
        self.reg_a
    }

    /// Get register B
    pub fn get_reg_b(&self) -> u32 {
        self.reg_b
    }

    /// Get register C
    pub fn get_reg_c(&self) -> u32 {
        self.reg_c
    }

    /// Get PC
    pub fn get_pc(&self) -> u32 {
        self.pc
    }

    /// Check if halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Load program into memory
    pub fn load_program(&mut self, _program: &[Instruction], start_addr: u32) -> Result<()> {
        self.pc = start_addr;
        // Program loading would encode instructions to memory
        // For now, just set PC
        Ok(())
    }

    /// Run program until halt or error
    pub fn run(&mut self, program: &[Instruction]) -> Result<()> {
        for instr in program {
            self.execute(instr.clone())?;
            if self.halted {
                break;
            }
        }
        Ok(())
    }
}
