// A2S v2r3 Processor Implementation
// Zero-address stack machine ISA

pub mod error;
pub mod fpu;
pub mod instruction;
pub mod memory;
pub mod processor;
pub mod stack;

pub use error::{FpuError, ProcessorError, Result};
pub use fpu::Fpu;
pub use instruction::{Instruction, Opcode};
pub use memory::Memory;
pub use processor::A2SProcessor;
pub use stack::Stack;
