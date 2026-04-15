//! Capability token signing and verification with Ed25519.
//!
//! This module provides cryptographic operations for capability tokens:
//! - `sign_capability()` - Sign a capability with Ed25519
//! - `verify_capability()` - Verify a capability signature
//! - `SignedCapability` - A capability with its cryptographic proof

use alloc::vec::Vec;
use core::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use agentvm_types::{Capability, CapabilityId, CapabilityProof, TimestampNs};

use crate::Hash;

/// Errors that can occur during signing operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningError {
    /// Invalid signing key
    InvalidKey,
    /// Signing operation failed
    SigningFailed,
    /// Verification failed
    VerificationFailed,
    /// Invalid signature format
    InvalidSignature,
    /// Capability data is malformed
    MalformedCapability,
}

impl fmt::Display for SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => write!(f, "invalid signing key"),
            Self::SigningFailed => write!(f, "signing operation failed"),
            Self::VerificationFailed => write!(f, "signature verification failed"),
            Self::InvalidSignature => write!(f, "invalid signature format"),
            Self::MalformedCapability => write!(f, "malformed capability data"),
        }
    }
}

/// A capability with its cryptographic proof attached.
#[derive(Debug, Clone)]
pub struct SignedCapability {
    /// The capability
    pub capability: Capability,
    /// The cryptographic proof
    pub proof: CapabilityProof,
}

impl SignedCapability {
    /// Create a new signed capability
    pub fn new(capability: Capability, proof: CapabilityProof) -> Self {
        Self { capability, proof }
    }

    /// Verify this signed capability against a public key
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        verify_capability(&self.capability, &self.proof, Some(verifying_key))
    }

    /// Get the capability ID
    pub fn id(&self) -> CapabilityId {
        self.capability.id
    }
}

/// Compute the canonical bytes of a capability for signing.
///
/// The canonical format ensures consistent signing across implementations:
/// - All multi-byte integers are little-endian
/// - Strings are UTF-8 encoded with length prefix
/// - Optional fields use a presence byte
fn capability_to_signing_bytes(capability: &Capability) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);

    // Capability ID (16 bytes)
    bytes.extend_from_slice(&capability.id.0);

    // Capability type (2 bytes, little-endian)
    bytes.extend_from_slice(&(capability.cap_type as u16).to_le_bytes());

    // Rights (4 bytes, little-endian)
    bytes.extend_from_slice(&capability.rights.0.to_le_bytes());

    // Quota fields (each 8 bytes, little-endian)
    bytes.extend_from_slice(&capability.quota.max_invocations.to_le_bytes());
    bytes.extend_from_slice(&capability.quota.used_invocations.to_le_bytes());
    bytes.extend_from_slice(&capability.quota.max_bytes.to_le_bytes());
    bytes.extend_from_slice(&capability.quota.used_bytes.to_le_bytes());
    bytes.extend_from_slice(&capability.quota.max_duration_ns.to_le_bytes());
    bytes.extend_from_slice(&capability.quota.used_duration_ns.to_le_bytes());

    // Expiration (8 bytes, little-endian)
    bytes.extend_from_slice(&capability.expires_at.to_le_bytes());

    // Parent capability (1 byte presence + optional 16 bytes)
    match &capability.parent {
        Some(parent_id) => {
            bytes.push(1);
            bytes.extend_from_slice(&parent_id.0);
        }
        None => {
            bytes.push(0);
        }
    }

    // Revoked flag (1 byte)
    bytes.push(if capability.revoked { 1 } else { 0 });

    bytes
}

/// Compute the hash of capability bytes for signing.
fn hash_capability_bytes(bytes: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Sign a capability with the given signing key.
///
/// Creates a `CapabilityProof` containing:
/// - The issuer's public key hash
/// - Ed25519 signature over the canonical capability bytes
/// - Issuance timestamp
///
/// # Arguments
/// * `capability` - The capability to sign
/// * `signing_key` - The Ed25519 signing key
/// * `issued_at` - The issuance timestamp
///
/// # Returns
/// A `CapabilityProof` that can be attached to the capability
///
/// # Example
/// ```ignore
/// let proof = sign_capability(&cap, &signing_key, current_time);
/// ```
pub fn sign_capability(
    capability: &Capability,
    signing_key: &SigningKey,
    issued_at: TimestampNs,
) -> CapabilityProof {
    // Get canonical bytes
    let message = capability_to_signing_bytes(capability);

    // Sign the message
    let signature = signing_key.sign(&message);

    // Compute issuer hash (SHA-256 of public key)
    let verifying_key = signing_key.verifying_key();
    let issuer = hash_capability_bytes(verifying_key.as_bytes());

    CapabilityProof {
        issuer,
        signature: signature.to_bytes(),
        issued_at,
    }
}

/// Verify a capability signature.
///
/// Checks that the signature in the proof is valid for the given capability
/// using either the provided verifying key or by looking up the issuer.
///
/// # Arguments
/// * `capability` - The capability to verify
/// * `proof` - The proof containing the signature
/// * `verifying_key` - Optional verifying key; if None, issuer lookup would be needed
///
/// # Returns
/// `true` if the signature is valid, `false` otherwise
pub fn verify_capability(
    capability: &Capability,
    proof: &CapabilityProof,
    verifying_key: Option<&VerifyingKey>,
) -> bool {
    // Get canonical bytes (same as signing)
    let message = capability_to_signing_bytes(capability);

    // Get verifying key
    let key = match verifying_key {
        Some(k) => k.clone(),
        None => {
            // In a real implementation, we would look up the key by issuer hash
            // For now, we require the key to be provided
            return false;
        }
    };

    // Verify issuer hash matches the key
    let expected_issuer = hash_capability_bytes(key.as_bytes());
    if proof.issuer != expected_issuer {
        return false;
    }

    // Parse signature - from_bytes takes a &[u8; 64] reference
    let sig_bytes: &[u8; 64] = match proof.signature.as_slice().try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(sig_bytes);

    // Verify signature
    key.verify(&message, &signature).is_ok()
}

/// Verify a capability and return detailed error information.
pub fn verify_capability_detailed(
    capability: &Capability,
    proof: &CapabilityProof,
    verifying_key: &VerifyingKey,
) -> Result<(), SigningError> {
    let message = capability_to_signing_bytes(capability);

    // Verify issuer hash
    let expected_issuer = hash_capability_bytes(verifying_key.as_bytes());
    if proof.issuer != expected_issuer {
        return Err(SigningError::InvalidKey);
    }

    // Parse signature - from_bytes takes a &[u8; 64] reference
    let sig_bytes: &[u8; 64] = proof.signature.as_slice()
        .try_into()
        .map_err(|_| SigningError::InvalidSignature)?;
    let signature = Signature::from_bytes(sig_bytes);

    // Verify
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| SigningError::VerificationFailed)
}

/// Create a signed capability in one step.
///
/// Convenience function that signs a capability and returns it wrapped
/// with its proof.
pub fn create_signed_capability(
    capability: Capability,
    signing_key: &SigningKey,
    issued_at: TimestampNs,
) -> SignedCapability {
    let proof = sign_capability(&capability, signing_key, issued_at);
    SignedCapability::new(capability, proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentvm_types::{CapabilityScope, CapabilityType, Quota, Rights};

    fn test_signing_key() -> SigningKey {
        // Fixed test key for reproducibility
        let secret = [42u8; 32];
        SigningKey::from_bytes(&secret)
    }

    fn test_capability() -> Capability {
        Capability {
            id: CapabilityId::from_bytes([1u8; 16]),
            cap_type: CapabilityType::FileRead,
            scope: CapabilityScope::Unrestricted,
            rights: Rights(Rights::READ | Rights::WRITE),
            quota: Quota {
                max_invocations: 100,
                used_invocations: 0,
                max_bytes: 1_000_000,
                used_bytes: 0,
                max_duration_ns: 60_000_000_000,
                used_duration_ns: 0,
            },
            expires_at: 2_000_000_000_000_000_000, // Far future
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0u8; 64],
                issued_at: 0,
            },
            revoked: false,
        }
    }

    #[test]
    fn test_sign_and_verify() {
        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let capability = test_capability();
        let issued_at = 1_000_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        assert!(verify_capability(&capability, &proof, Some(&verifying_key)));
    }

    #[test]
    fn test_verify_fails_on_tampered_capability() {
        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let mut capability = test_capability();
        let issued_at = 1_000_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        // Tamper with capability
        capability.quota.max_invocations = 999;

        assert!(!verify_capability(&capability, &proof, Some(&verifying_key)));
    }

    #[test]
    fn test_verify_fails_with_wrong_key() {
        let signing_key = test_signing_key();
        let wrong_key = SigningKey::from_bytes(&[99u8; 32]);
        let capability = test_capability();
        let issued_at = 1_000_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        assert!(!verify_capability(
            &capability,
            &proof,
            Some(&wrong_key.verifying_key())
        ));
    }

    #[test]
    fn test_signed_capability() {
        let signing_key = test_signing_key();
        let capability = test_capability();
        let issued_at = 1_000_000_000_000_000_000;

        let signed = create_signed_capability(capability, &signing_key, issued_at);

        assert!(signed.verify(&signing_key.verifying_key()));
    }

    #[test]
    fn test_capability_with_parent() {
        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let mut capability = test_capability();
        capability.parent = Some(CapabilityId::from_bytes([99u8; 16]));
        let issued_at = 1_000_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        assert!(verify_capability(&capability, &proof, Some(&verifying_key)));
    }

    #[test]
    fn test_verify_detailed_error() {
        let signing_key = test_signing_key();
        let wrong_key = SigningKey::from_bytes(&[99u8; 32]);
        let capability = test_capability();
        let issued_at = 1_000_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        let result = verify_capability_detailed(&capability, &proof, &wrong_key.verifying_key());
        assert_eq!(result, Err(SigningError::InvalidKey));
    }

    #[test]
    fn test_proof_timestamp() {
        let signing_key = test_signing_key();
        let capability = test_capability();
        let issued_at = 1_500_000_000_000_000_000;

        let proof = sign_capability(&capability, &signing_key, issued_at);

        assert_eq!(proof.issued_at, issued_at);
    }
}
