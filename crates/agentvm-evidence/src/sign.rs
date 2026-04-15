//! Signing functionality for evidence bundles

use alloc::string::String;
use alloc::vec::Vec;

use agentvm_types::evidence::EvidenceSignature;
use crate::Hash;

/// Trait for signing evidence
pub trait Signer {
    /// Get the key identifier
    fn key_id(&self) -> String;

    /// Sign data and return the signature
    fn sign(&self, data: &[u8]) -> Vec<u8>;

    /// Verify a signature
    fn verify(&self, data: &[u8], signature: &[u8]) -> bool;
}

/// Ed25519 signer implementation
pub struct Ed25519Signer {
    /// Key identifier
    key_id: String,
    /// Secret key (32 bytes)
    secret_key: [u8; 32],
    /// Public key (32 bytes)
    public_key: [u8; 32],
}

impl Ed25519Signer {
    /// Create a new signer with the given key pair
    pub fn new(key_id: String, secret_key: [u8; 32], public_key: [u8; 32]) -> Self {
        Self {
            key_id,
            secret_key,
            public_key,
        }
    }

    /// Generate a new key pair
    #[cfg(feature = "std")]
    pub fn generate(key_id: String) -> Self {
        // Simplified - would use proper cryptographic key generation
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let mut secret_key = [0u8; 32];
        let mut public_key = [0u8; 32];

        // Simple deterministic "key generation" for testing
        for i in 0..32 {
            secret_key[i] = ((seed >> (i % 8)) & 0xFF) as u8;
            public_key[i] = secret_key[i] ^ 0x5A;
        }

        Self {
            key_id,
            secret_key,
            public_key,
        }
    }

    /// Get the public key
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }
}

impl Signer for Ed25519Signer {
    fn key_id(&self) -> String {
        self.key_id.clone()
    }

    fn sign(&self, data: &[u8]) -> Vec<u8> {
        // Simplified signature - would use actual Ed25519 in production
        // This creates a deterministic "signature" for testing
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(&self.secret_key);
        hasher.update(data);
        let hash1: [u8; 32] = hasher.finalize().into();

        let mut hasher2 = Sha256::new();
        hasher2.update(&hash1);
        hasher2.update(&self.secret_key);
        let hash2: [u8; 32] = hasher2.finalize().into();

        let mut signature = Vec::with_capacity(64);
        signature.extend_from_slice(&hash1);
        signature.extend_from_slice(&hash2);
        signature
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        if signature.len() != 64 {
            return false;
        }

        // Verify by recomputing
        let expected = self.sign(data);
        expected == signature
    }
}

/// Create a signature for evidence data
pub fn sign_evidence<S: Signer>(signer: &S, data: &[u8]) -> EvidenceSignature {
    EvidenceSignature {
        keyid: signer.key_id(),
        sig: signer.sign(data),
    }
}

/// Verify an evidence signature
pub fn verify_evidence_signature<S: Signer>(
    signer: &S,
    data: &[u8],
    signature: &EvidenceSignature,
) -> bool {
    if signature.keyid != signer.key_id() {
        return false;
    }
    signer.verify(data, &signature.sig)
}

/// Multi-signer for threshold signatures
pub struct MultiSigner {
    signers: Vec<Box<dyn Signer>>,
    threshold: usize,
}

impl MultiSigner {
    /// Create a new multi-signer
    pub fn new(threshold: usize) -> Self {
        Self {
            signers: Vec::new(),
            threshold,
        }
    }

    /// Add a signer
    pub fn add_signer(&mut self, signer: Box<dyn Signer>) {
        self.signers.push(signer);
    }

    /// Sign with all signers
    pub fn sign_all(&self, data: &[u8]) -> Vec<EvidenceSignature> {
        self.signers
            .iter()
            .map(|s| sign_evidence(s.as_ref(), data))
            .collect()
    }

    /// Verify that at least threshold signatures are valid
    pub fn verify_threshold(&self, data: &[u8], signatures: &[EvidenceSignature]) -> bool {
        let valid_count = signatures
            .iter()
            .filter(|sig| {
                self.signers
                    .iter()
                    .any(|s| verify_evidence_signature(s.as_ref(), data, sig))
            })
            .count();

        valid_count >= self.threshold
    }
}
