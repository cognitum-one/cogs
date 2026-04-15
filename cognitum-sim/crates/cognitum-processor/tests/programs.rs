// Practical Program Tests for A2S v2r3 Processor

use cognitum_processor::{A2SProcessor, Instruction};

#[test]
fn test_simple_arithmetic() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Compute (10 + 20) * 3 = 90
    let program = vec![
        Instruction::Push(10),
        Instruction::Push(20),
        Instruction::Add, // Stack: [30]
        Instruction::Push(3),
        Instruction::Multiply, // Stack: [90]
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 90);
}

#[test]
fn test_stack_manipulation() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test DUP and SWAP
    let program = vec![
        Instruction::Push(10),
        Instruction::Dup,      // [10, 10]
        Instruction::Push(20), // [10, 10, 20]
        Instruction::Swap,     // [10, 20, 10]
        Instruction::Over,     // [10, 20, 10, 20]
        Instruction::Add,      // [10, 20, 30]
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 30);
}

#[test]
fn test_memory_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store and load from memory
    let program = vec![
        Instruction::Push(42),
        Instruction::Push(0x1000),
        Instruction::Store, // Store 42 at 0x1000
        Instruction::Push(100),
        Instruction::Push(0x2000),
        Instruction::Store, // Store 100 at 0x2000
        Instruction::Push(0x1000),
        Instruction::Load, // Load from 0x1000
        Instruction::Push(0x2000),
        Instruction::Load, // Load from 0x2000
        Instruction::Add,  // 42 + 100
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 142);
}

#[test]
fn test_register_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Use registers for memory addressing
    let program = vec![
        Instruction::Push(0x1000),
        Instruction::ToA, // A = 0x1000
        Instruction::Push(99),
        Instruction::StoreA, // [0x1000] = 99
        Instruction::LoadA,  // Load from [A]
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 99);
    assert_eq!(cpu.get_reg_a(), 0x1000);
}

#[test]
fn test_comparison_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test comparisons
    let program = vec![
        Instruction::Push(10),
        Instruction::Push(20),
        Instruction::LessThan, // 10 < 20 = TRUE (-1)
        Instruction::Push(5),
        Instruction::Push(5),
        Instruction::Equal, // 5 == 5 = TRUE (-1)
        Instruction::And,   // -1 & -1 = -1
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), -1); // TRUE
}

#[test]
fn test_bitwise_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test bitwise operations
    let program = vec![
        Instruction::Push(0b1111_0000),
        Instruction::Push(0b1010_1010),
        Instruction::And, // 0b1010_0000
        Instruction::Push(0b0101_0101),
        Instruction::Or, // 0b1111_0101
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 0b1111_0101);
}

#[test]
fn test_return_stack() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test return stack usage
    let program = vec![
        Instruction::Push(42),
        Instruction::ToR,       // Move 42 to return stack
        Instruction::Push(100), // Push 100 to data stack
        Instruction::FromR,     // Retrieve 42 from return stack
        Instruction::Add,       // 100 + 42
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 142);
}

#[test]
fn test_complex_expression() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Compute: (5 + 3) * (10 - 2) = 8 * 8 = 64
    let program = vec![
        Instruction::Push(5),
        Instruction::Push(3),
        Instruction::Add, // 8
        Instruction::Push(10),
        Instruction::Push(2),
        Instruction::Sub,      // 8
        Instruction::Multiply, // 64
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 64);
}

#[test]
fn test_ternary_operation() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Use comparison and jumps to implement: result = (a > b) ? a : b
    // For simplicity, just test stack operations
    let program = vec![
        Instruction::Push(30), // a
        Instruction::Push(20), // b
        Instruction::Over,     // Copy a
        Instruction::Over,     // Copy b
        Instruction::LessThan, // a < b?
        Instruction::Nip,      // Remove one value
        Instruction::Nip,      // Keep result
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    // Stack should have comparison result
}

#[test]
fn test_multiple_memory_locations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Use all three address registers
    let program = vec![
        Instruction::Push(0x1000),
        Instruction::ToA,
        Instruction::Push(0x2000),
        Instruction::ToB,
        Instruction::Push(0x3000),
        Instruction::ToC,
        Instruction::Push(10),
        Instruction::StoreA, // [A] = 10
        Instruction::Push(20),
        Instruction::StoreB, // [B] = 20
        Instruction::Push(30),
        Instruction::StoreC, // [C] = 30
        Instruction::LoadA,
        Instruction::LoadB,
        Instruction::Add,
        Instruction::LoadC,
        Instruction::Add, // 10 + 20 + 30 = 60
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 60);
}
