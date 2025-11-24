// TDD London School - Arithmetic Operation Tests

use cognitum_processor::{A2SProcessor, Instruction, ProcessorError};

#[test]
fn test_add_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Add).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 30);
    assert_eq!(cpu.stack_depth(), 1);
}

#[test]
fn test_add_negative_numbers() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(-5)).unwrap();
    cpu.execute(Instruction::Push(3)).unwrap();
    cpu.execute(Instruction::Add).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -2);
}

#[test]
fn test_sub_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(30)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Sub).unwrap();

    // ( 30 10 -- 20 ) -> 30 - 10 = 20
    assert_eq!(cpu.peek_stack().unwrap(), 20);
}

#[test]
fn test_multiply_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(6)).unwrap();
    cpu.execute(Instruction::Push(7)).unwrap();
    cpu.execute(Instruction::Multiply).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_divide_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(6)).unwrap();
    cpu.execute(Instruction::Divide).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 7);
}

#[test]
fn test_divide_by_zero() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(0)).unwrap();

    let result = cpu.execute(Instruction::Divide);
    assert_eq!(result, Err(ProcessorError::DivisionByZero));
}

#[test]
fn test_equal_true() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Equal).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), -1); // TRUE
}

#[test]
fn test_equal_false() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Equal).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0); // FALSE
}

#[test]
fn test_less_than_true() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::LessThan).unwrap();

    // 10 < 20 = TRUE
    assert_eq!(cpu.peek_stack().unwrap(), -1);
}

#[test]
fn test_less_than_false() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(20)).unwrap();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::LessThan).unwrap();

    // 20 < 10 = FALSE
    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_unsigned_less_than() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();
    cpu.execute(Instruction::Push(-1)).unwrap(); // 0xFFFFFFFF as unsigned
    cpu.execute(Instruction::UnsignedLessThan).unwrap();

    // 10 u< 0xFFFFFFFF = TRUE
    assert_eq!(cpu.peek_stack().unwrap(), -1);
}

#[test]
fn test_arithmetic_underflow() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(10)).unwrap();

    let result = cpu.execute(Instruction::Add);
    assert_eq!(result, Err(ProcessorError::StackUnderflow));
}
