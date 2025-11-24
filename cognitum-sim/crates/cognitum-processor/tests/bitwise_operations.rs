// TDD London School - Bitwise Logic Tests

use cognitum_processor::{A2SProcessor, Instruction};

#[test]
fn test_and_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b1111_0000)).unwrap();
    cpu.execute(Instruction::Push(0b1010_1010)).unwrap();
    cpu.execute(Instruction::And).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b1010_0000);
}

#[test]
fn test_or_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b1111_0000)).unwrap();
    cpu.execute(Instruction::Push(0b0000_1111)).unwrap();
    cpu.execute(Instruction::Or).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b1111_1111);
}

#[test]
fn test_xor_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b1111_0000)).unwrap();
    cpu.execute(Instruction::Push(0b1010_1010)).unwrap();
    cpu.execute(Instruction::Xor).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b0101_1010);
}

#[test]
fn test_not_instruction() {
    let mut cpu = A2SProcessor::with_default_memory();
    cpu.execute(Instruction::Push(0b0000_0000_1111_1111))
        .unwrap();
    cpu.execute(Instruction::Not).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), !0b0000_0000_1111_1111);
}

#[test]
fn test_bitwise_mask_operation() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Test common masking pattern
    cpu.execute(Instruction::Push(0x12345678)).unwrap();
    cpu.execute(Instruction::Push(0xFF)).unwrap();
    cpu.execute(Instruction::And).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0x78);
}

#[test]
fn test_bitwise_set_flags() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Set bit flags
    cpu.execute(Instruction::Push(0b0001)).unwrap();
    cpu.execute(Instruction::Push(0b0100)).unwrap();
    cpu.execute(Instruction::Or).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b0101);
}

#[test]
fn test_bitwise_clear_flags() {
    let mut cpu = A2SProcessor::with_default_memory();
    // Clear specific flags
    cpu.execute(Instruction::Push(0b1111)).unwrap();
    cpu.execute(Instruction::Push(0b0101)).unwrap();
    cpu.execute(Instruction::Not).unwrap();
    cpu.execute(Instruction::And).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 0b1010);
}
