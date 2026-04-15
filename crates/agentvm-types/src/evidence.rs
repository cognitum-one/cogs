//! Evidence types for audit trails

use alloc::string::String;
use alloc::vec::Vec;

use crate::budget::BudgetVector;
use crate::capability::CapabilityType;

/// Evidence level for logging granularity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EvidenceLevel {
    /// No evidence logging
    None,
    /// Summary only (aggregated)
    Summary,
    /// Full evidence logging
    #[default]
    Full,
}

/// Hash type (SHA-256)
pub type Hash = [u8; 32];

/// Evidence statement header
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceHeader {
    /// Run identifier (UUID)
    pub run_id: [u8; 16],
    /// Capsule identifier
    pub capsule_id: [u8; 16],
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// Evidence format version
    pub version: String,
    /// Parent run ID (if continuation)
    pub parent_run_id: Option<[u8; 16]>,
}

/// Evidence input specification
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceInputs {
    /// Hash of capsule manifest
    pub manifest_hash: Hash,
    /// Hash of initial workspace state
    pub workspace_hash: Hash,
    /// Hash of environment variables
    pub environment_hash: Option<Hash>,
    /// Hash of secrets (without values)
    pub secrets_hash: Option<Hash>,
    /// Command line arguments
    pub command: Vec<String>,
}

/// Record of a capability call
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityCallRecord {
    /// Sequence number within run
    pub sequence: u64,
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Capability type used
    pub capability_type: CapabilityType,
    /// Capability ID
    pub capability_id: u128,
    /// Operation performed
    pub operation: String,
    /// Hash of request
    pub request_hash: Hash,
    /// Hash of response
    pub response_hash: Hash,
    /// Budget consumed
    pub budget_consumed: BudgetVector,
    /// Duration in nanoseconds
    pub duration_ns: u64,
}

/// Network event record
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetworkEventRecord {
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Direction of traffic
    pub direction: NetworkDirection,
    /// Destination address
    pub destination: String,
    /// Bytes transferred
    pub bytes: u64,
    /// Whether the connection was allowed
    pub allowed: bool,
}

/// Network traffic direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NetworkDirection {
    Egress,
    Ingress,
}

/// Execution evidence
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceExecution {
    /// All capability calls made
    pub capability_calls: Vec<CapabilityCallRecord>,
    /// Network events
    pub network_events: Vec<NetworkEventRecord>,
    /// Total budget consumed
    pub budget_consumed: BudgetVector,
    /// Total execution duration in nanoseconds
    pub duration_ns: u64,
}

/// Output evidence
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceOutputs {
    /// Exit code
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

/// Artifact record
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtifactRecord {
    /// File path
    pub path: String,
    /// Content hash
    pub hash: Hash,
    /// File size
    pub size: u64,
}

/// Merkle chain information
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceChain {
    /// Sequence number in chain
    pub sequence: u64,
    /// Hash of previous evidence bundle
    pub previous_hash: Hash,
    /// Current Merkle root
    pub merkle_root: Hash,
    /// Inclusion proof hashes
    pub inclusion_proof: Vec<Hash>,
}

/// Complete evidence statement
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceStatement {
    /// Type identifier
    pub _type: String,
    /// Header information
    pub header: EvidenceHeader,
    /// Input specification
    pub inputs: EvidenceInputs,
    /// Execution evidence
    pub execution: EvidenceExecution,
    /// Output evidence
    pub outputs: EvidenceOutputs,
    /// Chain information
    pub chain: EvidenceChain,
}

impl EvidenceStatement {
    /// Create a new evidence statement
    pub fn new(header: EvidenceHeader) -> Self {
        Self {
            _type: String::from("https://agentvm.io/EvidenceStatement/v1"),
            header,
            inputs: EvidenceInputs {
                manifest_hash: [0u8; 32],
                workspace_hash: [0u8; 32],
                environment_hash: None,
                secrets_hash: None,
                command: Vec::new(),
            },
            execution: EvidenceExecution {
                capability_calls: Vec::new(),
                network_events: Vec::new(),
                budget_consumed: BudgetVector::zero(),
                duration_ns: 0,
            },
            outputs: EvidenceOutputs {
                exit_code: 0,
                workspace_diff_hash: [0u8; 32],
                artifacts: Vec::new(),
                stdout_hash: None,
                stderr_hash: None,
            },
            chain: EvidenceChain {
                sequence: 0,
                previous_hash: [0u8; 32],
                merkle_root: [0u8; 32],
                inclusion_proof: Vec::new(),
            },
        }
    }

    /// Calculate hash of this statement
    pub fn hash(&self) -> Hash {
        // Simple hash implementation - in reality would use proper serialization
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.header.run_id);
        hasher.update(&self.header.capsule_id);
        hasher.update(&self.header.timestamp_ns.to_le_bytes());
        hasher.finalize().into()
    }
}

/// Signature on evidence bundle
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceSignature {
    /// Key identifier
    pub keyid: String,
    /// Signature bytes
    pub sig: Vec<u8>,
}

/// Complete evidence bundle (DSSE envelope)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceBundle {
    /// Payload type
    pub payload_type: String,
    /// Base64-encoded payload
    pub payload: String,
    /// Signatures
    pub signatures: Vec<EvidenceSignature>,
}

impl EvidenceBundle {
    /// Create a new bundle from a statement
    pub fn from_statement(statement: &EvidenceStatement) -> Self {
        // In reality, would serialize and base64 encode
        Self {
            payload_type: String::from("application/vnd.agentvm.evidence+json"),
            payload: String::new(), // Would be base64(serialize(statement))
            signatures: Vec::new(),
        }
    }

    /// Add a signature to the bundle
    pub fn add_signature(&mut self, keyid: String, sig: Vec<u8>) {
        self.signatures.push(EvidenceSignature { keyid, sig });
    }

    /// Verify all signatures
    pub fn verify_signatures(&self) -> bool {
        // In reality, would verify each signature against the payload
        !self.signatures.is_empty()
    }
}
