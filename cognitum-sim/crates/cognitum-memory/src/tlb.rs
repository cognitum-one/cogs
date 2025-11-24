//! TLB (Translation Lookaside Buffer) implementation

use cognitum_core::{MemoryAddress, Result};

type PhysAddr = MemoryAddress;
type VirtAddr = MemoryAddress;

/// TLB entry
#[derive(Debug, Clone)]
pub struct TlbEntry {
    /// Virtual address
    pub virt: VirtAddr,
    /// Physical address
    pub phys: PhysAddr,
    /// Valid bit
    pub valid: bool,
}

/// TLB implementation
pub struct Tlb {
    /// TLB entries
    _entries: Vec<TlbEntry>,
}

impl Tlb {
    /// Create a new TLB
    pub fn new(size: usize) -> Self {
        Self {
            _entries: vec![
                TlbEntry {
                    virt: MemoryAddress::new(0),
                    phys: MemoryAddress::new(0),
                    valid: false
                };
                size
            ],
        }
    }

    /// Translate virtual address to physical
    pub fn translate(&self, _virt: VirtAddr) -> Result<Option<PhysAddr>> {
        // TODO: Implement TLB lookup
        Ok(None)
    }
}
