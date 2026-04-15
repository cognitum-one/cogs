//! Evidence statement types and serialization.
//!
//! This module defines the evidence statement schema per ADR-006,
//! providing structured evidence of agent execution.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Hash;

/// Evidence statement type identifier
pub const STATEMENT_TYPE: &str = "https://agentvm.io/EvidenceStatement/v1";

/// Complete evidence statement containing all execution evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidenceStatement {
    /// Type identifier for the statement
    #[cfg_attr(feature = "serde", serde(rename = "_type"))]
    pub type_id: String,

    /// Header information
    pub header: Header,

    /// Input hashes
    pub inputs: Inputs,

    /// Execution details
    pub execution: ExecutionInfo,

    /// Output information
    pub outputs: Outputs,

    /// Chain linkage
    pub chain: ChainInfo,
}

impl EvidenceStatement {
    /// Creates a new evidence statement with default type.
    pub fn new(
        header: Header,
        inputs: Inputs,
        execution: ExecutionInfo,
        outputs: Outputs,
        chain: ChainInfo,
    ) -> Self {
        Self {
            type_id: String::from(STATEMENT_TYPE),
            header,
            inputs,
            execution,
            outputs,
            chain,
        }
    }

    /// Validates the statement structure.
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Type check
        if self.type_id != STATEMENT_TYPE {
            return Err(ValidationError::InvalidType);
        }

        // Header validation
        if self.header.run_id.is_empty() {
            return Err(ValidationError::MissingField("header.run_id"));
        }
        if self.header.capsule_id.is_empty() {
            return Err(ValidationError::MissingField("header.capsule_id"));
        }

        // Inputs validation
        if self.inputs.manifest_hash.is_empty() {
            return Err(ValidationError::MissingField("inputs.manifest_hash"));
        }
        if self.inputs.workspace_hash.is_empty() {
            return Err(ValidationError::MissingField("inputs.workspace_hash"));
        }

        // Outputs validation
        if self.outputs.workspace_diff_hash.is_empty() {
            return Err(ValidationError::MissingField("outputs.workspace_diff_hash"));
        }

        // Chain validation
        if self.chain.merkle_root.is_empty() {
            return Err(ValidationError::MissingField("chain.merkle_root"));
        }

        Ok(())
    }

    /// Serializes the statement to JSON.
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, SerializationError> {
        serde_json::to_string(self).map_err(|e| SerializationError::JsonError(e.to_string()))
    }

    /// Deserializes the statement from JSON.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, SerializationError> {
        serde_json::from_str(json).map_err(|e| SerializationError::JsonError(e.to_string()))
    }

    /// Computes the hash of this statement.
    pub fn compute_hash(&self) -> Hash {
        #[cfg(feature = "serde")]
        {
            if let Ok(json) = self.to_json() {
                return crate::sha256(json.as_bytes());
            }
        }

        // Fallback: hash the structural fields
        self.compute_hash_manual()
    }

    /// Manually computes hash without serde.
    fn compute_hash_manual(&self) -> Hash {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Hash header
        hasher.update(self.header.run_id.as_bytes());
        hasher.update(self.header.capsule_id.as_bytes());
        hasher.update(self.header.timestamp_ns.to_le_bytes());
        hasher.update(self.header.version.as_bytes());

        // Hash inputs
        hasher.update(self.inputs.manifest_hash.as_bytes());
        hasher.update(self.inputs.workspace_hash.as_bytes());

        // Hash outputs
        hasher.update(self.outputs.exit_code.to_le_bytes());
        hasher.update(self.outputs.workspace_diff_hash.as_bytes());

        // Hash chain info
        hasher.update(self.chain.sequence.to_le_bytes());
        hasher.update(self.chain.previous_hash.as_bytes());
        hasher.update(self.chain.merkle_root.as_bytes());

        hasher.finalize().into()
    }
}

/// Header section of evidence statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    /// Unique identifier for this run (UUID format)
    pub run_id: String,

    /// Capsule identifier
    pub capsule_id: String,

    /// Timestamp in nanoseconds since Unix epoch
    pub timestamp_ns: u64,

    /// Version string
    pub version: String,

    /// Parent run ID for chained executions
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub parent_run_id: Option<String>,
}

impl Header {
    /// Creates a new header.
    pub fn new(run_id: impl Into<String>, capsule_id: impl Into<String>, timestamp_ns: u64) -> Self {
        Self {
            run_id: run_id.into(),
            capsule_id: capsule_id.into(),
            timestamp_ns,
            version: String::from("1.0.0"),
            parent_run_id: None,
        }
    }

    /// Sets the version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Sets the parent run ID.
    pub fn with_parent(mut self, parent_run_id: impl Into<String>) -> Self {
        self.parent_run_id = Some(parent_run_id.into());
        self
    }
}

/// Input hashes for the execution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Inputs {
    /// Hash of the capsule manifest
    pub manifest_hash: String,

    /// Hash of the workspace contents
    pub workspace_hash: String,

    /// Hash of environment variables
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub environment_hash: Option<String>,

    /// Hash of secrets (not the secrets themselves)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub secrets_hash: Option<String>,

    /// Command that was executed
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub command: Vec<String>,
}

impl Inputs {
    /// Creates new inputs with required fields.
    pub fn new(manifest_hash: impl Into<String>, workspace_hash: impl Into<String>) -> Self {
        Self {
            manifest_hash: manifest_hash.into(),
            workspace_hash: workspace_hash.into(),
            environment_hash: None,
            secrets_hash: None,
            command: Vec::new(),
        }
    }

    /// Sets the environment hash.
    pub fn with_environment_hash(mut self, hash: impl Into<String>) -> Self {
        self.environment_hash = Some(hash.into());
        self
    }

    /// Sets the secrets hash.
    pub fn with_secrets_hash(mut self, hash: impl Into<String>) -> Self {
        self.secrets_hash = Some(hash.into());
        self
    }

    /// Sets the command.
    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }
}

/// Execution information.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutionInfo {
    /// Capability calls made during execution
    pub capability_calls: Vec<CapabilityCall>,

    /// Network events during execution
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub network_events: Vec<NetworkEvent>,

    /// Budget consumed during execution
    pub budget_consumed: Budget,

    /// Duration of execution in nanoseconds
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub duration_ns: Option<u64>,
}

impl ExecutionInfo {
    /// Creates new execution info.
    pub fn new(budget_consumed: Budget) -> Self {
        Self {
            capability_calls: Vec::new(),
            network_events: Vec::new(),
            budget_consumed,
            duration_ns: None,
        }
    }

    /// Adds a capability call.
    pub fn add_capability_call(&mut self, call: CapabilityCall) {
        self.capability_calls.push(call);
    }

    /// Adds a network event.
    pub fn add_network_event(&mut self, event: NetworkEvent) {
        self.network_events.push(event);
    }

    /// Sets the duration.
    pub fn with_duration_ns(mut self, duration_ns: u64) -> Self {
        self.duration_ns = Some(duration_ns);
        self
    }
}

/// Record of a capability invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityCall {
    /// Sequence number within this execution
    pub sequence: u64,

    /// Timestamp of the call in nanoseconds
    pub timestamp_ns: u64,

    /// Type of capability
    pub capability_type: String,

    /// Capability identifier
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub capability_id: Option<String>,

    /// Operation performed
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub operation: Option<String>,

    /// Hash of the request
    pub request_hash: String,

    /// Hash of the response
    pub response_hash: String,

    /// Budget consumed by this call
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub budget_consumed: Option<Budget>,

    /// Duration of the call in nanoseconds
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub duration_ns: Option<u64>,
}

impl CapabilityCall {
    /// Creates a new capability call record.
    pub fn new(
        sequence: u64,
        timestamp_ns: u64,
        capability_type: impl Into<String>,
        request_hash: impl Into<String>,
        response_hash: impl Into<String>,
    ) -> Self {
        Self {
            sequence,
            timestamp_ns,
            capability_type: capability_type.into(),
            capability_id: None,
            operation: None,
            request_hash: request_hash.into(),
            response_hash: response_hash.into(),
            budget_consumed: None,
            duration_ns: None,
        }
    }

    /// Sets the capability ID.
    pub fn with_capability_id(mut self, id: impl Into<String>) -> Self {
        self.capability_id = Some(id.into());
        self
    }

    /// Sets the operation.
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Sets the budget consumed.
    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget_consumed = Some(budget);
        self
    }

    /// Sets the duration.
    pub fn with_duration_ns(mut self, duration_ns: u64) -> Self {
        self.duration_ns = Some(duration_ns);
        self
    }
}

/// Record of a network event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetworkEvent {
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,

    /// Direction: egress or ingress
    pub direction: NetworkDirection,

    /// Destination address or hostname
    pub destination: String,

    /// Bytes transferred
    pub bytes: u64,

    /// Whether the event was allowed
    pub allowed: bool,
}

/// Network direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum NetworkDirection {
    /// Outgoing traffic
    Egress,
    /// Incoming traffic
    Ingress,
}

/// Budget tracking.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Budget {
    /// CPU time consumed in milliseconds
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub cpu_time_ms: Option<u64>,

    /// Memory used in bytes
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub memory_bytes: Option<u64>,

    /// Network bytes transferred
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub network_bytes: Option<u64>,

    /// Disk bytes written
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub disk_write_bytes: Option<u64>,
}

impl Budget {
    /// Creates an empty budget.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a budget with all fields set.
    pub fn full(
        cpu_time_ms: u64,
        memory_bytes: u64,
        network_bytes: u64,
        disk_write_bytes: u64,
    ) -> Self {
        Self {
            cpu_time_ms: Some(cpu_time_ms),
            memory_bytes: Some(memory_bytes),
            network_bytes: Some(network_bytes),
            disk_write_bytes: Some(disk_write_bytes),
        }
    }

    /// Sets the CPU time.
    pub fn with_cpu_time_ms(mut self, ms: u64) -> Self {
        self.cpu_time_ms = Some(ms);
        self
    }

    /// Sets the memory usage.
    pub fn with_memory_bytes(mut self, bytes: u64) -> Self {
        self.memory_bytes = Some(bytes);
        self
    }

    /// Sets the network bytes.
    pub fn with_network_bytes(mut self, bytes: u64) -> Self {
        self.network_bytes = Some(bytes);
        self
    }

    /// Sets the disk write bytes.
    pub fn with_disk_write_bytes(mut self, bytes: u64) -> Self {
        self.disk_write_bytes = Some(bytes);
        self
    }
}

/// Output information.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Outputs {
    /// Process exit code
    pub exit_code: i32,

    /// Hash of workspace diff
    pub workspace_diff_hash: String,

    /// Artifacts produced
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub artifacts: Vec<Artifact>,

    /// Hash of stdout
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub stdout_hash: Option<String>,

    /// Hash of stderr
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub stderr_hash: Option<String>,
}

impl Outputs {
    /// Creates new outputs.
    pub fn new(exit_code: i32, workspace_diff_hash: impl Into<String>) -> Self {
        Self {
            exit_code,
            workspace_diff_hash: workspace_diff_hash.into(),
            artifacts: Vec::new(),
            stdout_hash: None,
            stderr_hash: None,
        }
    }

    /// Adds an artifact.
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
    }

    /// Sets stdout hash.
    pub fn with_stdout_hash(mut self, hash: impl Into<String>) -> Self {
        self.stdout_hash = Some(hash.into());
        self
    }

    /// Sets stderr hash.
    pub fn with_stderr_hash(mut self, hash: impl Into<String>) -> Self {
        self.stderr_hash = Some(hash.into());
        self
    }
}

/// An output artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Artifact {
    /// Path within workspace
    pub path: String,

    /// Hash of artifact contents
    pub hash: String,

    /// Size in bytes
    pub size: u64,
}

impl Artifact {
    /// Creates a new artifact.
    pub fn new(path: impl Into<String>, hash: impl Into<String>, size: u64) -> Self {
        Self {
            path: path.into(),
            hash: hash.into(),
            size,
        }
    }
}

/// Chain linkage information.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChainInfo {
    /// Sequence number in the chain
    pub sequence: u64,

    /// Hash of the previous evidence bundle
    pub previous_hash: String,

    /// Current Merkle tree root
    pub merkle_root: String,

    /// Inclusion proof for this bundle
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub inclusion_proof: Vec<String>,
}

impl ChainInfo {
    /// Creates new chain info.
    pub fn new(
        sequence: u64,
        previous_hash: impl Into<String>,
        merkle_root: impl Into<String>,
    ) -> Self {
        Self {
            sequence,
            previous_hash: previous_hash.into(),
            merkle_root: merkle_root.into(),
            inclusion_proof: Vec::new(),
        }
    }

    /// Creates the genesis chain info (first in chain).
    pub fn genesis(merkle_root: impl Into<String>) -> Self {
        Self::new(0, crate::format_hash(&[0u8; 32]), merkle_root)
    }

    /// Adds inclusion proof hashes.
    pub fn with_inclusion_proof(mut self, proof: Vec<String>) -> Self {
        self.inclusion_proof = proof;
        self
    }
}

/// Validation error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid statement type
    InvalidType,
    /// Required field is missing
    MissingField(&'static str),
    /// Field has invalid format
    InvalidFormat(&'static str),
}

/// Serialization error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationError {
    /// JSON serialization/deserialization error
    JsonError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_statement() -> EvidenceStatement {
        EvidenceStatement::new(
            Header::new("run-123", "capsule-abc", 1234567890),
            Inputs::new(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            ),
            ExecutionInfo::new(Budget::new()),
            Outputs::new(
                0,
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            ),
            ChainInfo::genesis(
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            ),
        )
    }

    #[test]
    fn test_statement_creation() {
        let stmt = create_test_statement();
        assert_eq!(stmt.type_id, STATEMENT_TYPE);
        assert_eq!(stmt.header.run_id, "run-123");
        assert_eq!(stmt.header.capsule_id, "capsule-abc");
    }

    #[test]
    fn test_statement_validation_valid() {
        let stmt = create_test_statement();
        assert!(stmt.validate().is_ok());
    }

    #[test]
    fn test_statement_validation_missing_run_id() {
        let mut stmt = create_test_statement();
        stmt.header.run_id = String::new();
        assert_eq!(
            stmt.validate(),
            Err(ValidationError::MissingField("header.run_id"))
        );
    }

    #[test]
    fn test_statement_validation_invalid_type() {
        let mut stmt = create_test_statement();
        stmt.type_id = String::from("invalid");
        assert_eq!(stmt.validate(), Err(ValidationError::InvalidType));
    }

    #[test]
    fn test_header_builder() {
        let header = Header::new("run-1", "cap-1", 12345)
            .with_version("2.0.0")
            .with_parent("parent-run");

        assert_eq!(header.run_id, "run-1");
        assert_eq!(header.version, "2.0.0");
        assert_eq!(header.parent_run_id, Some(String::from("parent-run")));
    }

    #[test]
    fn test_inputs_builder() {
        let inputs = Inputs::new("manifest-hash", "workspace-hash")
            .with_environment_hash("env-hash")
            .with_secrets_hash("secrets-hash")
            .with_command(alloc::vec![
                String::from("cmd"),
                String::from("arg1"),
            ]);

        assert_eq!(inputs.manifest_hash, "manifest-hash");
        assert_eq!(inputs.environment_hash, Some(String::from("env-hash")));
        assert_eq!(inputs.command.len(), 2);
    }

    #[test]
    fn test_capability_call_builder() {
        let call = CapabilityCall::new(1, 12345, "http", "req-hash", "resp-hash")
            .with_capability_id("cap-123")
            .with_operation("GET")
            .with_budget(Budget::new().with_network_bytes(1024))
            .with_duration_ns(1000000);

        assert_eq!(call.sequence, 1);
        assert_eq!(call.capability_type, "http");
        assert_eq!(call.capability_id, Some(String::from("cap-123")));
        assert_eq!(call.operation, Some(String::from("GET")));
        assert!(call.budget_consumed.is_some());
        assert_eq!(call.duration_ns, Some(1000000));
    }

    #[test]
    fn test_budget_builder() {
        let budget = Budget::new()
            .with_cpu_time_ms(100)
            .with_memory_bytes(1024 * 1024)
            .with_network_bytes(4096)
            .with_disk_write_bytes(2048);

        assert_eq!(budget.cpu_time_ms, Some(100));
        assert_eq!(budget.memory_bytes, Some(1024 * 1024));
        assert_eq!(budget.network_bytes, Some(4096));
        assert_eq!(budget.disk_write_bytes, Some(2048));
    }

    #[test]
    fn test_budget_full() {
        let budget = Budget::full(100, 200, 300, 400);
        assert_eq!(budget.cpu_time_ms, Some(100));
        assert_eq!(budget.memory_bytes, Some(200));
        assert_eq!(budget.network_bytes, Some(300));
        assert_eq!(budget.disk_write_bytes, Some(400));
    }

    #[test]
    fn test_outputs_builder() {
        let mut outputs = Outputs::new(0, "diff-hash")
            .with_stdout_hash("stdout-hash")
            .with_stderr_hash("stderr-hash");

        outputs.add_artifact(Artifact::new("path/to/file", "file-hash", 1024));

        assert_eq!(outputs.exit_code, 0);
        assert_eq!(outputs.stdout_hash, Some(String::from("stdout-hash")));
        assert_eq!(outputs.artifacts.len(), 1);
        assert_eq!(outputs.artifacts[0].path, "path/to/file");
    }

    #[test]
    fn test_chain_info_genesis() {
        let chain = ChainInfo::genesis("root-hash");
        assert_eq!(chain.sequence, 0);
        assert!(chain.previous_hash.starts_with("sha256:00000000"));
    }

    #[test]
    fn test_chain_info_with_proof() {
        let chain = ChainInfo::new(5, "prev-hash", "root-hash")
            .with_inclusion_proof(alloc::vec![
                String::from("sibling1"),
                String::from("sibling2"),
            ]);

        assert_eq!(chain.sequence, 5);
        assert_eq!(chain.inclusion_proof.len(), 2);
    }

    #[test]
    fn test_execution_info() {
        let mut exec = ExecutionInfo::new(Budget::new()).with_duration_ns(5000000);

        exec.add_capability_call(CapabilityCall::new(0, 1000, "http", "req", "resp"));

        exec.add_network_event(NetworkEvent {
            timestamp_ns: 2000,
            direction: NetworkDirection::Egress,
            destination: String::from("api.example.com"),
            bytes: 512,
            allowed: true,
        });

        assert_eq!(exec.capability_calls.len(), 1);
        assert_eq!(exec.network_events.len(), 1);
        assert_eq!(exec.duration_ns, Some(5000000));
    }

    #[test]
    fn test_statement_hash_deterministic() {
        let stmt1 = create_test_statement();
        let stmt2 = create_test_statement();

        let hash1 = stmt1.compute_hash();
        let hash2 = stmt2.compute_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_statement_hash_changes_with_content() {
        let stmt1 = create_test_statement();

        let mut stmt2 = create_test_statement();
        stmt2.header.run_id = String::from("different-run");

        let hash1 = stmt1.compute_hash();
        let hash2 = stmt2.compute_hash();

        assert_ne!(hash1, hash2);
    }

    #[cfg(feature = "serde")]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_statement_json_roundtrip() {
            let stmt = create_test_statement();
            let json = stmt.to_json().unwrap();
            let parsed = EvidenceStatement::from_json(&json).unwrap();

            assert_eq!(stmt, parsed);
        }

        #[test]
        fn test_json_format() {
            let stmt = create_test_statement();
            let json = stmt.to_json().unwrap();

            // Should contain expected fields
            assert!(json.contains("\"_type\""));
            assert!(json.contains("\"header\""));
            assert!(json.contains("\"inputs\""));
            assert!(json.contains("\"execution\""));
            assert!(json.contains("\"outputs\""));
            assert!(json.contains("\"chain\""));
        }
    }
}
