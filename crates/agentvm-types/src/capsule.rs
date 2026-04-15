//! Capsule identity and manifest types

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::budget::BudgetVector;
use crate::capability::CapabilityGrant;
use crate::evidence::EvidenceLevel;

/// Unique identifier for a capsule (128-bit)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleId(pub [u8; 16]);

impl CapsuleId {
    /// Create a new random CapsuleId
    pub fn new_random() -> Self {
        // In no_std, we'd need a proper RNG
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            let mut bytes = [0u8; 16];
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            bytes[0..8].copy_from_slice(&nanos.to_le_bytes());
            // Add some variation
            bytes[8..16].copy_from_slice(&(nanos.wrapping_mul(0x517cc1b727220a95)).to_le_bytes());
            Self(bytes)
        }
        #[cfg(not(feature = "std"))]
        {
            Self([0u8; 16]) // Placeholder for no_std
        }
    }

    /// Create from raw bytes
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Create a null/empty capsule ID
    pub const fn null() -> Self {
        Self([0u8; 16])
    }

    /// Check if this is a null ID
    pub fn is_null(&self) -> bool {
        self.0 == [0u8; 16]
    }
}

impl fmt::Debug for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapsuleId({:02x?})", &self.0[..4])
    }
}

impl fmt::Display for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl PartialOrd for CapsuleId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapsuleId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

/// Signature algorithm for capsule identity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SignatureAlgorithm {
    Ed25519 = 0,
    Secp256k1 = 1,
    Rsa2048 = 2,
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

/// Capsule identity for signing and verification
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleIdentity {
    pub algorithm: SignatureAlgorithm,
    pub public_key: [u8; 32],
    pub attestation: Option<String>,
}

impl CapsuleIdentity {
    pub fn new(algorithm: SignatureAlgorithm, public_key: [u8; 32]) -> Self {
        Self {
            algorithm,
            public_key,
            attestation: None,
        }
    }

    pub fn with_attestation(mut self, attestation: String) -> Self {
        self.attestation = Some(attestation);
        self
    }
}

/// Renewal policy for long-running capsules
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenewalPolicy {
    /// Renewal interval in seconds
    pub interval_secs: u64,
    /// Actions to perform on renewal
    pub actions: Vec<RenewalAction>,
    /// What to do if renewal fails
    pub on_failure: RenewalFailureAction,
}

impl Default for RenewalPolicy {
    fn default() -> Self {
        Self {
            interval_secs: 3600, // 1 hour
            actions: vec![
                RenewalAction::Checkpoint,
                RenewalAction::RevalidateCapabilities,
            ],
            on_failure: RenewalFailureAction::TerminateAndReplace,
        }
    }
}

/// Actions to perform during renewal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenewalAction {
    Checkpoint,
    RevalidateCapabilities,
    RotateSecrets,
    ConfirmBudget,
}

/// What to do when renewal fails
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenewalFailureAction {
    Continue,
    Pause,
    TerminateAndReplace,
    Terminate,
}

/// Capsule manifest defining capabilities, budgets, and policies
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleManifest {
    pub name: String,
    pub version: String,
    pub identity: CapsuleIdentity,
    pub capabilities: Vec<CapabilityGrant>,
    pub budget: BudgetVector,
    pub evidence_level: EvidenceLevel,
    pub renewal: RenewalPolicy,
}

impl CapsuleManifest {
    pub fn builder(name: impl Into<String>) -> CapsuleManifestBuilder {
        CapsuleManifestBuilder::new(name)
    }
}

/// Builder for CapsuleManifest
pub struct CapsuleManifestBuilder {
    name: String,
    version: String,
    identity: Option<CapsuleIdentity>,
    capabilities: Vec<CapabilityGrant>,
    budget: BudgetVector,
    evidence_level: EvidenceLevel,
    renewal: RenewalPolicy,
}

impl CapsuleManifestBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: String::from("1.0.0"),
            identity: None,
            capabilities: Vec::new(),
            budget: BudgetVector::default(),
            evidence_level: EvidenceLevel::Full,
            renewal: RenewalPolicy::default(),
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn identity(mut self, identity: CapsuleIdentity) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn capability(mut self, cap: CapabilityGrant) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn budget(mut self, budget: BudgetVector) -> Self {
        self.budget = budget;
        self
    }

    pub fn evidence_level(mut self, level: EvidenceLevel) -> Self {
        self.evidence_level = level;
        self
    }

    pub fn renewal(mut self, renewal: RenewalPolicy) -> Self {
        self.renewal = renewal;
        self
    }

    pub fn build(self) -> Result<CapsuleManifest, &'static str> {
        let identity = self.identity.ok_or("identity is required")?;

        Ok(CapsuleManifest {
            name: self.name,
            version: self.version,
            identity,
            capabilities: self.capabilities,
            budget: self.budget,
            evidence_level: self.evidence_level,
            renewal: self.renewal,
        })
    }
}
