//! DRAM simulation

use cognitum_core::{MemoryAddress, Result};

type PhysAddr = MemoryAddress;

/// DRAM controller
pub struct Dram {
    /// Memory size in bytes
    _size: usize,
    /// Backing storage
    memory: Vec<u8>,
}

impl Dram {
    /// Create a new DRAM
    pub fn new(size: usize) -> Self {
        Self {
            _size: size,
            memory: vec![0; size],
        }
    }

    /// Read from DRAM
    pub fn read(&self, addr: PhysAddr, len: usize) -> Result<&[u8]> {
        let start = addr.value() as usize;
        let end = start + len;
        Ok(&self.memory[start..end])
    }

    /// Write to DRAM
    pub fn write(&mut self, addr: PhysAddr, data: &[u8]) -> Result<()> {
        let start = addr.value() as usize;
        let end = start + data.len();
        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }
}
