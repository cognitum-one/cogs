//! Property-Based Tests for Cognitum Processor
//!
//! Uses proptest to generate random inputs and verify mathematical properties
//! hold for all valid inputs.

use cognitum_processor::{A2SProcessor, Instruction};
use proptest::prelude::*;

// ============================================================================
// Arithmetic Property Tests
// ============================================================================

proptest! {
    #[test]
    fn test_addition_commutative(a in -10000i32..10000i32, b in -10000i32..10000i32) {
        // a + b = b + a
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Add).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Add).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_addition_associative(a in -1000i32..1000i32, b in -1000i32..1000i32, c in -1000i32..1000i32) {
        // (a + b) + c = a + (b + c)
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Add).unwrap();
        cpu1.execute(Instruction::Push(c)).unwrap();
        cpu1.execute(Instruction::Add).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(c)).unwrap();
        cpu2.execute(Instruction::Add).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Add).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_addition_identity(a in -100000i32..100000i32) {
        // a + 0 = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(0)).unwrap();
        cpu.execute(Instruction::Add).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_multiplication_commutative(a in -1000i32..1000i32, b in -1000i32..1000i32) {
        // a * b = b * a
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Multiply).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Multiply).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_multiplication_identity(a in -100000i32..100000i32) {
        // a * 1 = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(1)).unwrap();
        cpu.execute(Instruction::Multiply).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_multiplication_zero(a in -100000i32..100000i32) {
        // a * 0 = 0
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(0)).unwrap();
        cpu.execute(Instruction::Multiply).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), 0);
    }

    #[test]
    fn test_multiplication_distributive(a in -100i32..100i32, b in -100i32..100i32, c in -100i32..100i32) {
        // a * (b + c) = (a * b) + (a * c)
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Push(c)).unwrap();
        cpu1.execute(Instruction::Add).unwrap();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Multiply).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Multiply).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Push(c)).unwrap();
        cpu2.execute(Instruction::Multiply).unwrap();
        cpu2.execute(Instruction::Add).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_subtraction_inverse(a in -100000i32..100000i32, b in -100000i32..100000i32) {
        // (a + b) - b = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Add).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Sub).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_division_then_multiply(a in -10000i32..10000i32, b in 1i32..1000i32) {
        // (a / b) * b = a - (a % b)
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Divide).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Multiply).unwrap();

        let result = cpu.peek_stack().unwrap();
        let expected = (a / b) * b;
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn test_double_negation(a in -100000i32..100000i32) {
        // -(-a) = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(-1)).unwrap();
        cpu.execute(Instruction::Multiply).unwrap();
        cpu.execute(Instruction::Push(-1)).unwrap();
        cpu.execute(Instruction::Multiply).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }
}

// ============================================================================
// Logical Operation Properties
// ============================================================================

proptest! {
    #[test]
    fn test_and_commutative(a: i32, b: i32) {
        // a & b = b & a
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::And).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::And).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_or_commutative(a: i32, b: i32) {
        // a | b = b | a
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Or).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Or).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_xor_commutative(a: i32, b: i32) {
        // a ^ b = b ^ a
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Xor).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Xor).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_xor_self_is_zero(a: i32) {
        // a ^ a = 0
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Xor).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), 0);
    }

    #[test]
    fn test_double_negation_bitwise(a: i32) {
        // ~~a = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Not).unwrap();
        cpu.execute(Instruction::Not).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_de_morgan_law_1(a: i32, b: i32) {
        // ~(a & b) = ~a | ~b
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::And).unwrap();
        cpu1.execute(Instruction::Not).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Not).unwrap();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Not).unwrap();
        cpu2.execute(Instruction::Or).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_de_morgan_law_2(a: i32, b: i32) {
        // ~(a | b) = ~a & ~b
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Or).unwrap();
        cpu1.execute(Instruction::Not).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Not).unwrap();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Not).unwrap();
        cpu2.execute(Instruction::And).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_and_with_zero(a: i32) {
        // a & 0 = 0
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(0)).unwrap();
        cpu.execute(Instruction::And).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), 0);
    }

    #[test]
    fn test_or_with_zero(a: i32) {
        // a | 0 = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(0)).unwrap();
        cpu.execute(Instruction::Or).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_and_with_all_ones(a: i32) {
        // a & -1 = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(-1)).unwrap();
        cpu.execute(Instruction::And).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_xor_with_zero(a: i32) {
        // a ^ 0 = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(0)).unwrap();
        cpu.execute(Instruction::Xor).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }
}

// ============================================================================
// Comparison Properties
// ============================================================================

proptest! {
    #[test]
    fn test_equality_reflexive(a: i32) {
        // a = a
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Equal).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), -1); // TRUE
    }

    #[test]
    fn test_equality_symmetric(a in -10000i32..10000i32, b in -10000i32..10000i32) {
        // (a = b) ⟺ (b = a)
        let mut cpu1 = A2SProcessor::with_default_memory();
        cpu1.execute(Instruction::Push(a)).unwrap();
        cpu1.execute(Instruction::Push(b)).unwrap();
        cpu1.execute(Instruction::Equal).unwrap();
        let result1 = cpu1.peek_stack().unwrap();

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::Equal).unwrap();
        let result2 = cpu2.peek_stack().unwrap();

        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn test_less_than_antisymmetric(a in -10000i32..10000i32, b in -10000i32..10000i32) {
        // if a < b, then !(b < a)
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::LessThan).unwrap();
        let a_lt_b = cpu.peek_stack().unwrap() == -1;

        let mut cpu2 = A2SProcessor::with_default_memory();
        cpu2.execute(Instruction::Push(b)).unwrap();
        cpu2.execute(Instruction::Push(a)).unwrap();
        cpu2.execute(Instruction::LessThan).unwrap();
        let b_lt_a = cpu2.peek_stack().unwrap() == -1;

        if a < b {
            prop_assert!(a_lt_b);
            prop_assert!(!b_lt_a);
        } else if a > b {
            prop_assert!(!a_lt_b);
            prop_assert!(b_lt_a);
        } else {
            prop_assert!(!a_lt_b);
            prop_assert!(!b_lt_a);
        }
    }

    #[test]
    fn test_less_than_transitive(
        a in -1000i32..1000i32,
        b in -1000i32..1000i32,
        c in -1000i32..1000i32
    ) {
        // if a < b and b < c, then a < c
        if a < b && b < c {
            let mut cpu = A2SProcessor::with_default_memory();
            cpu.execute(Instruction::Push(a)).unwrap();
            cpu.execute(Instruction::Push(c)).unwrap();
            cpu.execute(Instruction::LessThan).unwrap();

            prop_assert_eq!(cpu.peek_stack().unwrap(), -1); // a < c should be TRUE
        }
    }
}

// ============================================================================
// Stack Operation Properties
// ============================================================================

proptest! {
    #[test]
    fn test_dup_creates_duplicate(a: i32) {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Dup).unwrap();

        prop_assert_eq!(cpu.stack_depth(), 2);

        cpu.execute(Instruction::Pop).unwrap();
        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_swap_reverses_order(a: i32, b: i32) {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Swap).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), a);

        cpu.execute(Instruction::Pop).unwrap();
        prop_assert_eq!(cpu.peek_stack().unwrap(), b);
    }

    #[test]
    fn test_over_duplicates_second(a: i32, b: i32) {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Over).unwrap();

        prop_assert_eq!(cpu.stack_depth(), 3);
        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }

    #[test]
    fn test_double_swap_is_identity(a: i32, b: i32) {
        let mut cpu = A2SProcessor::with_default_memory();
        cpu.execute(Instruction::Push(a)).unwrap();
        cpu.execute(Instruction::Push(b)).unwrap();
        cpu.execute(Instruction::Swap).unwrap();
        cpu.execute(Instruction::Swap).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), b);

        cpu.execute(Instruction::Pop).unwrap();
        prop_assert_eq!(cpu.peek_stack().unwrap(), a);
    }
}

// ============================================================================
// Memory Operation Properties
// ============================================================================

proptest! {
    #[test]
    fn test_store_then_load(value: i32, addr_word in 256u32..512u32) {
        // Memory must be 4-byte aligned
        let addr = addr_word * 4;

        let mut cpu = A2SProcessor::with_default_memory();

        // Store
        cpu.execute(Instruction::Push(value)).unwrap();
        cpu.execute(Instruction::Push(addr as i32)).unwrap();
        cpu.execute(Instruction::Store).unwrap();

        // Load
        cpu.execute(Instruction::Push(addr as i32)).unwrap();
        cpu.execute(Instruction::Load).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), value);
    }

    #[test]
    fn test_memory_isolation(
        value1: i32,
        value2: i32,
        addr1_word in 256u32..448u32,
        offset_words in 64u32..200u32
    ) {
        // Memory must be 4-byte aligned
        let addr1 = addr1_word * 4;
        let addr2 = addr1 + (offset_words * 4);

        let mut cpu = A2SProcessor::with_default_memory();

        // Store to addr1
        cpu.execute(Instruction::Push(value1)).unwrap();
        cpu.execute(Instruction::Push(addr1 as i32)).unwrap();
        cpu.execute(Instruction::Store).unwrap();

        // Store to addr2
        cpu.execute(Instruction::Push(value2)).unwrap();
        cpu.execute(Instruction::Push(addr2 as i32)).unwrap();
        cpu.execute(Instruction::Store).unwrap();

        // Load from addr1 - should still be value1
        cpu.execute(Instruction::Push(addr1 as i32)).unwrap();
        cpu.execute(Instruction::Load).unwrap();

        prop_assert_eq!(cpu.peek_stack().unwrap(), value1);
    }
}
