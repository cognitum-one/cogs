# Cognitum Processor - A2S v2r3 CPU Implementation

## Overview

This crate implements the **A2S v2r3 processor core** - a zero-address stack machine with a rich instruction set architecture. The implementation follows TDD (Test-Driven Development) London School methodology with comprehensive test coverage.

## Architecture

### Zero-Address Stack Machine

The A2S v2r3 is a stack-based processor with:
- **Data Stack**: Main operand stack for computations
- **Return Stack**: For subroutine calls and temporary storage
- **Three Address Registers**: A, B, C for memory operations
- **Program Counter**: 32-bit instruction pointer

### ISA Characteristics

- **64 base instructions** (6-bit opcodes)
- **4096+ extended functions** (16-bit encoding)
- **VLIW-style bundling**: Up to 4 instructions per 16-bit word
- **Rich arithmetic**: Integer multiply/divide, FPU operations
- **Efficient memory access**: Auto-increment addressing modes

## Instruction Categories

### Stack Operations
```rust
DUP     // ( x -- x x )
SWAP    // ( x1 x2 -- x2 x1 )
OVER    // ( x1 x2 -- x1 x2 x1 )
ROT3    // ( x1 x2 x3 -- x2 x3 x1 )
DROP    // ( x1 x2 -- x1 )
```

### Arithmetic
```rust
ADD     // ( n1 n2 -- sum )
SUB     // ( n1 n2 -- diff )
MUL     // ( n1 n2 -- product )
DIV     // ( n1 n2 -- quotient )
```

### Logic & Bitwise
```rust
AND     // ( x1 x2 -- x3 )
OR      // ( x1 x2 -- x3 )
XOR     // ( x1 x2 -- x3 )
NOT     // ( x -- ~x )
```

### Comparison
```rust
=       // ( x1 x2 -- flag )
<       // ( n1 n2 -- flag )
u<      // ( u1 u2 -- flag )  // Unsigned
```

### Memory Operations
```rust
@       // ( addr -- x )      // Load
!       // ( x addr -- )      // Store
@a      // ( -- x ) A: addr   // Load via register A
!a      // ( x -- ) A: addr   // Store via register A
@a+     // Auto-increment after load
```

### Control Flow
```rust
CALL    // Call subroutine
RTN     // Return
JMP     // Unconditional jump
JZ      // Jump if zero
JN      // Jump if negative
```

## Implementation Features

### Test-Driven Development

The implementation follows TDD London School:
1. **Write tests first** (RED phase)
2. **Implement minimal code** (GREEN phase)
3. **Refactor for quality** (REFACTOR phase)

### Test Coverage

- **9 unit tests** for stack and memory primitives
- **12 arithmetic tests** including edge cases
- **7 bitwise operation tests**
- **10 memory operation tests**
- **10 control flow tests**
- **11 integration tests** with real programs

### Instruction Encoding Verification

All instruction opcodes match the Verilog reference implementation in `/workspaces/cognitum/src/A2S_v2r3/A2Sv2r3_ISA.v`:

```verilog
PUTA  = 6'b 00_0000,  // Matches Rust Opcode::PUTA
GETB  = 6'b 00_0101,  // Matches Opcode::GETB
ADD   = 6'b 10_1000,  // Matches Opcode::ADD
// ... etc
```

## Usage Examples

### Simple Arithmetic

```rust
use newport_processor::{A2SProcessor, Instruction};

let mut cpu = A2SProcessor::with_default_memory();

let program = vec![
    Instruction::Push(10),
    Instruction::Push(20),
    Instruction::Add,
    Instruction::Halt,
];

cpu.run(&program).unwrap();
assert_eq!(cpu.peek_stack().unwrap(), 30);
```

### Memory Operations

```rust
let program = vec![
    Instruction::Push(42),
    Instruction::Push(0x1000),
    Instruction::Store,           // [0x1000] = 42
    Instruction::Push(0x1000),
    Instruction::Load,            // Load from 0x1000
    Instruction::Halt,
];

cpu.run(&program).unwrap();
assert_eq!(cpu.peek_stack().unwrap(), 42);
```

### Using Address Registers

```rust
let program = vec![
    Instruction::Push(0x1000),
    Instruction::ToA,             // A = 0x1000
    Instruction::Push(99),
    Instruction::StoreA,          // [A] = 99
    Instruction::LoadA,           // Load from [A]
    Instruction::Halt,
];
```

### Complex Expression

```rust
// Compute: (5 + 3) * (10 - 2) = 64
let program = vec![
    Instruction::Push(5),
    Instruction::Push(3),
    Instruction::Add,             // 8
    Instruction::Push(10),
    Instruction::Push(2),
    Instruction::Sub,             // 8
    Instruction::Multiply,        // 64
    Instruction::Halt,
];
```

## Testing

Run all tests:
```bash
cargo test
```

Run specific test suites:
```bash
cargo test --test stack_operations
cargo test --test arithmetic_operations
cargo test --test memory_operations
cargo test --test control_flow
cargo test --test programs
```

## Error Handling

The processor uses Rust's type-safe error handling:

```rust
pub enum ProcessorError {
    StackUnderflow,
    StackOverflow,
    InvalidMemoryAddress(u32),
    InvalidOpcode(u8),
    DivisionByZero,
    ReturnStackUnderflow,
    ReturnStackOverflow,
}
```

## Performance Characteristics

- **Zero-copy stack operations**: Direct manipulation without allocation
- **Sparse memory**: HashMap-based for efficient memory usage
- **Wrapping arithmetic**: Matches hardware behavior
- **Aligned memory access**: 4-byte alignment enforced

## Future Enhancements

### Phase 2 - Extended Instructions
- [ ] Shift operations (ROL, ROR, LSL, LSR, ASR)
- [ ] Multiply/Divide extended (signed/unsigned variants)
- [ ] Population count and bit manipulation

### Phase 3 - Floating Point
- [ ] Single precision FPU (fused multiply-add, etc.)
- [ ] Double precision FPU
- [ ] Rounding mode support

### Phase 4 - I/O and Extensions
- [ ] I/O operations (read/write/swap)
- [ ] Spill/fill stack operations
- [ ] Condition code testing

## References

- [ISA Reference](../../../../docs/modules/a2s-processor/ISA_REFERENCE.md)
- [Verilog Implementation](../../../../src/A2S_v2r3/A2Sv2r3_ISA.v)
- [Architecture Documentation](../../../../docs/architecture/)

## License

MIT OR Apache-2.0

## Authors

Implementation: rUv.io, TekStart
Architecture: Advanced Architectures © 2023
