// TDD London School - Shift and Rotate Operations Tests
// Extended ISA implementation for A2S v2r3

use cognitum_processor::{A2SProcessor, Instruction};

// ============================================================================
// LOGICAL SHIFT LEFT (LSL) Tests
// ============================================================================

#[test]
fn test_shift_left_basic() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b1010)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap(); // Shift by 2
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b101000);
}

#[test]
fn test_shift_left_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFF)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap(); // Shift by 0
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0xFF);
}

#[test]
fn test_shift_left_max_shift() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Push(31)).unwrap(); // Maximum shift
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x80000000);
}

#[test]
fn test_shift_left_overflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0xFFFFFFFE);
}

#[test]
fn test_shift_left_imm() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b1111)).unwrap();
    cpu.execute(Instruction::ShiftLeftImm(4)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b11110000);
}

// ============================================================================
// LOGICAL SHIFT RIGHT (LSR) Tests
// ============================================================================

#[test]
fn test_shift_right_basic() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b101000)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap(); // Shift by 2
    cpu.execute(Instruction::ShiftRight).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b1010);
}

#[test]
fn test_shift_right_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFF)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap(); // Shift by 0
    cpu.execute(Instruction::ShiftRight).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0xFF);
}

#[test]
fn test_shift_right_fills_with_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-1)).unwrap(); // All bits set
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::ShiftRight).unwrap();

    // Logical shift fills with zeros from the left
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x7FFFFFFF);
}

#[test]
fn test_shift_right_imm() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b11110000)).unwrap();
    cpu.execute(Instruction::ShiftRightImm(4)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b1111);
}

// ============================================================================
// ARITHMETIC SHIFT RIGHT (ASR) Tests - Sign Extension
// ============================================================================

#[test]
fn test_shift_right_arith_positive() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x7FFFFFFF)).unwrap(); // Max positive
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::ShiftRightArith).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0x07FFFFFF);
}

#[test]
fn test_shift_right_arith_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-8)).unwrap(); // 0xFFFFFFF8
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::ShiftRightArith).unwrap();

    // Should preserve sign bit (fill with 1s)
    assert_eq!(cpu.peek_stack().unwrap(), -2); // 0xFFFFFFFE
}

#[test]
fn test_shift_right_arith_sign_extend() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-1)).unwrap(); // All bits set
    cpu.execute(Instruction::Push(8)).unwrap();
    cpu.execute(Instruction::ShiftRightArith).unwrap();

    // Should still be -1 (all bits remain set)
    assert_eq!(cpu.peek_stack().unwrap(), -1);
}

#[test]
fn test_shift_right_arith_imm_negative() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-16)).unwrap();
    cpu.execute(Instruction::ShiftRightArithImm(2)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -4);
}

#[test]
fn test_shift_right_arith_edge_case() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x80000000_u32 as i32)).unwrap(); // Min negative
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::ShiftRightArith).unwrap();

    // Should fill with 1s (sign extend)
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0xC0000000);
}

// ============================================================================
// ROTATE LEFT (ROL) Tests
// ============================================================================

#[test]
fn test_rotate_left_basic() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x12345678)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::RotateLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x23456781);
}

#[test]
fn test_rotate_left_full_rotation() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xABCDEF00_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(32)).unwrap(); // Full rotation (masked to 0)
    cpu.execute(Instruction::RotateLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0xABCDEF00); // Unchanged
}

#[test]
fn test_rotate_left_wrap_around() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x80000001_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::RotateLeft).unwrap();

    // MSB wraps to LSB
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x00000003);
}

#[test]
fn test_rotate_left_imm() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xF0000001_u32 as i32)).unwrap();
    cpu.execute(Instruction::RotateLeftImm(4)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x0000001F);
}

// ============================================================================
// ROTATE RIGHT (ROR) Tests
// ============================================================================

#[test]
fn test_rotate_right_basic() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x12345678)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::RotateRight).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x81234567);
}

#[test]
fn test_rotate_right_full_rotation() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xABCDEF00_u32 as i32)).unwrap();
    cpu.execute(Instruction::Push(32)).unwrap(); // Full rotation (masked to 0)
    cpu.execute(Instruction::RotateRight).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0xABCDEF00); // Unchanged
}

#[test]
fn test_rotate_right_wrap_around() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x00000003)).unwrap();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::RotateRight).unwrap();

    // LSB wraps to MSB
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x80000001);
}

#[test]
fn test_rotate_right_imm() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xF0000000_u32 as i32)).unwrap();
    cpu.execute(Instruction::RotateRightImm(4)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x0F000000);
}

// ============================================================================
// COMPREHENSIVE EDGE CASES
// ============================================================================

#[test]
fn test_shift_amount_masking() {
    // Verify that shift amounts > 31 are masked to 5 bits
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::Push(33)).unwrap(); // Should be masked to 1
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 2);
}

#[test]
fn test_rotate_preserves_all_bits() {
    let mut cpu = A2SProcessor::with_default_memory();
    let test_value: u32 = 0xDEADBEEF;
    cpu.execute(Instruction::Push(test_value as i32)).unwrap();
    cpu.execute(Instruction::Push(8)).unwrap();
    cpu.execute(Instruction::RotateLeft).unwrap();
    cpu.execute(Instruction::Push(8)).unwrap();
    cpu.execute(Instruction::RotateRight).unwrap();

    // Should get back original value
    assert_eq!(cpu.peek_stack().unwrap() as u32, test_value);
}

#[test]
fn test_shift_left_then_right() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0xFF)).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::ShiftLeft).unwrap();
    cpu.execute(Instruction::Push(4)).unwrap();
    cpu.execute(Instruction::ShiftRight).unwrap();

    // Should get back original value
    assert_eq!(cpu.peek_stack().unwrap(), 0xFF);
}

#[test]
fn test_arithmetic_vs_logical_shift() {
    let mut cpu1 = A2SProcessor::with_default_memory();
    let mut cpu2 = A2SProcessor::with_default_memory();

    // Arithmetic shift on negative number
    cpu1.execute(Instruction::Push(-16)).unwrap();
    cpu1.execute(Instruction::Push(2)).unwrap();
    cpu1.execute(Instruction::ShiftRightArith).unwrap();

    // Logical shift on same bit pattern
    cpu2.execute(Instruction::Push(-16)).unwrap();
    cpu2.execute(Instruction::Push(2)).unwrap();
    cpu2.execute(Instruction::ShiftRight).unwrap();

    // Results should differ
    let arith_result = cpu1.peek_stack().unwrap();
    let logical_result = cpu2.peek_stack().unwrap();

    assert_ne!(arith_result, logical_result);
    assert_eq!(arith_result, -4); // Sign extended
    assert_eq!(logical_result as u32, 0x3FFFFFFC); // Zero filled
}

// ============================================================================
// PRACTICAL USE CASES
// ============================================================================

#[test]
fn test_extract_byte_with_shift() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0x12345678)).unwrap();
    cpu.execute(Instruction::Push(16)).unwrap(); // Shift to get 0x1234
    cpu.execute(Instruction::ShiftRight).unwrap();
    cpu.execute(Instruction::Push(0xFF)).unwrap();
    cpu.execute(Instruction::And).unwrap(); // Mask to get 0x34

    // 0x12345678 >> 16 = 0x1234, then 0x1234 & 0xFF = 0x34
    assert_eq!(cpu.peek_stack().unwrap(), 0x34);
}

#[test]
fn test_build_value_with_shifts() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Build 0x12340000 from 0x1234
    cpu.execute(Instruction::Push(0x1234)).unwrap();
    cpu.execute(Instruction::Push(16)).unwrap();
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x12340000);
}

#[test]
fn test_power_of_two_multiply() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Multiply by 8 using shift (faster than multiply)
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap(); // Shift by 3 = multiply by 8
    cpu.execute(Instruction::ShiftLeft).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 336);
}

#[test]
fn test_power_of_two_divide() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Divide by 4 using shift (faster than divide)
    cpu.execute(Instruction::Push(100)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap(); // Shift by 2 = divide by 4
    cpu.execute(Instruction::ShiftRight).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 25);
}

#[test]
fn test_signed_divide_with_arith_shift() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Divide -100 by 4 using arithmetic shift
    cpu.execute(Instruction::Push(-100)).unwrap();
    cpu.execute(Instruction::Push(2)).unwrap();
    cpu.execute(Instruction::ShiftRightArith).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -25);
}

// ============================================================================
// IMMEDIATE VARIANTS COMPREHENSIVE TESTS
// ============================================================================

#[test]
fn test_all_immediate_variants() {
    // Test all immediate instruction variants work correctly
    let mut cpu = A2SProcessor::with_default_memory();

    // ShiftLeftImm
    cpu.execute(Instruction::Push(1)).unwrap();
    cpu.execute(Instruction::ShiftLeftImm(8)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 256);
    cpu.execute(Instruction::Pop).unwrap();

    // ShiftRightImm
    cpu.execute(Instruction::Push(256)).unwrap();
    cpu.execute(Instruction::ShiftRightImm(8)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 1);
    cpu.execute(Instruction::Pop).unwrap();

    // ShiftRightArithImm
    cpu.execute(Instruction::Push(-256)).unwrap();
    cpu.execute(Instruction::ShiftRightArithImm(4)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), -16);
    cpu.execute(Instruction::Pop).unwrap();

    // RotateLeftImm
    cpu.execute(Instruction::Push(0x0000FFFF)).unwrap();
    cpu.execute(Instruction::RotateLeftImm(16)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0xFFFF0000);
    cpu.execute(Instruction::Pop).unwrap();

    // RotateRightImm
    cpu.execute(Instruction::Push(0xFFFF0000_u32 as i32)).unwrap();
    cpu.execute(Instruction::RotateRightImm(16)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap() as u32, 0x0000FFFF);
}

// ============================================================================
// BOUNDARY AND STRESS TESTS
// ============================================================================

#[test]
fn test_all_shift_amounts() {
    // Test all possible shift amounts from 0 to 31
    for shift in 0..32 {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(0xFFFFFFFF_u32 as i32)).unwrap();
        cpu.execute(Instruction::Push(shift)).unwrap();
        cpu.execute(Instruction::ShiftLeft).unwrap();

        let expected = if shift == 0 {
            0xFFFFFFFF_u32
        } else {
            (0xFFFFFFFF_u32 << shift) & 0xFFFFFFFF
        };

        assert_eq!(
            cpu.peek_stack().unwrap() as u32,
            expected,
            "Failed at shift amount {}",
            shift
        );
    }
}

#[test]
fn test_all_rotate_amounts() {
    // Test all possible rotate amounts from 0 to 31
    let test_pattern: u32 = 0x12345678;
    for rotate in 0..32 {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(test_pattern as i32)).unwrap();
        cpu.execute(Instruction::Push(rotate)).unwrap();
        cpu.execute(Instruction::RotateLeft).unwrap();

        let expected = test_pattern.rotate_left(rotate as u32);

        assert_eq!(
            cpu.peek_stack().unwrap() as u32,
            expected,
            "Failed at rotate amount {}",
            rotate
        );
    }
}
