//! Capsule types - identity and configuration for agent capsules

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{CapabilityGrant, BudgetVector};

/// Unique identifier for a capsule (128-bit random)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleId(pub [u8; 16]);

impl CapsuleId {
    /// Generate a new random capsule ID
    pub fn generate() -> Self {
        // In no_std, we use a simple counter + hash approach
        // In production, use proper random source
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&count.to_le_bytes());
        // Add some entropy from address
        let ptr = &bytes as *const _ as u64;
        bytes[8..].copy_from_slice(&ptr.to_le_bytes());
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapsuleId(")?;
        for byte in &self.0[..4] {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, "...)")
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

/// Capsule manifest defining capabilities and budgets
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleManifest {
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Cryptographic identity
    pub identity: CapsuleIdentity,
    /// Granted capabilities
    pub capabilities: Vec<CapabilityGrant>,
    /// Resource budget
    pub budget: BudgetVector,
    /// Evidence collection level
    pub evidence_level: EvidenceLevel,
    /// Renewal policy for long-running agents
    pub renewal: RenewalPolicy,
    /// Snapshot policy
    pub snapshot: SnapshotPolicy,
}

/// Capsule cryptographic identity
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapsuleIdentity {
    /// Signature algorithm
    pub algorithm: SignatureAlgorithm,
    /// Public key bytes (32 bytes for Ed25519)
    pub public_key: [u8; 32],
    /// Optional attestation URI (e.g., sigstore://...)
    pub attestation: Option<String>,
}

/// Supported signature algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SignatureAlgorithm {
    /// Ed25519 (default)
    Ed25519 = 1,
    /// ECDSA P-256
    EcdsaP256 = 2,
    /// RSA 2048 (for TPM compatibility)
    Rsa2048 = 3,
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

/// Evidence collection level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum EvidenceLevel {
    /// No evidence collection
    None = 0,
    /// Summary only (hashes, counts)
    Summary = 1,
    /// Full evidence (all capability calls logged)
    Full = 2,
}

impl Default for EvidenceLevel {
    fn default() -> Self {
        Self::Full
    }
}

/// Renewal policy for long-running agents
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
            actions: alloc::vec![
                RenewalAction::Checkpoint,
                RenewalAction::RevalidateCapabilities,
                RenewalAction::RotateSecrets,
                RenewalAction::ConfirmBudget,
            ],
            on_failure: RenewalFailureAction::TerminateAndReplace,
        }
    }
}

/// Actions performed during renewal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum RenewalAction {
    /// Create checkpoint snapshot
    Checkpoint = 1,
    /// Revalidate all capabilities
    RevalidateCapabilities = 2,
    /// Rotate secret material
    RotateSecrets = 3,
    /// Confirm budget availability
    ConfirmBudget = 4,
}

/// Action when renewal fails
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum RenewalFailureAction {
    /// Continue with warning
    Warn = 1,
    /// Terminate gracefully
    Terminate = 2,
    /// Terminate and spawn replacement
    TerminateAndReplace = 3,
}

impl Default for RenewalFailureAction {
    fn default() -> Self {
        Self::TerminateAndReplace
    }
}

/// Snapshot policy
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SnapshotPolicy {
    /// When to create snapshots
    pub trigger: SnapshotTrigger,
    /// Storage type
    pub storage: SnapshotStorage,
    /// Maximum snapshots to retain
    pub max_snapshots: u32,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        Self {
            trigger: SnapshotTrigger::OnCapabilityCall,
            storage: SnapshotStorage::CopyOnWrite,
            max_snapshots: 10,
        }
    }
}

/// When to create snapshots
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SnapshotTrigger {
    /// On every capability call
    OnCapabilityCall = 1,
    /// On renewal only
    OnRenewal = 2,
    /// Manual only
    Manual = 3,
}

/// Snapshot storage type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SnapshotStorage {
    /// Copy-on-write (QCOW2)
    CopyOnWrite = 1,
    /// Full copy
    FullCopy = 2,
    /// Memory snapshot (for hot start)
    Memory = 3,
}
