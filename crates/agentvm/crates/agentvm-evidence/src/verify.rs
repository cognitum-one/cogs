//! Evidence verification and replay checking.
//!
//! This module provides verification of:
//! - Bundle signatures
//! - Chain integrity
//! - Replay comparison

use alloc::string::String;
use alloc::vec::Vec;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Verifier, VerifyingKey};

use crate::bundle::EvidenceBundle;
use crate::merkle::MerkleTree;
use crate::sign::{DsseEnvelope, SignedBundle, PAYLOAD_TYPE};
use crate::Hash;

/// Result of signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// All signatures verified successfully
    Valid,
    /// No signatures present
    NoSignatures,
    /// One or more signatures failed verification
    InvalidSignature(String),
    /// Payload type mismatch
    InvalidPayloadType,
    /// Payload could not be decoded
    InvalidPayload,
}

impl VerificationResult {
    /// Returns true if verification passed.
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid)
    }
}

/// Errors that can occur during verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// Invalid public key format
    InvalidPublicKey,
    /// Invalid signature format
    InvalidSignatureFormat,
    /// Key ID not found in provided keys
    KeyNotFound(String),
    /// Payload decoding failed
    PayloadDecodeFailed,
    /// Chain integrity violation
    ChainIntegrityViolation(String),
}

/// Public key registry for verification.
pub struct PublicKeyRegistry {
    keys: Vec<(String, VerifyingKey)>,
}

impl Default for PublicKeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PublicKeyRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Adds a public key to the registry.
    pub fn add_key(&mut self, keyid: impl Into<String>, key: VerifyingKey) {
        self.keys.push((keyid.into(), key));
    }

    /// Adds a public key from raw bytes.
    pub fn add_key_bytes(
        &mut self,
        keyid: impl Into<String>,
        key_bytes: &[u8; 32],
    ) -> Result<(), VerificationError> {
        let key =
            VerifyingKey::from_bytes(key_bytes).map_err(|_| VerificationError::InvalidPublicKey)?;
        self.add_key(keyid, key);
        Ok(())
    }

    /// Looks up a key by ID.
    pub fn get(&self, keyid: &str) -> Option<&VerifyingKey> {
        self.keys.iter().find(|(id, _)| id == keyid).map(|(_, k)| k)
    }

    /// Returns the number of keys in the registry.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Verifies all signatures on a signed bundle.
pub fn verify_bundle_signature(
    signed: &SignedBundle,
    registry: &PublicKeyRegistry,
) -> Result<VerificationResult, VerificationError> {
    verify_envelope_signatures(&signed.envelope, registry)
}

/// Verifies all signatures on a DSSE envelope.
pub fn verify_envelope_signatures(
    envelope: &DsseEnvelope,
    registry: &PublicKeyRegistry,
) -> Result<VerificationResult, VerificationError> {
    // Check payload type
    if envelope.payload_type != PAYLOAD_TYPE {
        return Ok(VerificationResult::InvalidPayloadType);
    }

    // Check we have signatures
    if envelope.signatures.is_empty() {
        return Ok(VerificationResult::NoSignatures);
    }

    // Decode payload
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(&envelope.payload)
        .map_err(|_| VerificationError::PayloadDecodeFailed)?;

    // Compute PAE
    let pae = compute_pae(&envelope.payload_type, &payload_bytes);

    // Verify each signature
    for sig in &envelope.signatures {
        let verifying_key = registry
            .get(&sig.keyid)
            .ok_or_else(|| VerificationError::KeyNotFound(sig.keyid.clone()))?;

        let sig_bytes = URL_SAFE_NO_PAD
            .decode(&sig.sig)
            .map_err(|_| VerificationError::InvalidSignatureFormat)?;

        let signature: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| VerificationError::InvalidSignatureFormat)?;

        let ed_sig = ed25519_dalek::Signature::from_bytes(&signature);

        if verifying_key.verify(&pae, &ed_sig).is_err() {
            return Ok(VerificationResult::InvalidSignature(sig.keyid.clone()));
        }
    }

    Ok(VerificationResult::Valid)
}

/// Computes the DSSE Pre-Authentication Encoding.
fn compute_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let type_bytes = payload_type.as_bytes();

    let mut pae = Vec::with_capacity(7 + 1 + 8 + 1 + type_bytes.len() + 1 + 8 + 1 + payload.len());

    pae.extend_from_slice(b"DSSEv1 ");
    pae.extend_from_slice(&(type_bytes.len() as u64).to_le_bytes());
    pae.push(b' ');
    pae.extend_from_slice(type_bytes);
    pae.push(b' ');
    pae.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    pae.push(b' ');
    pae.extend_from_slice(payload);

    pae
}

/// Verifies the integrity of a chain of evidence bundles.
pub fn verify_chain_integrity(bundles: &[EvidenceBundle]) -> Result<bool, VerificationError> {
    if bundles.is_empty() {
        return Ok(true);
    }

    // Build a Merkle tree and verify each bundle
    let mut tree = MerkleTree::new();

    for (i, bundle) in bundles.iter().enumerate() {
        let hash = bundle.compute_hash();

        // Verify sequence number
        if bundle.sequence() != i as u64 {
            return Err(VerificationError::ChainIntegrityViolation(alloc::format!(
                "Expected sequence {}, got {}",
                i,
                bundle.sequence()
            )));
        }

        // Verify previous hash (for non-genesis bundles)
        if i > 0 {
            let prev_hash = bundles[i - 1].compute_hash();
            let expected_prev = crate::format_hash(&prev_hash);

            if bundle.statement.chain.previous_hash != expected_prev {
                return Err(VerificationError::ChainIntegrityViolation(alloc::format!(
                    "Previous hash mismatch at bundle {}",
                    i
                )));
            }
        }

        // Add to tree and verify merkle root
        let new_root = tree.append(hash);
        let expected_root = crate::format_hash(&new_root);

        if bundle.merkle_root() != expected_root {
            return Err(VerificationError::ChainIntegrityViolation(alloc::format!(
                "Merkle root mismatch at bundle {}",
                i
            )));
        }
    }

    Ok(true)
}

/// Mismatch types for replay verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// Capability type differs at given index
    CapabilityType {
        /// Index of the mismatched capability call
        index: usize,
    },
    /// Request hash differs at given index
    RequestHash {
        /// Index of the mismatched capability call
        index: usize,
    },
    /// Response hash differs at given index
    ResponseHash {
        /// Index of the mismatched capability call
        index: usize,
    },
    /// Capability count differs
    CapabilityCount {
        /// Number of capability calls in original bundle
        original: usize,
        /// Number of capability calls in replay bundle
        replay: usize,
    },
    /// Workspace diff hash differs
    WorkspaceDiff,
    /// Exit code differs
    ExitCode {
        /// Exit code from original execution
        original: i32,
        /// Exit code from replay execution
        replay: i32,
    },
}

/// Result of replay verification.
#[derive(Debug, Clone)]
pub struct ReplayVerification {
    /// Whether the replay matched the original
    pub matches: bool,
    /// List of mismatches found
    pub mismatches: Vec<Mismatch>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

impl ReplayVerification {
    /// Creates a successful verification result.
    pub fn success() -> Self {
        Self {
            matches: true,
            mismatches: Vec::new(),
            confidence: 1.0,
        }
    }

    /// Creates a failed verification result with mismatches.
    pub fn failed(mismatches: Vec<Mismatch>) -> Self {
        let confidence = calculate_confidence(&mismatches);
        Self {
            matches: false,
            mismatches,
            confidence,
        }
    }
}

/// Verifies that a replay produces the same external effects as the original.
pub fn verify_replay(
    original: &EvidenceBundle,
    replay: &EvidenceBundle,
) -> ReplayVerification {
    let mut mismatches = Vec::new();

    let orig_calls = original.capability_calls();
    let replay_calls = replay.capability_calls();

    // Compare capability call counts
    if orig_calls.len() != replay_calls.len() {
        mismatches.push(Mismatch::CapabilityCount {
            original: orig_calls.len(),
            replay: replay_calls.len(),
        });
    }

    // Compare individual capability calls
    let min_len = orig_calls.len().min(replay_calls.len());
    for i in 0..min_len {
        let orig = &orig_calls[i];
        let replay_call = &replay_calls[i];

        if orig.capability_type != replay_call.capability_type {
            mismatches.push(Mismatch::CapabilityType { index: i });
        }

        if orig.request_hash != replay_call.request_hash {
            mismatches.push(Mismatch::RequestHash { index: i });
        }

        // Response hash may differ for non-deterministic services
        // but we still record it for investigation
        if orig.response_hash != replay_call.response_hash {
            mismatches.push(Mismatch::ResponseHash { index: i });
        }
    }

    // Compare workspace diff
    if original.statement.outputs.workspace_diff_hash
        != replay.statement.outputs.workspace_diff_hash
    {
        mismatches.push(Mismatch::WorkspaceDiff);
    }

    // Compare exit code
    if original.exit_code() != replay.exit_code() {
        mismatches.push(Mismatch::ExitCode {
            original: original.exit_code(),
            replay: replay.exit_code(),
        });
    }

    if mismatches.is_empty() {
        ReplayVerification::success()
    } else {
        ReplayVerification::failed(mismatches)
    }
}

/// Calculates confidence score based on mismatches.
fn calculate_confidence(mismatches: &[Mismatch]) -> f64 {
    if mismatches.is_empty() {
        return 1.0;
    }

    // Weight different types of mismatches
    let mut penalty = 0.0;

    for mismatch in mismatches {
        penalty += match mismatch {
            Mismatch::CapabilityType { .. } => 0.3,
            Mismatch::RequestHash { .. } => 0.2,
            Mismatch::ResponseHash { .. } => 0.05, // Non-deterministic responses are expected
            Mismatch::CapabilityCount { .. } => 0.4,
            Mismatch::WorkspaceDiff => 0.3,
            Mismatch::ExitCode { .. } => 0.25,
        };
    }

    (1.0_f64 - penalty).max(0.0_f64)
}

/// Batch verification of multiple bundles.
pub struct BatchVerifier {
    registry: PublicKeyRegistry,
    results: Vec<(usize, VerificationResult)>,
}

impl BatchVerifier {
    /// Creates a new batch verifier with the given key registry.
    pub fn new(registry: PublicKeyRegistry) -> Self {
        Self {
            registry,
            results: Vec::new(),
        }
    }

    /// Verifies a signed bundle and records the result.
    pub fn verify(&mut self, index: usize, signed: &SignedBundle) -> &VerificationResult {
        let result = verify_bundle_signature(signed, &self.registry)
            .unwrap_or_else(|e| VerificationResult::InvalidSignature(alloc::format!("{:?}", e)));

        self.results.push((index, result));
        &self.results.last().unwrap().1
    }

    /// Returns all verification results.
    pub fn results(&self) -> &[(usize, VerificationResult)] {
        &self.results
    }

    /// Returns the number of verified bundles.
    pub fn verified_count(&self) -> usize {
        self.results.iter().filter(|(_, r)| r.is_valid()).count()
    }

    /// Returns the number of failed verifications.
    pub fn failed_count(&self) -> usize {
        self.results.len() - self.verified_count()
    }

    /// Returns true if all verifications passed.
    pub fn all_valid(&self) -> bool {
        self.results.iter().all(|(_, r)| r.is_valid())
    }
}

/// Verifies Merkle inclusion proof for a bundle.
pub fn verify_merkle_inclusion(
    bundle: &EvidenceBundle,
    tree_size: usize,
    expected_root: &Hash,
) -> bool {
    let bundle_hash = bundle.compute_hash();
    let leaf_index = bundle.sequence() as usize;

    // Parse the inclusion proof from the chain info
    let proof: Vec<Hash> = bundle
        .statement
        .chain
        .inclusion_proof
        .iter()
        .filter_map(|s| crate::parse_hash(s))
        .collect();

    MerkleTree::verify_inclusion(&bundle_hash, leaf_index, tree_size, &proof, expected_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use crate::bundle::EvidenceBundleBuilder;
    use crate::sign::{sign_bundle, Ed25519Signer, SigningKeyTrait};
    use crate::statement::CapabilityCall;

    fn create_test_bundle(sequence: u64) -> EvidenceBundle {
        EvidenceBundleBuilder::new()
            .run_id([sequence as u8; 16])
            .capsule_id("test-capsule")
            .timestamp_ns(1234567890 + sequence)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(0)
            .workspace_diff_hash([3; 32])
            .sequence(sequence)
            .previous_hash([(sequence.saturating_sub(1)) as u8; 32])
            .merkle_root([4; 32])
            .build()
            .expect("should build")
    }

    #[test]
    fn test_verification_result_is_valid() {
        assert!(VerificationResult::Valid.is_valid());
        assert!(!VerificationResult::NoSignatures.is_valid());
        assert!(!VerificationResult::InvalidPayloadType.is_valid());
    }

    #[test]
    fn test_public_key_registry() {
        let mut registry = PublicKeyRegistry::new();
        assert!(registry.is_empty());

        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();
        let verifying_key = signer.verifying_key();

        registry.add_key(&keyid, verifying_key);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.get(&keyid).is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_verify_signed_bundle() {
        let bundle = create_test_bundle(0);
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        let signer_for_verify = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let mut registry = PublicKeyRegistry::new();
        registry.add_key(&keyid, signer_for_verify.verifying_key());

        let result = verify_bundle_signature(&signed, &registry).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_verify_wrong_key() {
        let bundle = create_test_bundle(0);
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        // Use a different key for verification
        let wrong_signer = Ed25519Signer::from_bytes(&[2u8; 32], "test");
        let mut registry = PublicKeyRegistry::new();
        registry.add_key(&keyid, wrong_signer.verifying_key());

        let result = verify_bundle_signature(&signed, &registry).unwrap();
        assert!(!result.is_valid());
        assert!(matches!(result, VerificationResult::InvalidSignature(_)));
    }

    #[test]
    fn test_verify_missing_key() {
        let bundle = create_test_bundle(0);
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        let registry = PublicKeyRegistry::new(); // Empty registry

        let result = verify_bundle_signature(&signed, &registry);
        assert!(matches!(result, Err(VerificationError::KeyNotFound(_))));
    }

    #[test]
    fn test_replay_verification_match() {
        let bundle1 = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(0)
            .workspace_diff_hash([3; 32])
            .add_capability_call(CapabilityCall::new(0, 1000, "http", "req1", "resp1"))
            .build()
            .unwrap();

        let bundle2 = EvidenceBundleBuilder::new()
            .run_id([1; 16])
            .capsule_id("test")
            .timestamp_ns(12346)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(0)
            .workspace_diff_hash([3; 32])
            .add_capability_call(CapabilityCall::new(0, 2000, "http", "req1", "resp1"))
            .build()
            .unwrap();

        let result = verify_replay(&bundle1, &bundle2);
        assert!(result.matches);
        assert!(result.mismatches.is_empty());
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_replay_verification_mismatch() {
        let bundle1 = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(0)
            .workspace_diff_hash([3; 32])
            .add_capability_call(CapabilityCall::new(0, 1000, "http", "req1", "resp1"))
            .build()
            .unwrap();

        let bundle2 = EvidenceBundleBuilder::new()
            .run_id([1; 16])
            .capsule_id("test")
            .timestamp_ns(12346)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(1) // Different exit code
            .workspace_diff_hash([4; 32]) // Different diff
            .add_capability_call(CapabilityCall::new(0, 2000, "file", "req2", "resp2")) // Different call
            .build()
            .unwrap();

        let result = verify_replay(&bundle1, &bundle2);
        assert!(!result.matches);
        assert!(!result.mismatches.is_empty());
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_mismatch_types() {
        let mismatch = Mismatch::CapabilityType { index: 0 };
        assert!(matches!(mismatch, Mismatch::CapabilityType { index: 0 }));

        let mismatch = Mismatch::ExitCode {
            original: 0,
            replay: 1,
        };
        assert!(matches!(
            mismatch,
            Mismatch::ExitCode {
                original: 0,
                replay: 1
            }
        ));
    }

    #[test]
    fn test_calculate_confidence() {
        // No mismatches = full confidence
        assert_eq!(calculate_confidence(&[]), 1.0);

        // Response hash mismatches have low penalty
        let mismatches = vec![Mismatch::ResponseHash { index: 0 }];
        let confidence = calculate_confidence(&mismatches);
        assert!(confidence > 0.9);

        // Major mismatches have high penalty
        let mismatches = vec![
            Mismatch::CapabilityCount {
                original: 5,
                replay: 3,
            },
            Mismatch::WorkspaceDiff,
        ];
        let confidence = calculate_confidence(&mismatches);
        assert!(confidence < 0.5);
    }

    #[test]
    fn test_batch_verifier() {
        let bundle = create_test_bundle(0);
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        let signer_for_verify = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let mut registry = PublicKeyRegistry::new();
        registry.add_key(&keyid, signer_for_verify.verifying_key());

        let mut verifier = BatchVerifier::new(registry);
        verifier.verify(0, &signed);

        assert_eq!(verifier.verified_count(), 1);
        assert_eq!(verifier.failed_count(), 0);
        assert!(verifier.all_valid());
    }

    #[test]
    fn test_batch_verifier_mixed() {
        let bundle = create_test_bundle(0);
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        // Registry with wrong key
        let wrong_signer = Ed25519Signer::from_bytes(&[2u8; 32], "test");
        let mut registry = PublicKeyRegistry::new();
        registry.add_key(&keyid, wrong_signer.verifying_key());

        let mut verifier = BatchVerifier::new(registry);
        verifier.verify(0, &signed);

        assert_eq!(verifier.verified_count(), 0);
        assert_eq!(verifier.failed_count(), 1);
        assert!(!verifier.all_valid());
    }

    #[test]
    fn test_public_key_registry_add_bytes() {
        let mut registry = PublicKeyRegistry::new();

        // Use a valid key from a known signer
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let valid_key: [u8; 32] = signer.public_key().try_into().unwrap();

        let result = registry.add_key_bytes("key1", &valid_key);
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);

        // Adding another key should work
        let signer2 = Ed25519Signer::from_bytes(&[2u8; 32], "test");
        let valid_key2: [u8; 32] = signer2.public_key().try_into().unwrap();

        let result = registry.add_key_bytes("key2", &valid_key2);
        assert!(result.is_ok());
        assert_eq!(registry.len(), 2);
    }
}
