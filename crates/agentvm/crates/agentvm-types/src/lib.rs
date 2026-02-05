//! # agentvm-types
//! 
//! Core types for Agentic VM - the accountable agent capsule runtime.
//! 
//! This crate provides shared types used across all Agentic VM components:
//! - Capsule identifiers and manifests
//! - Capability tokens and scopes
//! - Budget vectors and tracking
//! - Evidence bundle structures
//! 
//! ## Features
//! 
//! - `std` - Enable standard library support
//! - `serde` - Enable serialization/deserialization
//! - `alloc` - Enable alloc-only features (no_std compatible)

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod capsule;
pub mod capability;
pub mod budget;
pub mod evidence;
pub mod error;

// Re-exports for convenience
pub use capsule::{CapsuleId, CapsuleManifest, CapsuleIdentity, SignatureAlgorithm};
pub use capability::{
    CapabilityId, CapabilityType, Capability, CapabilityScope, CapabilityScopeType,
    Rights, Quota, CapabilityProof, CapabilityGrant,
};
pub use budget::{Budget, BudgetVector, QuotaConsumed};
pub use evidence::{
    EvidenceBundle, EvidenceStatement, EvidenceHeader,
    EvidenceInputs, EvidenceExecution, EvidenceOutputs,
    CapabilityCallRecord, NetworkEvent, MerkleProof,
};
pub use error::{AgentVmError, Result};

/// Hash type used throughout (SHA-256)
pub type Hash = [u8; 32];

/// Timestamp in nanoseconds since Unix epoch
pub type TimestampNs = u64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_id_generation() {
        let id1 = CapsuleId::generate();
        let id2 = CapsuleId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_capability_id_generation() {
        let id1 = CapabilityId::generate();
        let id2 = CapabilityId::generate();
        assert_ne!(id1, id2);
    }
}
