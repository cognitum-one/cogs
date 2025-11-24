//! RaceWay packet format implementation
//!
//! Implements the 97-bit packet structure:
//! - Bit 96: PUSH (valid)
//! - Bits 95:88: COMMAND
//! - Bits 87:80: TAG
//! - Bits 79:72: DEST
//! - Bits 71:64: SOURCE
//! - Bits 63:32: WRITE_DATA / READ_DATA0
//! - Bits 31:0: ADDRESS / READ_DATA1

use crate::error::{RaceWayError, Result};
pub use crate::tile::TileId;

/// RaceWay command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Write to memory (0x91)
    Write,
    /// Read from memory (0x89)
    Read,
    /// Atomic add operation (0x92)
    AtomicAdd,
    /// Atomic swap (0x93)
    AtomicSwap,
    /// Broadcast to all tiles in column (0xB1)
    Broadcast,
    /// Barrier synchronization (0xA0)
    BarrierSync,
    /// Multicast to specific group (0xB8)
    Multicast,
    /// Success acknowledgment (0x11)
    Success,
    /// Read data valid (0x09)
    ReadData,
    /// Operation failed (0xF0)
    Error,
    /// Broadcast acknowledged (0x60)
    BroadcastAck,
}

impl Command {
    pub fn to_u8(&self) -> u8 {
        match self {
            Command::Write => 0x91,
            Command::Read => 0x89,
            Command::AtomicAdd => 0x92,
            Command::AtomicSwap => 0x93,
            Command::Broadcast => 0xB1,
            Command::BarrierSync => 0xA0,
            Command::Multicast => 0xB8,
            Command::Success => 0x11,
            Command::ReadData => 0x09,
            Command::Error => 0xF0,
            Command::BroadcastAck => 0x60,
        }
    }

    pub fn from_u8(val: u8) -> Result<Self> {
        match val {
            0x91 => Ok(Command::Write),
            0x89 => Ok(Command::Read),
            0x92 => Ok(Command::AtomicAdd),
            0x93 => Ok(Command::AtomicSwap),
            0xB1 => Ok(Command::Broadcast),
            0xA0 => Ok(Command::BarrierSync),
            0xB8 => Ok(Command::Multicast),
            0x11 => Ok(Command::Success),
            0x09 => Ok(Command::ReadData),
            0xF0 => Ok(Command::Error),
            0x60 => Ok(Command::BroadcastAck),
            _ => Err(RaceWayError::InvalidCommand(val)),
        }
    }

    /// Check if this is a broadcast command (bit 93 set, which is bit 5 in the 8-bit command)
    /// Bit 93 in the packet = bit 5 in the command byte (counting from bit 88)
    /// Broadcast commands: 0xB1 (10110001), 0xA0 (10100000), 0xB8 (10111000)
    pub fn is_broadcast(&self) -> bool {
        let val = self.to_u8();
        // Bit 5 (0x20) set indicates broadcast
        (val & 0x20) != 0
    }
}

/// RaceWay packet builder
#[derive(Debug, Default)]
pub struct RaceWayPacketBuilder {
    source: Option<TileId>,
    dest: Option<TileId>,
    command: Option<Command>,
    tag: Option<u8>,
    data0: Option<u32>,
    data1: Option<u32>,
    push: bool,
}

impl RaceWayPacketBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(mut self, source: TileId) -> Self {
        self.source = Some(source);
        self
    }

    pub fn dest(mut self, dest: TileId) -> Self {
        self.dest = Some(dest);
        self
    }

    pub fn command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    pub fn tag(mut self, tag: u8) -> Self {
        self.tag = Some(tag);
        self
    }

    pub fn write_data(mut self, data: u32) -> Self {
        self.data0 = Some(data);
        self
    }

    pub fn address(mut self, addr: u32) -> Self {
        self.data1 = Some(addr);
        self
    }

    pub fn read_data0(mut self, data: u32) -> Self {
        self.data0 = Some(data);
        self
    }

    pub fn read_data1(mut self, data: u32) -> Self {
        self.data1 = Some(data);
        self
    }

    pub fn data(mut self, data: &[u8]) -> Self {
        if data.len() >= 4 {
            self.data0 = Some(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
        }
        if data.len() >= 8 {
            self.data1 = Some(u32::from_be_bytes([data[4], data[5], data[6], data[7]]));
        }
        self
    }

    pub fn push(mut self, push: bool) -> Self {
        self.push = push;
        self
    }

    pub fn build(self) -> Result<RaceWayPacket> {
        Ok(RaceWayPacket {
            source: self.source.unwrap_or(TileId(0)),
            dest: self.dest.unwrap_or(TileId(0)),
            command: self.command.unwrap_or(Command::Write),
            tag: self.tag.unwrap_or(0),
            data0: self.data0.unwrap_or(0),
            data1: self.data1.unwrap_or(0),
            push: self.push,
            reset_n: true,
        })
    }
}

/// RaceWay packet (97 bits)
#[derive(Debug, Clone, PartialEq)]
pub struct RaceWayPacket {
    source: TileId,
    dest: TileId,
    command: Command,
    tag: u8,
    data0: u32,
    data1: u32,
    push: bool,
    reset_n: bool,
}

impl RaceWayPacket {
    pub fn new() -> RaceWayPacketBuilder {
        RaceWayPacketBuilder::new()
    }

    pub fn source(&self) -> TileId {
        self.source
    }

    pub fn dest(&self) -> TileId {
        self.dest
    }

    pub fn command(&self) -> Command {
        self.command
    }

    pub fn tag(&self) -> u8 {
        self.tag
    }

    pub fn data0(&self) -> u32 {
        self.data0
    }

    pub fn data1(&self) -> u32 {
        self.data1
    }

    pub fn is_broadcast(&self) -> bool {
        self.command.is_broadcast()
    }

    /// Convert packet to 97-bit representation
    /// Bit layout: [96:PUSH, 95:88:COMMAND, 87:80:TAG, 79:72:DEST, 71:64:SOURCE, 63:32:DATA0, 31:0:DATA1]
    pub fn to_bits(&self) -> [bool; 97] {
        let mut bits = [false; 97];

        // Bit 96: PUSH
        bits[96] = self.push;

        // Bits 95:88: COMMAND (8 bits)
        let cmd = self.command.to_u8();
        for i in 0..8 {
            bits[88 + i] = ((cmd >> i) & 1) != 0;
        }

        // Bits 87:80: TAG (8 bits)
        for i in 0..8 {
            bits[80 + i] = ((self.tag >> i) & 1) != 0;
        }

        // Bits 79:72: DEST (8 bits)
        for i in 0..8 {
            bits[72 + i] = ((self.dest.0 >> i) & 1) != 0;
        }

        // Bits 71:64: SOURCE (8 bits)
        for i in 0..8 {
            bits[64 + i] = ((self.source.0 >> i) & 1) != 0;
        }

        // Bits 63:32: DATA0 (32 bits)
        for i in 0..32 {
            bits[32 + i] = ((self.data0 >> i) & 1) != 0;
        }

        // Bits 31:0: DATA1 (32 bits)
        for i in 0..32 {
            bits[i] = ((self.data1 >> i) & 1) != 0;
        }

        bits
    }

    /// Reconstruct packet from 97-bit representation
    pub fn from_bits(bits: &[bool; 97]) -> Result<Self> {
        let push = bits[96];

        let mut cmd = 0u8;
        for i in 0..8 {
            if bits[88 + i] {
                cmd |= 1 << i;
            }
        }

        let mut tag = 0u8;
        for i in 0..8 {
            if bits[80 + i] {
                tag |= 1 << i;
            }
        }

        let mut dest = 0u8;
        for i in 0..8 {
            if bits[72 + i] {
                dest |= 1 << i;
            }
        }

        let mut source = 0u8;
        for i in 0..8 {
            if bits[64 + i] {
                source |= 1 << i;
            }
        }

        let mut data0 = 0u32;
        for i in 0..32 {
            if bits[32 + i] {
                data0 |= 1 << i;
            }
        }

        let mut data1 = 0u32;
        for i in 0..32 {
            if bits[i] {
                data1 |= 1 << i;
            }
        }

        Ok(RaceWayPacket {
            source: TileId(source),
            dest: TileId(dest),
            command: Command::from_u8(cmd)?,
            tag,
            data0,
            data1,
            push,
            reset_n: true,
        })
    }

    /// Create a response packet (swaps source and destination)
    pub fn to_response(&self, ack_code: u8) -> Self {
        RaceWayPacket {
            source: self.dest, // Swap
            dest: self.source, // Swap
            command: Command::from_u8(ack_code).unwrap_or(Command::Success),
            tag: self.tag,     // Keep same tag
            data0: 0,          // Response data
            data1: self.data0, // Echo write data
            push: true,
            reset_n: true,
        }
    }
}

impl Default for RaceWayPacket {
    fn default() -> Self {
        RaceWayPacket {
            source: TileId(0),
            dest: TileId(0),
            command: Command::Write,
            tag: 0,
            data0: 0,
            data1: 0,
            push: false,
            reset_n: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_encoding() {
        assert_eq!(Command::Write.to_u8(), 0x91);
        assert_eq!(Command::Read.to_u8(), 0x89);
        assert_eq!(Command::Broadcast.to_u8(), 0xB1);
    }

    #[test]
    fn test_broadcast_detection() {
        assert!(Command::Broadcast.is_broadcast());
        assert!(Command::BarrierSync.is_broadcast());
        assert!(Command::Multicast.is_broadcast());
        assert!(!Command::Write.is_broadcast());
        assert!(!Command::Read.is_broadcast());
    }

    #[test]
    fn test_packet_builder() {
        let packet = RaceWayPacket::new()
            .source(TileId(0x11))
            .dest(TileId(0x42))
            .command(Command::Write)
            .tag(0x05)
            .build()
            .unwrap();

        assert_eq!(packet.source(), TileId(0x11));
        assert_eq!(packet.dest(), TileId(0x42));
    }
}
