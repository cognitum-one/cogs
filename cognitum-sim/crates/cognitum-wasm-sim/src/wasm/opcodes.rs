//! WASM opcode definitions
//!
//! Complete WASM MVP opcodes plus SIMD and neural extensions

/// Operation type categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpType {
    /// Control flow operations
    Control = 0,

    /// Parametric operations (drop, select)
    Parametric = 1,

    /// Variable operations (local, global)
    Variable = 2,

    /// Memory operations (load, store)
    Memory = 3,

    /// i32 numeric operations
    I32 = 4,

    /// i64 numeric operations
    I64 = 5,

    /// f32 numeric operations
    F32 = 6,

    /// f64 numeric operations
    F64 = 7,

    /// Type conversion operations
    Convert = 8,

    /// SIMD v128 operations
    Simd = 9,

    /// Custom neural operations
    Neural = 10,

    /// Extended operations (multi-byte)
    Extended = 11,
}

/// WASM opcodes (MVP + extensions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // ===== Control Flow (0x00-0x11) =====
    Unreachable = 0x00,
    Nop = 0x01,
    Block = 0x02,
    Loop = 0x03,
    If = 0x04,
    Else = 0x05,
    End = 0x0B,
    Br = 0x0C,
    BrIf = 0x0D,
    BrTable = 0x0E,
    Return = 0x0F,
    Call = 0x10,
    CallIndirect = 0x11,

    // ===== Parametric (0x1A-0x1B) =====
    Drop = 0x1A,
    Select = 0x1B,

    // ===== Variable Access (0x20-0x24) =====
    LocalGet = 0x20,
    LocalSet = 0x21,
    LocalTee = 0x22,
    GlobalGet = 0x23,
    GlobalSet = 0x24,

    // ===== Memory Operations (0x28-0x40) =====
    I32Load = 0x28,
    I64Load = 0x29,
    F32Load = 0x2A,
    F64Load = 0x2B,
    I32Load8S = 0x2C,
    I32Load8U = 0x2D,
    I32Load16S = 0x2E,
    I32Load16U = 0x2F,
    I64Load8S = 0x30,
    I64Load8U = 0x31,
    I64Load16S = 0x32,
    I64Load16U = 0x33,
    I64Load32S = 0x34,
    I64Load32U = 0x35,
    I32Store = 0x36,
    I64Store = 0x37,
    F32Store = 0x38,
    F64Store = 0x39,
    I32Store8 = 0x3A,
    I32Store16 = 0x3B,
    I64Store8 = 0x3C,
    I64Store16 = 0x3D,
    I64Store32 = 0x3E,
    MemorySize = 0x3F,
    MemoryGrow = 0x40,

    // ===== i32 Constants (0x41) =====
    I32Const = 0x41,
    I64Const = 0x42,
    F32Const = 0x43,
    F64Const = 0x44,

    // ===== i32 Comparison (0x45-0x4F) =====
    I32Eqz = 0x45,
    I32Eq = 0x46,
    I32Ne = 0x47,
    I32LtS = 0x48,
    I32LtU = 0x49,
    I32GtS = 0x4A,
    I32GtU = 0x4B,
    I32LeS = 0x4C,
    I32LeU = 0x4D,
    I32GeS = 0x4E,
    I32GeU = 0x4F,

    // ===== i64 Comparison (0x50-0x5A) =====
    I64Eqz = 0x50,
    I64Eq = 0x51,
    I64Ne = 0x52,
    I64LtS = 0x53,
    I64LtU = 0x54,
    I64GtS = 0x55,
    I64GtU = 0x56,
    I64LeS = 0x57,
    I64LeU = 0x58,
    I64GeS = 0x59,
    I64GeU = 0x5A,

    // ===== f32 Comparison (0x5B-0x60) =====
    F32Eq = 0x5B,
    F32Ne = 0x5C,
    F32Lt = 0x5D,
    F32Gt = 0x5E,
    F32Le = 0x5F,
    F32Ge = 0x60,

    // ===== f64 Comparison (0x61-0x66) =====
    F64Eq = 0x61,
    F64Ne = 0x62,
    F64Lt = 0x63,
    F64Gt = 0x64,
    F64Le = 0x65,
    F64Ge = 0x66,

    // ===== i32 Arithmetic (0x67-0x78) =====
    I32Clz = 0x67,
    I32Ctz = 0x68,
    I32Popcnt = 0x69,
    I32Add = 0x6A,
    I32Sub = 0x6B,
    I32Mul = 0x6C,
    I32DivS = 0x6D,
    I32DivU = 0x6E,
    I32RemS = 0x6F,
    I32RemU = 0x70,
    I32And = 0x71,
    I32Or = 0x72,
    I32Xor = 0x73,
    I32Shl = 0x74,
    I32ShrS = 0x75,
    I32ShrU = 0x76,
    I32Rotl = 0x77,
    I32Rotr = 0x78,

    // ===== i64 Arithmetic (0x79-0x8A) =====
    I64Clz = 0x79,
    I64Ctz = 0x7A,
    I64Popcnt = 0x7B,
    I64Add = 0x7C,
    I64Sub = 0x7D,
    I64Mul = 0x7E,
    I64DivS = 0x7F,
    I64DivU = 0x80,
    I64RemS = 0x81,
    I64RemU = 0x82,
    I64And = 0x83,
    I64Or = 0x84,
    I64Xor = 0x85,
    I64Shl = 0x86,
    I64ShrS = 0x87,
    I64ShrU = 0x88,
    I64Rotl = 0x89,
    I64Rotr = 0x8A,

    // ===== f32 Arithmetic (0x8B-0x98) =====
    F32Abs = 0x8B,
    F32Neg = 0x8C,
    F32Ceil = 0x8D,
    F32Floor = 0x8E,
    F32Trunc = 0x8F,
    F32Nearest = 0x90,
    F32Sqrt = 0x91,
    F32Add = 0x92,
    F32Sub = 0x93,
    F32Mul = 0x94,
    F32Div = 0x95,
    F32Min = 0x96,
    F32Max = 0x97,
    F32Copysign = 0x98,

    // ===== f64 Arithmetic (0x99-0xA6) =====
    F64Abs = 0x99,
    F64Neg = 0x9A,
    F64Ceil = 0x9B,
    F64Floor = 0x9C,
    F64Trunc = 0x9D,
    F64Nearest = 0x9E,
    F64Sqrt = 0x9F,
    F64Add = 0xA0,
    F64Sub = 0xA1,
    F64Mul = 0xA2,
    F64Div = 0xA3,
    F64Min = 0xA4,
    F64Max = 0xA5,
    F64Copysign = 0xA6,

    // ===== Conversions (0xA7-0xBF) =====
    I32WrapI64 = 0xA7,
    I32TruncF32S = 0xA8,
    I32TruncF32U = 0xA9,
    I32TruncF64S = 0xAA,
    I32TruncF64U = 0xAB,
    I64ExtendI32S = 0xAC,
    I64ExtendI32U = 0xAD,
    I64TruncF32S = 0xAE,
    I64TruncF32U = 0xAF,
    I64TruncF64S = 0xB0,
    I64TruncF64U = 0xB1,
    F32ConvertI32S = 0xB2,
    F32ConvertI32U = 0xB3,
    F32ConvertI64S = 0xB4,
    F32ConvertI64U = 0xB5,
    F32DemoteF64 = 0xB6,
    F64ConvertI32S = 0xB7,
    F64ConvertI32U = 0xB8,
    F64ConvertI64S = 0xB9,
    F64ConvertI64U = 0xBA,
    F64PromoteF32 = 0xBB,
    I32ReinterpretF32 = 0xBC,
    I64ReinterpretF64 = 0xBD,
    F32ReinterpretI32 = 0xBE,
    F64ReinterpretI64 = 0xBF,

    // ===== Sign Extension (0xC0-0xC4) =====
    I32Extend8S = 0xC0,
    I32Extend16S = 0xC1,
    I64Extend8S = 0xC2,
    I64Extend16S = 0xC3,
    I64Extend32S = 0xC4,

    // ===== Prefix bytes =====
    /// SIMD prefix (0xFD)
    SimdPrefix = 0xFD,

    /// Neural extension prefix (0xFC)
    NeuralPrefix = 0xFC,
}

impl Opcode {
    /// Get operation type for this opcode
    pub fn op_type(&self) -> OpType {
        match *self as u8 {
            0x00..=0x11 => OpType::Control,
            0x1A..=0x1B => OpType::Parametric,
            0x20..=0x24 => OpType::Variable,
            0x28..=0x40 => OpType::Memory,
            0x41 | 0x45..=0x78 => OpType::I32,
            0x42 | 0x50..=0x5A | 0x79..=0x8A => OpType::I64,
            0x43 | 0x5B..=0x60 | 0x8B..=0x98 => OpType::F32,
            0x44 | 0x61..=0x66 | 0x99..=0xA6 => OpType::F64,
            0xA7..=0xC4 => OpType::Convert,
            0xFD => OpType::Simd,
            0xFC => OpType::Neural,
            _ => OpType::Extended,
        }
    }

    /// Try to decode from byte
    pub fn from_byte(byte: u8) -> Option<Self> {
        // Use a match for valid opcodes
        Some(match byte {
            0x00 => Opcode::Unreachable,
            0x01 => Opcode::Nop,
            0x02 => Opcode::Block,
            0x03 => Opcode::Loop,
            0x04 => Opcode::If,
            0x05 => Opcode::Else,
            0x0B => Opcode::End,
            0x0C => Opcode::Br,
            0x0D => Opcode::BrIf,
            0x0E => Opcode::BrTable,
            0x0F => Opcode::Return,
            0x10 => Opcode::Call,
            0x11 => Opcode::CallIndirect,
            0x1A => Opcode::Drop,
            0x1B => Opcode::Select,
            0x20 => Opcode::LocalGet,
            0x21 => Opcode::LocalSet,
            0x22 => Opcode::LocalTee,
            0x23 => Opcode::GlobalGet,
            0x24 => Opcode::GlobalSet,
            0x28 => Opcode::I32Load,
            0x36 => Opcode::I32Store,
            0x3F => Opcode::MemorySize,
            0x40 => Opcode::MemoryGrow,
            0x41 => Opcode::I32Const,
            0x45 => Opcode::I32Eqz,
            0x46 => Opcode::I32Eq,
            0x47 => Opcode::I32Ne,
            0x48 => Opcode::I32LtS,
            0x49 => Opcode::I32LtU,
            0x4A => Opcode::I32GtS,
            0x4B => Opcode::I32GtU,
            0x4C => Opcode::I32LeS,
            0x4D => Opcode::I32LeU,
            0x4E => Opcode::I32GeS,
            0x4F => Opcode::I32GeU,
            0x67 => Opcode::I32Clz,
            0x68 => Opcode::I32Ctz,
            0x69 => Opcode::I32Popcnt,
            0x6A => Opcode::I32Add,
            0x6B => Opcode::I32Sub,
            0x6C => Opcode::I32Mul,
            0x6D => Opcode::I32DivS,
            0x6E => Opcode::I32DivU,
            0x6F => Opcode::I32RemS,
            0x70 => Opcode::I32RemU,
            0x71 => Opcode::I32And,
            0x72 => Opcode::I32Or,
            0x73 => Opcode::I32Xor,
            0x74 => Opcode::I32Shl,
            0x75 => Opcode::I32ShrS,
            0x76 => Opcode::I32ShrU,
            0x77 => Opcode::I32Rotl,
            0x78 => Opcode::I32Rotr,
            0xFD => Opcode::SimdPrefix,
            0xFC => Opcode::NeuralPrefix,
            _ => return None,
        })
    }

    /// Check if opcode requires immediate bytes
    pub fn has_immediate(&self) -> bool {
        matches!(
            self,
            Opcode::Block
                | Opcode::Loop
                | Opcode::If
                | Opcode::Br
                | Opcode::BrIf
                | Opcode::BrTable
                | Opcode::Call
                | Opcode::CallIndirect
                | Opcode::LocalGet
                | Opcode::LocalSet
                | Opcode::LocalTee
                | Opcode::GlobalGet
                | Opcode::GlobalSet
                | Opcode::I32Load
                | Opcode::I32Store
                | Opcode::I32Const
                | Opcode::I64Const
                | Opcode::F32Const
                | Opcode::F64Const
        )
    }

    /// Get number of stack operands consumed
    pub fn stack_consume(&self) -> usize {
        match self {
            // Binary operations consume 2
            Opcode::I32Add | Opcode::I32Sub | Opcode::I32Mul | Opcode::I32DivS
            | Opcode::I32DivU | Opcode::I32RemS | Opcode::I32RemU | Opcode::I32And
            | Opcode::I32Or | Opcode::I32Xor | Opcode::I32Shl | Opcode::I32ShrS
            | Opcode::I32ShrU | Opcode::I32Rotl | Opcode::I32Rotr | Opcode::I32Eq
            | Opcode::I32Ne | Opcode::I32LtS | Opcode::I32LtU | Opcode::I32GtS
            | Opcode::I32GtU | Opcode::I32LeS | Opcode::I32LeU | Opcode::I32GeS
            | Opcode::I32GeU => 2,

            // Unary operations consume 1
            Opcode::I32Eqz | Opcode::I32Clz | Opcode::I32Ctz | Opcode::I32Popcnt
            | Opcode::Drop => 1,

            // Select consumes 3
            Opcode::Select => 3,

            // Store consumes 2 (address + value)
            Opcode::I32Store => 2,

            // Load consumes 1 (address)
            Opcode::I32Load => 1,

            // LocalSet consumes 1
            Opcode::LocalSet => 1,

            // LocalTee consumes 1 (but also produces 1)
            Opcode::LocalTee => 1,

            // GlobalSet consumes 1
            Opcode::GlobalSet => 1,

            // BrIf consumes 1 (condition)
            Opcode::BrIf => 1,

            // Return, Br consume based on block type
            Opcode::Return | Opcode::Br => 0,

            // Others
            _ => 0,
        }
    }

    /// Get number of stack values produced
    pub fn stack_produce(&self) -> usize {
        match self {
            // Most operations produce 1 value
            Opcode::I32Add | Opcode::I32Sub | Opcode::I32Mul | Opcode::I32DivS
            | Opcode::I32DivU | Opcode::I32RemS | Opcode::I32RemU | Opcode::I32And
            | Opcode::I32Or | Opcode::I32Xor | Opcode::I32Shl | Opcode::I32ShrS
            | Opcode::I32ShrU | Opcode::I32Rotl | Opcode::I32Rotr | Opcode::I32Eq
            | Opcode::I32Ne | Opcode::I32LtS | Opcode::I32LtU | Opcode::I32GtS
            | Opcode::I32GtU | Opcode::I32LeS | Opcode::I32LeU | Opcode::I32GeS
            | Opcode::I32GeU | Opcode::I32Eqz | Opcode::I32Clz | Opcode::I32Ctz
            | Opcode::I32Popcnt | Opcode::I32Const | Opcode::I32Load | Opcode::LocalGet
            | Opcode::LocalTee | Opcode::GlobalGet | Opcode::Select | Opcode::MemorySize
            | Opcode::MemoryGrow => 1,

            // Drop, Store, Set produce nothing
            Opcode::Drop | Opcode::I32Store | Opcode::LocalSet | Opcode::GlobalSet => 0,

            // Others
            _ => 0,
        }
    }
}

/// SIMD opcodes (after 0xFD prefix)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SimdOpcode {
    // v128 Memory
    V128Load = 0x00,
    V128Store = 0x0B,

    // v128 Const
    V128Const = 0x0C,

    // v128 Shuffle/Swizzle
    I8x16Shuffle = 0x0D,
    I8x16Swizzle = 0x0E,

    // v128 Splat
    I8x16Splat = 0x0F,
    I16x8Splat = 0x10,
    I32x4Splat = 0x11,
    I64x2Splat = 0x12,
    F32x4Splat = 0x13,
    F64x2Splat = 0x14,

    // i8x16 Lane extract/replace
    I8x16ExtractLaneS = 0x15,
    I8x16ExtractLaneU = 0x16,
    I8x16ReplaceLane = 0x17,

    // i16x8 Lane extract/replace
    I16x8ExtractLaneS = 0x18,
    I16x8ExtractLaneU = 0x19,
    I16x8ReplaceLane = 0x1A,

    // i32x4 Lane extract/replace
    I32x4ExtractLane = 0x1B,
    I32x4ReplaceLane = 0x1C,

    // i64x2 Lane
    I64x2ExtractLane = 0x1D,
    I64x2ReplaceLane = 0x1E,

    // f32x4 Lane
    F32x4ExtractLane = 0x1F,
    F32x4ReplaceLane = 0x20,

    // f64x2 Lane
    F64x2ExtractLane = 0x21,
    F64x2ReplaceLane = 0x22,

    // i8x16 Comparison
    I8x16Eq = 0x23,
    I8x16Ne = 0x24,
    I8x16LtS = 0x25,
    I8x16LtU = 0x26,
    I8x16GtS = 0x27,
    I8x16GtU = 0x28,
    I8x16LeS = 0x29,
    I8x16LeU = 0x2A,
    I8x16GeS = 0x2B,
    I8x16GeU = 0x2C,

    // i8x16 Arithmetic
    I8x16Abs = 0x60,
    I8x16Neg = 0x61,
    I8x16Add = 0x6E,
    I8x16Sub = 0x71,

    // i16x8 Arithmetic
    I16x8Add = 0x8E,
    I16x8Sub = 0x91,
    I16x8Mul = 0x95,

    // i32x4 Arithmetic
    I32x4Add = 0xAE,
    I32x4Sub = 0xB1,
    I32x4Mul = 0xB5,

    // v128 Bitwise
    V128Not = 0x4D,
    V128And = 0x4E,
    V128AndNot = 0x4F,
    V128Or = 0x50,
    V128Xor = 0x51,
    V128Bitselect = 0x52,
}

/// Neural extension opcodes (after 0xFC prefix)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum NeuralOpcode {
    /// Multiply-accumulate
    NeuralMac = 0x00,

    /// ReLU activation
    NeuralRelu = 0x01,

    /// Sigmoid activation
    NeuralSigmoid = 0x02,

    /// Tanh activation
    NeuralTanh = 0x03,

    /// Softmax
    NeuralSoftmax = 0x04,

    /// Matrix multiply
    NeuralMatmul = 0x05,

    /// 2D convolution
    NeuralConv2d = 0x06,

    /// Max pooling
    NeuralPoolMax = 0x07,

    /// Average pooling
    NeuralPoolAvg = 0x08,

    /// Batch normalization
    NeuralBatchNorm = 0x09,

    /// Dropout
    NeuralDropout = 0x0A,

    /// Self-attention
    NeuralAttention = 0x0B,
}
