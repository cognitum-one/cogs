//! WASM linear memory with bounds checking
//!
//! Implements the 8KB code + 8KB data + 64KB work memory architecture
//! with WASM-standard page management (64KB pages).

use crate::error::{Result, WasmSimError, WasmTrap};

/// WASM linear memory manager
pub struct WasmMemory {
    /// Code memory (8KB default - 4K x 16-bit instructions)
    code: Vec<u8>,

    /// Data memory (8KB default - 2K x 32-bit words)
    data: Vec<u8>,

    /// Work memory (64KB default - 16K x 32-bit words)
    work: Vec<u8>,

    /// WASM linear memory (page-based)
    linear: Vec<u8>,

    /// Current number of WASM pages
    current_pages: u32,

    /// Maximum WASM pages
    max_pages: u32,

    /// Page size (64KB)
    page_size: usize,

    /// Memory access statistics
    stats: MemoryStats,
}

/// Memory access statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub code_reads: u64,
    pub data_reads: u64,
    pub data_writes: u64,
    pub work_reads: u64,
    pub work_writes: u64,
    pub linear_reads: u64,
    pub linear_writes: u64,
    pub bounds_errors: u64,
}

impl WasmMemory {
    /// Create new memory with specified sizes
    pub fn new(
        code_size: usize,
        data_size: usize,
        work_size: usize,
        initial_pages: u32,
        max_pages: u32,
    ) -> Result<Self> {
        let page_size = 64 * 1024; // 64KB WASM page

        Ok(Self {
            code: vec![0; code_size],
            data: vec![0; data_size],
            work: vec![0; work_size],
            linear: vec![0; initial_pages as usize * page_size],
            current_pages: initial_pages,
            max_pages,
            page_size,
            stats: MemoryStats::default(),
        })
    }

    /// Load code into code memory
    pub fn load_code(&mut self, bytecode: &[u8]) -> Result<()> {
        if bytecode.len() > self.code.len() {
            return Err(WasmSimError::MemoryOutOfBounds {
                address: 0,
                size: bytecode.len() as u32,
            });
        }
        self.code[..bytecode.len()].copy_from_slice(bytecode);
        Ok(())
    }

    /// Read byte from code memory
    pub fn read_code(&self, addr: u32) -> Result<u8> {
        let idx = addr as usize;
        if idx >= self.code.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        Ok(self.code[idx])
    }

    /// Read 16-bit instruction from code memory
    pub fn read_code_u16(&self, addr: u32) -> Result<u16> {
        let idx = addr as usize;
        if idx + 1 >= self.code.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        Ok(u16::from_le_bytes([self.code[idx], self.code[idx + 1]]))
    }

    /// Read 32-bit value from data memory
    pub fn read_data(&self, addr: u32) -> Result<i32> {
        let idx = addr as usize;
        if idx + 3 >= self.data.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        Ok(i32::from_le_bytes([
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]))
    }

    /// Write 32-bit value to data memory
    pub fn write_data(&mut self, addr: u32, value: i32) -> Result<()> {
        let idx = addr as usize;
        if idx + 3 >= self.data.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        let bytes = value.to_le_bytes();
        self.data[idx..idx + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Read from work memory
    pub fn read_work(&self, addr: u32) -> Result<i32> {
        let idx = addr as usize;
        if idx + 3 >= self.work.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        Ok(i32::from_le_bytes([
            self.work[idx],
            self.work[idx + 1],
            self.work[idx + 2],
            self.work[idx + 3],
        ]))
    }

    /// Write to work memory
    pub fn write_work(&mut self, addr: u32, value: i32) -> Result<()> {
        let idx = addr as usize;
        if idx + 3 >= self.work.len() {
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }
        let bytes = value.to_le_bytes();
        self.work[idx..idx + 4].copy_from_slice(&bytes);
        Ok(())
    }

    // ===== WASM Linear Memory Operations =====

    /// Read i32 from linear memory (WASM i32.load)
    pub fn load_i32(&mut self, addr: u32, offset: u32) -> Result<i32> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 3 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;

        Ok(i32::from_le_bytes([
            self.linear[effective_addr],
            self.linear[effective_addr + 1],
            self.linear[effective_addr + 2],
            self.linear[effective_addr + 3],
        ]))
    }

    /// Store i32 to linear memory (WASM i32.store)
    pub fn store_i32(&mut self, addr: u32, offset: u32, value: i32) -> Result<()> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 3 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_writes += 1;

        let bytes = value.to_le_bytes();
        self.linear[effective_addr..effective_addr + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Load byte (signed) from linear memory
    pub fn load_i8_s(&mut self, addr: u32, offset: u32) -> Result<i32> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;
        Ok(self.linear[effective_addr] as i8 as i32)
    }

    /// Load byte (unsigned) from linear memory
    pub fn load_i8_u(&mut self, addr: u32, offset: u32) -> Result<i32> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;
        Ok(self.linear[effective_addr] as i32)
    }

    /// Load 16-bit (signed) from linear memory
    pub fn load_i16_s(&mut self, addr: u32, offset: u32) -> Result<i32> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 1 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;
        let val = i16::from_le_bytes([
            self.linear[effective_addr],
            self.linear[effective_addr + 1],
        ]);
        Ok(val as i32)
    }

    /// Load 16-bit (unsigned) from linear memory
    pub fn load_i16_u(&mut self, addr: u32, offset: u32) -> Result<i32> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 1 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;
        let val = u16::from_le_bytes([
            self.linear[effective_addr],
            self.linear[effective_addr + 1],
        ]);
        Ok(val as i32)
    }

    /// Store byte to linear memory
    pub fn store_i8(&mut self, addr: u32, offset: u32, value: i32) -> Result<()> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_writes += 1;
        self.linear[effective_addr] = value as u8;
        Ok(())
    }

    /// Store 16-bit to linear memory
    pub fn store_i16(&mut self, addr: u32, offset: u32, value: i32) -> Result<()> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 1 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_writes += 1;
        let bytes = (value as i16).to_le_bytes();
        self.linear[effective_addr..effective_addr + 2].copy_from_slice(&bytes);
        Ok(())
    }

    /// Load v128 from linear memory (for SIMD)
    pub fn load_v128(&mut self, addr: u32, offset: u32) -> Result<[i32; 4]> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 15 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_reads += 1;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let base = effective_addr + i * 4;
            result[i] = i32::from_le_bytes([
                self.linear[base],
                self.linear[base + 1],
                self.linear[base + 2],
                self.linear[base + 3],
            ]);
        }

        Ok(result)
    }

    /// Store v128 to linear memory (for SIMD)
    pub fn store_v128(&mut self, addr: u32, offset: u32, value: [i32; 4]) -> Result<()> {
        let effective_addr = (addr.wrapping_add(offset)) as usize;

        if effective_addr + 15 >= self.linear.len() {
            self.stats.bounds_errors += 1;
            return Err(WasmSimError::Trap(WasmTrap::MemoryAccessError));
        }

        self.stats.linear_writes += 1;

        for i in 0..4 {
            let base = effective_addr + i * 4;
            let bytes = value[i].to_le_bytes();
            self.linear[base..base + 4].copy_from_slice(&bytes);
        }

        Ok(())
    }

    /// Get current memory size in pages (memory.size)
    pub fn size(&self) -> u32 {
        self.current_pages
    }

    /// Grow memory by delta pages (memory.grow)
    /// Returns previous size or -1 on failure
    pub fn grow(&mut self, delta: u32) -> i32 {
        let new_pages = self.current_pages.saturating_add(delta);

        if new_pages > self.max_pages {
            return -1;
        }

        let new_size = new_pages as usize * self.page_size;
        self.linear.resize(new_size, 0);

        let prev_pages = self.current_pages;
        self.current_pages = new_pages;

        prev_pages as i32
    }

    /// Get memory statistics
    pub fn stats(&self) -> &MemoryStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = MemoryStats::default();
    }

    /// Get code memory size
    pub fn code_size(&self) -> usize {
        self.code.len()
    }

    /// Get data memory size
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    /// Get work memory size
    pub fn work_size(&self) -> usize {
        self.work.len()
    }

    /// Get linear memory size in bytes
    pub fn linear_size(&self) -> usize {
        self.linear.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();
        assert_eq!(mem.code_size(), 8192);
        assert_eq!(mem.data_size(), 8192);
        assert_eq!(mem.work_size(), 65536);
        assert_eq!(mem.size(), 1);
    }

    #[test]
    fn test_load_store() {
        let mut mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();

        mem.store_i32(0, 0, 0x12345678).unwrap();
        assert_eq!(mem.load_i32(0, 0).unwrap(), 0x12345678);
    }

    #[test]
    fn test_bounds_checking() {
        let mut mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();

        // Try to access beyond bounds
        let result = mem.load_i32(0xFFFFFF, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_grow() {
        let mut mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();

        let prev = mem.grow(1);
        assert_eq!(prev, 1);
        assert_eq!(mem.size(), 2);

        // Try to grow beyond max
        let result = mem.grow(300);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_byte_access() {
        let mut mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();

        mem.store_i8(0, 0, 0xFF).unwrap();
        assert_eq!(mem.load_i8_u(0, 0).unwrap(), 255);
        assert_eq!(mem.load_i8_s(0, 0).unwrap(), -1);
    }

    #[test]
    fn test_v128() {
        let mut mem = WasmMemory::new(8192, 8192, 65536, 1, 256).unwrap();

        let value = [1, 2, 3, 4];
        mem.store_v128(0, 0, value).unwrap();

        let loaded = mem.load_v128(0, 0).unwrap();
        assert_eq!(loaded, value);
    }
}
