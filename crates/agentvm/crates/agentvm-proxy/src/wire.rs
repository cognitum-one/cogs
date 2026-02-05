//! Wire protocol for capability proxy communication.
//!
//! Implements the message framing defined in ADR-005.

use crate::error::ParseError;
use crate::types::{
    CapabilityId, DeriveRequest, InvokeRequest, InvokeResponse, Operation, Quota,
};
use crate::{WIRE_MAGIC, WIRE_VERSION};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// Message type codes per ADR-005
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // Requests (guest -> proxy)
    /// Invoke capability
    Invoke = 0x0001,
    /// Derive child capability
    Derive = 0x0002,
    /// Revoke capability
    Revoke = 0x0003,
    /// Query remaining quota
    QueryQuota = 0x0004,

    // Responses (proxy -> guest)
    /// Invocation result
    InvokeResult = 0x0101,
    /// Derivation result
    DeriveResult = 0x0102,
    /// Revocation result
    RevokeResult = 0x0103,
    /// Quota query result
    QuotaResult = 0x0104,

    // Errors
    /// Error response
    Error = 0x0200,

    // Control
    /// Keepalive ping
    Ping = 0x0300,
    /// Keepalive response
    Pong = 0x0301,
    /// Graceful shutdown
    Shutdown = 0x0302,
}

impl MessageType {
    /// Convert from u16 code
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
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
        matches!(
            self,
            Self::Invoke | Self::Derive | Self::Revoke | Self::QueryQuota
        )
    }

    /// Check if this is a response type
    pub fn is_response(&self) -> bool {
        matches!(
            self,
            Self::InvokeResult
                | Self::DeriveResult
                | Self::RevokeResult
                | Self::QuotaResult
                | Self::Error
        )
    }

    /// Check if this is a control message
    pub fn is_control(&self) -> bool {
        matches!(self, Self::Ping | Self::Pong | Self::Shutdown)
    }
}

/// Message envelope flags
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageFlags(pub u16);

impl MessageFlags {
    /// Request expects a response
    pub const EXPECT_RESPONSE: u16 = 1 << 0;
    /// Message is compressed (zstd)
    pub const COMPRESSED: u16 = 1 << 1;
    /// Message is encrypted
    pub const ENCRYPTED: u16 = 1 << 2;
    /// Message is part of a stream
    pub const STREAMING: u16 = 1 << 3;
    /// Last message in stream
    pub const END_OF_STREAM: u16 = 1 << 4;

    /// Check if flag is set
    pub fn has(&self, flag: u16) -> bool {
        self.0 & flag == flag
    }

    /// Set a flag
    pub fn set(&mut self, flag: u16) {
        self.0 |= flag;
    }
}

/// Message envelope per ADR-005 wire protocol
///
/// ```text
/// +------------------------------------------------------------------+
/// |  magic: u32 = 0x43415056 ("CAPV")                               |
/// |  version: u16 = 1                                                |
/// |  flags: u16                                                      |
/// |  sequence: u64                                                   |
/// |  capability_id: u128                                             |
/// |  message_type: u16                                               |
/// |  payload_len: u32                                                |
/// |  payload: [u8; payload_len]                                      |
/// |  checksum: u32 (CRC32C)                                          |
/// +------------------------------------------------------------------+
/// Total header: 38 bytes + payload + 4 bytes checksum
/// ```
#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    /// Protocol magic number
    pub magic: u32,
    /// Protocol version
    pub version: u16,
    /// Message flags
    pub flags: MessageFlags,
    /// Sequence number for request/response matching
    pub sequence: u64,
    /// Capability ID (128-bit)
    pub capability_id: CapabilityId,
    /// Message type
    pub message_type: MessageType,
    /// Payload data
    pub payload: Bytes,
}

impl MessageEnvelope {
    /// Header size in bytes (without payload and checksum)
    /// magic(4) + version(2) + flags(2) + sequence(8) + capability_id(16) + message_type(2) + payload_len(4) = 38
    pub const HEADER_SIZE: usize = 38;
    /// Maximum payload size
    pub const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024; // 16 MB

    /// Create a new message envelope
    pub fn new(
        sequence: u64,
        capability_id: CapabilityId,
        message_type: MessageType,
        payload: Bytes,
    ) -> Self {
        Self {
            magic: WIRE_MAGIC,
            version: WIRE_VERSION,
            flags: MessageFlags::default(),
            sequence,
            capability_id,
            message_type,
            payload,
        }
    }

    /// Create an invoke request envelope
    pub fn invoke_request(
        sequence: u64,
        capability_id: CapabilityId,
        request: &InvokeRequest,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(request)?;
        let mut envelope = Self::new(
            sequence,
            capability_id,
            MessageType::Invoke,
            Bytes::from(payload),
        );
        envelope.flags.set(MessageFlags::EXPECT_RESPONSE);
        Ok(envelope)
    }

    /// Create an invoke response envelope
    pub fn invoke_response(
        sequence: u64,
        capability_id: CapabilityId,
        response: &InvokeResponse,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(response)?;
        Ok(Self::new(
            sequence,
            capability_id,
            MessageType::InvokeResult,
            Bytes::from(payload),
        ))
    }

    /// Create an error response envelope
    pub fn error_response(
        sequence: u64,
        capability_id: CapabilityId,
        error: &ErrorPayload,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(error)?;
        Ok(Self::new(
            sequence,
            capability_id,
            MessageType::Error,
            Bytes::from(payload),
        ))
    }

    /// Create a ping message
    pub fn ping(sequence: u64) -> Self {
        Self::new(
            sequence,
            CapabilityId(0),
            MessageType::Ping,
            Bytes::new(),
        )
    }

    /// Create a pong message
    pub fn pong(sequence: u64) -> Self {
        Self::new(
            sequence,
            CapabilityId(0),
            MessageType::Pong,
            Bytes::new(),
        )
    }

    /// Create a shutdown message
    pub fn shutdown(sequence: u64) -> Self {
        Self::new(
            sequence,
            CapabilityId(0),
            MessageType::Shutdown,
            Bytes::new(),
        )
    }

    /// Parse an envelope from a buffer
    pub fn parse(buf: &[u8]) -> Result<(Self, usize), ParseError> {
        if buf.len() < Self::HEADER_SIZE {
            return Err(ParseError::BufferTooSmall {
                needed: Self::HEADER_SIZE,
                have: buf.len(),
            });
        }

        let mut cursor = std::io::Cursor::new(buf);

        // Read header fields
        let magic = cursor.get_u32_le();
        if magic != WIRE_MAGIC {
            return Err(ParseError::InvalidMagic {
                expected: WIRE_MAGIC,
                got: magic,
            });
        }

        let version = cursor.get_u16_le();
        if version != WIRE_VERSION {
            return Err(ParseError::UnsupportedVersion(version));
        }

        let flags = MessageFlags(cursor.get_u16_le());
        let sequence = cursor.get_u64_le();

        let mut cap_bytes = [0u8; 16];
        cursor.copy_to_slice(&mut cap_bytes);
        let capability_id = CapabilityId::from_bytes(cap_bytes);

        let message_type_code = cursor.get_u16_le();
        let message_type = MessageType::from_code(message_type_code)
            .ok_or(ParseError::InvalidMessageType(message_type_code))?;

        let payload_len = cursor.get_u32_le() as usize;
        if payload_len > Self::MAX_PAYLOAD_SIZE {
            return Err(ParseError::PayloadTooLarge(payload_len as u32));
        }

        let total_len = Self::HEADER_SIZE + payload_len + 4; // +4 for checksum
        if buf.len() < total_len {
            return Err(ParseError::BufferTooSmall {
                needed: total_len,
                have: buf.len(),
            });
        }

        // Read payload
        let payload_start = cursor.position() as usize;
        let payload = Bytes::copy_from_slice(&buf[payload_start..payload_start + payload_len]);

        // Verify checksum
        let checksum_pos = payload_start + payload_len;
        let expected_checksum =
            u32::from_le_bytes(buf[checksum_pos..checksum_pos + 4].try_into().unwrap());
        let computed_checksum = crc32c::crc32c(&buf[..checksum_pos]);

        if expected_checksum != computed_checksum {
            return Err(ParseError::ChecksumMismatch {
                expected: expected_checksum,
                got: computed_checksum,
            });
        }

        Ok((
            Self {
                magic,
                version,
                flags,
                sequence,
                capability_id,
                message_type,
                payload,
            },
            total_len,
        ))
    }

    /// Serialize the envelope to bytes
    pub fn serialize(&self) -> BytesMut {
        let total_size = Self::HEADER_SIZE + self.payload.len() + 4;
        let mut buf = BytesMut::with_capacity(total_size);

        // Write header
        buf.put_u32_le(self.magic);
        buf.put_u16_le(self.version);
        buf.put_u16_le(self.flags.0);
        buf.put_u64_le(self.sequence);
        buf.put_slice(&self.capability_id.to_bytes());
        buf.put_u16_le(self.message_type as u16);
        buf.put_u32_le(self.payload.len() as u32);

        // Write payload
        buf.put_slice(&self.payload);

        // Compute and write checksum
        let checksum = crc32c::crc32c(&buf);
        buf.put_u32_le(checksum);

        buf
    }

    /// Decode payload as JSON
    pub fn decode_payload<T: for<'de> Deserialize<'de>>(&self) -> Result<T, ParseError> {
        serde_json::from_slice(&self.payload)
            .map_err(|e| ParseError::InvalidPayload(e.to_string()))
    }
}

/// Error payload for error responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Additional details
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorPayload {
    /// Create a new error payload
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create with details
    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

/// Derive request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivePayload {
    /// Derive parameters
    pub request: DeriveRequest,
}

/// Derive response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveResponsePayload {
    /// New capability ID
    pub capability_id: CapabilityId,
    /// Expiration time
    pub expires_at: u64,
}

/// Revoke request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokePayload {
    /// Capability ID to revoke
    pub capability_id: CapabilityId,
    /// Reason for revocation
    #[serde(default)]
    pub reason: Option<String>,
}

/// Revoke response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeResponsePayload {
    /// Number of capabilities revoked (including derived)
    pub revoked_count: u32,
}

/// Quota query response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaResponsePayload {
    /// Remaining quota
    pub quota: Quota,
    /// Whether the capability is still valid
    pub valid: bool,
}

/// Codec for framing messages over a stream
pub struct MessageCodec {
    /// Maximum message size
    max_message_size: usize,
}

impl MessageCodec {
    /// Create a new codec
    pub fn new(max_message_size: usize) -> Self {
        Self { max_message_size }
    }

    /// Try to decode a message from a buffer
    pub fn decode(&self, buf: &mut BytesMut) -> Result<Option<MessageEnvelope>, ParseError> {
        if buf.len() < MessageEnvelope::HEADER_SIZE {
            return Ok(None);
        }

        // Peek at payload length to check if we have the full message
        // payload_len is at offset 34: magic(4) + version(2) + flags(2) + sequence(8) + capability_id(16) + message_type(2) = 34
        let payload_len =
            u32::from_le_bytes(buf[34..38].try_into().unwrap()) as usize;
        let total_len = MessageEnvelope::HEADER_SIZE + payload_len + 4;

        if total_len > self.max_message_size {
            return Err(ParseError::PayloadTooLarge(total_len as u32));
        }

        if buf.len() < total_len {
            return Ok(None);
        }

        let (envelope, consumed) = MessageEnvelope::parse(buf)?;
        buf.advance(consumed);
        Ok(Some(envelope))
    }

    /// Encode a message to a buffer
    pub fn encode(&self, envelope: &MessageEnvelope, buf: &mut BytesMut) {
        buf.extend_from_slice(&envelope.serialize());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Operation, QuotaConsumed};

    #[test]
    fn test_roundtrip_invoke_request() {
        let request = InvokeRequest {
            operation: Operation::HttpRequest {
                method: "GET".to_string(),
                url: "https://api.example.com/data".to_string(),
                headers: vec![("Accept".to_string(), "application/json".to_string())],
                body: None,
            },
            params: serde_json::Value::Null,
            deadline_ns: 1000000000,
            idempotency_key: None,
        };

        let cap_id = CapabilityId::generate();
        let envelope = MessageEnvelope::invoke_request(42, cap_id, &request).unwrap();

        let serialized = envelope.serialize();
        let (parsed, _) = MessageEnvelope::parse(&serialized).unwrap();

        assert_eq!(parsed.sequence, 42);
        assert_eq!(parsed.capability_id, cap_id);
        assert_eq!(parsed.message_type, MessageType::Invoke);

        let decoded: InvokeRequest = parsed.decode_payload().unwrap();
        assert_eq!(
            decoded.deadline_ns,
            request.deadline_ns
        );
    }

    #[test]
    fn test_ping_pong() {
        let ping = MessageEnvelope::ping(100);
        let serialized = ping.serialize();
        let (parsed, _) = MessageEnvelope::parse(&serialized).unwrap();

        assert_eq!(parsed.message_type, MessageType::Ping);
        assert_eq!(parsed.sequence, 100);

        let pong = MessageEnvelope::pong(100);
        let serialized = pong.serialize();
        let (parsed, _) = MessageEnvelope::parse(&serialized).unwrap();

        assert_eq!(parsed.message_type, MessageType::Pong);
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = BytesMut::with_capacity(50);
        buf.put_u32_le(0xDEADBEEF); // Invalid magic
        buf.put_u16_le(1);
        buf.resize(50, 0);

        let result = MessageEnvelope::parse(&buf);
        assert!(matches!(result, Err(ParseError::InvalidMagic { .. })));
    }

    #[test]
    fn test_checksum_verification() {
        let envelope = MessageEnvelope::ping(1);
        let mut serialized = envelope.serialize();

        // Corrupt one byte
        serialized[10] ^= 0xFF;

        let result = MessageEnvelope::parse(&serialized);
        assert!(matches!(result, Err(ParseError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_codec() {
        let codec = MessageCodec::new(1024 * 1024);
        let envelope = MessageEnvelope::ping(123);

        let mut buf = BytesMut::new();
        codec.encode(&envelope, &mut buf);

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.sequence, 123);
        assert!(buf.is_empty());
    }
}
