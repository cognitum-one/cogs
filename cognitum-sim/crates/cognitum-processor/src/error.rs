use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ProcessorError {
    #[error("Stack underflow")]
    StackUnderflow,

    #[error("Stack overflow")]
    StackOverflow,

    #[error("Invalid memory address: {0:#x}")]
    InvalidMemoryAddress(u32),

    #[error("Invalid instruction opcode: {0:#x}")]
    InvalidOpcode(u8),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Invalid instruction encoding")]
    InvalidEncoding,

    #[error("Return stack underflow")]
    ReturnStackUnderflow,

    #[error("Return stack overflow")]
    ReturnStackOverflow,

    #[error("FPU error: {0}")]
    FpuError(#[from] FpuError),
}

#[derive(Error, Debug, PartialEq)]
pub enum FpuError {
    #[error("Invalid floating-point operation")]
    InvalidOperation,

    #[error("Floating-point division by zero")]
    DivisionByZero,

    #[error("Floating-point overflow")]
    Overflow,

    #[error("Floating-point underflow")]
    Underflow,

    #[error("Inexact floating-point result")]
    Inexact,

    #[error("Invalid conversion")]
    InvalidConversion,
}

pub type Result<T> = std::result::Result<T, ProcessorError>;
