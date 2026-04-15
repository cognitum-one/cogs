//! Evidence chain implementation for Agentic VM
//!
//! This crate implements Merkle-chained evidence logs with DSSE-format
//! attestations for tamper-evident audit trails.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod bundle;
pub mod merkle;
pub mod sign;
pub mod verify;

pub use bundle::{EvidenceBuilder, EvidenceLogger};
pub use merkle::{MerkleTree, InclusionProof, ConsistencyProof};
pub use sign::{Signer, Ed25519Signer};
pub use verify::{verify_inclusion, verify_consistency, verify_bundle};

/// Hash type (SHA-256)
pub type Hash = [u8; 32];

/// Compute SHA-256 hash of data
pub fn sha256(data: &[u8]) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash two values together (for Merkle tree)
pub fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests;
