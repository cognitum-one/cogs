//! WASM trap handling tests
//!
//! Tests all trap codes and error conditions

use cognitum_wasm_sim::{
    WasmSimulator,
    scale::ScaleLevel,
    wasm::{WasmTile, WasmConfig, WasmStack},
    error::{WasmSimError, WasmTrap},
};

/// Test tile creation with valid config
#[test]
fn test_tile_creation_valid() {
    let tile = WasmTile::new(0, WasmConfig::default());
    assert!(tile.is_ok());
}

/// Test tile creation with all tile IDs
#[test]
fn test_all_tile_ids() {
    for id in 0..256u16 {
        let tile = WasmTile::new(id, WasmConfig::default());
        assert!(tile.is_ok(), "Failed to create tile {}", id);
    }
}

/// Test stack underflow detection
#[test]
fn test_stack_underflow() {
    let mut stack = WasmStack::new(16, 32);

    // Pop from empty stack should fail
    let result = stack.pop();
    assert!(matches!(result, Err(WasmSimError::Trap(WasmTrap::StackUnderflow))));
}

/// Test call stack overflow
#[test]
fn test_call_stack_overflow() {
    let mut stack = WasmStack::new(16, 4); // Small shadow stack

    // Push return addresses until overflow
    for i in 0..10 {
        let result = stack.push_return(i as u32);
        if result.is_err() {
            assert!(matches!(result, Err(WasmSimError::Trap(WasmTrap::CallStackOverflow))));
            return;
        }
    }

    // Should have overflowed
    panic!("Expected call stack overflow");
}

/// Test memory creation and bounds checking
#[test]
fn test_memory_bounds() {
    use cognitum_wasm_sim::wasm::WasmMemory;

    // Memory with 0 pages is valid (empty linear memory)
    let result = WasmMemory::new(8192, 8192, 65536, 0, 256);
    assert!(result.is_ok());

    // But accessing it should fail - verify bounds checking works
    let mut mem = result.unwrap();
    let oob_result = mem.load_i32(0, 0); // 0 pages = 0 bytes, so any access is OOB
    assert!(oob_result.is_err());
}

/// Test WASM bytecode loading
#[test]
fn test_bytecode_loading() {
    let mut tile = WasmTile::new(0, WasmConfig::default()).unwrap();

    // Valid bytecode
    let valid = vec![0x01, 0x01, 0x01]; // nop, nop, nop
    assert!(tile.load_bytecode(&valid).is_ok());

    // Empty bytecode
    let empty: Vec<u8> = vec![];
    assert!(tile.load_bytecode(&empty).is_ok());
}

/// Test error display formatting
#[test]
fn test_error_display() {
    let trap = WasmTrap::StackUnderflow;
    let err = WasmSimError::Trap(trap);
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

/// Test stack push/pop operations
#[test]
fn test_stack_operations() {
    let mut stack = WasmStack::new(16, 32);

    // Push some values
    stack.push(100).unwrap();
    stack.push(200).unwrap();
    stack.push(300).unwrap();

    assert_eq!(stack.depth(), 3);

    // Pop and verify LIFO order
    assert_eq!(stack.pop().unwrap(), 300);
    assert_eq!(stack.pop().unwrap(), 200);
    assert_eq!(stack.pop().unwrap(), 100);

    assert!(stack.is_empty());
}

/// Test stack peek operations
#[test]
fn test_stack_peek() {
    let mut stack = WasmStack::new(16, 32);

    stack.push(42).unwrap();

    // Peek should return value without removing
    assert_eq!(stack.peek(), Some(42));
    assert_eq!(stack.peek(), Some(42));
    assert_eq!(stack.depth(), 1);
}

/// Test shadow stack for call returns
#[test]
fn test_shadow_stack() {
    let mut stack = WasmStack::new(16, 32);

    stack.push_return(0x1000).unwrap();
    stack.push_return(0x2000).unwrap();

    assert_eq!(stack.call_depth(), 2);

    assert_eq!(stack.pop_return().unwrap(), 0x2000);
    assert_eq!(stack.pop_return().unwrap(), 0x1000);
}

/// Test stack with spill to memory
#[test]
fn test_stack_spill_to_memory() {
    let mut stack = WasmStack::new(4, 32); // Small register file

    // Push more than register depth
    for i in 0..20i32 {
        stack.push(i).unwrap();
    }

    assert_eq!(stack.depth(), 20);

    // Pop all and verify order
    for i in (0..20i32).rev() {
        assert_eq!(stack.pop().unwrap(), i);
    }

    assert!(stack.is_empty());
}

/// Test simulator creation doesn't panic
#[test]
fn test_simulator_no_panic() {
    let result = WasmSimulator::with_scale(ScaleLevel::Development);
    assert!(result.is_ok());
}

/// Test loading to invalid tile
#[test]
fn test_load_invalid_tile() {
    let mut sim = WasmSimulator::with_scale(ScaleLevel::Small).unwrap();

    // Try to load to non-existent tile
    let result = sim.load_wasm(999, &[0x01]);
    assert!(result.is_err());
}

/// Test stack clear
#[test]
fn test_stack_clear() {
    let mut stack = WasmStack::new(16, 32);

    stack.push(1).unwrap();
    stack.push(2).unwrap();
    stack.push_return(0x1000).unwrap();

    stack.clear();

    assert!(stack.is_empty());
    assert_eq!(stack.call_depth(), 0);
}

/// Test stack dup operation
#[test]
fn test_stack_dup() {
    let mut stack = WasmStack::new(16, 32);

    stack.push(42).unwrap();
    stack.dup().unwrap();

    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.pop().unwrap(), 42);
    assert_eq!(stack.pop().unwrap(), 42);
}

/// Test stack swap operation
#[test]
fn test_stack_swap() {
    let mut stack = WasmStack::new(16, 32);

    stack.push(1).unwrap();
    stack.push(2).unwrap();
    stack.swap().unwrap();

    assert_eq!(stack.pop().unwrap(), 1);
    assert_eq!(stack.pop().unwrap(), 2);
}
