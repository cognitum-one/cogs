// TDD London School - Memory Operation Tests

use cognitum_processor::{A2SProcessor, Instruction, ProcessorError};

#[test]
fn test_store_and_load() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store 42 at address 0x1000
    cpu.execute(Instruction::Push(42)).unwrap();
    cpu.execute(Instruction::Push(0x1000)).unwrap();
    cpu.execute(Instruction::Store).unwrap();

    // Load from address 0x1000
    cpu.execute(Instruction::Push(0x1000)).unwrap();
    cpu.execute(Instruction::Load).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_load_uninitialized_memory() {
    let mut cpu = A2SProcessor::with_default_memory();

    cpu.execute(Instruction::Push(0x2000)).unwrap();
    cpu.execute(Instruction::Load).unwrap();

    // Uninitialized memory should read as 0
    assert_eq!(cpu.peek_stack().unwrap(), 0);
}

#[test]
fn test_store_register_a() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Set register A to 0x1000
    cpu.execute(Instruction::Push(0x1000)).unwrap();
    cpu.execute(Instruction::ToA).unwrap();

    // Store value via register A
    cpu.execute(Instruction::Push(99)).unwrap();
    cpu.execute(Instruction::StoreA).unwrap();

    // Verify by loading from same address
    cpu.execute(Instruction::Push(0x1000)).unwrap();
    cpu.execute(Instruction::Load).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 99);
}

#[test]
fn test_load_register_b() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store value at 0x2000
    cpu.execute(Instruction::Push(77)).unwrap();
    cpu.execute(Instruction::Push(0x2000)).unwrap();
    cpu.execute(Instruction::Store).unwrap();

    // Set register B to 0x2000
    cpu.execute(Instruction::Push(0x2000)).unwrap();
    cpu.execute(Instruction::ToB).unwrap();

    // Load via register B
    cpu.execute(Instruction::LoadB).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 77);
}

#[test]
fn test_load_register_c() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Setup
    cpu.execute(Instruction::Push(88)).unwrap();
    cpu.execute(Instruction::Push(0x3000)).unwrap();
    cpu.execute(Instruction::Store).unwrap();

    cpu.execute(Instruction::Push(0x3000)).unwrap();
    cpu.execute(Instruction::ToC).unwrap();

    cpu.execute(Instruction::LoadC).unwrap();

    assert_eq!(cpu.peek_stack().unwrap(), 88);
}

#[test]
fn test_memory_alignment_error() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Try to load from unaligned address
    cpu.execute(Instruction::Push(0x1001)).unwrap();
    let result = cpu.execute(Instruction::Load);

    assert!(matches!(
        result,
        Err(ProcessorError::InvalidMemoryAddress(_))
    ));
}

#[test]
fn test_register_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test >a and a> (to/from register A)
    cpu.execute(Instruction::Push(0x1234)).unwrap();
    cpu.execute(Instruction::ToA).unwrap();
    assert_eq!(cpu.get_reg_a(), 0x1234);

    cpu.execute(Instruction::FromA).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 0x1234);
}

#[test]
fn test_multiple_memory_locations() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store different values at different locations
    for i in 0..10 {
        cpu.execute(Instruction::Push(i * 10)).unwrap();
        cpu.execute(Instruction::Push(0x1000 + (i as i32 * 4)))
            .unwrap();
        cpu.execute(Instruction::Store).unwrap();
    }

    // Verify one of them
    cpu.execute(Instruction::Push(0x1010)).unwrap(); // 4th location
    cpu.execute(Instruction::Load).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 40);
}
