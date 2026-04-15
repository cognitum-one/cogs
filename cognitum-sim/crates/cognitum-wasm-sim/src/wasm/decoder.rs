//! WASM bytecode decoder
//!
//! Decodes WASM bytecode into internal operation format

use super::opcodes::{Opcode, OpType, SimdOpcode, NeuralOpcode};
use super::memory::WasmMemory;
use crate::error::{Result, WasmSimError};

/// Decoded instruction
#[derive(Debug, Clone)]
pub struct DecodedInstruction {
    /// Primary opcode
    pub opcode: Opcode,

    /// Operation type
    pub op_type: OpType,

    /// Immediate value (LEB128 decoded)
    pub immediate: i64,

    /// Secondary immediate (for memory ops: offset)
    pub immediate2: u32,

    /// SIMD opcode (if SIMD instruction)
    pub simd_opcode: Option<SimdOpcode>,

    /// Neural opcode (if neural instruction)
    pub neural_opcode: Option<NeuralOpcode>,

    /// Instruction length in bytes
    pub length: u8,

    /// Memory size (for load/store)
    pub mem_size: MemSize,

    /// Signed flag (for load operations)
    pub is_signed: bool,
}

/// Memory access size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemSize {
    #[default]
    Byte = 0,
    Half = 1,
    Word = 2,
    Double = 3,
    V128 = 4,
}

/// Decoder state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderState {
    Idle,
    Decode,
    SimdPrefix,
    NeuralPrefix,
    ImmBytes,
    Output,
}

/// WASM bytecode decoder
pub struct WasmDecoder {
    /// Current state
    state: DecoderState,

    /// Immediate byte buffer
    imm_buffer: [u8; 16],

    /// Bytes collected
    imm_bytes_collected: usize,

    /// Expected immediate bytes
    imm_bytes_expected: usize,
}

impl WasmDecoder {
    /// Create new decoder
    pub fn new() -> Self {
        Self {
            state: DecoderState::Idle,
            imm_buffer: [0u8; 16],
            imm_bytes_collected: 0,
            imm_bytes_expected: 0,
        }
    }

    /// Reset decoder state
    pub fn reset(&mut self) {
        self.state = DecoderState::Idle;
        self.imm_bytes_collected = 0;
        self.imm_bytes_expected = 0;
    }

    /// Decode instruction at PC
    pub fn decode(&mut self, first_byte: u8, memory: &WasmMemory, pc: u32) -> Result<DecodedInstruction> {
        let mut length = 1u8;
        let mut immediate: i64 = 0;
        let mut immediate2: u32 = 0;
        let mut simd_opcode = None;
        let mut neural_opcode = None;
        let mut mem_size = MemSize::Word;
        let mut is_signed = false;

        // Decode primary opcode
        let opcode = Opcode::from_byte(first_byte)
            .ok_or(WasmSimError::InvalidOpcode(first_byte))?;

        let op_type = opcode.op_type();

        // Handle prefix bytes
        match opcode {
            Opcode::SimdPrefix => {
                // Read SIMD opcode (LEB128 u32)
                let (simd_op, bytes) = self.read_leb128_u32(memory, pc + 1)?;
                length += bytes as u8;
                simd_opcode = Some(self.decode_simd_opcode(simd_op)?);
            }

            Opcode::NeuralPrefix => {
                // Read neural opcode (LEB128 u32)
                let (neural_op, bytes) = self.read_leb128_u32(memory, pc + 1)?;
                length += bytes as u8;
                neural_opcode = Some(self.decode_neural_opcode(neural_op)?);
            }

            _ => {
                // Handle immediates for standard opcodes
                if opcode.has_immediate() {
                    let (imm, bytes) = self.decode_immediate(&opcode, memory, pc + 1)?;
                    immediate = imm;
                    length += bytes as u8;

                    // Memory ops have alignment + offset
                    if matches!(opcode, Opcode::I32Load | Opcode::I32Store) {
                        // Skip alignment (already read), read offset
                        let offset_pos = pc + 1 + bytes as u32;
                        let (offset, offset_bytes) = self.read_leb128_u32(memory, offset_pos)?;
                        immediate2 = offset;
                        length += offset_bytes as u8;
                    }
                }

                // Set memory size for load/store
                match opcode {
                    Opcode::I32Load8S => {
                        mem_size = MemSize::Byte;
                        is_signed = true;
                    }
                    Opcode::I32Load8U => {
                        mem_size = MemSize::Byte;
                        is_signed = false;
                    }
                    Opcode::I32Load16S => {
                        mem_size = MemSize::Half;
                        is_signed = true;
                    }
                    Opcode::I32Load16U => {
                        mem_size = MemSize::Half;
                        is_signed = false;
                    }
                    Opcode::I32Load | Opcode::I32Store => {
                        mem_size = MemSize::Word;
                    }
                    _ => {}
                }
            }
        }

        Ok(DecodedInstruction {
            opcode,
            op_type,
            immediate,
            immediate2,
            simd_opcode,
            neural_opcode,
            length,
            mem_size,
            is_signed,
        })
    }

    /// Decode immediate value based on opcode
    fn decode_immediate(&self, opcode: &Opcode, memory: &WasmMemory, pos: u32) -> Result<(i64, usize)> {
        match opcode {
            // Signed LEB128 immediates
            Opcode::I32Const => {
                let (val, bytes) = self.read_leb128_i32(memory, pos)?;
                Ok((val as i64, bytes))
            }

            Opcode::I64Const => {
                self.read_leb128_i64(memory, pos)
            }

            // Unsigned LEB128 immediates
            Opcode::LocalGet | Opcode::LocalSet | Opcode::LocalTee
            | Opcode::GlobalGet | Opcode::GlobalSet
            | Opcode::Br | Opcode::BrIf | Opcode::Call
            | Opcode::Block | Opcode::Loop | Opcode::If => {
                let (val, bytes) = self.read_leb128_u32(memory, pos)?;
                Ok((val as i64, bytes))
            }

            // Memory ops: alignment (we ignore it but must read)
            Opcode::I32Load | Opcode::I32Store
            | Opcode::I32Load8S | Opcode::I32Load8U
            | Opcode::I32Load16S | Opcode::I32Load16U => {
                let (val, bytes) = self.read_leb128_u32(memory, pos)?;
                Ok((val as i64, bytes))
            }

            // Float constants (4 bytes for f32, 8 for f64)
            Opcode::F32Const => {
                let bytes = self.read_bytes(memory, pos, 4)?;
                let val = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                Ok((val as i64, 4))
            }

            Opcode::F64Const => {
                let bytes = self.read_bytes(memory, pos, 8)?;
                let val = u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                Ok((val as i64, 8))
            }

            _ => Ok((0, 0)),
        }
    }

    /// Read LEB128 unsigned 32-bit value
    fn read_leb128_u32(&self, memory: &WasmMemory, pos: u32) -> Result<(u32, usize)> {
        let mut result: u32 = 0;
        let mut shift = 0;
        let mut bytes_read = 0;

        loop {
            let byte = memory.read_code(pos + bytes_read as u32)?;
            bytes_read += 1;

            result |= ((byte & 0x7F) as u32) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                break;
            }

            if bytes_read >= 5 {
                return Err(WasmSimError::InvalidBytecode("LEB128 overflow".into()));
            }
        }

        Ok((result, bytes_read))
    }

    /// Read LEB128 signed 32-bit value
    fn read_leb128_i32(&self, memory: &WasmMemory, pos: u32) -> Result<(i32, usize)> {
        let mut result: i32 = 0;
        let mut shift = 0;
        let mut bytes_read = 0;
        let mut byte;

        loop {
            byte = memory.read_code(pos + bytes_read as u32)?;
            bytes_read += 1;

            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                break;
            }

            if bytes_read >= 5 {
                return Err(WasmSimError::InvalidBytecode("LEB128 overflow".into()));
            }
        }

        // Sign extend if needed
        if shift < 32 && (byte & 0x40) != 0 {
            result |= !0 << shift;
        }

        Ok((result, bytes_read))
    }

    /// Read LEB128 signed 64-bit value
    fn read_leb128_i64(&self, memory: &WasmMemory, pos: u32) -> Result<(i64, usize)> {
        let mut result: i64 = 0;
        let mut shift = 0;
        let mut bytes_read = 0;
        let mut byte;

        loop {
            byte = memory.read_code(pos + bytes_read as u32)?;
            bytes_read += 1;

            result |= ((byte & 0x7F) as i64) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                break;
            }

            if bytes_read >= 10 {
                return Err(WasmSimError::InvalidBytecode("LEB128 overflow".into()));
            }
        }

        // Sign extend if needed
        if shift < 64 && (byte & 0x40) != 0 {
            result |= !0i64 << shift;
        }

        Ok((result, bytes_read))
    }

    /// Read raw bytes
    fn read_bytes(&self, memory: &WasmMemory, pos: u32, count: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(count);
        for i in 0..count {
            bytes.push(memory.read_code(pos + i as u32)?);
        }
        Ok(bytes)
    }

    /// Decode SIMD opcode
    fn decode_simd_opcode(&self, opcode: u32) -> Result<SimdOpcode> {
        Ok(match opcode {
            0x00 => SimdOpcode::V128Load,
            0x0B => SimdOpcode::V128Store,
            0x0C => SimdOpcode::V128Const,
            0x0D => SimdOpcode::I8x16Shuffle,
            0x0E => SimdOpcode::I8x16Swizzle,
            0x0F => SimdOpcode::I8x16Splat,
            0x10 => SimdOpcode::I16x8Splat,
            0x11 => SimdOpcode::I32x4Splat,
            0x6E => SimdOpcode::I8x16Add,
            0x71 => SimdOpcode::I8x16Sub,
            0x8E => SimdOpcode::I16x8Add,
            0x91 => SimdOpcode::I16x8Sub,
            0x95 => SimdOpcode::I16x8Mul,
            0xAE => SimdOpcode::I32x4Add,
            0xB1 => SimdOpcode::I32x4Sub,
            0xB5 => SimdOpcode::I32x4Mul,
            0x4D => SimdOpcode::V128Not,
            0x4E => SimdOpcode::V128And,
            0x4F => SimdOpcode::V128AndNot,
            0x50 => SimdOpcode::V128Or,
            0x51 => SimdOpcode::V128Xor,
            0x52 => SimdOpcode::V128Bitselect,
            _ => return Err(WasmSimError::InvalidOpcode(opcode as u8)),
        })
    }

    /// Decode neural opcode
    fn decode_neural_opcode(&self, opcode: u32) -> Result<NeuralOpcode> {
        Ok(match opcode {
            0x00 => NeuralOpcode::NeuralMac,
            0x01 => NeuralOpcode::NeuralRelu,
            0x02 => NeuralOpcode::NeuralSigmoid,
            0x03 => NeuralOpcode::NeuralTanh,
            0x04 => NeuralOpcode::NeuralSoftmax,
            0x05 => NeuralOpcode::NeuralMatmul,
            0x06 => NeuralOpcode::NeuralConv2d,
            0x07 => NeuralOpcode::NeuralPoolMax,
            0x08 => NeuralOpcode::NeuralPoolAvg,
            0x09 => NeuralOpcode::NeuralBatchNorm,
            0x0A => NeuralOpcode::NeuralDropout,
            0x0B => NeuralOpcode::NeuralAttention,
            _ => return Err(WasmSimError::InvalidOpcode(opcode as u8)),
        })
    }
}

impl Default for WasmDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder = WasmDecoder::new();
        assert_eq!(decoder.state, DecoderState::Idle);
    }

    #[test]
    fn test_decode_nop() {
        let mut decoder = WasmDecoder::new();
        let memory = WasmMemory::new(1024, 1024, 1024, 1, 16).unwrap();

        // NOP is 0x01
        let result = decoder.decode(0x01, &memory, 0);
        assert!(result.is_ok());

        let instr = result.unwrap();
        assert_eq!(instr.opcode, Opcode::Nop);
        assert_eq!(instr.length, 1);
    }
}
