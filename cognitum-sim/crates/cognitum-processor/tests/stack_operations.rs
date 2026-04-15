// TDD London School - Stack Operation Tests (RED phase)

use cognitum_processor::{A2SProcessor, Instruction, ProcessorError};

#[test]
fn test_push_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 42);
    assert_eq!(cpu.stack_depth(), 1);
}

#[test]
fn test_push_multiple() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(30)).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 30);
    assert_eq!(cpu.stack_depth(), 3);
}

#[test]
fn test_pop_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Pop).unwrap();

    assert_eq!(cpu.stack_depth(), 0);
}

#[test]
fn test_pop_underflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    let result = cpu.execute(Instruction::Pop);

    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}

#[test]
fn test_dup_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Dup).unwrap();

    assert_eq!(cpu.stack_depth(), 2);
    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_swap_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Swap).unwrap();

    // After swap: 20, 10 (top is 10)
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_over_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Over).unwrap();

    // ( 10 20 -- 10 20 10 )
    assert_eq!(cpu.stack_depth(), 3);
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_rot3_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(30)).unwrap();
    cpu.execute(Instruction::Rot3).unwrap();

    // ( 10 20 30 -- 20 30 10 )
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_rot4_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(30)).unwrap();
    cpu.execute(Instruction::Push(40)).unwrap();
    cpu.execute(Instruction::Rot4).unwrap();

    // ( 10 20 30 40 -- 20 30 40 10 )
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_drop_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Drop).unwrap();

    // ( 10 20 -- 10 )
    assert_eq!(cpu.stack_depth(), 1);
    assert_eq!(cpu.peek_stack().unwrap(), 10);
}

#[test]
fn test_nip_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Nip).unwrap();

    // ( 10 20 -- 20 )
    assert_eq!(cpu.stack_depth(), 1);
    assert_eq!(cpu.peek_stack().unwrap(), 20);
}
