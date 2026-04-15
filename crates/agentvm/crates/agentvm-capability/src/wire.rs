//! Wire protocol for capability messages.
//!
//! Defines the binary wire format for capability protocol messages:
//! - 42-byte message envelope header
//! - Message types: Invoke, Derive, Revoke, QueryQuota, etc.
//! - CRC32C checksums for integrity

use alloc::vec::Vec;
use core::fmt;

use agentvm_types::{CapabilityId, CapabilityProof, Quota, Rights};

use crate::{AgentId, SequenceNumber, Timestamp};

/// Magic number for capability protocol messages: "CAPV" in little-endian
pub const MAGIC_NUMBER: u32 = 0x56504143; // "CAPV" as bytes: 0x43, 0x41, 0x50, 0x56

/// Current protocol version
pub const VERSION: u16 = 1;

/// Header size in bytes
pub const HEADER_SIZE: usize = 42;

/// Maximum payload size (64KB)
pub const MAX_PAYLOAD_SIZE: usize = 65536;

/// Message type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    /// Invoke a capability
    Invoke = 0x0001,
    /// Result of an invoke
    InvokeResult = 0x0002,
    /// Derive a new capability
    Derive = 0x0003,
    /// Result of derivation
    DeriveResult = 0x0004,
    /// Revoke a capability
    Revoke = 0x0005,
    /// Result of revocation
    RevokeResult = 0x0006,
    /// Query quota remaining
    QueryQuota = 0x0007,
    /// Result of quota query
    QuotaResult = 0x0008,
    /// Validate a capability
    Validate = 0x0009,
    /// Result of validation
    ValidateResult = 0x000A,
    /// Error response
    Error = 0x00FF,
    /// Heartbeat/ping
    Ping = 0x0100,
    /// Heartbeat response
    Pong = 0x0101,
}

impl MessageType {
    /// Convert from u16
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::Invoke),
            0x0002 => Some(Self::InvokeResult),
            0x0003 => Some(Self::Derive),
            0x0004 => Some(Self::DeriveResult),
            0x0005 => Some(Self::Revoke),
            0x0006 => Some(Self::RevokeResult),
            0x0007 => Some(Self::QueryQuota),
            0x0008 => Some(Self::QuotaResult),
            0x0009 => Some(Self::Validate),
            0x000A => Some(Self::ValidateResult),
            0x00FF => Some(Self::Error),
            0x0100 => Some(Self::Ping),
            0x0101 => Some(Self::Pong),
            _ => None,
        }
    }

    /// Check if this is a request type
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            Self::Invoke
                | Self::Derive
                | Self::Revoke
                | Self::QueryQuota
                | Self::Validate
                | Self::Ping
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
                | Self::ValidateResult
                | Self::Error
                | Self::Pong
        )
    }
}

/// Message flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageFlags(u16);

impl MessageFlags {
    /// No flags
    pub const NONE: MessageFlags = MessageFlags(0);
    /// Request requires acknowledgment
    pub const REQUIRES_ACK: MessageFlags = MessageFlags(1 << 0);
    /// Message is encrypted
    pub const ENCRYPTED: MessageFlags = MessageFlags(1 << 1);
    /// Message is compressed
    pub const COMPRESSED: MessageFlags = MessageFlags(1 << 2);
    /// Message is part of a batch
    pub const BATCHED: MessageFlags = MessageFlags(1 << 3);
    /// Final message in batch
    pub const BATCH_END: MessageFlags = MessageFlags(1 << 4);
    /// Message has extended header
    pub const EXTENDED_HEADER: MessageFlags = MessageFlags(1 << 5);
    /// Priority message
    pub const PRIORITY: MessageFlags = MessageFlags(1 << 6);

    /// Create from raw value
    pub const fn from_bits(bits: u16) -> Self {
        MessageFlags(bits)
    }

    /// Get raw value
    pub const fn bits(&self) -> u16 {
        self.0
    }

    /// Check if flag is set
    pub const fn contains(&self, other: MessageFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Set a flag
    pub const fn set(&self, other: MessageFlags) -> Self {
        MessageFlags(self.0 | other.0)
    }
}

impl Default for MessageFlags {
    fn default() -> Self {
        Self::NONE
    }
}

/// Wire protocol errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Invalid magic number
    InvalidMagic,
    /// Unsupported protocol version
    UnsupportedVersion,
    /// Invalid message type
    InvalidMessageType,
    /// Buffer too small for header
    BufferTooSmall,
    /// Payload exceeds maximum size
    PayloadTooLarge,
    /// CRC checksum mismatch
    ChecksumMismatch,
    /// Malformed message
    MalformedMessage,
    /// Unexpected message type
    UnexpectedMessageType,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "invalid magic number"),
            Self::UnsupportedVersion => write!(f, "unsupported protocol version"),
            Self::InvalidMessageType => write!(f, "invalid message type"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::PayloadTooLarge => write!(f, "payload too large"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::MalformedMessage => write!(f, "malformed message"),
            Self::UnexpectedMessageType => write!(f, "unexpected message type"),
        }
    }
}

/// 42-byte message envelope header.
///
/// Wire format (little-endian):
/// ```text
/// Offset  Size  Field
/// 0       4     magic (0x43415056 = "CAPV")
/// 4       2     version
/// 6       2     flags
/// 8       8     sequence
/// 16      16    capability_id
/// 32      2     message_type
/// 34      4     payload_len
/// 38      4     checksum (CRC32C)
/// ------- 42 bytes total -------
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEnvelope {
    /// Protocol magic number
    pub magic: u32,
    /// Protocol version
    pub version: u16,
    /// Message flags
    pub flags: MessageFlags,
    /// Sequence number for request/response correlation
    pub sequence: SequenceNumber,
    /// Capability ID this message relates to
    pub capability_id: CapabilityId,
    /// Type of message
    pub message_type: MessageType,
    /// Length of payload following header
    pub payload_len: u32,
    /// CRC32C checksum of header (excluding checksum field)
    pub checksum: u32,
}

impl MessageEnvelope {
    /// Create a new message envelope
    pub fn new(
        sequence: SequenceNumber,
        capability_id: CapabilityId,
        message_type: MessageType,
        payload_len: u32,
        flags: MessageFlags,
    ) -> Self {
        let mut envelope = Self {
            magic: MAGIC_NUMBER,
            version: VERSION,
            flags,
            sequence,
            capability_id,
            message_type,
            payload_len,
            checksum: 0,
        };
        envelope.checksum = envelope.compute_checksum();
        envelope
    }

    /// Compute CRC32C checksum of header (excluding checksum field)
    fn compute_checksum(&self) -> u32 {
        let mut data = [0u8; 38]; // Header without checksum
        data[0..4].copy_from_slice(&self.magic.to_le_bytes());
        data[4..6].copy_from_slice(&self.version.to_le_bytes());
        data[6..8].copy_from_slice(&self.flags.bits().to_le_bytes());
        data[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        data[16..32].copy_from_slice(&self.capability_id.0);
        data[32..34].copy_from_slice(&(self.message_type as u16).to_le_bytes());
        data[34..38].copy_from_slice(&self.payload_len.to_le_bytes());
        crc32c::crc32c(&data)
    }

    /// Serialize envelope to bytes
    pub fn serialize(&self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.version.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.flags.bits().to_le_bytes());
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..32].copy_from_slice(&self.capability_id.0);
        bytes[32..34].copy_from_slice(&(self.message_type as u16).to_le_bytes());
        bytes[34..38].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes[38..42].copy_from_slice(&self.checksum.to_le_bytes());
        bytes
    }

    /// Parse envelope from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < HEADER_SIZE {
            return Err(WireError::BufferTooSmall);
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != MAGIC_NUMBER {
            return Err(WireError::InvalidMagic);
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version > VERSION {
            return Err(WireError::UnsupportedVersion);
        }

        let flags = MessageFlags::from_bits(u16::from_le_bytes([bytes[6], bytes[7]]));
        let sequence = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);

        let mut capability_id = [0u8; 16];
        capability_id.copy_from_slice(&bytes[16..32]);

        let message_type_raw = u16::from_le_bytes([bytes[32], bytes[33]]);
        let message_type =
            MessageType::from_u16(message_type_raw).ok_or(WireError::InvalidMessageType)?;

        let payload_len = u32::from_le_bytes([bytes[34], bytes[35], bytes[36], bytes[37]]);
        if payload_len as usize > MAX_PAYLOAD_SIZE {
            return Err(WireError::PayloadTooLarge);
        }

        let checksum = u32::from_le_bytes([bytes[38], bytes[39], bytes[40], bytes[41]]);

        let envelope = Self {
            magic,
            version,
            flags,
            sequence,
            capability_id: CapabilityId(capability_id),
            message_type,
            payload_len,
            checksum,
        };

        // Verify checksum
        if envelope.compute_checksum() != checksum {
            return Err(WireError::ChecksumMismatch);
        }

        Ok(envelope)
    }

    /// Validate that this is the expected message type
    pub fn expect_type(&self, expected: MessageType) -> Result<(), WireError> {
        if self.message_type == expected {
            Ok(())
        } else {
            Err(WireError::UnexpectedMessageType)
        }
    }
}

/// Serialize CapabilityProof to bytes (32 + 64 + 8 = 104 bytes)
fn proof_to_bytes(proof: &CapabilityProof) -> [u8; 104] {
    let mut bytes = [0u8; 104];
    bytes[0..32].copy_from_slice(&proof.issuer);
    bytes[32..96].copy_from_slice(&proof.signature);
    bytes[96..104].copy_from_slice(&proof.issued_at.to_le_bytes());
    bytes
}

/// Deserialize CapabilityProof from bytes
fn proof_from_bytes(bytes: &[u8; 104]) -> CapabilityProof {
    let mut issuer = [0u8; 32];
    issuer.copy_from_slice(&bytes[0..32]);
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&bytes[32..96]);
    let issued_at = u64::from_le_bytes([
        bytes[96], bytes[97], bytes[98], bytes[99],
        bytes[100], bytes[101], bytes[102], bytes[103],
    ]);
    CapabilityProof {
        issuer,
        signature,
        issued_at,
    }
}

/// Invoke request payload
#[derive(Debug, Clone)]
pub struct InvokeRequest {
    /// Capability proof (104 bytes: issuer 32 + signature 64 + issued_at 8)
    pub proof: CapabilityProof,
    /// Operation code
    pub operation: u32,
    /// Operation arguments (variable length)
    pub arguments: Vec<u8>,
}

impl InvokeRequest {
    /// Create a new invoke request
    pub fn new(proof: CapabilityProof, operation: u32, arguments: Vec<u8>) -> Self {
        Self {
            proof,
            operation,
            arguments,
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(108 + self.arguments.len());
        bytes.extend_from_slice(&proof_to_bytes(&self.proof));
        bytes.extend_from_slice(&self.operation.to_le_bytes());
        bytes.extend_from_slice(&self.arguments);
        bytes
    }

    /// Parse from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 108 {
            return Err(WireError::BufferTooSmall);
        }

        let mut proof_bytes = [0u8; 104];
        proof_bytes.copy_from_slice(&bytes[0..104]);
        let proof = proof_from_bytes(&proof_bytes);

        let operation = u32::from_le_bytes([bytes[104], bytes[105], bytes[106], bytes[107]]);
        let arguments = bytes[108..].to_vec();

        Ok(Self {
            proof,
            operation,
            arguments,
        })
    }

    /// Compute payload length
    pub fn payload_len(&self) -> u32 {
        (108 + self.arguments.len()) as u32
    }
}

/// Result codes for responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResultCode {
    /// Success
    Success = 0,
    /// Generic failure
    Failure = 1,
    /// Capability expired
    Expired = 2,
    /// Capability revoked
    Revoked = 3,
    /// Quota exhausted
    QuotaExhausted = 4,
    /// Scope violation
    ScopeViolation = 5,
    /// Invalid signature
    InvalidSignature = 6,
    /// Insufficient rights
    InsufficientRights = 7,
    /// Internal error
    InternalError = 255,
}

impl ResultCode {
    /// Convert from u32
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Success),
            1 => Some(Self::Failure),
            2 => Some(Self::Expired),
            3 => Some(Self::Revoked),
            4 => Some(Self::QuotaExhausted),
            5 => Some(Self::ScopeViolation),
            6 => Some(Self::InvalidSignature),
            7 => Some(Self::InsufficientRights),
            255 => Some(Self::InternalError),
            _ => None,
        }
    }

    /// Check if successful
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

/// Invoke response payload
#[derive(Debug, Clone)]
pub struct InvokeResponse {
    /// Result code
    pub result: ResultCode,
    /// Remaining quota invocations after operation
    pub remaining_quota: u64,
    /// Response data (variable length)
    pub data: Vec<u8>,
}

impl InvokeResponse {
    /// Create a successful response
    pub fn success(remaining_quota: u64, data: Vec<u8>) -> Self {
        Self {
            result: ResultCode::Success,
            remaining_quota,
            data,
        }
    }

    /// Create an error response
    pub fn error(result: ResultCode, remaining_quota: u64) -> Self {
        Self {
            result,
            remaining_quota,
            data: Vec::new(),
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + self.data.len());
        bytes.extend_from_slice(&(self.result as u32).to_le_bytes());
        bytes.extend_from_slice(&self.remaining_quota.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Parse from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 12 {
            return Err(WireError::BufferTooSmall);
        }

        let result_raw = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let result = ResultCode::from_u32(result_raw).ok_or(WireError::MalformedMessage)?;

        let remaining_quota = u64::from_le_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
        ]);

        let data = bytes[12..].to_vec();

        Ok(Self {
            result,
            remaining_quota,
            data,
        })
    }

    /// Compute payload length
    pub fn payload_len(&self) -> u32 {
        (12 + self.data.len()) as u32
    }
}

/// Derive request payload
#[derive(Debug, Clone)]
pub struct DeriveRequestPayload {
    /// Parent capability proof
    pub parent_proof: CapabilityProof,
    /// New capability ID
    pub new_id: CapabilityId,
    /// New agent ID
    pub new_agent_id: AgentId,
    /// New rights
    pub new_rights: Rights,
    /// New expiry
    pub new_expires_at: Timestamp,
    /// New quota
    pub new_quota: Quota,
}

impl DeriveRequestPayload {
    /// Serialize to bytes
    /// Format: proof (104) + new_id (16) + new_agent_id (16) + rights (4) + expires (8) + quota (8) = 156 bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(156);
        bytes.extend_from_slice(&proof_to_bytes(&self.parent_proof));
        bytes.extend_from_slice(&self.new_id.0);
        bytes.extend_from_slice(&self.new_agent_id);
        bytes.extend_from_slice(&self.new_rights.bits().to_le_bytes());
        bytes.extend_from_slice(&self.new_expires_at.to_le_bytes());
        bytes.extend_from_slice(&self.new_quota.remaining_invocations().to_le_bytes());
        bytes
    }

    /// Parse from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, WireError> {
        // 104 + 16 + 16 + 4 + 8 + 8 = 156 bytes minimum
        if bytes.len() < 156 {
            return Err(WireError::BufferTooSmall);
        }

        let mut proof_bytes = [0u8; 104];
        proof_bytes.copy_from_slice(&bytes[0..104]);
        let parent_proof = proof_from_bytes(&proof_bytes);

        let mut new_id = [0u8; 16];
        new_id.copy_from_slice(&bytes[104..120]);

        let mut new_agent_id = [0u8; 16];
        new_agent_id.copy_from_slice(&bytes[120..136]);

        let new_rights = Rights::from_bits(u32::from_le_bytes([
            bytes[136],
            bytes[137],
            bytes[138],
            bytes[139],
        ]));

        let new_expires_at = u64::from_le_bytes([
            bytes[140],
            bytes[141],
            bytes[142],
            bytes[143],
            bytes[144],
            bytes[145],
            bytes[146],
            bytes[147],
        ]);

        let new_quota = u64::from_le_bytes([
            bytes[148],
            bytes[149],
            bytes[150],
            bytes[151],
            bytes[152],
            bytes[153],
            bytes[154],
            bytes[155],
        ]);

        Ok(Self {
            parent_proof,
            new_id: CapabilityId(new_id),
            new_agent_id,
            new_rights,
            new_expires_at,
            new_quota: Quota {
                max_invocations: new_quota,
                used_invocations: 0,
                max_bytes: u64::MAX,
                used_bytes: 0,
                max_duration_ns: u64::MAX,
                used_duration_ns: 0,
            },
        })
    }

    /// Compute payload length
    pub fn payload_len(&self) -> u32 {
        156 // Fixed size: proof (104) + new_id (16) + agent_id (16) + rights (4) + expires (8) + quota (8)
    }
}

/// Complete wire message (header + payload)
#[derive(Debug, Clone)]
pub struct WireMessage {
    /// Message envelope
    pub envelope: MessageEnvelope,
    /// Payload bytes
    pub payload: Vec<u8>,
}

impl WireMessage {
    /// Create a new wire message
    pub fn new(envelope: MessageEnvelope, payload: Vec<u8>) -> Self {
        Self { envelope, payload }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        bytes.extend_from_slice(&self.envelope.serialize());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Parse from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, WireError> {
        let envelope = MessageEnvelope::parse(bytes)?;
        let payload_start = HEADER_SIZE;
        let payload_end = payload_start + envelope.payload_len as usize;

        if bytes.len() < payload_end {
            return Err(WireError::BufferTooSmall);
        }

        let payload = bytes[payload_start..payload_end].to_vec();

        Ok(Self { envelope, payload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_envelope_roundtrip() {
        let envelope = MessageEnvelope::new(
            12345,
            CapabilityId([1u8; 16]),
            MessageType::Invoke,
            100,
            MessageFlags::REQUIRES_ACK,
        );

        let bytes = envelope.serialize();
        assert_eq!(bytes.len(), HEADER_SIZE);

        let parsed = MessageEnvelope::parse(&bytes).unwrap();
        assert_eq!(parsed.magic, MAGIC_NUMBER);
        assert_eq!(parsed.version, VERSION);
        assert_eq!(parsed.sequence, 12345);
        assert_eq!(parsed.capability_id.0, [1u8; 16]);
        assert_eq!(parsed.message_type, MessageType::Invoke);
        assert_eq!(parsed.payload_len, 100);
        assert!(parsed.flags.contains(MessageFlags::REQUIRES_ACK));
    }

    #[test]
    fn test_envelope_checksum() {
        let envelope = MessageEnvelope::new(1, CapabilityId([0u8; 16]), MessageType::Ping, 0, MessageFlags::NONE);

        let mut bytes = envelope.serialize();
        let parsed = MessageEnvelope::parse(&bytes).unwrap();
        assert_eq!(envelope.checksum, parsed.checksum);

        // Tamper with message
        bytes[10] ^= 0xFF;
        assert!(matches!(
            MessageEnvelope::parse(&bytes),
            Err(WireError::ChecksumMismatch)
        ));
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        assert!(matches!(
            MessageEnvelope::parse(&bytes),
            Err(WireError::InvalidMagic)
        ));
    }

    #[test]
    fn test_invoke_request_roundtrip() {
        let request = InvokeRequest {
            proof: CapabilityProof {
                issuer: [2u8; 32],
                signature: [1u8; 64],
                issued_at: 1_000_000_000,
            },
            operation: 42,
            arguments: vec![1, 2, 3, 4, 5],
        };

        let bytes = request.serialize();
        let parsed = InvokeRequest::parse(&bytes).unwrap();

        assert_eq!(parsed.proof.issuer, [2u8; 32]);
        assert_eq!(parsed.proof.signature, [1u8; 64]);
        assert_eq!(parsed.proof.issued_at, 1_000_000_000);
        assert_eq!(parsed.operation, 42);
        assert_eq!(parsed.arguments, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_invoke_response_roundtrip() {
        let response = InvokeResponse::success(50, vec![6, 7, 8, 9]);

        let bytes = response.serialize();
        let parsed = InvokeResponse::parse(&bytes).unwrap();

        assert!(parsed.result.is_success());
        assert_eq!(parsed.remaining_quota, 50);
        assert_eq!(parsed.data, vec![6, 7, 8, 9]);
    }

    #[test]
    fn test_wire_message_roundtrip() {
        let envelope = MessageEnvelope::new(
            1,
            CapabilityId([5u8; 16]),
            MessageType::InvokeResult,
            16,
            MessageFlags::NONE,
        );
        let payload = vec![0xAB; 16];

        let message = WireMessage::new(envelope, payload.clone());
        let bytes = message.serialize();
        let parsed = WireMessage::parse(&bytes).unwrap();

        assert_eq!(parsed.envelope.message_type, MessageType::InvokeResult);
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_message_type_classification() {
        assert!(MessageType::Invoke.is_request());
        assert!(MessageType::Derive.is_request());
        assert!(!MessageType::InvokeResult.is_request());

        assert!(MessageType::InvokeResult.is_response());
        assert!(MessageType::Error.is_response());
        assert!(!MessageType::Invoke.is_response());
    }

    #[test]
    fn test_derive_request_roundtrip() {
        let request = DeriveRequestPayload {
            parent_proof: CapabilityProof {
                issuer: [4u8; 32],
                signature: [3u8; 64],
                issued_at: 500_000_000,
            },
            new_id: CapabilityId([10u8; 16]),
            new_agent_id: [20u8; 16],
            new_rights: Rights(Rights::READ | Rights::WRITE),
            new_expires_at: 1000000,
            new_quota: Quota {
                max_invocations: 50,
                used_invocations: 0,
                max_bytes: u64::MAX,
                used_bytes: 0,
                max_duration_ns: u64::MAX,
                used_duration_ns: 0,
            },
        };

        let bytes = request.serialize();
        let parsed = DeriveRequestPayload::parse(&bytes).unwrap();

        assert_eq!(parsed.parent_proof.issuer, [4u8; 32]);
        assert_eq!(parsed.parent_proof.signature, [3u8; 64]);
        assert_eq!(parsed.parent_proof.issued_at, 500_000_000);
        assert_eq!(parsed.new_id.0, [10u8; 16]);
        assert_eq!(parsed.new_agent_id, [20u8; 16]);
        assert!(parsed.new_rights.has(Rights::READ));
        assert!(parsed.new_rights.has(Rights::WRITE));
        assert_eq!(parsed.new_expires_at, 1000000);
        assert_eq!(parsed.new_quota.max_invocations, 50);
    }

    #[test]
    fn test_header_size_constant() {
        // Verify header size calculation
        let size = 4 // magic
            + 2 // version
            + 2 // flags
            + 8 // sequence
            + 16 // capability_id
            + 2 // message_type
            + 4 // payload_len
            + 4; // checksum
        assert_eq!(size, HEADER_SIZE);
        assert_eq!(HEADER_SIZE, 42);
    }

    #[test]
    fn test_flags() {
        let flags = MessageFlags::NONE
            .set(MessageFlags::REQUIRES_ACK)
            .set(MessageFlags::ENCRYPTED);

        assert!(flags.contains(MessageFlags::REQUIRES_ACK));
        assert!(flags.contains(MessageFlags::ENCRYPTED));
        assert!(!flags.contains(MessageFlags::COMPRESSED));
    }
}
