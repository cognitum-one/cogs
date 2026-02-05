//! # agentvm-evidence
//!
//! Evidence chain implementation for Agentic VM.
//!
//! This crate provides tamper-evident, cryptographically-signed evidence bundles
//! for tracking agent execution. It implements the evidence chain architecture
//! from ADR-006, combining patterns from Sigstore, in-toto, and Certificate
//! Transparency.
//!
//! ## Features
//!
//! - **Merkle Tree**: Append-only log with inclusion and consistency proofs
//! - **Evidence Bundles**: Structured evidence of agent execution
//! - **DSSE Signing**: Dead Simple Signing Envelope format
//! - **Verification**: Bundle signature and chain integrity verification
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agentvm_evidence::{
//!     merkle::MerkleTree,
//!     bundle::EvidenceBundleBuilder,
//!     sign::Ed25519Signer,
//! };
//!
//! // Create a Merkle tree for evidence chaining
//! let mut tree = MerkleTree::new();
//!
//! // Build an evidence bundle
//! let bundle = EvidenceBundleBuilder::new()
//!     .run_id([0u8; 16])
//!     .capsule_id("my-capsule")
//!     .timestamp_ns(1234567890)
//!     .manifest_hash([0u8; 32])
//!     .workspace_hash([0u8; 32])
//!     .exit_code(0)
//!     .workspace_diff_hash([0u8; 32])
//!     .build()
//!     .expect("valid bundle");
//!
//! // Add bundle hash to the tree
//! let bundle_hash = bundle.compute_hash();
//! let new_root = tree.append(bundle_hash);
//! ```
//!
//! ## no_std Support
//!
//! This crate is `no_std` compatible with the `alloc` crate.
//! Enable the `std` feature for standard library support.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

extern crate alloc;

pub mod bundle;
pub mod merkle;
pub mod sign;
pub mod statement;
pub mod verify;

// Re-exports for convenience
pub use bundle::{EvidenceBundle, EvidenceBundleBuilder};
pub use merkle::MerkleTree;
pub use sign::{DsseEnvelope, Ed25519Signer, Signature, SignedBundle, SigningKeyTrait};
pub use statement::{
    Budget, CapabilityCall, ChainInfo, EvidenceStatement, ExecutionInfo, Header, Inputs,
    NetworkEvent, Outputs,
};
pub use verify::{Mismatch, ReplayVerification, VerificationError, VerificationResult};

/// Hash type alias - SHA-256 produces 32 bytes
pub type Hash = [u8; 32];

/// Computes SHA-256 hash of the input data
pub fn sha256(data: &[u8]) -> Hash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Formats a hash as a prefixed hex string (sha256:...)
pub fn format_hash(hash: &Hash) -> alloc::string::String {
    use alloc::format;
    let hex: alloc::string::String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sha256:{}", hex)
}

/// Parses a prefixed hash string (sha256:...) into bytes
pub fn parse_hash(s: &str) -> Option<Hash> {
    let hex_str = s.strip_prefix("sha256:")?;
    if hex_str.len() != 64 {
        return None;
    }

    let mut hash = [0u8; 32];
    for (i, chunk) in hex_str.as_bytes().chunks(2).enumerate() {
        let high = hex_char_to_nibble(chunk[0])?;
        let low = hex_char_to_nibble(chunk[1])?;
        hash[i] = (high << 4) | low;
    }
    Some(hash)
}

fn hex_char_to_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let data = b"hello world";
        let hash = sha256(data);
        // Known SHA-256 hash of "hello world"
        let expected = [
            0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52, 0xd7, 0xda, 0x7d,
            0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88, 0xf7, 0xac,
            0xe2, 0xef, 0xcd, 0xe9,
        ];
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_format_hash() {
        let hash = [0u8; 32];
        let formatted = format_hash(&hash);
        assert!(formatted.starts_with("sha256:"));
        assert_eq!(formatted.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_parse_hash_roundtrip() {
        let original = [
            0xab, 0xcd, 0xef, 0x12, 0xab, 0xcd, 0xef, 0x12,
            0xab, 0xcd, 0xef, 0x12, 0xab, 0xcd, 0xef, 0x12,
            0xab, 0xcd, 0xef, 0x12, 0xab, 0xcd, 0xef, 0x12,
            0xab, 0xcd, 0xef, 0x12, 0xab, 0xcd, 0xef, 0x12,
        ];
        let formatted = format_hash(&original);
        let parsed = parse_hash(&formatted).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_parse_hash_invalid() {
        assert!(parse_hash("invalid").is_none());
        assert!(parse_hash("sha256:abc").is_none()); // Too short
        assert!(parse_hash("md5:0000000000000000000000000000000000000000000000000000000000000000")
            .is_none()); // Wrong prefix
    }
}
