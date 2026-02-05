//! Evidence signing with DSSE envelope format.
//!
//! This module implements signing for evidence bundles using the
//! Dead Simple Signing Envelope (DSSE) format, compatible with Sigstore.

use alloc::string::String;
use alloc::vec::Vec;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signer as DalekSigner, SigningKey, VerifyingKey};

use crate::bundle::EvidenceBundle;

/// DSSE payload type for AgentVM evidence
pub const PAYLOAD_TYPE: &str = "application/vnd.agentvm.evidence+json";

/// A cryptographic signature.
#[derive(Debug, Clone)]
pub struct Signature {
    /// Key identifier (e.g., "capsule:sha256:abc123...")
    pub keyid: String,
    /// Base64url-encoded signature bytes
    pub sig: String,
}

impl Signature {
    /// Creates a new signature.
    pub fn new(keyid: impl Into<String>, sig: impl Into<String>) -> Self {
        Self {
            keyid: keyid.into(),
            sig: sig.into(),
        }
    }

    /// Returns the raw signature bytes.
    pub fn sig_bytes(&self) -> Option<Vec<u8>> {
        URL_SAFE_NO_PAD.decode(&self.sig).ok()
    }
}

/// DSSE (Dead Simple Signing Envelope) format.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DsseEnvelope {
    /// The payload type URI
    #[cfg_attr(feature = "serde", serde(rename = "payloadType"))]
    pub payload_type: String,

    /// Base64url-encoded payload (the evidence statement)
    pub payload: String,

    /// List of signatures
    pub signatures: Vec<DsseSignature>,
}

/// A signature entry in a DSSE envelope.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DsseSignature {
    /// Key identifier
    pub keyid: String,
    /// Base64url-encoded signature
    pub sig: String,
}

impl DsseEnvelope {
    /// Creates a new DSSE envelope from a payload.
    pub fn new(payload: &[u8]) -> Self {
        Self {
            payload_type: String::from(PAYLOAD_TYPE),
            payload: URL_SAFE_NO_PAD.encode(payload),
            signatures: Vec::new(),
        }
    }

    /// Adds a signature to the envelope.
    pub fn add_signature(&mut self, keyid: impl Into<String>, sig: &[u8]) {
        self.signatures.push(DsseSignature {
            keyid: keyid.into(),
            sig: URL_SAFE_NO_PAD.encode(sig),
        });
    }

    /// Returns the decoded payload bytes.
    pub fn payload_bytes(&self) -> Option<Vec<u8>> {
        URL_SAFE_NO_PAD.decode(&self.payload).ok()
    }

    /// Computes the Pre-Authentication Encoding (PAE) for signing.
    ///
    /// PAE = "DSSEv1" + SP + LEN(payload_type) + SP + payload_type + SP + LEN(payload) + SP + payload
    pub fn pae(&self) -> Vec<u8> {
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(&self.payload)
            .unwrap_or_else(|_| Vec::new());
        compute_pae(&self.payload_type, &payload_bytes)
    }

    /// Serializes the envelope to JSON.
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, SigningError> {
        serde_json::to_string(self).map_err(|e| SigningError::SerializationFailed(e.to_string()))
    }

    /// Deserializes an envelope from JSON.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, SigningError> {
        serde_json::from_str(json).map_err(|e| SigningError::DeserializationFailed(e.to_string()))
    }
}

/// Computes the DSSE Pre-Authentication Encoding.
fn compute_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let type_bytes = payload_type.as_bytes();

    // PAE format: "DSSEv1" SP LEN(type) SP type SP LEN(payload) SP payload
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

/// A signed evidence bundle.
#[derive(Debug, Clone)]
pub struct SignedBundle {
    /// The DSSE envelope containing the signed evidence
    pub envelope: DsseEnvelope,
    /// The original evidence bundle
    pub bundle: EvidenceBundle,
}

impl SignedBundle {
    /// Returns the signatures on this bundle.
    pub fn signatures(&self) -> &[DsseSignature] {
        &self.envelope.signatures
    }

    /// Returns the number of signatures.
    pub fn signature_count(&self) -> usize {
        self.envelope.signatures.len()
    }

    /// Checks if a specific key has signed this bundle.
    pub fn has_signature_from(&self, keyid: &str) -> bool {
        self.envelope.signatures.iter().any(|s| s.keyid == keyid)
    }
}

/// Trait for signing keys.
pub trait SigningKeyTrait {
    /// Returns the key identifier.
    fn keyid(&self) -> String;

    /// Signs the data and returns the signature bytes.
    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, SigningError>;

    /// Returns the public key for verification.
    fn public_key(&self) -> Vec<u8>;
}

/// Ed25519 signer implementation.
pub struct Ed25519Signer {
    /// The signing key
    signing_key: SigningKey,
    /// Key identifier prefix (e.g., "capsule", "host", "tpm")
    key_type: String,
}

impl Ed25519Signer {
    /// Creates a new Ed25519 signer from secret key bytes.
    pub fn from_bytes(secret_key: &[u8; 32], key_type: impl Into<String>) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(secret_key),
            key_type: key_type.into(),
        }
    }

    /// Generates a new random signing key.
    pub fn generate<R: rand_core::CryptoRngCore>(rng: &mut R, key_type: impl Into<String>) -> Self {
        Self {
            signing_key: SigningKey::generate(rng),
            key_type: key_type.into(),
        }
    }

    /// Returns the verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Returns the secret key bytes.
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

impl SigningKeyTrait for Ed25519Signer {
    fn keyid(&self) -> String {
        use alloc::format;

        // Compute hash of public key
        let public_key = self.signing_key.verifying_key().to_bytes();
        let hash = crate::sha256(&public_key);
        let hash_hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();

        format!("{}:sha256:{}", self.key_type, &hash_hex[..16])
    }

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, SigningError> {
        let signature = self.signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    fn public_key(&self) -> Vec<u8> {
        self.signing_key.verifying_key().to_bytes().to_vec()
    }
}

/// Signs an evidence bundle with the provided keys.
pub fn sign_bundle<K: SigningKeyTrait>(
    bundle: &EvidenceBundle,
    keys: &[K],
) -> Result<SignedBundle, SigningError> {
    // Serialize the statement
    #[cfg(feature = "serde")]
    let payload = bundle
        .to_json()
        .map_err(|e| SigningError::SerializationFailed(alloc::format!("{:?}", e)))?;

    #[cfg(not(feature = "serde"))]
    let payload = {
        // Without serde, use a deterministic representation
        let hash = bundle.compute_hash();
        crate::format_hash(&hash)
    };

    let payload_bytes = payload.as_bytes();

    // Create DSSE envelope
    let mut envelope = DsseEnvelope::new(payload_bytes);

    // Compute PAE for signing
    let pae = compute_pae(&envelope.payload_type, payload_bytes);

    // Sign with each key
    for key in keys {
        let sig = key.sign(&pae)?;
        envelope.add_signature(key.keyid(), &sig);
    }

    Ok(SignedBundle {
        envelope,
        bundle: bundle.clone(),
    })
}

/// Verifies a signature using an Ed25519 public key.
pub fn verify_ed25519_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8],
) -> Result<bool, SigningError> {
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| SigningError::InvalidPublicKey)?;

    let sig_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| SigningError::InvalidSignature)?;

    let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);

    Ok(verifying_key.verify_strict(message, &sig).is_ok())
}

/// Errors that can occur during signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    /// Serialization failed
    SerializationFailed(String),
    /// Deserialization failed
    DeserializationFailed(String),
    /// Invalid public key
    InvalidPublicKey,
    /// Invalid signature format
    InvalidSignature,
    /// Key not found
    KeyNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::EvidenceBundleBuilder;

    fn create_test_bundle() -> EvidenceBundle {
        EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test-capsule")
            .timestamp_ns(1234567890)
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .exit_code(0)
            .workspace_diff_hash([3; 32])
            .merkle_root([4; 32])
            .build()
            .expect("should build")
    }

    #[test]
    fn test_dsse_envelope_creation() {
        let payload = b"test payload";
        let envelope = DsseEnvelope::new(payload);

        assert_eq!(envelope.payload_type, PAYLOAD_TYPE);
        assert!(!envelope.payload.is_empty());
        assert!(envelope.signatures.is_empty());

        // Verify roundtrip
        let decoded = envelope.payload_bytes().unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_dsse_envelope_add_signature() {
        let mut envelope = DsseEnvelope::new(b"test");
        envelope.add_signature("key1", b"sig1");
        envelope.add_signature("key2", b"sig2");

        assert_eq!(envelope.signatures.len(), 2);
        assert_eq!(envelope.signatures[0].keyid, "key1");
        assert_eq!(envelope.signatures[1].keyid, "key2");
    }

    #[test]
    fn test_pae_format() {
        let envelope = DsseEnvelope::new(b"hello");
        let pae = envelope.pae();

        // PAE should start with "DSSEv1 "
        assert!(pae.starts_with(b"DSSEv1 "));
    }

    #[test]
    fn test_ed25519_signer_keyid() {
        let signer = Ed25519Signer::from_bytes(&[0u8; 32], "capsule");
        let keyid = signer.keyid();

        assert!(keyid.starts_with("capsule:sha256:"));
    }

    #[test]
    fn test_ed25519_sign_verify() {
        let signer = Ed25519Signer::from_bytes(&[42u8; 32], "test");
        let message = b"test message";

        let signature = signer.sign(message).unwrap();
        let public_key: [u8; 32] = signer.public_key().try_into().unwrap();

        let valid = verify_ed25519_signature(&public_key, message, &signature).unwrap();
        assert!(valid);

        // Wrong message should fail
        let valid = verify_ed25519_signature(&public_key, b"wrong message", &signature).unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_sign_bundle_single_key() {
        let bundle = create_test_bundle();
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "capsule");

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        assert_eq!(signed.signature_count(), 1);
        assert!(signed.signatures()[0].keyid.starts_with("capsule:"));
    }

    #[test]
    fn test_sign_bundle_multiple_keys() {
        let bundle = create_test_bundle();
        let capsule_key = Ed25519Signer::from_bytes(&[1u8; 32], "capsule");
        let host_key = Ed25519Signer::from_bytes(&[2u8; 32], "host");

        let signed = sign_bundle(&bundle, &[capsule_key, host_key]).unwrap();

        assert_eq!(signed.signature_count(), 2);
        assert!(signed.has_signature_from(&signed.signatures()[0].keyid));
        assert!(signed.has_signature_from(&signed.signatures()[1].keyid));
    }

    #[test]
    fn test_signed_bundle_has_signature_from() {
        let bundle = create_test_bundle();
        let signer = Ed25519Signer::from_bytes(&[1u8; 32], "test");
        let keyid = signer.keyid();

        let signed = sign_bundle(&bundle, &[signer]).unwrap();

        assert!(signed.has_signature_from(&keyid));
        assert!(!signed.has_signature_from("nonexistent:key:123"));
    }

    #[test]
    fn test_signature_bytes() {
        let sig = Signature::new("key1", URL_SAFE_NO_PAD.encode(b"test signature"));
        let bytes = sig.sig_bytes().unwrap();
        assert_eq!(bytes, b"test signature");
    }

    #[test]
    fn test_signature_invalid_base64() {
        let sig = Signature::new("key1", "not valid base64!!!");
        assert!(sig.sig_bytes().is_none());
    }

    #[cfg(feature = "serde")]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_dsse_envelope_json_roundtrip() {
            let mut envelope = DsseEnvelope::new(b"test payload");
            envelope.add_signature("key1", b"signature bytes");

            let json = envelope.to_json().unwrap();
            let parsed = DsseEnvelope::from_json(&json).unwrap();

            assert_eq!(parsed.payload_type, envelope.payload_type);
            assert_eq!(parsed.payload, envelope.payload);
            assert_eq!(parsed.signatures.len(), 1);
            assert_eq!(parsed.signatures[0].keyid, "key1");
        }

        #[test]
        fn test_dsse_json_format() {
            let mut envelope = DsseEnvelope::new(b"test");
            envelope.add_signature("key1", b"sig");

            let json = envelope.to_json().unwrap();

            assert!(json.contains("payloadType"));
            assert!(json.contains("payload"));
            assert!(json.contains("signatures"));
            assert!(json.contains("keyid"));
            assert!(json.contains("sig"));
        }
    }
}
