// This file contains the FPU instruction handlers to be integrated into processor.rs
// These handlers replace the unimplemented!() calls for FPU operations

// Helper functions for float/int bit conversion
fn f32_to_bits(f: f32) -> i32 {
    f.to_bits() as i32
}

fn f32_from_bits(bits: i32) -> f32 {
    f32::from_bits(bits as u32)
}

fn f64_to_bits(d: f64) -> (i32, i32) {
    let bits = d.to_bits();
    let low = (bits & 0xFFFFFFFF) as i32;
    let high = (bits >> 32) as i32;
    (low, high)
}

fn f64_from_bits(low: i32, high: i32) -> f64 {
    let bits = ((high as u64) << 32) | ((low as u32) as u64);
    f64::from_bits(bits)
}

// FPU instruction handlers (replace lines 403-438 in processor.rs):

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
