//! Test utilities and helpers for Newport ASIC simulator tests

use newport_core::*;
use std::collections::HashMap;

/// Builder pattern for creating test memory instances
pub struct TestMemoryBuilder {
    size: usize,
    initial_data: HashMap<u32, u8>,
    read_only: bool,
}

impl TestMemoryBuilder {
    pub fn new() -> Self {
        TestMemoryBuilder {
            size: 1024 * 1024,
            initial_data: HashMap::new(),
            read_only: false,
        }
    }

    pub fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn with_data(mut self, address: u32, data: &[u8]) -> Self {
        for (i, &byte) in data.iter().enumerate() {
            self.initial_data.insert(address + i as u32, byte);
        }
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn build(self) -> TestMemory {
        TestMemory {
            data: self.initial_data,
            size: self.size,
            read_only: self.read_only,
            access_log: Vec::new(),
        }
    }
}

impl Default for TestMemoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test memory implementation with access logging
pub struct TestMemory {
    data: HashMap<u32, u8>,
    size: usize,
    read_only: bool,
    access_log: Vec<MemoryAccess>,
}

#[derive(Debug, Clone)]
pub enum MemoryAccess {
    Read { address: u32, value: u8 },
    Write { address: u32, value: u8 },
}

impl TestMemory {
    pub fn new() -> Self {
        TestMemoryBuilder::new().build()
    }

    pub fn with_size(size: usize) -> Self {
        TestMemoryBuilder::new().with_size(size).build()
    }

    pub fn get_access_log(&self) -> &[MemoryAccess] {
        &self.access_log
    }

    pub fn clear_log(&mut self) {
        self.access_log.clear();
    }

    pub fn dump_range(&self, start: u32, end: u32) -> Vec<u8> {
        (start..=end)
            .map(|addr| *self.data.get(&addr).unwrap_or(&0))
            .collect()
    }
}

impl Default for TestMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory for TestMemory {
    fn read_byte(&self, addr: MemoryAddress) -> Result<u8> {
        let address = addr.value();
        if address as usize >= self.size {
            return Err(NewportError::InvalidAddress(address));
        }

        let value = *self.data.get(&address).unwrap_or(&0);
        Ok(value)
    }

    fn write_byte(&mut self, addr: MemoryAddress, value: u8) -> Result<()> {
        let address = addr.value();
        if address as usize >= self.size {
            return Err(NewportError::InvalidAddress(address));
        }

        if self.read_only {
            return Err(NewportError::MemoryAccessViolation(address));
        }

        self.access_log.push(MemoryAccess::Write { address, value });
        self.data.insert(address, value);
        Ok(())
    }

    fn size(&self) -> usize {
        self.size
    }
}

/// Helper for creating test packets
pub struct PacketBuilder {
    source: Option<TileId>,
    dest: Option<TileId>,
    packet_type: PacketType,
    data: Vec<u8>,
}

impl PacketBuilder {
    pub fn new() -> Self {
        PacketBuilder {
            source: None,
            dest: None,
            packet_type: PacketType::Data,
            data: Vec::new(),
        }
    }

    pub fn source(mut self, id: u8) -> Self {
        self.source = Some(TileId::new(id as u16).unwrap());
        self
    }

    pub fn dest(mut self, id: u8) -> Self {
        self.dest = Some(TileId::new(id as u16).unwrap());
        self
    }

    pub fn packet_type(mut self, ptype: PacketType) -> Self {
        self.packet_type = ptype;
        self
    }

    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    pub fn build(self) -> Result<RaceWayPacket> {
        let source = self.source.ok_or(NewportError::InvalidPacket("No source".to_string()))?;
        let dest = self.dest.ok_or(NewportError::InvalidPacket("No dest".to_string()))?;

        Ok(RaceWayPacket::new(source, dest, self.packet_type, self.data))
    }
}

impl Default for PacketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate test data patterns
pub mod patterns {
    pub fn sequential(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i & 0xFF) as u8).collect()
    }

    pub fn alternating(len: usize) -> Vec<u8> {
        (0..len).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }).collect()
    }

    pub fn random_seeded(len: usize, seed: u64) -> Vec<u8> {
        // Simple LCG random number generator
        let mut rng = seed;
        (0..len).map(|_| {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            (rng >> 16) as u8
        }).collect()
    }

    pub fn zeros(len: usize) -> Vec<u8> {
        vec![0; len]
    }

    pub fn ones(len: usize) -> Vec<u8> {
        vec![0xFF; len]
    }
}

/// Assertion helpers
pub mod assertions {
    use super::*;

    pub fn assert_memory_contains(mem: &TestMemory, addr: u32, expected: &[u8]) {
        let actual = mem.dump_range(addr, addr + expected.len() as u32 - 1);
        assert_eq!(actual, expected,
                  "Memory mismatch at address 0x{:08x}", addr);
    }

    pub fn assert_packet_valid(packet: &RaceWayPacket) {
        assert!(packet.source.value() < 256, "Invalid source tile");
        assert!(packet.dest.value() < 256, "Invalid dest tile");
        assert!(!packet.data.is_empty(), "Packet has no data");
    }

    pub fn assert_tiles_different(t1: TileId, t2: TileId) {
        assert_ne!(t1, t2, "Tiles should be different");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_builder() {
        let mem = TestMemoryBuilder::new()
            .with_size(256)
            .with_data(0x100, &[0xAA, 0xBB, 0xCC])
            .build();

        assert_eq!(mem.size(), 256);
        assert_eq!(mem.read_byte(MemoryAddress::new(0x100)).unwrap(), 0xAA);
        assert_eq!(mem.read_byte(MemoryAddress::new(0x101)).unwrap(), 0xBB);
        assert_eq!(mem.read_byte(MemoryAddress::new(0x102)).unwrap(), 0xCC);
    }

    #[test]
    fn test_read_only_memory() {
        let mut mem = TestMemoryBuilder::new()
            .read_only()
            .build();

        let addr = MemoryAddress::new(0x100);
        assert!(mem.write_byte(addr, 0xAA).is_err());
    }

    #[test]
    fn test_packet_builder() {
        let packet = PacketBuilder::new()
            .source(0)
            .dest(255)
            .data(vec![0x11, 0x22])
            .build()
            .unwrap();

        assert_eq!(packet.source.value(), 0);
        assert_eq!(packet.dest.value(), 255);
        assert_eq!(packet.data, vec![0x11, 0x22]);
    }

    #[test]
    fn test_patterns_sequential() {
        let data = patterns::sequential(10);
        assert_eq!(data, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_patterns_alternating() {
        let data = patterns::alternating(4);
        assert_eq!(data, vec![0xAA, 0x55, 0xAA, 0x55]);
    }

    #[test]
    fn test_patterns_random_seeded() {
        let data1 = patterns::random_seeded(100, 42);
        let data2 = patterns::random_seeded(100, 42);
        assert_eq!(data1, data2); // Same seed = same sequence
    }

    #[test]
    fn test_memory_access_logging() {
        let mut mem = TestMemory::new();

        mem.write_byte(MemoryAddress::new(0x100), 0xAA).unwrap();
        mem.write_byte(MemoryAddress::new(0x101), 0xBB).unwrap();

        let log = mem.get_access_log();
        assert_eq!(log.len(), 2);
    }
}
