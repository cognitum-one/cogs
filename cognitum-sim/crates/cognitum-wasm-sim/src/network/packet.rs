//! Network packet definitions
//!
//! Supports multiple packet formats for different topologies

use serde::{Deserialize, Serialize};

/// Network packet (compatible with RaceWay and datacenter formats)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    /// Packet validity
    pub valid: bool,

    /// Source node ID
    pub source: u16,

    /// Destination node ID
    pub destination: u16,

    /// Command/operation type
    pub command: PacketCommand,

    /// Transaction tag (for request/response matching)
    pub tag: u8,

    /// Payload data (up to 128 bits / 16 bytes)
    pub payload: [u8; 16],

    /// Payload length
    pub payload_len: usize,

    /// Priority level (0 = highest)
    pub priority: Priority,

    /// Quality of Service class
    pub qos: QoS,

    /// Timestamp (simulation cycles)
    pub timestamp: u64,

    /// Hop count (for routing)
    pub hops: u8,

    /// Maximum hops (TTL)
    pub max_hops: u8,
}

/// Packet command types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketCommand {
    // ===== RaceWay Commands =====
    /// Write data to address
    Write = 0x91,

    /// Read data from address
    Read = 0x89,

    /// Atomic add operation
    AtomicAdd = 0x92,

    /// Atomic swap operation
    AtomicSwap = 0x93,

    /// Column broadcast
    Broadcast = 0xB1,

    /// Global barrier sync
    BarrierSync = 0xA0,

    /// Multicast to subset
    Multicast = 0xB8,

    // ===== Response Commands =====
    /// Success acknowledgement
    Success = 0xC1,

    /// Read data response
    ReadData = 0xC9,

    /// Error response
    Error = 0xE1,

    // ===== Datacenter Extensions =====
    /// Flow control credit
    Credit = 0xD0,

    /// Heartbeat/keepalive
    Heartbeat = 0xD1,

    /// Congestion notification
    Congestion = 0xD2,

    /// Route discovery
    RouteDiscovery = 0xD3,

    /// Storage read (Nutanix)
    StorageRead = 0xF0,

    /// Storage write (Nutanix)
    StorageWrite = 0xF1,

    /// Replication sync (Nutanix)
    ReplicationSync = 0xF2,
}

/// Priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    /// Critical (real-time control)
    Critical = 0,

    /// High (command/response)
    High = 1,

    /// Normal (default traffic)
    Normal = 2,

    /// Low (best-effort, prefetch)
    Low = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Quality of Service classes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum QoS {
    /// Best effort (no guarantees)
    BestEffort = 0,

    /// Low latency (prioritize speed)
    LowLatency = 1,

    /// High throughput (prioritize bandwidth)
    HighThroughput = 2,

    /// Reliable (guaranteed delivery)
    Reliable = 3,

    /// Real-time (deadline-aware)
    RealTime = 4,
}

impl Default for QoS {
    fn default() -> Self {
        QoS::BestEffort
    }
}

impl Packet {
    /// Create a new packet
    pub fn new(source: u16, destination: u16, command: PacketCommand) -> Self {
        Self {
            valid: true,
            source,
            destination,
            command,
            tag: 0,
            payload: [0; 16],
            payload_len: 0,
            priority: Priority::Normal,
            qos: QoS::BestEffort,
            timestamp: 0,
            hops: 0,
            max_hops: 64,
        }
    }

    /// Create write packet
    pub fn write(source: u16, destination: u16, address: u32, data: u32) -> Self {
        let mut pkt = Self::new(source, destination, PacketCommand::Write);
        pkt.set_address(address);
        pkt.set_data(data);
        pkt
    }

    /// Create read packet
    pub fn read(source: u16, destination: u16, address: u32) -> Self {
        let mut pkt = Self::new(source, destination, PacketCommand::Read);
        pkt.set_address(address);
        pkt
    }

    /// Create broadcast packet
    pub fn broadcast(source: u16, data: &[u8]) -> Self {
        let mut pkt = Self::new(source, 0xFFFF, PacketCommand::Broadcast);
        pkt.set_payload(data);
        pkt
    }

    /// Set address in payload (bytes 0-3)
    pub fn set_address(&mut self, address: u32) {
        let bytes = address.to_le_bytes();
        self.payload[0..4].copy_from_slice(&bytes);
        self.payload_len = self.payload_len.max(4);
    }

    /// Get address from payload
    pub fn address(&self) -> u32 {
        u32::from_le_bytes([
            self.payload[0],
            self.payload[1],
            self.payload[2],
            self.payload[3],
        ])
    }

    /// Set data in payload (bytes 4-7)
    pub fn set_data(&mut self, data: u32) {
        let bytes = data.to_le_bytes();
        self.payload[4..8].copy_from_slice(&bytes);
        self.payload_len = self.payload_len.max(8);
    }

    /// Get data from payload
    pub fn data(&self) -> u32 {
        u32::from_le_bytes([
            self.payload[4],
            self.payload[5],
            self.payload[6],
            self.payload[7],
        ])
    }

    /// Set raw payload
    pub fn set_payload(&mut self, data: &[u8]) {
        let len = data.len().min(16);
        self.payload[..len].copy_from_slice(&data[..len]);
        self.payload_len = len;
    }

    /// Get payload slice
    pub fn payload_slice(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }

    /// Set priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set QoS
    pub fn with_qos(mut self, qos: QoS) -> Self {
        self.qos = qos;
        self
    }

    /// Set tag
    pub fn with_tag(mut self, tag: u8) -> Self {
        self.tag = tag;
        self
    }

    /// Increment hop count
    pub fn hop(&mut self) {
        self.hops = self.hops.saturating_add(1);
    }

    /// Check if packet has exceeded TTL
    pub fn expired(&self) -> bool {
        self.hops >= self.max_hops
    }

    /// Check if this is a response packet
    pub fn is_response(&self) -> bool {
        matches!(
            self.command,
            PacketCommand::Success | PacketCommand::ReadData | PacketCommand::Error
        )
    }

    /// Check if this is a broadcast/multicast
    pub fn is_broadcast(&self) -> bool {
        matches!(
            self.command,
            PacketCommand::Broadcast | PacketCommand::Multicast
        )
    }

    /// Create response packet
    pub fn response(&self, command: PacketCommand, data: Option<u32>) -> Self {
        let mut resp = Self::new(self.destination, self.source, command);
        resp.tag = self.tag;
        resp.priority = self.priority;
        resp.qos = self.qos;
        if let Some(d) = data {
            resp.set_data(d);
        }
        resp
    }

    /// Serialize to bytes (97-bit RaceWay format, padded to 128 bits)
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];

        // Bit 96: PUSH (valid)
        bytes[12] = if self.valid { 0x01 } else { 0x00 };

        // Bits 95:88: COMMAND
        bytes[11] = self.command as u8;

        // Bits 87:80: TAG
        bytes[10] = self.tag;

        // Bits 79:72: DESTINATION
        bytes[9] = self.destination as u8;

        // Bits 71:64: SOURCE
        bytes[8] = self.source as u8;

        // Bits 63:0: Payload (address + data)
        bytes[0..8].copy_from_slice(&self.payload[0..8]);

        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8; 16]) -> Self {
        let mut pkt = Self::new(0, 0, PacketCommand::Write);

        pkt.valid = bytes[12] != 0;
        pkt.command = match bytes[11] {
            0x91 => PacketCommand::Write,
            0x89 => PacketCommand::Read,
            0x92 => PacketCommand::AtomicAdd,
            0x93 => PacketCommand::AtomicSwap,
            0xB1 => PacketCommand::Broadcast,
            0xA0 => PacketCommand::BarrierSync,
            0xB8 => PacketCommand::Multicast,
            0xC1 => PacketCommand::Success,
            0xC9 => PacketCommand::ReadData,
            0xE1 => PacketCommand::Error,
            _ => PacketCommand::Write,
        };
        pkt.tag = bytes[10];
        pkt.destination = bytes[9] as u16;
        pkt.source = bytes[8] as u16;
        pkt.payload[0..8].copy_from_slice(&bytes[0..8]);
        pkt.payload_len = 8;

        pkt
    }
}

impl Default for Packet {
    fn default() -> Self {
        Self::new(0, 0, PacketCommand::Write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_creation() {
        let pkt = Packet::write(1, 2, 0x1000, 0xDEADBEEF);
        assert_eq!(pkt.source, 1);
        assert_eq!(pkt.destination, 2);
        assert_eq!(pkt.address(), 0x1000);
        assert_eq!(pkt.data(), 0xDEADBEEF);
    }

    #[test]
    fn test_serialization() {
        let pkt = Packet::write(5, 10, 0x2000, 0x12345678);
        let bytes = pkt.to_bytes();
        let restored = Packet::from_bytes(&bytes);

        assert_eq!(restored.source, 5);
        assert_eq!(restored.destination, 10);
        assert_eq!(restored.command, PacketCommand::Write);
    }

    #[test]
    fn test_response() {
        let req = Packet::read(1, 2, 0x1000).with_tag(42);
        let resp = req.response(PacketCommand::ReadData, Some(0xCAFEBABE));

        assert_eq!(resp.source, 2);
        assert_eq!(resp.destination, 1);
        assert_eq!(resp.tag, 42);
        assert_eq!(resp.data(), 0xCAFEBABE);
    }
}
