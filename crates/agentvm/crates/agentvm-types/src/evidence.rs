//! Evidence types - audit trail for agent executions

use alloc::string::String;
use alloc::vec::Vec;

use crate::{CapsuleId, CapabilityId, CapabilityType, Hash, TimestampNs, Budget, QuotaConsumed};

/// Complete evidence bundle for an agent execution
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceBundle {
    /// Evidence statement (the payload)
    pub statement: EvidenceStatement,
    /// Signatures over the statement
    pub signatures: Vec<EvidenceSignature>,
}

/// Evidence statement containing all execution details
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceStatement {
    /// Statement type identifier
    #[cfg_attr(feature = "serde", serde(rename = "_type"))]
    pub statement_type: String,
    /// Header with run metadata
    pub header: EvidenceHeader,
    /// Inputs to the execution
    pub inputs: EvidenceInputs,
    /// Execution details
    pub execution: EvidenceExecution,
    /// Outputs from the execution
    pub outputs: EvidenceOutputs,
    /// Chain linkage
    pub chain: ChainLink,
}

impl EvidenceStatement {
    /// Standard statement type
    pub const TYPE: &'static str = "https://agentvm.io/EvidenceStatement/v1";

    /// Create a new evidence statement
    pub fn new(header: EvidenceHeader) -> Self {
        Self {
            statement_type: Self::TYPE.into(),
            header,
            inputs: EvidenceInputs::default(),
            execution: EvidenceExecution::default(),
            outputs: EvidenceOutputs::default(),
            chain: ChainLink::default(),
        }
    }
}

/// Header with run metadata
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceHeader {
    /// Unique run identifier
    pub run_id: [u8; 16],
    /// Capsule that executed
    pub capsule_id: CapsuleId,
    /// Timestamp when execution started
    pub timestamp_ns: TimestampNs,
    /// Evidence format version
    pub version: String,
    /// Parent run (for chained executions)
    pub parent_run_id: Option<[u8; 16]>,
}

/// Inputs to the execution
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceInputs {
    /// Hash of the capsule manifest
    pub manifest_hash: Hash,
    /// Hash of workspace state before execution
    pub workspace_hash: Hash,
    /// Hash of environment variables
    pub environment_hash: Option<Hash>,
    /// Hash of secrets (not the secrets themselves)
    pub secrets_hash: Option<Hash>,
    /// Command that was executed
    pub command: Vec<String>,
}

/// Execution details
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceExecution {
    /// All capability calls made
    pub capability_calls: Vec<CapabilityCallRecord>,
    /// Network events observed
    pub network_events: Vec<NetworkEvent>,
    /// Total budget consumed
    pub budget_consumed: Budget,
    /// Execution duration in nanoseconds
    pub duration_ns: u64,
}

/// Record of a single capability invocation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityCallRecord {
    /// Sequence number within this execution
    pub sequence: u64,
    /// Timestamp of the call
    pub timestamp_ns: TimestampNs,
    /// Type of capability used
    pub capability_type: CapabilityType,
    /// ID of the capability used
    pub capability_id: CapabilityId,
    /// Operation performed
    pub operation: String,
    /// Hash of the request
    pub request_hash: Hash,
    /// Hash of the response
    pub response_hash: Hash,
    /// Budget consumed by this call
    pub quota_consumed: QuotaConsumed,
    /// Duration of the call in nanoseconds
    pub duration_ns: u64,
}

/// Network event observed during execution
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetworkEvent {
    /// Timestamp of the event
    pub timestamp_ns: TimestampNs,
    /// Direction (egress/ingress)
    pub direction: NetworkDirection,
    /// Destination/source address
    pub endpoint: String,
    /// Bytes transferred
    pub bytes: u64,
    /// Whether the connection was allowed
    pub allowed: bool,
}

/// Network traffic direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum NetworkDirection {
    /// Outbound traffic
    Egress = 1,
    /// Inbound traffic
    Ingress = 2,
}

/// Outputs from the execution
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceOutputs {
    /// Exit code of the execution
    pub exit_code: i32,
    /// Hash of workspace diff
    pub workspace_diff_hash: Hash,
    /// Artifacts produced
    pub artifacts: Vec<ArtifactRecord>,
    /// Hash of stdout
    pub stdout_hash: Option<Hash>,
    /// Hash of stderr
    pub stderr_hash: Option<Hash>,
}

/// Record of an artifact produced
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtifactRecord {
    /// Path to the artifact
    pub path: String,
    /// Hash of the artifact contents
    pub hash: Hash,
    /// Size in bytes
    pub size: u64,
}

/// Chain linkage for Merkle tree
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChainLink {
    /// Sequence number in the chain
    pub sequence: u64,
    /// Hash of previous evidence bundle
    pub previous_hash: Hash,
    /// Merkle tree root after this bundle
    pub merkle_root: Hash,
    /// Inclusion proof (hashes from leaf to root)
    pub inclusion_proof: Vec<Hash>,
}

/// Signature over evidence statement
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceSignature {
    /// Key identifier (e.g., "capsule:sha256:...", "host:sha256:...")
    pub keyid: String,
    /// Signature bytes (base64 encoded in JSON)
    pub sig: Vec<u8>,
}

/// Merkle proof for inclusion verification
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MerkleProof {
    /// Leaf index in the tree
    pub leaf_index: u64,
    /// Tree size at time of proof
    pub tree_size: u64,
    /// Sibling hashes from leaf to root
    pub hashes: Vec<Hash>,
    /// Root hash
    pub root: Hash,
}

impl MerkleProof {
    /// Verify that a leaf hash is included in the tree
    pub fn verify(&self, leaf_hash: &Hash) -> bool {
        let mut computed = *leaf_hash;
        let mut index = self.leaf_index;

        for sibling in &self.hashes {
            computed = if index % 2 == 0 {
                hash_pair(&computed, sibling)
            } else {
                hash_pair(sibling, &computed)
            };
            index /= 2;
        }

        computed == self.root
    }
}

/// Hash two values together (for Merkle tree)
fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}
