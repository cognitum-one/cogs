# Cognitum Core

Core types and memory system for Cognitum ASIC simulator.

## Features

- **Type Safety**: Strongly-typed wrappers for TileId, MemoryAddress, Register, and Instruction
- **Memory System**: Trait-based memory abstraction with bounds checking
- **Error Handling**: Comprehensive error types with detailed context
- **100% Test Coverage**: Complete test suite using TDD London School approach

## Types

### TileId
- Range: 0-255
- Validated construction
- Serializable

### MemoryAddress
- 32-bit addresses
- Alignment checking and manipulation
- Address arithmetic with wrapping

### Register
- 32-bit general-purpose registers
- Mutable value access

### Instruction
- 16-bit instruction encoding
- Opcode, register, and immediate field extraction

## Memory System

### Memory Trait
Generic memory interface supporting:
- Read/write operations
- Bounds checking
- Base address and size queries

### RAM Implementation
- Configurable base address and size
- Automatic alignment validation (4-byte)
- Comprehensive bounds checking
- Zero initialization

## Usage

```rust
use newport_core::{MemoryAddress, RAM, Memory};

// Create RAM at 0x1000 with 1KB capacity
let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);

// Write data
ram.write(MemoryAddress::new(0x1000), 0xDEADBEEF)?;

// Read data
let value = ram.read(MemoryAddress::new(0x1000))?;
```

## Error Handling

```rust
use cognitum_core::{TileId, CognitumError};

match TileId::new(256) {
    Ok(id) => println!("Valid tile: {}", id.value()),
    Err(CognitumError::InvalidTileId(id)) => {
        eprintln!("Invalid tile ID: {}", id);
    }
    _ => unreachable!(),
}
```

## Development

Built using TDD London School methodology:
- Tests written first (Red phase)
- Minimal implementation (Green phase)
- Refactoring for quality (Refactor phase)

Run tests:
```bash
cargo test
```

Check coverage:
```bash
cargo tarpaulin --out Html
```
