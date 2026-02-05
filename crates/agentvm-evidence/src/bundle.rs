//! Evidence bundle creation and management

use alloc::string::String;
use alloc::vec::Vec;

use agentvm_types::{
    BudgetVector, CapabilityType, CapsuleId,
    evidence::{
        ArtifactRecord, CapabilityCallRecord, EvidenceBundle, EvidenceChain,
        EvidenceExecution, EvidenceHeader, EvidenceInputs, EvidenceOutputs,
        EvidenceSignature, EvidenceStatement, NetworkDirection, NetworkEventRecord,
    },
};

use crate::{Hash, MerkleTree, sha256};

/// Builder for evidence statements
pub struct EvidenceBuilder {
    header: EvidenceHeader,
    inputs: EvidenceInputs,
    capability_calls: Vec<CapabilityCallRecord>,
    network_events: Vec<NetworkEventRecord>,
    budget_consumed: BudgetVector,
    start_time: u64,
}

impl EvidenceBuilder {
    /// Create a new evidence builder
    pub fn new(capsule_id: CapsuleId, run_id: [u8; 16]) -> Self {
        let timestamp_ns = Self::current_time_ns();

        Self {
            header: EvidenceHeader {
                run_id,
                capsule_id: *capsule_id.as_bytes(),
                timestamp_ns,
                version: String::from("1.0"),
                parent_run_id: None,
            },
            inputs: EvidenceInputs {
                manifest_hash: [0u8; 32],
                workspace_hash: [0u8; 32],
                environment_hash: None,
                secrets_hash: None,
                command: Vec::new(),
            },
            capability_calls: Vec::new(),
            network_events: Vec::new(),
            budget_consumed: BudgetVector::zero(),
            start_time: timestamp_ns,
        }
    }

    /// Set the parent run ID (for continuations)
    pub fn parent_run(mut self, parent_id: [u8; 16]) -> Self {
        self.header.parent_run_id = Some(parent_id);
        self
    }

    /// Set the manifest hash
    pub fn manifest_hash(mut self, hash: Hash) -> Self {
        self.inputs.manifest_hash = hash;
        self
    }

    /// Set the workspace hash
    pub fn workspace_hash(mut self, hash: Hash) -> Self {
        self.inputs.workspace_hash = hash;
        self
    }

    /// Set the environment hash
    pub fn environment_hash(mut self, hash: Hash) -> Self {
        self.inputs.environment_hash = Some(hash);
        self
    }

    /// Set the command
    pub fn command(mut self, cmd: Vec<String>) -> Self {
        self.inputs.command = cmd;
        self
    }

    /// Record a capability call
    pub fn record_capability_call(&mut self, record: CapabilityCallRecord) {
        self.budget_consumed = self.budget_consumed.saturating_add(&record.budget_consumed);
        self.capability_calls.push(record);
    }

    /// Record a network event
    pub fn record_network_event(&mut self, event: NetworkEventRecord) {
        self.network_events.push(event);
    }

    /// Build the evidence statement
    pub fn build(
        self,
        exit_code: i32,
        workspace_diff_hash: Hash,
        artifacts: Vec<ArtifactRecord>,
        chain: EvidenceChain,
    ) -> EvidenceStatement {
        let duration_ns = Self::current_time_ns().saturating_sub(self.start_time);

        EvidenceStatement {
            _type: String::from("https://agentvm.io/EvidenceStatement/v1"),
            header: self.header,
            inputs: self.inputs,
            execution: EvidenceExecution {
                capability_calls: self.capability_calls,
                network_events: self.network_events,
                budget_consumed: self.budget_consumed,
                duration_ns,
            },
            outputs: EvidenceOutputs {
                exit_code,
                workspace_diff_hash,
                artifacts,
                stdout_hash: None,
                stderr_hash: None,
            },
            chain,
        }
    }

    fn current_time_ns() -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

/// Evidence logger maintaining chain state
pub struct EvidenceLogger {
    /// Merkle tree of evidence bundles
    merkle_tree: MerkleTree,
    /// Sequence number
    sequence: u64,
    /// Previous evidence hash
    previous_hash: Hash,
}

impl EvidenceLogger {
    /// Create a new evidence logger
    pub fn new() -> Self {
        Self {
            merkle_tree: MerkleTree::new(),
            sequence: 0,
            previous_hash: [0u8; 32],
        }
    }

    /// Log a new evidence statement
    pub fn log(&mut self, statement: &EvidenceStatement) -> EvidenceChain {
        let statement_hash = statement.hash();

        // Add to Merkle tree
        let merkle_root = self.merkle_tree.append(statement_hash);

        // Generate inclusion proof
        let inclusion_proof = self
            .merkle_tree
            .inclusion_proof(self.sequence as usize)
            .map(|p| p.proof.iter().map(|e| e.hash).collect())
            .unwrap_or_default();

        let chain = EvidenceChain {
            sequence: self.sequence,
            previous_hash: self.previous_hash,
            merkle_root,
            inclusion_proof,
        };

        // Update state
        self.sequence += 1;
        self.previous_hash = statement_hash;

        chain
    }

    /// Get the current Merkle root
    pub fn root(&self) -> Hash {
        self.merkle_tree.root()
    }

    /// Get the current sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Get the tree size
    pub fn tree_size(&self) -> usize {
        self.merkle_tree.len()
    }

    /// Generate an inclusion proof for a specific sequence
    pub fn inclusion_proof(&self, sequence: u64) -> Option<crate::InclusionProof> {
        self.merkle_tree.inclusion_proof(sequence as usize)
    }

    /// Generate a consistency proof from an old tree size
    pub fn consistency_proof(&self, old_size: usize) -> Option<crate::ConsistencyProof> {
        self.merkle_tree.consistency_proof(old_size)
    }
}

impl Default for EvidenceLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Create an evidence bundle from a statement with signatures
pub fn create_bundle(
    statement: &EvidenceStatement,
    signatures: Vec<EvidenceSignature>,
) -> EvidenceBundle {
    // In a real implementation, we would serialize and base64 encode the statement
    let payload = encode_statement(statement);

    EvidenceBundle {
        payload_type: String::from("application/vnd.agentvm.evidence+json"),
        payload,
        signatures,
    }
}

fn encode_statement(_statement: &EvidenceStatement) -> String {
    // Simplified - would use proper serialization
    String::from("<encoded_statement>")
}

/// Create a capability call record
pub fn capability_call_record(
    sequence: u64,
    cap_type: CapabilityType,
    cap_id: u128,
    operation: &str,
    request_data: &[u8],
    response_data: &[u8],
    budget: BudgetVector,
    duration_ns: u64,
) -> CapabilityCallRecord {
    CapabilityCallRecord {
        sequence,
        timestamp_ns: EvidenceBuilder::current_time_ns(),
        capability_type: cap_type,
        capability_id: cap_id,
        operation: String::from(operation),
        request_hash: sha256(request_data),
        response_hash: sha256(response_data),
        budget_consumed: budget,
        duration_ns,
    }
}

/// Create a network event record
pub fn network_event_record(
    direction: NetworkDirection,
    destination: &str,
    bytes: u64,
    allowed: bool,
) -> NetworkEventRecord {
    NetworkEventRecord {
        timestamp_ns: EvidenceBuilder::current_time_ns(),
        direction,
        destination: String::from(destination),
        bytes,
        allowed,
    }
}

impl EvidenceBuilder {
    fn current_time_ns() -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}
