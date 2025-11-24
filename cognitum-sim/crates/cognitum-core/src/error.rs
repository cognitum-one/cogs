// Error types for Cognitum Core
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CognitumError {
    #[error("Invalid address: {0:#010x}")]
    InvalidAddress(u32),

    #[error("Invalid tile ID: {0} (must be 0-255)")]
    InvalidTileId(u16),

    #[error("Address out of bounds: {address:#010x} (max: {max:#010x})")]
    AddressOutOfBounds { address: u32, max: u32 },

    #[error("Memory access violation at address {0:#010x}")]
    MemoryAccessViolation(u32),

    #[error(
        "Unaligned memory access: address {address:#010x} requires {alignment}-byte alignment"
    )]
    UnalignedAccess { address: u32, alignment: u32 },

    #[error("Memory region overlap: base={base:#010x}, size={size}")]
    MemoryRegionOverlap { base: u32, size: usize },

    #[error("Invalid instruction encoding: {0:#06x}")]
    InvalidInstruction(u16),

    #[error("Register index out of range: {0}")]
    InvalidRegister(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let err = CognitumError::InvalidAddress(0x1234);
        assert_eq!(err.to_string(), "Invalid address: 0x00001234");

        let err = CognitumError::InvalidTileId(256);
        assert!(err.to_string().contains("must be 0-255"));

        let err = CognitumError::AddressOutOfBounds {
            address: 0x10000,
            max: 0xFFFF,
        };
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_error_equality() {
        let err1 = CognitumError::InvalidAddress(0x1234);
        let err2 = CognitumError::InvalidAddress(0x1234);
        let err3 = CognitumError::InvalidAddress(0x5678);

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_error_clone() {
        let err1 = CognitumError::MemoryAccessViolation(0xDEAD);
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_unaligned_access_error() {
        let err = CognitumError::UnalignedAccess {
            address: 0x1001,
            alignment: 4,
        };
        assert!(err.to_string().contains("Unaligned memory access"));
        assert!(err.to_string().contains("4-byte alignment"));
    }

    #[test]
    fn test_memory_region_overlap_error() {
        let err = CognitumError::MemoryRegionOverlap {
            base: 0x10000,
            size: 0x1000,
        };
        assert!(err.to_string().contains("overlap"));
    }

    #[test]
    fn test_invalid_instruction_error() {
        let err = CognitumError::InvalidInstruction(0xFFFF);
        assert!(err.to_string().contains("0xffff"));
    }

    #[test]
    fn test_invalid_register_error() {
        let err = CognitumError::InvalidRegister(42);
        assert!(err.to_string().contains("42"));
    }
}
