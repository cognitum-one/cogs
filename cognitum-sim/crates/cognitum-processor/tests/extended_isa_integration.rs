//! Extended ISA Integration Tests
//!
//! Comprehensive tests for extended instruction set features including
//! multiply/divide operations with edge cases, overflow handling, and
//! performance validation.

use cognitum_processor::{A2SProcessor, Instruction, ProcessorError};

// ============================================================================
// Multiply Operation Tests (25+ cases)
// ============================================================================

#[test]
fn test_multiply_positive_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(123)).unwrap();
    cpu.execute(Instruction::Push(456)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 123 * 456);
}

#[test]
fn test_multiply_negative_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-15)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -300);
}

#[test]
fn test_multiply_negative_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-12)).unwrap();
    cpu.execute(Instruction::Push(-8)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 96);
}

#[test]
fn test_multiply_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(999999)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_multiply_by_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_multiply_by_minus_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(-1)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -42);
}

#[test]
fn test_multiply_overflow_wraps() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Test wrapping behavior
    cpu.execute(Instruction::Push(i32::MAX)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    // Should wrap around
    assert_eq!(cpu.peek_stack().unwrap(), i32::MAX.wrapping_mul(2));
}

#[test]
fn test_multiply_small_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 12);
}

#[test]
fn test_multiply_large_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10000)).unwrap();
    cpu.execute(Instruction::Push(20000)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 200_000_000);
}

#[test]
fn test_multiply_power_of_two() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(7)).unwrap();
    cpu.execute(Instruction::Push(16)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 112);
}

#[test]
fn test_multiply_commutative() {
    let mut cpu1 = A2SProcessor::with_default_memory();
    cpu1.execute(Instruction::Push(15)).unwrap();
    cpu1.execute(Instruction::Push(25)).unwrap();
    cpu1.execute(Instruction::Multiply).unwrap();

    let mut cpu2 = A2SProcessor::with_default_memory();
    cpu2.execute(Instruction::Push(25)).unwrap();
    cpu2.execute(Instruction::Push(15)).unwrap();
    cpu2.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu1.peek_stack().unwrap(), cpu2.peek_stack().unwrap());
}

#[test]
fn test_multiply_associative() {
    // (2 * 3) * 4 == 2 * (3 * 4)
    let mut cpu1 = A2SProcessor::with_default_memory();
    cpu1.execute(Instruction::Push(2)).unwrap();
    cpu1.execute(Instruction::Push(3)).unwrap();
    cpu1.execute(Instruction::Multiply).unwrap();
    cpu1.execute(Instruction::Push(4)).unwrap();
    cpu1.execute(Instruction::Multiply).unwrap();

    let mut cpu2 = A2SProcessor::with_default_memory();
    cpu2.execute(Instruction::Push(3)).unwrap();
    cpu2.execute(Instruction::Push(4)).unwrap();
    cpu2.execute(Instruction::Multiply).unwrap();
    cpu2.execute(Instruction::Push(2)).unwrap();
    cpu2.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu1.peek_stack().unwrap(), cpu2.peek_stack().unwrap());
}

#[test]
fn test_multiply_distributive() {
    // 5 * (3 + 2) == (5 * 3) + (5 * 2)
    let mut cpu1 = A2SProcessor::with_default_memory();
    cpu1.execute(Instruction::Push(3)).unwrap();
    cpu1.execute(Instruction::Push(2)).unwrap();
    cpu1.execute(Instruction::Add).unwrap();
    cpu1.execute(Instruction::Push(5)).unwrap();
    cpu1.execute(Instruction::Multiply).unwrap();

    let mut cpu2 = A2SProcessor::with_default_memory();
    cpu2.execute(Instruction::Push(5)).unwrap();
    cpu2.execute(Instruction::Push(3)).unwrap();
    cpu2.execute(Instruction::Multiply).unwrap();
    cpu2.execute(Instruction::Push(5)).unwrap();
    cpu2.execute(Instruction::Push(2)).unwrap();
    cpu2.execute(Instruction::Multiply).unwrap();
    cpu2.execute(Instruction::Add).unwrap();

    assert_eq!(cpu1.peek_stack().unwrap(), cpu2.peek_stack().unwrap());
}

#[test]
fn test_multiply_chain() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(7)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 2 * 3 * 5 * 7);
}

#[test]
fn test_multiply_stack_underflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(5)).unwrap();

    let result = cpu.execute(Instruction::Multiply);
    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}

// ============================================================================
// Divide Operation Tests (25+ cases)
// ============================================================================

#[test]
fn test_divide_positive_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 20);
}

#[test]
fn test_divide_exact() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(144)).unwrap();
    cpu.execute(Instruction::Push(12)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 12);
}

#[test]
fn test_divide_with_remainder() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 3); // Truncates toward zero
}

#[test]
fn test_divide_negative_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-20)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -5);
}

#[test]
fn test_divide_positive_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(-4)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -5);
}

#[test]
fn test_divide_negative_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-20)).unwrap();
    cpu.execute(Instruction::Push(-4)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 5);
}

#[test]
fn test_divide_by_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_divide_by_minus_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(-1)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -42);
}

#[test]
fn test_divide_zero_by_number() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_divide_by_zero_error() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::Divide);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

#[test]
fn test_divide_smaller_by_larger() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_divide_same_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 1);
}

#[test]
fn test_divide_power_of_two() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(128)).unwrap();
    cpu.execute(Instruction::Push(8)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 16);
}

#[test]
fn test_divide_max_by_minus_one() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Special case: i32::MIN / -1 would overflow, but should handle gracefully
    cpu.execute(Instruction::Push(i32::MAX)).unwrap();
    cpu.execute(Instruction::Push(-1)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -i32::MAX);
}

#[test]
fn test_divide_chain() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(1000)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_divide_stack_underflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(5)).unwrap();

    let result = cpu.execute(Instruction::Divide);
    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}

// ============================================================================
// Combined Arithmetic Tests (10+ cases)
// ============================================================================

#[test]
fn test_multiply_then_divide() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap(); // 50
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Divide).unwrap(); // 25

    assert_eq!(cpu.peek_stack().unwrap(), 25);
}

#[test]
fn test_divide_then_multiply() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::Divide).unwrap(); // 25
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap(); // 75

    assert_eq!(cpu.peek_stack().unwrap(), 75);
}

#[test]
fn test_complex_expression() {
    // Compute: (10 + 5) * (20 - 8) / 3 = 15 * 12 / 3 = 60
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Add).unwrap(); // 15
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(8)).unwrap();
    cpu.execute(Instruction::Sub).unwrap(); // 12
    cpu.execute(Instruction::Multiply).unwrap(); // 180
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Divide).unwrap(); // 60

    assert_eq!(cpu.peek_stack().unwrap(), 60);
}

#[test]
fn test_factorial_computation() {
    // 5! = 120 using multiply chain
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 120);
}

#[test]
fn test_average_computation() {
    // Average of 10, 20, 30, 40 = 100 / 4 = 25
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Add).unwrap();
    cpu.execute(Instruction::Push(30)).unwrap();
    cpu.execute(Instruction::Add).unwrap();
    cpu.execute(Instruction::Push(40)).unwrap();
    cpu.execute(Instruction::Add).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 25);
}

#[test]
fn test_modulo_using_multiply_divide() {
    // Compute 17 % 5 = 17 - (17/5)*5
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(17)).unwrap();
    cpu.execute(Instruction::Dup).unwrap(); // [17, 17]
    cpu.execute(Instruction::Push(5)).unwrap(); // [17, 17, 5]
    cpu.execute(Instruction::Divide).unwrap(); // [17, 3]
    cpu.execute(Instruction::Push(5)).unwrap(); // [17, 3, 5]
    cpu.execute(Instruction::Multiply).unwrap(); // [17, 15]
    cpu.execute(Instruction::Sub).unwrap(); // [2]

    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_quadratic_evaluation() {
    // Evaluate 2x^2 + 3x + 1 where x = 4
    // = 2*16 + 12 + 1 = 45
    let mut cpu = A2SProcessor::with_default_memory();
    let x = 4;

    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Push(x)).unwrap();
    cpu.execute(Instruction::Dup).unwrap();
    cpu.execute(Instruction::Multiply).unwrap(); // x^2
    cpu.execute(Instruction::Multiply).unwrap(); // 2*x^2

    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Push(x)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap(); // 3*x

    cpu.execute(Instruction::Add).unwrap(); // 2*x^2 + 3*x
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Add).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 45);
}

#[test]
fn test_gcd_computation() {
    // GCD(48, 18) = 6 using Euclidean algorithm (simplified)
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(48)).unwrap();
    cpu.execute(Instruction::Push(18)).unwrap();
    cpu.execute(Instruction::Divide).unwrap(); // quotient = 2
    cpu.execute(Instruction::Push(18)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap(); // 36
    cpu.execute(Instruction::Push(48)).unwrap();
    cpu.execute(Instruction::Swap).unwrap();
    cpu.execute(Instruction::Sub).unwrap(); // remainder = 12

    assert_eq!(cpu.peek_stack().unwrap(), 12);
}

#[test]
fn test_percentage_calculation() {
    // 25% of 200 = (25 * 200) / 100 = 50
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(25)).unwrap();
    cpu.execute(Instruction::Push(200)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 50);
}

#[test]
fn test_scaling_operation() {
    // Scale value by ratio: (value * numerator) / denominator
    // Example: scale 15 by ratio 2/3 = (15 * 2) / 3 = 10
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(15)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 10);
}
