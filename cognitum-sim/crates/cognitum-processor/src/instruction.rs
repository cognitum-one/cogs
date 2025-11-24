// A2S v2r3 Instruction Set
// Matches encodings from A2Sv2r3_ISA.v

use crate::error::{ProcessorError, Result};

/// 6-bit base opcodes (matches Verilog localparam)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // Memory Operations (0-15)
    PUTA = 0b00_0000, // !a      ( x -- )(A: a-addr)
    PUTB = 0b00_0001, // !b      ( x -- )(B: a-addr)
    PUTC = 0b00_0010, // !c      ( x -- )(C: a-addr)
    PUT = 0b00_0011,  // !       ( x a-addr -- )
    GETA = 0b00_0100, // @a      ( -- x )(A: a-addr)
    GETB = 0b00_0101, // @b      ( -- x )(B: a-addr)
    GETC = 0b00_0110, // @c      ( -- x )(C: a-addr)
    GET = 0b00_0111,  // @       ( a-addr -- x )
    PTAP = 0b00_1000, // !a+     ( x -- )(A: a-addr -- a-addr+4)
    PTBP = 0b00_1001, // !b+     ( x -- )(B: a-addr -- a-addr+4)
    PTCP = 0b00_1010, // !c+     ( x -- )(C: a-addr -- a-addr+4)
    XCHG = 0b00_1011, // @!      ( x1 a-addr -- x2) atomic
    GTAP = 0b00_1100, // @a+     ( -- x )(A: a-addr -- a-addr+4)
    GTBP = 0b00_1101, // @b+     ( -- x )(B: a-addr -- a-addr+4)
    GTCP = 0b00_1110, // @c+     ( -- x )(C: a-addr -- a-addr+4)

    // Register Transfer (16-31)
    DECA = 0b01_0000, // a-      (A: a-addr -- a-addr-4)
    DECB = 0b01_0001, // b-      (B: a-addr -- a-addr-4)
    DECC = 0b01_0010, // c-      (C: a-addr -- a-addr-4)
    SNB = 0b01_0011,  // snb     skip to next bundle
    SEL = 0b01_0100,  // ?:      ( x1 x2 flag -- x3 )
    NOT = 0b01_0111,  // ~       ( x -- ~x)
    T2A = 0b01_1000,  // >a      ( x -- )(A: -- x)
    T2B = 0b01_1001,  // >b      ( x -- )(B: -- x)
    T2C = 0b01_1010,  // >c      ( x -- )(C: -- x)
    T2R = 0b01_1011,  // >r      ( x -- )(R: -- x)
    A2T = 0b01_1100,  // a>      ( -- x )(A: x -- x)
    B2T = 0b01_1101,  // b>      ( -- x )(B: x -- x)
    C2T = 0b01_1110,  // c>      ( -- x )(C: x -- x)
    R2T = 0b01_1111,  // r>      ( -- x )(R: x --)

    // Stack Manipulation (32-39)
    NIP = 0b10_0000,  // nip     ( x1 x2 -- x2 )
    DROP = 0b10_0001, // drop    ( x1 x2 -- x1 )
    DUP = 0b10_0010,  // dup     ( x -- x x )
    OVER = 0b10_0011, // over    ( x1 x2 -- x1 x2 x1 )
    SWAP = 0b10_0100, // ><      ( x1 x2 -- x2 x1 )
    ROT3 = 0b10_0101, // ><<     ( x1 x2 x3 -- x2 x3 x1 )
    ROT4 = 0b10_0110, // ><<<    ( x1 x2 x3 x4 -- x2 x3 x4 x1)

    // Arithmetic (40-47)
    ADD = 0b10_1000, // +       ( n1 n2 -- sum )
    SUB = 0b10_1001, // -       ( n1 n2 -- diff )
    SLT = 0b10_1010, // <       ( n1 n2 -- flag )
    ULT = 0b10_1011, // u<      ( u1 u2 -- flag )
    EQ = 0b10_1100,  // =       ( x1 x2 -- flag )
    XOR = 0b10_1101, // ^       ( x1 x2 -- x3 )
    IOR = 0b10_1110, // |       ( x1 x2 -- x3 )
    AND = 0b10_1111, // &       ( x1 x2 -- x3 )

    // Constants (48-54)
    ZERO = 0b11_0000, // 0       ( -- 0 )
    ONE = 0b11_0001,  // 1       ( -- 1 )
    CO = 0b11_0010,   // co      coroutine call
    RTN = 0b11_0011,  // rtn     ( -- )(R: rtn-addr --)
    NOP = 0b11_0100,  // nop     ( -- )
    PFX = 0b11_0101,  // prefix  extend literal
    LIT = 0b11_0110,  // lit     ( -- x )
    EXT = 0b11_0111,  // ext     extended function

    // Control Flow (57-63)
    CALL = 0b11_1001, // call    ( -- )(R: -- rtn-addr)
    JN = 0b11_1010,   // n->     ( n -- )
    JZ = 0b11_1011,   // 0->     ( x -- )
    JANZ = 0b11_1100, // a-->    (A: u -- u-1)
    JBNZ = 0b11_1101, // b-->    (B: u -- u-1)
    JCNZ = 0b11_1110, // c-->    (C: u -- u-1)
    JMP = 0b11_1111,  // ->      unconditional jump
}

impl Opcode {
    pub fn from_u8(byte: u8) -> Result<Self> {
        match byte {
            0b00_0000 => Ok(Opcode::PUTA),
            0b00_0001 => Ok(Opcode::PUTB),
            0b00_0010 => Ok(Opcode::PUTC),
            0b00_0011 => Ok(Opcode::PUT),
            0b00_0100 => Ok(Opcode::GETA),
            0b00_0101 => Ok(Opcode::GETB),
            0b00_0110 => Ok(Opcode::GETC),
            0b00_0111 => Ok(Opcode::GET),
            0b00_1000 => Ok(Opcode::PTAP),
            0b00_1001 => Ok(Opcode::PTBP),
            0b00_1010 => Ok(Opcode::PTCP),
            0b00_1011 => Ok(Opcode::XCHG),
            0b00_1100 => Ok(Opcode::GTAP),
            0b00_1101 => Ok(Opcode::GTBP),
            0b00_1110 => Ok(Opcode::GTCP),
            0b01_0000 => Ok(Opcode::DECA),
            0b01_0001 => Ok(Opcode::DECB),
            0b01_0010 => Ok(Opcode::DECC),
            0b01_0011 => Ok(Opcode::SNB),
            0b01_0100 => Ok(Opcode::SEL),
            0b01_0111 => Ok(Opcode::NOT),
            0b01_1000 => Ok(Opcode::T2A),
            0b01_1001 => Ok(Opcode::T2B),
            0b01_1010 => Ok(Opcode::T2C),
            0b01_1011 => Ok(Opcode::T2R),
            0b01_1100 => Ok(Opcode::A2T),
            0b01_1101 => Ok(Opcode::B2T),
            0b01_1110 => Ok(Opcode::C2T),
            0b01_1111 => Ok(Opcode::R2T),
            0b10_0000 => Ok(Opcode::NIP),
            0b10_0001 => Ok(Opcode::DROP),
            0b10_0010 => Ok(Opcode::DUP),
            0b10_0011 => Ok(Opcode::OVER),
            0b10_0100 => Ok(Opcode::SWAP),
            0b10_0101 => Ok(Opcode::ROT3),
            0b10_0110 => Ok(Opcode::ROT4),
            0b10_1000 => Ok(Opcode::ADD),
            0b10_1001 => Ok(Opcode::SUB),
            0b10_1010 => Ok(Opcode::SLT),
            0b10_1011 => Ok(Opcode::ULT),
            0b10_1100 => Ok(Opcode::EQ),
            0b10_1101 => Ok(Opcode::XOR),
            0b10_1110 => Ok(Opcode::IOR),
            0b10_1111 => Ok(Opcode::AND),
            0b11_0000 => Ok(Opcode::ZERO),
            0b11_0001 => Ok(Opcode::ONE),
            0b11_0010 => Ok(Opcode::CO),
            0b11_0011 => Ok(Opcode::RTN),
            0b11_0100 => Ok(Opcode::NOP),
            0b11_0101 => Ok(Opcode::PFX),
            0b11_0110 => Ok(Opcode::LIT),
            0b11_0111 => Ok(Opcode::EXT),
            0b11_1001 => Ok(Opcode::CALL),
            0b11_1010 => Ok(Opcode::JN),
            0b11_1011 => Ok(Opcode::JZ),
            0b11_1100 => Ok(Opcode::JANZ),
            0b11_1101 => Ok(Opcode::JBNZ),
            0b11_1110 => Ok(Opcode::JCNZ),
            0b11_1111 => Ok(Opcode::JMP),
            _ => Err(ProcessorError::InvalidOpcode(byte)),
        }
    }
}

/// Extended instruction opcodes (accessed via EXT opcode)
pub const EXT_MULS: u16 = 0x40;  // Signed multiply (32x32 -> 64-bit)
pub const EXT_MULU: u16 = 0x41;  // Unsigned multiply (32x32 -> 64-bit)
pub const EXT_MULH: u16 = 0x42;  // Multiply high signed (upper 32 bits)
pub const EXT_MULHU: u16 = 0x43; // Multiply high unsigned (upper 32 bits)
pub const EXT_DIVS: u16 = 0x44;  // Signed divide
pub const EXT_DIVU: u16 = 0x45;  // Unsigned divide
pub const EXT_MODS: u16 = 0x46;  // Signed modulo
pub const EXT_MODU: u16 = 0x47;  // Unsigned modulo

/// Instruction representation
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Stack operations
    Push(i32),
    Pop,
    Dup,
    Swap,
    Over,
    Rot3,
    Rot4,
    Drop,
    Nip,

    // Arithmetic
    Add,
    Sub,
    Multiply,
    Divide,

    // Extended Multiply Operations
    MultiplySigned,        // MULS: 32x32 -> 64-bit (push low, then high)
    MultiplyUnsigned,      // MULU: 32x32 -> 64-bit (push low, then high)
    MultiplyHighSigned,    // MULH: 32x32 -> upper 32 bits only
    MultiplyHighUnsigned,  // MULHU: 32x32 -> upper 32 bits only

    // Extended Divide Operations
    DivideSigned,          // DIVS: signed division (quotient)
    DivideUnsigned,        // DIVU: unsigned division (quotient)
    ModuloSigned,          // MODS: signed modulo (remainder)
    ModuloUnsigned,        // MODU: unsigned modulo (remainder)

    // Logic
    And,
    Or,
    Xor,
    Not,

    // Comparison
    Equal,
    LessThan,
    UnsignedLessThan,

    // Memory
    Load,
    Store,
    LoadA,
    LoadB,
    LoadC,
    StoreA,
    StoreB,
    StoreC,

    // Register operations
    ToA,
    ToB,
    ToC,
    ToR,
    FromA,
    FromB,
    FromC,
    FromR,

    // Control flow
    Jump(i16),
    JumpZero(i16),
    JumpNegative(i16),
    Call(i16),
    Return,

    // Shift and Rotate operations
    ShiftLeft,              // LSL - Logical shift left (pop shift amount)
    ShiftRight,             // LSR - Logical shift right (pop shift amount)
    ShiftRightArith,        // ASR - Arithmetic shift right (sign extend)
    ShiftLeftImm(u8),       // LSLI - Logical shift left immediate
    ShiftRightImm(u8),      // LSRI - Logical shift right immediate
    ShiftRightArithImm(u8), // ASRI - Arithmetic shift right immediate
    RotateLeft,             // ROL - Rotate left (pop shift amount)
    RotateRight,            // ROR - Rotate right (pop shift amount)
    RotateLeftImm(u8),      // ROLI - Rotate left immediate
    RotateRightImm(u8),     // RORI - Rotate right immediate

    // ========================================
    // Floating-Point Operations (IEEE 754)
    // ========================================

    // Single Precision (f32) - Arithmetic
    FAdd,    // ( f1 f2 -- f3 ) f3 = f1 + f2
    FSub,    // ( f1 f2 -- f3 ) f3 = f1 - f2
    FMul,    // ( f1 f2 -- f3 ) f3 = f1 * f2
    FDiv,    // ( f1 f2 -- f3 ) f3 = f1 / f2
    FSqrt,   // ( f1 -- f2 ) f2 = sqrt(f1)

    // Single Precision - Comparison
    FCmp,    // ( f1 f2 -- n ) n = -1 if f1<f2, 0 if f1==f2, 1 if f1>f2
    FClt,    // ( f1 f2 -- flag ) flag = f1 < f2
    FCeq,    // ( f1 f2 -- flag ) flag = f1 == f2
    FCle,    // ( f1 f2 -- flag ) flag = f1 <= f2

    // Single Precision - Conversion
    F2I,     // ( f -- n ) float to signed int
    I2F,     // ( n -- f ) signed int to float

    // Single Precision - Utilities
    FAbs,    // ( f -- |f| )
    FChs,    // ( f -- -f )
    FMax,    // ( f1 f2 -- f3 ) f3 = max(f1, f2)
    FMin,    // ( f1 f2 -- f3 ) f3 = min(f1, f2)
    FNan,    // ( f -- f' ) f' = 0.0 if f is NaN, else f

    // Double Precision (f64) - Arithmetic
    DAdd,    // ( d1 d2 -- d3 ) d3 = d1 + d2
    DSub,    // ( d1 d2 -- d3 ) d3 = d1 - d2
    DMul,    // ( d1 d2 -- d3 ) d3 = d1 * d2
    DDiv,    // ( d1 d2 -- d3 ) d3 = d1 / d2
    DSqrt,   // ( d1 -- d2 ) d2 = sqrt(d1)

    // Double Precision - Comparison
    DCmp,    // ( d1 d2 -- n ) n = -1 if d1<d2, 0 if d1==d2, 1 if d1>d2
    DClt,    // ( d1 d2 -- flag ) flag = d1 < d2
    DCeq,    // ( d1 d2 -- flag ) flag = d1 == d2
    DCle,    // ( d1 d2 -- flag ) flag = d1 <= d2

    // Double Precision - Conversion
    D2I,     // ( d -- n ) double to signed int
    I2D,     // ( n -- d ) signed int to double

    // Double Precision - Utilities
    DAbs,    // ( d -- |d| )
    DChs,    // ( d -- -d )
    DMax,    // ( d1 d2 -- d3 ) d3 = max(d1, d2)
    DMin,    // ( d1 d2 -- d3 ) d3 = min(d1, d2)
    DNan,    // ( d -- d' ) d' = 0.0 if d is NaN, else d

    // Precision Conversion
    F2D,     // ( f -- d ) single to double
    D2F,     // ( d -- f ) double to single

    // Special
    Nop,
    Halt,
}

impl Instruction {
    /// Decode instruction from opcode
    pub fn from_opcode(opcode: Opcode, operand: Option<u16>) -> Result<Self> {
        match opcode {
            Opcode::DUP => Ok(Instruction::Dup),
            Opcode::DROP => Ok(Instruction::Drop),
            Opcode::SWAP => Ok(Instruction::Swap),
            Opcode::OVER => Ok(Instruction::Over),
            Opcode::ROT3 => Ok(Instruction::Rot3),
            Opcode::ROT4 => Ok(Instruction::Rot4),
            Opcode::NIP => Ok(Instruction::Nip),

            Opcode::ADD => Ok(Instruction::Add),
            Opcode::SUB => Ok(Instruction::Sub),

            Opcode::AND => Ok(Instruction::And),
            Opcode::IOR => Ok(Instruction::Or),
            Opcode::XOR => Ok(Instruction::Xor),
            Opcode::NOT => Ok(Instruction::Not),

            Opcode::EQ => Ok(Instruction::Equal),
            Opcode::SLT => Ok(Instruction::LessThan),
            Opcode::ULT => Ok(Instruction::UnsignedLessThan),

            Opcode::GET => Ok(Instruction::Load),
            Opcode::PUT => Ok(Instruction::Store),
            Opcode::GETA => Ok(Instruction::LoadA),
            Opcode::GETB => Ok(Instruction::LoadB),
            Opcode::GETC => Ok(Instruction::LoadC),
            Opcode::PUTA => Ok(Instruction::StoreA),
            Opcode::PUTB => Ok(Instruction::StoreB),
            Opcode::PUTC => Ok(Instruction::StoreC),

            Opcode::T2A => Ok(Instruction::ToA),
            Opcode::T2B => Ok(Instruction::ToB),
            Opcode::T2C => Ok(Instruction::ToC),
            Opcode::T2R => Ok(Instruction::ToR),
            Opcode::A2T => Ok(Instruction::FromA),
            Opcode::B2T => Ok(Instruction::FromB),
            Opcode::C2T => Ok(Instruction::FromC),
            Opcode::R2T => Ok(Instruction::FromR),

            Opcode::LIT => {
                let value = operand.ok_or(ProcessorError::InvalidEncoding)? as i32;
                Ok(Instruction::Push(value))
            }
            Opcode::ZERO => Ok(Instruction::Push(0)),
            Opcode::ONE => Ok(Instruction::Push(1)),

            Opcode::JMP => {
                let offset = operand.ok_or(ProcessorError::InvalidEncoding)? as i16;
                Ok(Instruction::Jump(offset))
            }
            Opcode::JZ => {
                let offset = operand.ok_or(ProcessorError::InvalidEncoding)? as i16;
                Ok(Instruction::JumpZero(offset))
            }
            Opcode::JN => {
                let offset = operand.ok_or(ProcessorError::InvalidEncoding)? as i16;
                Ok(Instruction::JumpNegative(offset))
            }
            Opcode::CALL => {
                let offset = operand.ok_or(ProcessorError::InvalidEncoding)? as i16;
                Ok(Instruction::Call(offset))
            }
            Opcode::RTN => Ok(Instruction::Return),

            Opcode::NOP => Ok(Instruction::Nop),

            Opcode::EXT => {
                let ext_code = operand.ok_or(ProcessorError::InvalidEncoding)?;
                Self::from_extended(ext_code)
            }

            _ => Err(ProcessorError::InvalidEncoding),
        }
    }

    /// Decode extended instruction from EXT opcode operand
    pub fn from_extended(ext_code: u16) -> Result<Self> {
        match ext_code {
            EXT_MULS => Ok(Instruction::MultiplySigned),
            EXT_MULU => Ok(Instruction::MultiplyUnsigned),
            EXT_MULH => Ok(Instruction::MultiplyHighSigned),
            EXT_MULHU => Ok(Instruction::MultiplyHighUnsigned),
            EXT_DIVS => Ok(Instruction::DivideSigned),
            EXT_DIVU => Ok(Instruction::DivideUnsigned),
            EXT_MODS => Ok(Instruction::ModuloSigned),
            EXT_MODU => Ok(Instruction::ModuloUnsigned),
            _ => Err(ProcessorError::InvalidEncoding),
        }
    }
}
