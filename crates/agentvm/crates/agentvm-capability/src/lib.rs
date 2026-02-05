//! # agentvm-capability
//!
//! Capability protocol implementation for Agentic VM.
//!
//! This crate provides:
//! - **Wire protocol** for capability messages (42-byte header envelope)
//! - **Capability derivation** (attenuation) for least-privilege delegation
//! - **Capability validation** against operations
//! - **Ed25519 signature** generation and verification
//! - **Enhanced scope matching** for network and filesystem patterns
//!
//! ## Architecture
//!
//! The capability protocol follows object-capability security principles:
//! - Capabilities are unforgeable tokens granting specific rights
//! - Capabilities can be derived (attenuated) but never amplified
//! - All capability operations are cryptographically signed
//! - Wire protocol enables secure transmission of capability messages
//!
//! ## Wire Protocol
//!
//! Messages use a 42-byte envelope header:
//! ```text
//! Offset  Size  Field
//! 0       4     magic (0x43415056 = "CAPV")
//! 4       2     version
//! 6       2     flags
//! 8       8     sequence
//! 16      16    capability_id
//! 32      2     message_type
//! 34      4     payload_len
//! 38      4     checksum (CRC32C)
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod derive;
pub mod scope;
pub mod token;
pub mod validate;
pub mod wire;

// Re-export core types from agentvm-types for convenience
pub use agentvm_types::{
    Capability, CapabilityGrant, CapabilityId, CapabilityProof, CapabilityScope,
    CapabilityType, Quota, Rights, TimestampNs,
};
pub use agentvm_types::capability::CapabilityScopeType;

// Re-exports from this crate's modules
pub use derive::{derive_capability, DeriveError, DeriveRequest, DeriveResult};
pub use scope::{HostPattern, PathPattern, ScopeChecker};
pub use token::{sign_capability, verify_capability, SignedCapability, SigningError};
pub use validate::{validate_capability, ValidationContext, ValidationOptions, ValidationResult};
pub use wire::{
    DeriveRequestPayload, InvokeRequest, InvokeResponse, MessageEnvelope, MessageFlags,
    MessageType, ResultCode, WireError, WireMessage, HEADER_SIZE, MAGIC_NUMBER, MAX_PAYLOAD_SIZE,
    VERSION,
};

/// Agent/Capsule identifier (128-bit)
pub type AgentId = [u8; 16];

/// Unix timestamp in seconds (for wire protocol compatibility)
pub type Timestamp = u64;

/// Sequence number for wire messages
pub type SequenceNumber = u64;

/// Hash type (SHA-256, 32 bytes)
pub type Hash = [u8; 32];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_reexports() {
        // Verify core types are accessible
        let _cap_id = CapabilityId::generate();
        let _rights = Rights::NONE;
        let _quota = Quota::UNLIMITED;
    }
}
