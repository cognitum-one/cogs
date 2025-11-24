# A2S v2r3 ISA Opcode Mapping
## Verilog to Rust Cross-Reference

**Generated**: 2025-11-24
**Purpose**: Validate instruction set implementation between Verilog HDL and Rust simulator

---

## Summary

| Category | Verilog | Rust | Status |
|----------|---------|------|--------|
| **Base Opcodes (6-bit)** | 64 | 64 | ✅ COMPLETE |
| **I/O Operations** | 4 | ❌ | ⚠️ MISSING |
| **Shift Immediate** | 5 | ❌ | ⚠️ MISSING |
| **Condition Codes** | 1 | ❌ | ⚠️ MISSING |
| **Spill/Fill** | 4 | ❌ | ⚠️ MISSING |
| **Population Count** | 4 | ❌ | ⚠️ MISSING |
| **Byte Swap** | 1 | ❌ | ⚠️ MISSING |
| **Shift Relative** | 5 | ❌ | ⚠️ MISSING |
| **Multiply (64 variants)** | 12 | ❌ | ⚠️ MISSING |
| **Divide (64 variants)** | 12 | ❌ | ⚠️ MISSING |
| **Float Single (256 codes)** | 32+ | ❌ | ⚠️ MISSING |
| **Float Double (256 codes)** | 32+ | ❌ | ⚠️ MISSING |
| **TOTAL** | 4,096+ | 64 | **98.4% MISSING** |

---

## 1. Base Opcodes (6-bit) - COMPLETE ✅

### Memory Operations (0-15)

| Opcode | Binary | Verilog | Rust | Status | Description |
|--------|--------|---------|------|--------|-------------|
| 0 | `00_0000` | `PUTA` | `Opcode::PUTA` | ✅ | Store to address in A |
| 1 | `00_0001` | `PUTB` | `Opcode::PUTB` | ✅ | Store to address in B |
| 2 | `00_0010` | `PUTC` | `Opcode::PUTC` | ✅ | Store to address in C |
| 3 | `00_0011` | `PUT` | `Opcode::PUT` | ✅ | Store ( x a-addr -- ) |
| 4 | `00_0100` | `GETA` | `Opcode::GETA` | ✅ | Load from address in A |
| 5 | `00_0101` | `GETB` | `Opcode::GETB` | ✅ | Load from address in B |
| 6 | `00_0110` | `GETC` | `Opcode::GETC` | ✅ | Load from address in C |
| 7 | `00_0111` | `GET` | `Opcode::GET` | ✅ | Load ( a-addr -- x ) |
| 8 | `00_1000` | `PTAP` | `Opcode::PTAP` | ✅ | Store and increment A |
| 9 | `00_1001` | `PTBP` | `Opcode::PTBP` | ✅ | Store and increment B |
| 10 | `00_1010` | `PTCP` | `Opcode::PTCP` | ✅ | Store and increment C |
| 11 | `00_1011` | `XCHG` | `Opcode::XCHG` | ✅ | Atomic exchange |
| 12 | `00_1100` | `GTAP` | `Opcode::GTAP` | ✅ | Load and increment A |
| 13 | `00_1101` | `GTBP` | `Opcode::GTBP` | ✅ | Load and increment B |
| 14 | `00_1110` | `GTCP` | `Opcode::GTCP` | ✅ | Load and increment C |
| 15 | `00_1111` | `res1` | ❌ | ⚠️ | Reserved |

### Register Transfer (16-31)

| Opcode | Binary | Verilog | Rust | Status | Description |
|--------|--------|---------|------|--------|-------------|
| 16 | `01_0000` | `DECA` | `Opcode::DECA` | ✅ | Decrement A register |
| 17 | `01_0001` | `DECB` | `Opcode::DECB` | ✅ | Decrement B register |
| 18 | `01_0010` | `DECC` | `Opcode::DECC` | ✅ | Decrement C register |
| 19 | `01_0011` | `SNB` | `Opcode::SNB` | ✅ | Skip to next bundle |
| 20 | `01_0100` | `SEL` | `Opcode::SEL` | ✅ | Conditional select ?: |
| 21-22 | `01_0101-110` | `res2-4` | ❌ | ⚠️ | Reserved |
| 23 | `01_0111` | `NOT` | `Opcode::NOT` | ✅ | Bitwise NOT |
| 24 | `01_1000` | `T2A` | `Opcode::T2A` | ✅ | Top to A register |
| 25 | `01_1001` | `T2B` | `Opcode::T2B` | ✅ | Top to B register |
| 26 | `01_1010` | `T2C` | `Opcode::T2C` | ✅ | Top to C register |
| 27 | `01_1011` | `T2R` | `Opcode::T2R` | ✅ | Top to R (return stack) |
| 28 | `01_1100` | `A2T` | `Opcode::A2T` | ✅ | A to top |
| 29 | `01_1101` | `B2T` | `Opcode::B2T` | ✅ | B to top |
| 30 | `01_1110` | `C2T` | `Opcode::C2T` | ✅ | C to top |
| 31 | `01_1111` | `R2T` | `Opcode::R2T` | ✅ | R to top |

### Stack Manipulation (32-39)

| Opcode | Binary | Verilog | Rust | Status | Description |
|--------|--------|---------|------|--------|-------------|
| 32 | `10_0000` | `NIP` | `Opcode::NIP` | ✅ | ( x1 x2 -- x2 ) |
| 33 | `10_0001` | `DROP` | `Opcode::DROP` | ✅ | ( x1 x2 -- x1 ) |
| 34 | `10_0010` | `DUP` | `Opcode::DUP` | ✅ | ( x -- x x ) |
| 35 | `10_0011` | `OVER` | `Opcode::OVER` | ✅ | ( x1 x2 -- x1 x2 x1 ) |
| 36 | `10_0100` | `SWAP` | `Opcode::SWAP` | ✅ | ( x1 x2 -- x2 x1 ) |
| 37 | `10_0101` | `ROT3` | `Opcode::ROT3` | ✅ | ( x1 x2 x3 -- x2 x3 x1 ) |
| 38 | `10_0110` | `ROT4` | `Opcode::ROT4` | ✅ | ( x1 x2 x3 x4 -- x2 x3 x4 x1) |
| 39 | `10_0111` | ❌ | ❌ | ⚠️ | Reserved |

### Arithmetic & Logic (40-47)

| Opcode | Binary | Verilog | Rust | Status | Description |
|--------|--------|---------|------|--------|-------------|
| 40 | `10_1000` | `ADD` | `Opcode::ADD` | ✅ | Addition |
| 41 | `10_1001` | `SUB` | `Opcode::SUB` | ✅ | Subtraction |
| 42 | `10_1010` | `SLT` | `Opcode::SLT` | ✅ | Signed less than |
| 43 | `10_1011` | `ULT` | `Opcode::ULT` | ✅ | Unsigned less than |
| 44 | `10_1100` | `EQ` | `Opcode::EQ` | ✅ | Equality test |
| 45 | `10_1101` | `XOR` | `Opcode::XOR` | ✅ | Bitwise XOR |
| 46 | `10_1110` | `IOR` | `Opcode::IOR` | ✅ | Bitwise OR |
| 47 | `10_1111` | `AND` | `Opcode::AND` | ✅ | Bitwise AND |

### Constants & Control (48-63)

| Opcode | Binary | Verilog | Rust | Status | Description |
|--------|--------|---------|------|--------|-------------|
| 48 | `11_0000` | `ZERO` | `Opcode::ZERO` | ✅ | Push 0 |
| 49 | `11_0001` | `ONE` | `Opcode::ONE` | ✅ | Push 1 |
| 50 | `11_0010` | `CO` | `Opcode::CO` | ✅ | Coroutine call |
| 51 | `11_0011` | `RTN` | `Opcode::RTN` | ✅ | Return |
| 52 | `11_0100` | `NOP` | `Opcode::NOP` | ✅ | No operation |
| 53 | `11_0101` | `PFX` | `Opcode::PFX` | ✅ | Prefix (extend literal) |
| 54 | `11_0110` | `LIT` | `Opcode::LIT` | ✅ | Literal value |
| 55 | `11_0111` | `EXT` | `Opcode::EXT` | ✅ | Extended function |
| 56 | `11_1000` | `res5` | ❌ | ⚠️ | Reserved |
| 57 | `11_1001` | `CALL` | `Opcode::CALL` | ✅ | Function call |
| 58 | `11_1010` | `JN` | `Opcode::JN` | ✅ | Jump if negative |
| 59 | `11_1011` | `JZ` | `Opcode::JZ` | ✅ | Jump if zero |
| 60 | `11_1100` | `JANZ` | `Opcode::JANZ` | ✅ | Jump if A non-zero |
| 61 | `11_1101` | `JBNZ` | `Opcode::JBNZ` | ✅ | Jump if B non-zero |
| 62 | `11_1110` | `JCNZ` | `Opcode::JCNZ` | ✅ | Jump if C non-zero |
| 63 | `11_1111` | `JMP` | `Opcode::JMP` | ✅ | Unconditional jump |

---

## 2. Extended Instructions (16-bit) - MISSING ⚠️

### I/O Operations (0x0000-0x7FFF)

**Verilog**: `A2Sv2r3_ISA.v` lines 108-121
**Rust**: ❌ NOT IMPLEMENTED

| Pattern | Opcode | Pop/Push | Description |
|---------|--------|----------|-------------|
| `000?_????_????_????` | `IORC` | p0p1 | Read and clear I/O register |
| `001?_????_????_????` | `IORD` | p0p1 | Read I/O register |
| `010?_????_????_????` | `IOWR` | p1p0 | Write I/O register |
| `011?_????_????_????` | `IOSW` | p1p1 | Atomic swap I/O register |

**Encoding**: 4 command codes + 13-bit I/O address = **8,192 I/O instructions**

### Shift Immediate (0x8000-0x80FF)

**Verilog**: `A2Sv2r3_ISA.v` lines 126-131
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `RORI` | `0x8000 + imm8` | Rotate right immediate |
| `ROLI` | `0x8100 + imm8` | Rotate left immediate |
| `LSRI` | `0x8200 + imm8` | Logical shift right immediate |
| `LSLI` | `0x8300 + imm8` | Logical shift left immediate |
| `ASRI` | `0x8400 + imm8` | Arithmetic shift right immediate |

**Total**: 5 × 256 = **1,280 instructions**

### Condition Code (0x8800-0x88FF)

**Verilog**: `A2Sv2r3_ISA.v` line 137
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `CC` | `0x8800 + imm8` | Test condition code flag |

**Total**: **256 instructions**

### Spill and Fill (0xFB60-0xFB63)

**Verilog**: `A2Sv2r3_ISA.v` lines 148-151
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `SPILR` | `0xFB60` | Spill R-stack to data memory |
| `SPILD` | `0xFB61` | Spill D-stack to data memory |
| `FILLR` | `0xFB62` | Fill R-stack from data memory |
| `FILLD` | `0xFB63` | Fill D-stack from data memory |

### Population Count (0xFB70-0xFB73)

**Verilog**: `A2Sv2r3_ISA.v` lines 156-159
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `POPC` | `0xFB70` | Population count (number of 1 bits) |
| `EXCS` | `0xFB71` | Excess population count |
| `CLZ` | `0xFB72` | Count leading zeros |
| `CTZ` | `0xFB73` | Count trailing zeros |

### Byte Swap (0xFB77)

**Verilog**: `A2Sv2r3_ISA.v` line 163
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `BSWP` | `0xFB77` | Byte swap (endian conversion) |

### Shift Relative (0xFB78-0xFB7C)

**Verilog**: `A2Sv2r3_ISA.v` lines 169-173
**Rust**: ❌ NOT IMPLEMENTED

| Opcode | Hex | Description |
|--------|-----|-------------|
| `ROR` | `0xFB78` | Rotate right (by TOS) |
| `ROL` | `0xFB79` | Rotate left (by TOS) |
| `LSR` | `0xFB7A` | Logical shift right |
| `LSL` | `0xFB7B` | Logical shift left |
| `ASR` | `0xFB7C` | Arithmetic shift right |

### Multiply Operations (0xFB80-0xFBBF)

**Verilog**: `A2Sv2r3_ISA.v` lines 178-201
**Rust**: ❌ NOT IMPLEMENTED

**64 multiply codes, 12 implemented:**

| Opcode | Hex | Signedness | Result | Description |
|--------|-----|------------|--------|-------------|
| `MPYuu` | `0xFB80` | unsigned × unsigned | double | Full product |
| `MPYus` | `0xFB81` | unsigned × signed | double | Full product |
| `MPYsu` | `0xFB82` | signed × unsigned | double | Full product |
| `MPYss` | `0xFB83` | signed × signed | double | Full product (default) |
| `MPHuu` | `0xFB84` | unsigned × unsigned | upper | High word only |
| `MPHus` | `0xFB85` | unsigned × signed | upper | High word only |
| `MPHsu` | `0xFB86` | signed × unsigned | upper | High word only |
| `MPHss` | `0xFB87` | signed × signed | upper | High word (default) |
| `MPLuu` | `0xFB88` | unsigned × unsigned | lower | Low word only |
| `MPLus` | `0xFB89` | unsigned × signed | lower | Low word only |
| `MPLsu` | `0xFB8A` | signed × unsigned | lower | Low word only |
| `MPLss` | `0xFB8B` | signed × signed | lower | Low word (default) |

### Divide Operations (0xFBC0-0xFBFF)

**Verilog**: `A2Sv2r3_ISA.v` lines 206-226
**Rust**: ❌ NOT IMPLEMENTED

**64 divide codes, 12 implemented:**

| Opcode | Hex | Signedness | Result | Description |
|--------|-----|------------|--------|-------------|
| `DIVuu` | `0xFBC0` | unsigned / unsigned | both | Quotient + remainder |
| `DIVus` | `0xFBC1` | unsigned / signed | both | Quotient + remainder |
| `DIVsu` | `0xFBC2` | signed / unsigned | both | Quotient + remainder |
| `DIVss` | `0xFBC3` | signed / signed | both | Quotient + remainder (default) |
| `MODuu` | `0xFBC4` | unsigned / unsigned | remainder | Remainder only |
| `MODus` | `0xFBC5` | unsigned / signed | remainder | Remainder only |
| `MODsu` | `0xFBC6` | signed / unsigned | remainder | Remainder only |
| `MODss` | `0xFBC7` | signed / signed | remainder | Remainder only (default) |
| `QUOuu` | `0xFBC8` | unsigned / unsigned | quotient | Quotient only |
| `QUOus` | `0xFBC9` | unsigned / signed | quotient | Quotient only |
| `QUOsu` | `0xFBCA` | signed / unsigned | quotient | Quotient only |
| `QUOss` | `0xFBCB` | signed / signed | quotient | Quotient only (default) |

### Floating Point - Single Precision (0xFD00-0xFDFF)

**Verilog**: `A2Sv2r3_ISA.v` lines 236-284
**Rust**: ❌ NOT IMPLEMENTED

**256 single-precision codes, 32+ implemented:**

#### Fused Multiply-Add (0xFD00-0xFD1F)

| Opcode | Hex | Description |
|--------|-----|-------------|
| `FMAD` | `0xFD00 + rm` | f1 × f2 + f3 |
| `FMSB` | `0xFD08 + rm` | f1 × f2 - f3 |
| `FMNA` | `0xFD10 + rm` | -f1 × f2 + f3 |
| `FMNS` | `0xFD18 + rm` | -f1 × f2 - f3 |

#### Arithmetic (0xFD20-0xFD5F)

| Opcode | Hex | Description |
|--------|-----|-------------|
| `FMUL` | `0xFD20 + rm` | Multiply |
| `FADD` | `0xFD28 + rm` | Add |
| `FSUB` | `0xFD30 + rm` | Subtract |
| `FRSB` | `0xFD38 + rm` | Reverse subtract |
| `FDIV` | `0xFD40 + rm` | Divide |
| `FREM` | `0xFD48 + rm` | Remainder (IEEE) |
| `FMOD` | `0xFD50 + rm` | Modulo |
| `FSQR` | `0xFD58 + rm` | Square root |

#### Conversion (0xFD60-0xFD9F)

| Opcode | Hex | Description |
|--------|-----|-------------|
| `FF2S` | `0xFD60 + rm` | Float to signed int |
| `FS2F` | `0xFD68 + rm` | Signed int to float |
| `FF2U` | `0xFD70 + rm` | Float to unsigned int |
| `FU2F` | `0xFD78 + rm` | Unsigned int to float |

#### Comparison (0xFD80-0xFD8F)

| Opcode | Hex | Description |
|--------|-----|-------------|
| `FCLT` | `0xFD81` | Less than |
| `FCEQ` | `0xFD82` | Equal |
| `FCLE` | `0xFD83` | Less or equal |
| `FCGT` | `0xFD84` | Greater than |
| `FCNE` | `0xFD85` | Not equal |
| `FCGE` | `0xFD86` | Greater or equal |
| `FMAX` | `0xFD88` | Maximum |
| `FMIN` | `0xFD89` | Minimum |
| `FSAT` | `0xFD8A` | Saturate |

#### Unary (0xFD8C-0xFD8E)

| Opcode | Hex | Description |
|--------|-----|-------------|
| `FNAN` | `0xFD8C` | Replace NaN with 0.0 |
| `FABS` | `0xFD8D` | Absolute value |
| `FCHS` | `0xFD8E` | Change sign (negate) |

### Floating Point - Double Precision (0xFE00-0xFEFF)

**Verilog**: `A2Sv2r3_ISA.v` lines 288-328
**Rust**: ❌ NOT IMPLEMENTED

**256 double-precision codes (similar structure to single-precision):**

- Fused multiply-add: `DMAD`, `DMSB`, `DMNA`, `DMNS`
- Arithmetic: `DMUL`, `DADD`, `DSUB`, `DRSB`, `DDIV`, `DREM`, `DMOD`, `DSQR`
- Conversion: `DF2S`, `DS2F`, `DF2U`, `DU2F`, `DF2F` (double→single), `FF2D` (single→double)
- Comparison: `DCLT`, `DCEQ`, `DCLE`, `DCGT`, `DCNE`, `DCGE`, `DMAX`, `DMIN`, `DSAT`, `DSEL`
- Unary: `DNAN`, `DABS`, `DCHS`

---

## 3. Implementation Recommendations

### Priority 1: Critical Missing Features

1. **Integer Multiply/Divide** (`0xFB80-0xFBFF`)
   - Required for cryptography and general computation
   - 12 core operations (signed/unsigned variants)
   - Estimated effort: 2-3 days

2. **Shift Operations** (`0x8000-0x80FF`, `0xFB78-0xFB7C`)
   - Essential for bit manipulation
   - Used in crypto algorithms
   - Estimated effort: 1 day

3. **I/O Operations** (`0x0000-0x7FFF`)
   - Required for hardware interaction
   - 4 core operations × 8,192 addresses
   - Estimated effort: 2 days

### Priority 2: Numeric Computing

4. **Single-Precision Float** (`0xFD00-0xFDFF`)
   - 32+ operations for scientific computing
   - IEEE 754 compliance required
   - Estimated effort: 5-7 days

5. **Double-Precision Float** (`0xFE00-0xFEFF`)
   - 32+ operations (similar to single)
   - Estimated effort: 3-5 days (after single-precision)

### Priority 3: Utility Functions

6. **Population Count** (`0xFB70-0xFB73`)
   - Useful for cryptography and bit manipulation
   - Estimated effort: 1 day

7. **Spill/Fill** (`0xFB60-0xFB63`)
   - Stack overflow handling
   - Estimated effort: 1 day

### Priority 4: Advanced Features

8. **Condition Codes** (`0x8800-0x88FF`)
   - Status flag testing
   - Estimated effort: 1 day

---

## 4. Divergence Analysis

### Coverage Statistics

- **Base opcodes**: 64/64 = **100% coverage** ✅
- **Extended instructions**: 0/4,032+ = **0% coverage** ❌
- **Total instruction set**: 64/4,096+ = **1.6% coverage** ❌

### Critical Gaps

1. **No integer multiplication** - Cannot implement cryptography efficiently
2. **No floating-point** - Scientific computing impossible
3. **No I/O operations** - Cannot interact with hardware peripherals
4. **No bit manipulation** - Cryptographic algorithms severely limited

### Impact Assessment

| Feature Missing | Impact | Workaround | Performance Loss |
|----------------|--------|------------|------------------|
| Integer multiply | HIGH | Software loops | 100-1000× slower |
| Integer divide | HIGH | Software loops | 100-1000× slower |
| Floating-point | MEDIUM | Fixed-point | 10-100× slower |
| I/O operations | HIGH | None | BLOCKED |
| Shift operations | MEDIUM | Repeated add/sub | 10-50× slower |

---

## 5. Validation Test Cases

### Recommended Test Suite

```rust
#[test]
fn test_multiply_signed_signed() {
    // MPYss: (-5) × 7 = -35
    let result = cpu.execute(Instruction::Multiply(MPYss));
    assert_eq!(result, (-35, 0)); // lower, upper
}

#[test]
fn test_float_multiply_add() {
    // FMAD: 2.0 × 3.0 + 4.0 = 10.0
    let result = cpu.execute(Instruction::FloatMAD);
    assert_eq!(result, 10.0);
}

#[test]
fn test_io_read() {
    // IORD: Read from I/O address 0x1234
    let result = cpu.execute(Instruction::IORead(0x1234));
    assert!(result.is_ok());
}
```

---

## 6. References

- **Verilog Source**: `/home/user/newport/src/A2S_v2r3/A2Sv2r3_ISA.v`
- **Rust Implementation**: `/home/user/newport/newport-sim/crates/newport-processor/src/instruction.rs`
- **ISA Documentation**: `/home/user/newport/docs/modules/a2s-processor/ISA_REFERENCE.md`

---

## Conclusion

The Rust implementation has **complete coverage of the 64 base opcodes** but is **missing 98.4% of the extended instruction set**. To achieve <0.1% divergence, the following must be implemented:

1. Integer multiply/divide (24 instructions)
2. Floating-point operations (64+ instructions)
3. I/O operations (4 core patterns)
4. Shift operations (10 instructions)
5. Utility functions (9 instructions)

**Estimated total effort**: 15-25 days of development work.

**Recommendation**: Prioritize integer multiply/divide and I/O operations first, as these are critical for basic functionality and hardware interaction.
