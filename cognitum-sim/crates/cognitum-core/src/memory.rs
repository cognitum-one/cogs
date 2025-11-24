// Memory system for Cognitum ASIC
use crate::error::CognitumError;
use crate::types::MemoryAddress;
use crate::Result;

/// Memory trait for read/write operations
pub trait Memory {
    /// Read a 32-bit word from memory
    fn read(&self, addr: MemoryAddress) -> Result<u32>;

    /// Write a 32-bit word to memory
    fn write(&mut self, addr: MemoryAddress, data: u32) -> Result<()>;

    /// Get the base address of this memory region
    fn base(&self) -> MemoryAddress;

    /// Get the size of this memory region in bytes
    fn size(&self) -> usize;

    /// Check if an address is within this memory region
    fn contains(&self, addr: MemoryAddress) -> bool {
        let offset = addr.value().wrapping_sub(self.base().value());
        offset < self.size() as u32
    }
}

/// Simple RAM implementation with bounds checking
#[derive(Debug)]
pub struct RAM {
    data: Vec<u32>,
    base: MemoryAddress,
}

impl RAM {
    /// Create a new RAM with given size (in 32-bit words)
    pub fn new(base: MemoryAddress, size_words: usize) -> Self {
        RAM {
            data: vec![0; size_words],
            base,
        }
    }

    /// Create RAM from existing data
    pub fn from_data(base: MemoryAddress, data: Vec<u32>) -> Self {
        RAM { data, base }
    }

    /// Get word index from address
    fn word_index(&self, addr: MemoryAddress) -> Result<usize> {
        // Check alignment
        if !addr.is_aligned(4) {
            return Err(CognitumError::UnalignedAccess {
                address: addr.value(),
                alignment: 4,
            });
        }

        // Check bounds
        let offset = addr.value().wrapping_sub(self.base.value());
        let word_offset = (offset / 4) as usize;

        if word_offset >= self.data.len() {
            return Err(CognitumError::AddressOutOfBounds {
                address: addr.value(),
                max: self.base.value() + (self.data.len() * 4) as u32 - 1,
            });
        }

        Ok(word_offset)
    }

    /// Clear all memory to zero
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Get slice of memory data
    pub fn data(&self) -> &[u32] {
        &self.data
    }
}

impl Memory for RAM {
    fn read(&self, addr: MemoryAddress) -> Result<u32> {
        let idx = self.word_index(addr)?;
        Ok(self.data[idx])
    }

    fn write(&mut self, addr: MemoryAddress, data: u32) -> Result<()> {
        let idx = self.word_index(addr)?;
        self.data[idx] = data;
        Ok(())
    }

    fn base(&self) -> MemoryAddress {
        self.base
    }

    fn size(&self) -> usize {
        self.data.len() * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_creation() {
        let ram = RAM::new(MemoryAddress::new(0x1000), 256);
        assert_eq!(ram.base().value(), 0x1000);
        assert_eq!(ram.size(), 1024); // 256 words * 4 bytes
        assert_eq!(ram.data.len(), 256);
    }

    #[test]
    fn test_ram_from_data() {
        let data = vec![1, 2, 3, 4];
        let ram = RAM::from_data(MemoryAddress::new(0x2000), data);
        assert_eq!(ram.data.len(), 4);
        assert_eq!(ram.base().value(), 0x2000);
    }

    #[test]
    fn test_ram_read_write_success() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);

        // Write and read back
        let addr = MemoryAddress::new(0x1000);
        ram.write(addr, 0xDEADBEEF).unwrap();
        assert_eq!(ram.read(addr).unwrap(), 0xDEADBEEF);

        // Write to different address
        let addr2 = MemoryAddress::new(0x1004);
        ram.write(addr2, 0x12345678).unwrap();
        assert_eq!(ram.read(addr2).unwrap(), 0x12345678);

        // First address should still have old value
        assert_eq!(ram.read(addr).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_ram_unaligned_read() {
        let ram = RAM::new(MemoryAddress::new(0x1000), 256);
        let addr = MemoryAddress::new(0x1001); // Unaligned

        match ram.read(addr) {
            Err(CognitumError::UnalignedAccess { address, alignment }) => {
                assert_eq!(address, 0x1001);
                assert_eq!(alignment, 4);
            }
            _ => panic!("Expected UnalignedAccess error"),
        }
    }

    #[test]
    fn test_ram_unaligned_write() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);
        let addr = MemoryAddress::new(0x1002); // Unaligned

        match ram.write(addr, 0) {
            Err(CognitumError::UnalignedAccess { .. }) => {}
            _ => panic!("Expected UnalignedAccess error"),
        }
    }

    #[test]
    fn test_ram_out_of_bounds_read() {
        let ram = RAM::new(MemoryAddress::new(0x1000), 256);
        let addr = MemoryAddress::new(0x2000); // Out of bounds

        match ram.read(addr) {
            Err(CognitumError::AddressOutOfBounds { address, max }) => {
                assert_eq!(address, 0x2000);
                assert_eq!(max, 0x13FF); // 0x1000 + 1024 - 1
            }
            _ => panic!("Expected AddressOutOfBounds error"),
        }
    }

    #[test]
    fn test_ram_out_of_bounds_write() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);
        let addr = MemoryAddress::new(0x1400); // Out of bounds

        assert!(ram.write(addr, 0).is_err());
    }

    #[test]
    fn test_ram_boundary_addresses() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);

        // First address
        let first = MemoryAddress::new(0x1000);
        assert!(ram.write(first, 0xAAAA).is_ok());
        assert_eq!(ram.read(first).unwrap(), 0xAAAA);

        // Last valid address
        let last = MemoryAddress::new(0x13FC);
        assert!(ram.write(last, 0xBBBB).is_ok());
        assert_eq!(ram.read(last).unwrap(), 0xBBBB);

        // Just beyond last address
        let beyond = MemoryAddress::new(0x1400);
        assert!(ram.write(beyond, 0).is_err());
    }

    #[test]
    fn test_ram_contains() {
        let ram = RAM::new(MemoryAddress::new(0x1000), 256);

        assert!(ram.contains(MemoryAddress::new(0x1000)));
        assert!(ram.contains(MemoryAddress::new(0x1100)));
        assert!(ram.contains(MemoryAddress::new(0x13FF)));
        assert!(!ram.contains(MemoryAddress::new(0x0FFF)));
        assert!(!ram.contains(MemoryAddress::new(0x1400)));
    }

    #[test]
    fn test_ram_clear() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);

        // Write some data
        ram.write(MemoryAddress::new(0x1000), 0xAAAA).unwrap();
        ram.write(MemoryAddress::new(0x1004), 0xBBBB).unwrap();

        // Clear
        ram.clear();

        // Verify all zero
        assert_eq!(ram.read(MemoryAddress::new(0x1000)).unwrap(), 0);
        assert_eq!(ram.read(MemoryAddress::new(0x1004)).unwrap(), 0);
    }

    #[test]
    fn test_ram_data_access() {
        let mut ram = RAM::new(MemoryAddress::new(0x1000), 4);

        ram.write(MemoryAddress::new(0x1000), 1).unwrap();
        ram.write(MemoryAddress::new(0x1004), 2).unwrap();
        ram.write(MemoryAddress::new(0x1008), 3).unwrap();
        ram.write(MemoryAddress::new(0x100C), 4).unwrap();

        let data = ram.data();
        assert_eq!(data, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_memory_trait_base_and_size() {
        let ram: Box<dyn Memory> = Box::new(RAM::new(MemoryAddress::new(0x8000), 512));

        assert_eq!(ram.base().value(), 0x8000);
        assert_eq!(ram.size(), 2048); // 512 * 4
    }

    #[test]
    fn test_ram_multiple_operations() {
        let mut ram = RAM::new(MemoryAddress::new(0x10000), 1024);

        // Sequential writes
        for i in 0..10 {
            let addr = MemoryAddress::new(0x10000 + i * 4);
            ram.write(addr, i).unwrap();
        }

        // Sequential reads
        for i in 0..10 {
            let addr = MemoryAddress::new(0x10000 + i * 4);
            assert_eq!(ram.read(addr).unwrap(), i);
        }
    }

    // Property-based tests
    #[cfg(test)]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_aligned_addresses_work(offset in 0u32..256u32) {
                let mut ram = RAM::new(MemoryAddress::new(0x1000), 256);
                let addr = MemoryAddress::new(0x1000 + offset * 4);
                let data = offset;

                prop_assert!(ram.write(addr, data).is_ok());
                prop_assert_eq!(ram.read(addr).unwrap(), data);
            }

            #[test]
            fn test_unaligned_addresses_fail(offset in 1u32..4u32) {
                let ram = RAM::new(MemoryAddress::new(0x1000), 256);
                let addr = MemoryAddress::new(0x1000 + offset);

                prop_assert!(ram.read(addr).is_err());
            }

            #[test]
            fn test_out_of_bounds_addresses_fail(offset in 1024u32..2048u32) {
                let ram = RAM::new(MemoryAddress::new(0x1000), 256);
                let addr = MemoryAddress::new(0x1000 + offset);

                prop_assert!(ram.read(addr).is_err());
            }
        }
    }
}
