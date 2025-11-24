use crate::error::{ProcessorError, Result};
use std::collections::HashMap;

/// Memory trait for dependency injection
pub trait Memory {
    fn read(&self, addr: u32) -> Result<i32>;
    fn write(&mut self, addr: u32, value: i32) -> Result<()>;
    fn atomic_swap(&mut self, addr: u32, value: i32) -> Result<i32>;
}

/// Simple memory implementation using HashMap for sparse storage
pub struct SimpleMemory {
    data: HashMap<u32, i32>,
    max_addr: u32,
}

impl SimpleMemory {
    pub fn new(max_addr: u32) -> Self {
        Self {
            data: HashMap::new(),
            max_addr,
        }
    }

    pub fn with_default_size() -> Self {
        Self::new(0xFFFF_FFFF) // Full 32-bit address space
    }

    fn validate_address(&self, addr: u32) -> Result<()> {
        if addr > self.max_addr || (addr & 0x3) != 0 {
            // Check alignment to 4-byte boundary
            Err(ProcessorError::InvalidMemoryAddress(addr))
        } else {
            Ok(())
        }
    }
}

impl Memory for SimpleMemory {
    fn read(&self, addr: u32) -> Result<i32> {
        self.validate_address(addr)?;
        Ok(*self.data.get(&addr).unwrap_or(&0))
    }

    fn write(&mut self, addr: u32, value: i32) -> Result<()> {
        self.validate_address(addr)?;
        self.data.insert(addr, value);
        Ok(())
    }

    fn atomic_swap(&mut self, addr: u32, value: i32) -> Result<i32> {
        self.validate_address(addr)?;
        let old_value = *self.data.get(&addr).unwrap_or(&0);
        self.data.insert(addr, value);
        Ok(old_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_read_write() {
        let mut mem = SimpleMemory::with_default_size();
        mem.write(0x1000, 42).unwrap();
        assert_eq!(mem.read(0x1000).unwrap(), 42);
    }

    #[test]
    fn test_memory_uninitialized_reads_zero() {
        let mem = SimpleMemory::with_default_size();
        assert_eq!(mem.read(0x1000).unwrap(), 0);
    }

    #[test]
    fn test_memory_alignment() {
        let mem = SimpleMemory::with_default_size();
        assert!(mem.read(0x1001).is_err()); // Unaligned
        assert!(mem.read(0x1000).is_ok()); // Aligned
    }

    #[test]
    fn test_atomic_swap() {
        let mut mem = SimpleMemory::with_default_size();
        mem.write(0x1000, 10).unwrap();
        let old = mem.atomic_swap(0x1000, 20).unwrap();
        assert_eq!(old, 10);
        assert_eq!(mem.read(0x1000).unwrap(), 20);
    }
}
