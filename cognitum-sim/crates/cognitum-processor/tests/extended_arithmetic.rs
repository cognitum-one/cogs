// Extended ISA Multiply/Divide Operations Tests
// Comprehensive test suite for extended arithmetic instructions

use cognitum_processor::{A2SProcessor, Instruction, ProcessorError};

// ============================================================================
// MULTIPLY OPERATIONS TESTS
// ============================================================================

#[test]
fn test_multiply_signed_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(200)).unwrap();
    cpu.execute(Instruction::MultiplySigned).unwrap();

    // Result: 100 * 200 = 20000 (0x4E20)
    // Stack: [low, high]
    assert_eq!(cpu.stack_depth(), 2);
    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low, 20000); // Low 32 bits
    assert_eq!(high, 0);    // High 32 bits (result fits in 32 bits)
}

#[test]
fn test_multiply_signed_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-100)).unwrap();
    cpu.execute(Instruction::Push(200)).unwrap();
    cpu.execute(Instruction::MultiplySigned).unwrap();

    // Result: -100 * 200 = -20000
    assert_eq!(cpu.stack_depth(), 2);
    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low, -20000); // Low 32 bits
    assert_eq!(high, -1);    // High 32 bits (sign extension)
}

#[test]
fn test_multiply_signed_overflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Large numbers that will overflow 32 bits
    cpu.execute(Instruction::Push(0x7FFFFFFF_u32 as i32)).unwrap(); // i32::MAX
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::MultiplySigned).unwrap();

    // Result: 2147483647 * 2 = 4294967294 (0xFFFFFFFE)
    assert_eq!(cpu.stack_depth(), 2);
    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    // Verify 64-bit result
    let result_64 = ((high as i64) << 32) | ((low as u32) as i64);
    assert_eq!(result_64, 0x7FFFFFFF_i64 * 2);
}

#[test]
fn test_multiply_unsigned_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(1000)).unwrap();
    cpu.execute(Instruction::Push(2000)).unwrap();
    cpu.execute(Instruction::MultiplyUnsigned).unwrap();

    // Result: 1000 * 2000 = 2000000
    assert_eq!(cpu.stack_depth(), 2);
    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low, 2000000);
    assert_eq!(high, 0);
}

#[test]
fn test_multiply_unsigned_large() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Large unsigned numbers
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap(); // u32::MAX
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::MultiplyUnsigned).unwrap();

    // Result: 4294967295 * 2 = 8589934590 (0x1_FFFFFFFE)
    assert_eq!(cpu.stack_depth(), 2);
    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low as u32, 0xFFFFFFFE_u32); // Low 32 bits
    assert_eq!(high as u32, 0x1_u32);       // High 32 bits
}

#[test]
fn test_multiply_high_signed_no_overflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(200)).unwrap();
    cpu.execute(Instruction::MultiplyHighSigned).unwrap();

    // Result high: upper 32 bits of (100 * 200) = 0
    assert_eq!(cpu.peek_stack().unwrap(), 0);
    assert_eq!(cpu.stack_depth(), 1);
}

#[test]
fn test_multiply_high_signed_with_overflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x7FFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(0x7FFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::MultiplyHighSigned).unwrap();

    // Result: upper 32 bits should be non-zero
    let high = cpu.peek_stack().unwrap();
    assert_eq!(high, 0x3FFFFFFF); // Expected upper 32 bits
}

#[test]
fn test_multiply_high_unsigned() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::MultiplyHighUnsigned).unwrap();

    // Result: upper 32 bits of (u32::MAX * u32::MAX)
    let high = cpu.peek_stack().unwrap() as u32;
    assert_eq!(high, 0xFFFFFFFE_u32);
}

#[test]
fn test_multiply_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(12345)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();
    cpu.execute(Instruction::MultiplySigned).unwrap();

    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low, 0);
    assert_eq!(high, 0);
}

// ============================================================================
// DIVIDE OPERATIONS TESTS
// ============================================================================

#[test]
fn test_divide_signed_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // 100 / 10 = 10
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_divide_signed_negative_dividend() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-100)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // -100 / 10 = -10
    assert_eq!(cpu.peek_stack().unwrap(), -10);
}

#[test]
fn test_divide_signed_negative_divisor() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(-10)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // 100 / -10 = -10
    assert_eq!(cpu.peek_stack().unwrap(), -10);
}

#[test]
fn test_divide_signed_both_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-100)).unwrap();
    cpu.execute(Instruction::Push(-10)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // -100 / -10 = 10
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_divide_signed_truncate_toward_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(7)).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // 7 / 3 = 2 (truncate toward zero)
    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_divide_unsigned() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::DivideUnsigned).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_divide_unsigned_large() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Test with large unsigned values
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap(); // u32::MAX
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::DivideUnsigned).unwrap();

    // 4294967295 / 2 = 2147483647
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x7FFFFFFF_u32);
}

#[test]
fn test_divide_signed_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::DivideSigned);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

#[test]
fn test_divide_unsigned_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::DivideUnsigned);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

// ============================================================================
// MODULO OPERATIONS TESTS
// ============================================================================

#[test]
fn test_modulo_signed_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::ModuloSigned).unwrap();

    // 17 % 5 = 2
    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_modulo_signed_negative_dividend() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-17)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::ModuloSigned).unwrap();

    // -17 % 5 = -2 (Rust's behavior)
    assert_eq!(cpu.peek_stack().unwrap(), -2);
}

#[test]
fn test_modulo_signed_negative_divisor() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(-5)).unwrap();
    cpu.execute(Instruction::ModuloSigned).unwrap();

    // 17 % -5 = 2
    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_modulo_unsigned() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::ModuloUnsigned).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_modulo_unsigned_large() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::ModuloUnsigned).unwrap();

    // 4294967295 % 10 = 5
    assert_eq!(cpu.peek_stack().unwrap(), 5);
}

#[test]
fn test_modulo_signed_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::ModuloSigned);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

#[test]
fn test_modulo_unsigned_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::ModuloUnsigned);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

#[test]
fn test_modulo_exact_division() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::ModuloSigned).unwrap();

    // 20 % 5 = 0 (exact division)
    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

// ============================================================================
// EDGE CASE AND COMBINED TESTS
// ============================================================================

#[test]
fn test_multiply_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(12345)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::MultiplySigned).unwrap();

    let high = cpu.peek_stack().unwrap();
    cpu.execute(Instruction::Pop).unwrap();
    let low = cpu.peek_stack().unwrap();

    assert_eq!(low, 12345);
    assert_eq!(high, 0);
}

#[test]
fn test_divide_by_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(12345)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 12345);
}

#[test]
fn test_divide_smaller_by_larger() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::DivideSigned).unwrap();

    // 5 / 10 = 0
    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_stack_underflow_multiply() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();

    let result = cpu.execute(Instruction::MultiplySigned);
    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}

#[test]
fn test_stack_underflow_divide() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();

    let result = cpu.execute(Instruction::DivideSigned);
    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}
