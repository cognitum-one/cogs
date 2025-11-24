//! Cache implementation

use cognitum_core::{MemoryAddress, Result};

type PhysAddr = MemoryAddress;

/// Cache line size in bytes
pub const CACHE_LINE_SIZE: usize = 64;

/// Cache implementation
pub struct Cache {
    /// Cache size in bytes
    _size: usize,
    /// Associativity
    _associativity: usize,
}

impl Cache {
    /// Create a new cache
    pub fn new(size: usize, associativity: usize) -> Self {
        Self {
            _size: size,
            _associativity: associativity,
        }
    }

    /// Read from cache
    pub fn read(&self, _addr: PhysAddr) -> Result<Option<Vec<u8>>> {
        // TODO: Implement cache lookup
        Ok(None)
    }

    /// Write to cache
    pub fn write(&mut self, _addr: PhysAddr, _data: &[u8]) -> Result<()> {
        // TODO: Implement cache write
        Ok(())
    }
}
