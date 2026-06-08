//! Newport ASIC core API surface.
//!
//! This crate provides the small, self-contained core API that the Cognitum
//! test suite (`tests/stress_tests.rs`, `tests/test_utils.rs`,
//! `tests/verilog_cross_validation.rs`) was originally written against, before
//! the project was renamed from "Newport" to "Cognitum". The simulator crates
//! under `cognitum-sim/` later diverged to a different (word-addressed) memory
//! model and packet format, so this crate preserves the legacy byte-addressed
//! API contract those tests rely on.
//!
//! Everything here is a real, working implementation — nothing is stubbed.

use thiserror::Error;

/// Errors produced by the Newport core memory and interconnect models.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NewportError {
    /// Access to an address outside the addressable region.
    #[error("invalid address: 0x{0:08x}")]
    InvalidAddress(u32),

    /// Write attempted against a read-only / protected region.
    #[error("memory access violation at 0x{0:08x}")]
    MemoryAccessViolation(u32),

    /// A RaceWay packet could not be constructed or decoded.
    #[error("invalid packet: {0}")]
    InvalidPacket(String),
}

/// Convenience result alias used throughout the Newport core API.
pub type Result<T> = std::result::Result<T, NewportError>;

/// A byte-addressable memory location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryAddress(u32);

impl MemoryAddress {
    /// Create a new memory address from a raw byte offset.
    #[inline]
    pub fn new(addr: u32) -> Self {
        MemoryAddress(addr)
    }

    /// Return the raw byte offset.
    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Return the raw byte offset as a `u32` (alias of [`value`](Self::value)).
    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

/// Byte-addressable memory model.
///
/// Implementors only need to provide [`read_byte`](Memory::read_byte),
/// [`write_byte`](Memory::write_byte), and [`size`](Memory::size). Word access
/// is provided by default as little-endian 4-byte operations composed from the
/// byte primitives.
pub trait Memory {
    /// Read a single byte.
    fn read_byte(&self, addr: MemoryAddress) -> Result<u8>;

    /// Write a single byte.
    fn write_byte(&mut self, addr: MemoryAddress, value: u8) -> Result<()>;

    /// Total addressable size in bytes.
    fn size(&self) -> usize;

    /// Read a little-endian 32-bit word starting at `addr`.
    fn read_word(&self, addr: MemoryAddress) -> Result<u32> {
        let base = addr.value();
        let b0 = self.read_byte(MemoryAddress::new(base))? as u32;
        let b1 = self.read_byte(MemoryAddress::new(base.wrapping_add(1)))? as u32;
        let b2 = self.read_byte(MemoryAddress::new(base.wrapping_add(2)))? as u32;
        let b3 = self.read_byte(MemoryAddress::new(base.wrapping_add(3)))? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    /// Write a little-endian 32-bit word starting at `addr`.
    fn write_word(&mut self, addr: MemoryAddress, value: u32) -> Result<()> {
        let base = addr.value();
        self.write_byte(MemoryAddress::new(base), (value & 0xFF) as u8)?;
        self.write_byte(MemoryAddress::new(base.wrapping_add(1)), ((value >> 8) & 0xFF) as u8)?;
        self.write_byte(MemoryAddress::new(base.wrapping_add(2)), ((value >> 16) & 0xFF) as u8)?;
        self.write_byte(MemoryAddress::new(base.wrapping_add(3)), ((value >> 24) & 0xFF) as u8)?;
        Ok(())
    }
}

/// Identifier for a tile in the 16x16 (256-tile) array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId(u8);

impl TileId {
    /// Create a tile id. The Newport array holds 256 tiles (0..=255), so any
    /// value greater than 255 is rejected.
    #[inline]
    pub fn new(id: u16) -> Result<Self> {
        if id > 255 {
            Err(NewportError::InvalidPacket(format!(
                "tile id {} out of range (0..=255)",
                id
            )))
        } else {
            Ok(TileId(id as u8))
        }
    }

    /// Construct a tile id from `(row, col)` grid coordinates in the 16x16
    /// array. The tile index is `(row << 4) | col`; both coordinates must be
    /// in `0..16`.
    #[inline]
    pub fn from_coords(row: u8, col: u8) -> Result<Self> {
        if row > 15 || col > 15 {
            return Err(NewportError::InvalidPacket(format!(
                "tile coords ({}, {}) out of range (0..16)",
                row, col
            )));
        }
        Ok(TileId((row << 4) | col))
    }

    /// Return the raw tile index.
    #[inline]
    pub fn value(&self) -> u8 {
        self.0
    }
}

/// Kind of payload carried by a RaceWay packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketType {
    /// Ordinary data payload.
    Data,
    /// Control / signaling payload.
    Control,
}

impl Default for PacketType {
    fn default() -> Self {
        PacketType::Data
    }
}

/// A packet on the RaceWay interconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaceWayPacket {
    /// Source tile.
    pub source: TileId,
    /// Destination tile.
    pub dest: TileId,
    /// Packet kind.
    pub packet_type: PacketType,
    /// Payload bytes.
    pub data: Vec<u8>,
}

impl RaceWayPacket {
    /// Construct a packet with an explicit packet type.
    pub fn new(source: TileId, dest: TileId, packet_type: PacketType, data: Vec<u8>) -> Self {
        RaceWayPacket {
            source,
            dest,
            packet_type,
            data,
        }
    }

    /// Construct a `Data` packet.
    pub fn data(source: TileId, dest: TileId, data: Vec<u8>) -> Self {
        RaceWayPacket::new(source, dest, PacketType::Data, data)
    }

    /// Serialize the packet to a wire byte buffer.
    ///
    /// Layout: `[source(1)] [dest(1)] [type(1)] [len(4, LE)] [payload(len)]`.
    pub fn to_bits(&self) -> Vec<u8> {
        let mut bits = Vec::with_capacity(7 + self.data.len());
        bits.push(self.source.value());
        bits.push(self.dest.value());
        bits.push(match self.packet_type {
            PacketType::Data => 0,
            PacketType::Control => 1,
        });
        let len = self.data.len() as u32;
        bits.extend_from_slice(&len.to_le_bytes());
        bits.extend_from_slice(&self.data);
        bits
    }

    /// Decode a packet from a wire byte buffer produced by [`to_bits`](Self::to_bits).
    pub fn from_bits(bits: &[u8]) -> Result<Self> {
        if bits.len() < 7 {
            return Err(NewportError::InvalidPacket(format!(
                "packet too short: {} bytes (need at least 7)",
                bits.len()
            )));
        }
        let source = TileId::new(bits[0] as u16)?;
        let dest = TileId::new(bits[1] as u16)?;
        let packet_type = match bits[2] {
            0 => PacketType::Data,
            1 => PacketType::Control,
            other => {
                return Err(NewportError::InvalidPacket(format!(
                    "unknown packet type byte: {}",
                    other
                )))
            }
        };
        let len = u32::from_le_bytes([bits[3], bits[4], bits[5], bits[6]]) as usize;
        if bits.len() != 7 + len {
            return Err(NewportError::InvalidPacket(format!(
                "length mismatch: header says {} payload bytes, buffer has {}",
                len,
                bits.len() - 7
            )));
        }
        let data = bits[7..].to_vec();
        Ok(RaceWayPacket {
            source,
            dest,
            packet_type,
            data,
        })
    }
}
