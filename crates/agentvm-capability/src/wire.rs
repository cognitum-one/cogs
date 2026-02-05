//! Wire protocol for capability messages
//!
//! Implements the message envelope format for communication between
//! guest capsules and the capability proxy over virtio-vsock.

use alloc::vec::Vec;
use core::convert::TryInto;

/// Magic bytes for capability protocol messages ("CAPV")
pub const MAGIC: u32 = 0x43415056;

/// Current protocol version
pub const VERSION: u16 = 1;

/// Header size in bytes
pub const HEADER_SIZE: usize = 42;

/// Message type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // Requests (guest -> proxy)
    Invoke = 0x0001,
    Derive = 0x0002,
    Revoke = 0x0003,
    QueryQuota = 0x0004,

    // Responses (proxy -> guest)
    InvokeResult = 0x0101,
    DeriveResult = 0x0102,
    RevokeResult = 0x0103,
    QuotaResult = 0x0104,

    // Errors
    Error = 0x0200,

    // Control
    Ping = 0x0300,
    Pong = 0x0301,
    Shutdown = 0x0302,
}

impl MessageType {
    /// Parse message type from u16
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::Invoke),
            0x0002 => Some(Self::Derive),
            0x0003 => Some(Self::Revoke),
            0x0004 => Some(Self::QueryQuota),
            0x0101 => Some(Self::InvokeResult),
            0x0102 => Some(Self::DeriveResult),
            0x0103 => Some(Self::RevokeResult),
            0x0104 => Some(Self::QuotaResult),
            0x0200 => Some(Self::Error),
            0x0300 => Some(Self::Ping),
            0x0301 => Some(Self::Pong),
            0x0302 => Some(Self::Shutdown),
            _ => None,
        }
    }

    /// Check if this is a request type
    pub fn is_request(&self) -> bool {
        (*self as u16) < 0x0100
    }

    /// Check if this is a response type
    pub fn is_response(&self) -> bool {
        let val = *self as u16;
        val >= 0x0100 && val < 0x0200
    }

    /// Check if this is an error type
    pub fn is_error(&self) -> bool {
        *self as u16 == 0x0200
    }

    /// Check if this is a control type
    pub fn is_control(&self) -> bool {
        (*self as u16) >= 0x0300
    }
}

/// Message flags
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageFlags(pub u16);

impl MessageFlags {
    /// Requires acknowledgment
    pub const REQUIRES_ACK: u16 = 1 << 0;
    /// Is a response to a previous message
    pub const IS_RESPONSE: u16 = 1 << 1;
    /// Message is encrypted
    pub const ENCRYPTED: u16 = 1 << 2;
    /// Message is compressed
    pub const COMPRESSED: u16 = 1 << 3;

    pub fn has(&self, flag: u16) -> bool {
        (self.0 & flag) == flag
    }

    pub fn set(&mut self, flag: u16) {
        self.0 |= flag;
    }
}

/// Message envelope for capability protocol
#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    /// Magic bytes (must be 0x43415056)
    pub magic: u32,
    /// Protocol version
    pub version: u16,
    /// Message flags
    pub flags: MessageFlags,
    /// Sequence number for ordering and matching responses
    pub sequence: u64,
    /// Capability ID this message relates to
    pub capability_id: [u8; 16],
    /// Message type
    pub message_type: MessageType,
    /// Payload length
    pub payload_len: u32,
    /// Payload data
    pub payload: Vec<u8>,
    /// CRC32C checksum
    pub checksum: u32,
}

impl MessageEnvelope {
    /// Create a new message envelope
    pub fn new(
        message_type: MessageType,
        capability_id: [u8; 16],
        sequence: u64,
        payload: Vec<u8>,
    ) -> Self {
        let payload_len = payload.len() as u32;
        let mut envelope = Self {
            magic: MAGIC,
            version: VERSION,
            flags: MessageFlags::default(),
            sequence,
            capability_id,
            message_type,
            payload_len,
            payload,
            checksum: 0,
        };
        envelope.checksum = envelope.calculate_checksum();
        envelope
    }

    /// Create a request message
    pub fn request(
        message_type: MessageType,
        capability_id: [u8; 16],
        sequence: u64,
        payload: Vec<u8>,
    ) -> Self {
        let mut envelope = Self::new(message_type, capability_id, sequence, payload);
        envelope.flags.set(MessageFlags::REQUIRES_ACK);
        envelope.checksum = envelope.calculate_checksum();
        envelope
    }

    /// Create a response message
    pub fn response(
        message_type: MessageType,
        capability_id: [u8; 16],
        sequence: u64,
        payload: Vec<u8>,
    ) -> Self {
        let mut envelope = Self::new(message_type, capability_id, sequence, payload);
        envelope.flags.set(MessageFlags::IS_RESPONSE);
        envelope.checksum = envelope.calculate_checksum();
        envelope
    }

    /// Create a Ping message
    pub fn ping(sequence: u64) -> Self {
        Self::new(MessageType::Ping, [0u8; 16], sequence, Vec::new())
    }

    /// Create a Pong message (response to Ping)
    pub fn pong(sequence: u64) -> Self {
        Self::response(MessageType::Pong, [0u8; 16], sequence, Vec::new())
    }

    /// Parse a message envelope from bytes
    pub fn parse(buf: &[u8]) -> Result<Self, ParseError> {
        if buf.len() < HEADER_SIZE {
            return Err(ParseError::BufferTooSmall {
                needed: HEADER_SIZE,
                got: buf.len(),
            });
        }

        // Parse magic
        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Err(ParseError::InvalidMagic { got: magic });
        }

        // Parse version
        let version = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(ParseError::UnsupportedVersion { version });
        }

        // Parse flags
        let flags = MessageFlags(u16::from_le_bytes(buf[6..8].try_into().unwrap()));

        // Parse sequence
        let sequence = u64::from_le_bytes(buf[8..16].try_into().unwrap());

        // Parse capability ID
        let mut capability_id = [0u8; 16];
        capability_id.copy_from_slice(&buf[16..32]);

        // Parse message type
        let message_type_raw = u16::from_le_bytes(buf[32..34].try_into().unwrap());
        let message_type = MessageType::from_u16(message_type_raw)
            .ok_or(ParseError::InvalidMessageType { type_id: message_type_raw })?;

        // Parse payload length
        let payload_len = u32::from_le_bytes(buf[34..38].try_into().unwrap());

        // Validate buffer has enough data
        let total_len = HEADER_SIZE + payload_len as usize + 4; // +4 for checksum
        if buf.len() < total_len {
            return Err(ParseError::BufferTooSmall {
                needed: total_len,
                got: buf.len(),
            });
        }

        // Parse payload
        let payload_end = HEADER_SIZE - 4 + payload_len as usize; // -4 because HEADER_SIZE includes the 4 bytes before payload_len
        let payload = buf[38..38 + payload_len as usize].to_vec();

        // Parse checksum
        let checksum_offset = 38 + payload_len as usize;
        let checksum = u32::from_le_bytes(
            buf[checksum_offset..checksum_offset + 4].try_into().unwrap(),
        );

        let envelope = Self {
            magic,
            version,
            flags,
            sequence,
            capability_id,
            message_type,
            payload_len,
            payload,
            checksum,
        };

        // Verify checksum
        let calculated_checksum = envelope.calculate_checksum();
        if checksum != calculated_checksum {
            return Err(ParseError::ChecksumMismatch {
                expected: calculated_checksum,
                got: checksum,
            });
        }

        Ok(envelope)
    }

    /// Serialize the envelope to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let total_len = HEADER_SIZE + self.payload.len() + 4;
        let mut buf = Vec::with_capacity(total_len);

        // Magic (4 bytes)
        buf.extend_from_slice(&self.magic.to_le_bytes());
        // Version (2 bytes)
        buf.extend_from_slice(&self.version.to_le_bytes());
        // Flags (2 bytes)
        buf.extend_from_slice(&self.flags.0.to_le_bytes());
        // Sequence (8 bytes)
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        // Capability ID (16 bytes)
        buf.extend_from_slice(&self.capability_id);
        // Message type (2 bytes)
        buf.extend_from_slice(&(self.message_type as u16).to_le_bytes());
        // Payload length (4 bytes)
        buf.extend_from_slice(&self.payload_len.to_le_bytes());
        // Payload
        buf.extend_from_slice(&self.payload);
        // Checksum (4 bytes)
        buf.extend_from_slice(&self.checksum.to_le_bytes());

        buf
    }

    /// Calculate CRC32C checksum of the envelope (excluding checksum field)
    pub fn calculate_checksum(&self) -> u32 {
        // Simple checksum implementation - in reality would use CRC32C
        let mut sum: u32 = 0;
        sum = sum.wrapping_add(self.magic);
        sum = sum.wrapping_add(self.version as u32);
        sum = sum.wrapping_add(self.flags.0 as u32);
        sum = sum.wrapping_add((self.sequence & 0xFFFFFFFF) as u32);
        sum = sum.wrapping_add((self.sequence >> 32) as u32);
        for byte in &self.capability_id {
            sum = sum.wrapping_add(*byte as u32);
        }
        sum = sum.wrapping_add(self.message_type as u32);
        sum = sum.wrapping_add(self.payload_len);
        for byte in &self.payload {
            sum = sum.wrapping_add(*byte as u32);
        }
        sum
    }

    /// Verify the checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.calculate_checksum()
    }

    /// Get total message size in bytes
    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.payload.len() + 4
    }
}

/// Parse errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    BufferTooSmall { needed: usize, got: usize },
    InvalidMagic { got: u32 },
    UnsupportedVersion { version: u16 },
    InvalidMessageType { type_id: u16 },
    ChecksumMismatch { expected: u32, got: u32 },
    InvalidPayload { reason: &'static str },
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall { needed, got } => {
                write!(f, "buffer too small: needed {} bytes, got {}", needed, got)
            }
            Self::InvalidMagic { got } => {
                write!(f, "invalid magic: expected 0x{:08X}, got 0x{:08X}", MAGIC, got)
            }
            Self::UnsupportedVersion { version } => {
                write!(f, "unsupported version: {}", version)
            }
            Self::InvalidMessageType { type_id } => {
                write!(f, "invalid message type: 0x{:04X}", type_id)
            }
            Self::ChecksumMismatch { expected, got } => {
                write!(f, "checksum mismatch: expected 0x{:08X}, got 0x{:08X}", expected, got)
            }
            Self::InvalidPayload { reason } => {
                write!(f, "invalid payload: {}", reason)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}
