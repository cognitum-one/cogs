// Core types for Cognitum ASIC
use crate::error::CognitumError;
use crate::Result;
use serde::{Deserialize, Serialize};

/// Tile identifier (0-255)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId(u8);

impl TileId {
    /// Create a new TileId with validation
    pub fn new(id: u16) -> Result<Self> {
        if id > 255 {
            Err(CognitumError::InvalidTileId(id))
        } else {
            Ok(TileId(id as u8))
        }
    }

    /// Get the raw tile ID value as u8
    pub fn value(&self) -> u8 {
        self.0
    }
}

impl std::fmt::Display for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Memory address (32-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemoryAddress(u32);

/// Physical address type alias
pub type PhysAddr = MemoryAddress;

/// Virtual address type alias
pub type VirtAddr = MemoryAddress;

impl MemoryAddress {
    /// Create a new memory address
    pub fn new(addr: u32) -> Self {
        MemoryAddress(addr)
    }

    /// Get the raw address value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Check if address is aligned to given boundary
    pub fn is_aligned(&self, alignment: u32) -> bool {
        self.0.is_multiple_of(alignment)
    }

    /// Align address down to boundary
    pub fn align_down(&self, alignment: u32) -> Self {
        MemoryAddress(self.0 & !(alignment - 1))
    }

    /// Align address up to boundary
    pub fn align_up(&self, alignment: u32) -> Self {
        let mask = alignment - 1;
        MemoryAddress((self.0 + mask) & !mask)
    }

    /// Add offset to address
    pub fn offset(&self, offset: u32) -> Self {
        MemoryAddress(self.0.wrapping_add(offset))
    }
}

/// Register (32-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register(u32);

impl Register {
    /// Create a new register with value
    pub fn new(value: u32) -> Self {
        Register(value)
    }

    /// Get register value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Set register value
    pub fn set(&mut self, value: u32) {
        self.0 = value;
    }
}

/// Instruction (16-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction(u16);

impl Instruction {
    /// Create a new instruction
    pub fn new(encoding: u16) -> Self {
        Instruction(encoding)
    }

    /// Get instruction encoding
    pub fn encoding(&self) -> u16 {
        self.0
    }

    /// Extract opcode (upper 4 bits)
    pub fn opcode(&self) -> u8 {
        (self.0 >> 12) as u8
    }

    /// Extract register field (bits 11-8)
    pub fn register(&self) -> u8 {
        ((self.0 >> 8) & 0xF) as u8
    }

    /// Extract immediate value (lower 8 bits)
    pub fn immediate(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TileId tests
    #[test]
    fn test_tile_id_valid_range() {
        assert!(TileId::new(0).is_ok());
        assert!(TileId::new(255).is_ok());
        assert_eq!(TileId::new(127).unwrap().value(), 127);
    }

    #[test]
    fn test_tile_id_invalid() {
        assert!(TileId::new(256).is_err());
        assert!(TileId::new(1000).is_err());

        match TileId::new(256) {
            Err(CognitumError::InvalidTileId(id)) => assert_eq!(id, 256),
            _ => panic!("Expected InvalidTileId error"),
        }
    }

    #[test]
    fn test_tile_id_equality() {
        let t1 = TileId::new(42).unwrap();
        let t2 = TileId::new(42).unwrap();
        let t3 = TileId::new(43).unwrap();

        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    // MemoryAddress tests
    #[test]
    fn test_memory_address_creation() {
        let addr = MemoryAddress::new(0x1000);
        assert_eq!(addr.value(), 0x1000);
    }

    #[test]
    fn test_memory_address_alignment_check() {
        let addr = MemoryAddress::new(0x1000);
        assert!(addr.is_aligned(4));
        assert!(addr.is_aligned(16));

        let unaligned = MemoryAddress::new(0x1001);
        assert!(!unaligned.is_aligned(4));
        assert!(unaligned.is_aligned(1));
    }

    #[test]
    fn test_memory_address_align_down() {
        let addr = MemoryAddress::new(0x1007);
        assert_eq!(addr.align_down(4).value(), 0x1004);
        assert_eq!(addr.align_down(8).value(), 0x1000);
    }

    #[test]
    fn test_memory_address_align_up() {
        let addr = MemoryAddress::new(0x1001);
        assert_eq!(addr.align_up(4).value(), 0x1004);

        let aligned = MemoryAddress::new(0x1000);
        assert_eq!(aligned.align_up(4).value(), 0x1000);
    }

    #[test]
    fn test_memory_address_offset() {
        let addr = MemoryAddress::new(0x1000);
        assert_eq!(addr.offset(0x100).value(), 0x1100);
    }

    #[test]
    fn test_memory_address_wrapping() {
        let addr = MemoryAddress::new(0xFFFF_FFFF);
        assert_eq!(addr.offset(1).value(), 0x0000_0000);
    }

    #[test]
    fn test_memory_address_ordering() {
        let addr1 = MemoryAddress::new(0x1000);
        let addr2 = MemoryAddress::new(0x2000);

        assert!(addr1 < addr2);
        assert!(addr2 > addr1);
    }

    // Register tests
    #[test]
    fn test_register_creation() {
        let reg = Register::new(0xDEADBEEF);
        assert_eq!(reg.value(), 0xDEADBEEF);
    }

    #[test]
    fn test_register_set() {
        let mut reg = Register::new(0);
        reg.set(0x12345678);
        assert_eq!(reg.value(), 0x12345678);
    }

    #[test]
    fn test_register_equality() {
        let r1 = Register::new(42);
        let r2 = Register::new(42);
        let r3 = Register::new(43);

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    // Instruction tests
    #[test]
    fn test_instruction_creation() {
        let instr = Instruction::new(0x1234);
        assert_eq!(instr.encoding(), 0x1234);
    }

    #[test]
    fn test_instruction_opcode_extraction() {
        let instr = Instruction::new(0xABCD);
        assert_eq!(instr.opcode(), 0xA);
    }

    #[test]
    fn test_instruction_register_extraction() {
        let instr = Instruction::new(0x1B34);
        assert_eq!(instr.register(), 0xB);
    }

    #[test]
    fn test_instruction_immediate_extraction() {
        let instr = Instruction::new(0x12CD);
        assert_eq!(instr.immediate(), 0xCD);
    }

    #[test]
    fn test_instruction_field_combination() {
        // Opcode=0xF, Register=0x3, Immediate=0x42
        let instr = Instruction::new(0xF342);
        assert_eq!(instr.opcode(), 0xF);
        assert_eq!(instr.register(), 0x3);
        assert_eq!(instr.immediate(), 0x42);
    }

    // Serialization tests
    #[test]
    fn test_types_serialization() {
        let tile = TileId::new(42).unwrap();
        let json = serde_json::to_string(&tile).unwrap();
        let decoded: TileId = serde_json::from_str(&json).unwrap();
        assert_eq!(tile, decoded);
    }
}
