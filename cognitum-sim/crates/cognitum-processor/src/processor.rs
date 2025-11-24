use crate::{
    error::{ProcessorError, Result},
    fpu::Fpu,
    instruction::Instruction,
    memory::{Memory, SimpleMemory},
    stack::{DataStack, ReturnStack, Stack},
};

// FPU Helper Functions
#[inline]
fn f32_to_bits(f: f32) -> i32 {
    f.to_bits() as i32
}

#[inline]
fn f32_from_bits(bits: i32) -> f32 {
    f32::from_bits(bits as u32)
}

#[inline]
fn f64_to_bits(d: f64) -> (i32, i32) {
    let bits = d.to_bits();
    let low = (bits & 0xFFFFFFFF) as i32;
    let high = (bits >> 32) as i32;
    (low, high)
}

#[inline]
fn f64_from_bits(low: i32, high: i32) -> f64 {
    let bits = ((high as u64) << 32) | ((low as u32) as u64);
    f64::from_bits(bits)
}


/// A2S v2r3 Processor Core
pub struct A2SProcessor {
    /// Data stack
    stack: DataStack,

    /// Return stack (for subroutine calls)
    rstack: ReturnStack,

    /// Memory subsystem
    memory: Box<dyn Memory>,

    /// Floating-point unit
    fpu: Fpu,

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
            fpu: Fpu::new(),
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

    /// Get reference to FPU
    pub fn fpu(&self) -> &Fpu {
        &self.fpu
    }

    /// Get mutable reference to FPU
    pub fn fpu_mut(&mut self) -> &mut Fpu {
        &mut self.fpu
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

            // Floating-point operations
            // Single Precision Arithmetic
Instruction::FAdd => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fadd(a, b);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FSub => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fsub(a, b);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FMul => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fmul(a, b);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FDiv => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fdiv(a, b)?;
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FSqrt => {
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let result = self.fpu.fsqrt(a)?;
    self.stack.push(f32_to_bits(result))?;
}

// Single Precision Comparison
Instruction::FCmp => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fcmp(a, b);
    self.stack.push(result)?;
}
Instruction::FClt => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fclt(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}
Instruction::FCeq => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fceq(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}
Instruction::FCle => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fcle(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}

// Single Precision Conversion
Instruction::F2I => {
    let f_bits = self.stack.pop()?;
    let f = f32_from_bits(f_bits);
    let result = self.fpu.f2i(f)?;
    self.stack.push(result)?;
}
Instruction::I2F => {
    let n = self.stack.pop()?;
    let result = self.fpu.i2f(n);
    self.stack.push(f32_to_bits(result))?;
}

// Single Precision Utilities
Instruction::FAbs => {
    let f_bits = self.stack.pop()?;
    let f = f32_from_bits(f_bits);
    let result = self.fpu.fabs(f);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FChs => {
    let f_bits = self.stack.pop()?;
    let f = f32_from_bits(f_bits);
    let result = self.fpu.fchs(f);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FMax => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fmax(a, b);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FMin => {
    let b_bits = self.stack.pop()?;
    let a_bits = self.stack.pop()?;
    let a = f32_from_bits(a_bits);
    let b = f32_from_bits(b_bits);
    let result = self.fpu.fmin(a, b);
    self.stack.push(f32_to_bits(result))?;
}
Instruction::FNan => {
    let f_bits = self.stack.pop()?;
    let f = f32_from_bits(f_bits);
    let result = self.fpu.fnan(f);
    self.stack.push(f32_to_bits(result))?;
}

// Double Precision Arithmetic
Instruction::DAdd => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dadd(a, b);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DSub => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dsub(a, b);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DMul => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dmul(a, b);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DDiv => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.ddiv(a, b)?;
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DSqrt => {
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let result = self.fpu.dsqrt(a)?;
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}

// Double Precision Comparison
Instruction::DCmp => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dcmp(a, b);
    self.stack.push(result)?;
}
Instruction::DClt => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dclt(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}
Instruction::DCeq => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dceq(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}
Instruction::DCle => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dcle(a, b);
    self.stack.push(if result { -1 } else { 0 })?;
}

// Double Precision Conversion
Instruction::D2I => {
    let high = self.stack.pop()?;
    let low = self.stack.pop()?;
    let d = f64_from_bits(low, high);
    let result = self.fpu.d2i(d)?;
    self.stack.push(result)?;
}
Instruction::I2D => {
    let n = self.stack.pop()?;
    let result = self.fpu.i2d(n);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}

// Double Precision Utilities
Instruction::DAbs => {
    let high = self.stack.pop()?;
    let low = self.stack.pop()?;
    let d = f64_from_bits(low, high);
    let result = self.fpu.dabs(d);
    let (res_low, res_high) = f64_to_bits(result);
    self.stack.push(res_low)?;
    self.stack.push(res_high)?;
}
Instruction::DChs => {
    let high = self.stack.pop()?;
    let low = self.stack.pop()?;
    let d = f64_from_bits(low, high);
    let result = self.fpu.dchs(d);
    let (res_low, res_high) = f64_to_bits(result);
    self.stack.push(res_low)?;
    self.stack.push(res_high)?;
}
Instruction::DMax => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dmax(a, b);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DMin => {
    let b_high = self.stack.pop()?;
    let b_low = self.stack.pop()?;
    let a_high = self.stack.pop()?;
    let a_low = self.stack.pop()?;
    let a = f64_from_bits(a_low, a_high);
    let b = f64_from_bits(b_low, b_high);
    let result = self.fpu.dmin(a, b);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::DNan => {
    let high = self.stack.pop()?;
    let low = self.stack.pop()?;
    let d = f64_from_bits(low, high);
    let result = self.fpu.dnan(d);
    let (res_low, res_high) = f64_to_bits(result);
    self.stack.push(res_low)?;
    self.stack.push(res_high)?;
}

// Precision Conversion
Instruction::F2D => {
    let f_bits = self.stack.pop()?;
    let f = f32_from_bits(f_bits);
    let result = self.fpu.f2d(f);
    let (low, high) = f64_to_bits(result);
    self.stack.push(low)?;
    self.stack.push(high)?;
}
Instruction::D2F => {
    let high = self.stack.pop()?;
    let low = self.stack.pop()?;
    let d = f64_from_bits(low, high);
    let result = self.fpu.d2f(d);
    self.stack.push(f32_to_bits(result))?;
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
