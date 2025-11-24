// TDD London School - Control Flow Tests

use cognitum_processor::{A2SProcessor, Instruction};

#[test]
fn test_unconditional_jump() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    cpu.execute(Instruction::Jump(100)).unwrap();

    assert_eq!(cpu.get_pc(), initial_pc + 100);
}

#[test]
fn test_jump_zero_taken() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    cpu.execute(Instruction::Push(0)).unwrap();
    cpu.execute(Instruction::JumpZero(50)).unwrap();

    assert_eq!(cpu.get_pc(), initial_pc + 50);
    assert_eq!(cpu.stack_depth(), 0); // Value consumed
}

#[test]
fn test_jump_zero_not_taken() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::JumpZero(50)).unwrap();

    assert_eq!(cpu.get_pc(), initial_pc); // PC unchanged
}

#[test]
fn test_jump_negative_taken() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    cpu.execute(Instruction::Push(-5)).unwrap();
    cpu.execute(Instruction::JumpNegative(30)).unwrap();

    assert_eq!(cpu.get_pc(), initial_pc + 30);
}

#[test]
fn test_jump_negative_not_taken() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    cpu.execute(Instruction::Push(5)).unwrap();
    cpu.execute(Instruction::JumpNegative(30)).unwrap();

    assert_eq!(cpu.get_pc(), initial_pc);
}

#[test]
fn test_call_and_return() {
    let mut cpu = A2SProcessor::with_default_memory();
    let initial_pc = cpu.get_pc();

    // Call subroutine
    cpu.execute(Instruction::Call(100)).unwrap();
    assert_eq!(cpu.get_pc(), initial_pc + 100);

    // Return from subroutine
    cpu.execute(Instruction::Return).unwrap();
    assert_eq!(cpu.get_pc(), initial_pc);
}

#[test]
fn test_nested_calls() {
    let mut cpu = A2SProcessor::with_default_memory();
    let pc0 = cpu.get_pc();

    // First call
    cpu.execute(Instruction::Call(100)).unwrap();
    let pc1 = cpu.get_pc();

    // Nested call
    cpu.execute(Instruction::Call(50)).unwrap();
    let pc2 = cpu.get_pc();

    // Return from nested call
    cpu.execute(Instruction::Return).unwrap();
    assert_eq!(cpu.get_pc(), pc1);

    // Return from first call
    cpu.execute(Instruction::Return).unwrap();
    assert_eq!(cpu.get_pc(), pc0);
}

#[test]
fn test_return_stack_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Push to return stack
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::ToR).unwrap();

    assert_eq!(cpu.stack_depth(), 0);

    // Pop from return stack
    cpu.execute(Instruction::FromR).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_backward_jump() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Simulate jumping backward (loop)
    cpu.execute(Instruction::Jump(100)).unwrap();
    let forward_pc = cpu.get_pc();

    cpu.execute(Instruction::Jump(-50)).unwrap();
    let backward_pc = cpu.get_pc();

    assert!(backward_pc < forward_pc);
}

#[test]
fn test_halt_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();

    cpu.execute(Instruction::Halt).unwrap();
    assert!(cpu.is_halted());

    // Further instructions should not execute
    cpu.execute(Instruction::Push(42)).unwrap();
    assert_eq!(cpu.stack_depth(), 0); // Stack unchanged
}
