// IEEE 754 FPU Comprehensive Tests
// Tests all floating-point operations with edge cases

use cognitum_processor::{A2SProcessor, Instruction};

// Helper to push f32 to stack
fn push_f32(proc: &mut A2SProcessor, f: f32) -> Result<(), Box<dyn std::error::Error>> {
    let bits = f.to_bits() as i32;
    proc.execute(Instruction::Push(bits))?;
    Ok(())
}

// Helper to pop f32 from stack
fn pop_f32(proc: &mut A2SProcessor) -> Result<f32, Box<dyn std::error::Error>> {
    let bits = proc.peek_stack()? as u32;
    proc.execute(Instruction::Pop)?;
    Ok(f32::from_bits(bits))
}

// Helper to push f64 to stack (low, then high)
fn push_f64(proc: &mut A2SProcessor, d: f64) -> Result<(), Box<dyn std::error::Error>> {
    let bits = d.to_bits();
    let low = (bits & 0xFFFFFFFF) as i32;
    let high = (bits >> 32) as i32;
    proc.execute(Instruction::Push(low))?;
    proc.execute(Instruction::Push(high))?;
    Ok(())
}

// Helper to pop f64 from stack
fn pop_f64(proc: &mut A2SProcessor) -> Result<f64, Box<dyn std::error::Error>> {
    let high_bits = proc.peek_stack()? as u32 as u64;
    proc.execute(Instruction::Pop)?;
    let low_bits = proc.peek_stack()? as u32 as u64;
    proc.execute(Instruction::Pop)?;
    let bits = (high_bits << 32) | low_bits;
    Ok(f64::from_bits(bits))
}

#[test]
fn test_fadd_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 1.5).unwrap();
    push_f32(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::FAdd).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 4.0);
}

#[test]
fn test_fsub_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 5.5).unwrap();
    push_f32(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::FSub).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_fmul_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 3.0).unwrap();
    push_f32(&mut proc, 4.0).unwrap();
    proc.execute(Instruction::FMul).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 12.0);
}

#[test]
fn test_fdiv_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 10.0).unwrap();
    push_f32(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::FDiv).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 5.0);
}

#[test]
fn test_fdiv_by_zero() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 5.0).unwrap();
    push_f32(&mut proc, 0.0).unwrap();
    proc.execute(Instruction::FDiv).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert!(result.is_infinite());
    assert!(proc.fpu().flags.division_by_zero);
}

#[test]
fn test_fsqrt_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 16.0).unwrap();
    proc.execute(Instruction::FSqrt).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 4.0);
}

#[test]
fn test_fsqrt_negative() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, -1.0).unwrap();
    proc.execute(Instruction::FSqrt).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert!(result.is_nan());
    assert!(proc.fpu().flags.invalid);
}

#[test]
fn test_fcmp_operations() {
    let mut proc = A2SProcessor::with_default_memory();

    // Test less than
    push_f32(&mut proc, 1.0).unwrap();
    push_f32(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::FCmp).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1);
    proc.execute(Instruction::Pop).unwrap();

    // Test greater than
    push_f32(&mut proc, 3.0).unwrap();
    push_f32(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::FCmp).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, 1);
    proc.execute(Instruction::Pop).unwrap();

    // Test equal
    push_f32(&mut proc, 2.5).unwrap();
    push_f32(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::FCmp).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_f2i_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 3.7).unwrap();
    proc.execute(Instruction::F2I).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, 3);
}

#[test]
fn test_i2f_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    proc.execute(Instruction::Push(42)).unwrap();
    proc.execute(Instruction::I2F).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 42.0);
}

#[test]
fn test_fabs() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, -3.5).unwrap();
    proc.execute(Instruction::FAbs).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 3.5);
}

#[test]
fn test_fchs() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::FChs).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, -2.5);
}

#[test]
fn test_fmax() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 2.5).unwrap();
    push_f32(&mut proc, 3.7).unwrap();
    proc.execute(Instruction::FMax).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 3.7);
}

#[test]
fn test_fmin() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 2.5).unwrap();
    push_f32(&mut proc, 3.7).unwrap();
    proc.execute(Instruction::FMin).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 2.5);
}

// ========================================
// Double Precision Tests
// ========================================

#[test]
fn test_dadd_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 1.5).unwrap();
    push_f64(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::DAdd).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 4.0);
}

#[test]
fn test_dsub_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 5.5).unwrap();
    push_f64(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::DSub).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_dmul_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 3.0).unwrap();
    push_f64(&mut proc, 4.0).unwrap();
    proc.execute(Instruction::DMul).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 12.0);
}

#[test]
fn test_ddiv_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 10.0).unwrap();
    push_f64(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::DDiv).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 5.0);
}

#[test]
fn test_dsqrt_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 16.0).unwrap();
    proc.execute(Instruction::DSqrt).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 4.0);
}

#[test]
fn test_d2i_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 3.7).unwrap();
    proc.execute(Instruction::D2I).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, 3);
}

#[test]
fn test_i2d_basic() {
    let mut proc = A2SProcessor::with_default_memory();
    proc.execute(Instruction::Push(42)).unwrap();
    proc.execute(Instruction::I2D).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    assert_eq!(result, 42.0);
}

// ========================================
// Precision Conversion Tests
// ========================================

#[test]
fn test_f2d_conversion() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 3.14159).unwrap();
    proc.execute(Instruction::F2D).unwrap();
    let result = pop_f64(&mut proc).unwrap();
    // Allow for slight precision difference
    assert!((result - 3.14159).abs() < 0.00001);
}

#[test]
fn test_d2f_conversion() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 3.14159265359).unwrap();
    proc.execute(Instruction::D2F).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    // Check rough equality (f32 has less precision)
    assert!((result - 3.14159).abs() < 0.00001);
}

// ========================================
// Special Value Tests (IEEE 754)
// ========================================

#[test]
fn test_nan_propagation() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, f32::NAN).unwrap();
    push_f32(&mut proc, 1.0).unwrap();
    proc.execute(Instruction::FAdd).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert!(result.is_nan());
}

#[test]
fn test_infinity_arithmetic() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, f32::INFINITY).unwrap();
    push_f32(&mut proc, 1.0).unwrap();
    proc.execute(Instruction::FAdd).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert!(result.is_infinite());
}

#[test]
fn test_negative_zero() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, -0.0).unwrap();
    push_f32(&mut proc, 0.0).unwrap();
    proc.execute(Instruction::FAdd).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 0.0);
}

#[test]
fn test_subnormal_numbers() {
    let mut proc = A2SProcessor::with_default_memory();
    let tiny = f32::MIN_POSITIVE / 2.0; // Subnormal number
    push_f32(&mut proc, tiny).unwrap();
    push_f32(&mut proc, tiny).unwrap();
    proc.execute(Instruction::FAdd).unwrap();
    let _result = pop_f32(&mut proc).unwrap();
    // Subnormal operations may set underflow flag
    // This is implementation-dependent
}

#[test]
fn test_fnan_filter() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, f32::NAN).unwrap();
    proc.execute(Instruction::FNan).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 0.0);

    // Non-NaN should pass through
    push_f32(&mut proc, 3.5).unwrap();
    proc.execute(Instruction::FNan).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert_eq!(result, 3.5);
}

#[test]
fn test_overflow_detection() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, f32::MAX).unwrap();
    push_f32(&mut proc, f32::MAX).unwrap();
    proc.execute(Instruction::FMul).unwrap();
    let result = pop_f32(&mut proc).unwrap();
    assert!(result.is_infinite());
    assert!(proc.fpu().flags.overflow);
}

// ========================================
// Comparison Tests
// ========================================

#[test]
fn test_fclt() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 1.0).unwrap();
    push_f32(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::FClt).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}

#[test]
fn test_fceq() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 2.5).unwrap();
    push_f32(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::FCeq).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}

#[test]
fn test_fcle() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f32(&mut proc, 1.0).unwrap();
    push_f32(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::FCle).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}

#[test]
fn test_dclt() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 1.0).unwrap();
    push_f64(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::DClt).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}

#[test]
fn test_dceq() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 2.5).unwrap();
    push_f64(&mut proc, 2.5).unwrap();
    proc.execute(Instruction::DCeq).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}

#[test]
fn test_dcle() {
    let mut proc = A2SProcessor::with_default_memory();
    push_f64(&mut proc, 1.0).unwrap();
    push_f64(&mut proc, 2.0).unwrap();
    proc.execute(Instruction::DCle).unwrap();
    let result = proc.peek_stack().unwrap();
    assert_eq!(result, -1); // True
}
