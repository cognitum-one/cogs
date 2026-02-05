//! Evidence bundle creation and management.
//!
//! This module provides the `EvidenceBundle` type and a builder pattern
//! for constructing evidence bundles from agent execution data.

use alloc::string::String;
use alloc::vec::Vec;

use crate::statement::{
    Artifact, Budget, CapabilityCall, ChainInfo, EvidenceStatement, ExecutionInfo, Header, Inputs,
    NetworkEvent, Outputs,
};
use crate::Hash;

/// A complete evidence bundle ready for signing.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceBundle {
    /// The evidence statement
    pub statement: EvidenceStatement,

    /// Pre-computed hash of the statement
    statement_hash: Option<Hash>,
}

impl EvidenceBundle {
    /// Creates a new evidence bundle from a statement.
    pub fn new(statement: EvidenceStatement) -> Self {
        Self {
            statement,
            statement_hash: None,
        }
    }

    /// Validates the bundle.
    pub fn validate(&self) -> Result<(), BundleError> {
        self.statement
            .validate()
            .map_err(BundleError::ValidationError)
    }

    /// Computes and caches the hash of this bundle.
    pub fn compute_hash(&self) -> Hash {
        if let Some(hash) = self.statement_hash {
            return hash;
        }
        self.statement.compute_hash()
    }

    /// Returns the run ID.
    pub fn run_id(&self) -> &str {
        &self.statement.header.run_id
    }

    /// Returns the capsule ID.
    pub fn capsule_id(&self) -> &str {
        &self.statement.header.capsule_id
    }

    /// Returns the timestamp in nanoseconds.
    pub fn timestamp_ns(&self) -> u64 {
        self.statement.header.timestamp_ns
    }

    /// Returns the exit code.
    pub fn exit_code(&self) -> i32 {
        self.statement.outputs.exit_code
    }

    /// Returns the capability calls.
    pub fn capability_calls(&self) -> &[CapabilityCall] {
        &self.statement.execution.capability_calls
    }

    /// Returns the sequence number in the chain.
    pub fn sequence(&self) -> u64 {
        self.statement.chain.sequence
    }

    /// Returns the Merkle root.
    pub fn merkle_root(&self) -> &str {
        &self.statement.chain.merkle_root
    }

    /// Serializes the bundle to JSON.
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, BundleError> {
        self.statement
            .to_json()
            .map_err(BundleError::SerializationError)
    }

    /// Deserializes a bundle from JSON.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, BundleError> {
        let statement =
            EvidenceStatement::from_json(json).map_err(BundleError::SerializationError)?;
        Ok(Self::new(statement))
    }
}

/// Builder for constructing evidence bundles.
#[derive(Debug, Default)]
pub struct EvidenceBundleBuilder {
    // Header fields
    run_id: Option<[u8; 16]>,
    capsule_id: Option<String>,
    timestamp_ns: Option<u64>,
    version: Option<String>,
    parent_run_id: Option<[u8; 16]>,

    // Input fields
    manifest_hash: Option<Hash>,
    workspace_hash: Option<Hash>,
    environment_hash: Option<Hash>,
    secrets_hash: Option<Hash>,
    command: Vec<String>,

    // Execution fields
    capability_calls: Vec<CapabilityCall>,
    network_events: Vec<NetworkEvent>,
    budget_consumed: Budget,
    duration_ns: Option<u64>,

    // Output fields
    exit_code: Option<i32>,
    workspace_diff_hash: Option<Hash>,
    artifacts: Vec<Artifact>,
    stdout_hash: Option<Hash>,
    stderr_hash: Option<Hash>,

    // Chain fields
    sequence: Option<u64>,
    previous_hash: Option<Hash>,
    merkle_root: Option<Hash>,
    inclusion_proof: Vec<Hash>,
}

impl EvidenceBundleBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    // ===== Header setters =====

    /// Sets the run ID (UUID bytes).
    pub fn run_id(mut self, run_id: [u8; 16]) -> Self {
        self.run_id = Some(run_id);
        self
    }

    /// Sets the capsule ID.
    pub fn capsule_id(mut self, capsule_id: impl Into<String>) -> Self {
        self.capsule_id = Some(capsule_id.into());
        self
    }

    /// Sets the timestamp in nanoseconds.
    pub fn timestamp_ns(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = Some(timestamp_ns);
        self
    }

    /// Sets the version string.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Sets the parent run ID for chained executions.
    pub fn parent_run_id(mut self, parent_run_id: [u8; 16]) -> Self {
        self.parent_run_id = Some(parent_run_id);
        self
    }

    // ===== Input setters =====

    /// Sets the manifest hash.
    pub fn manifest_hash(mut self, hash: Hash) -> Self {
        self.manifest_hash = Some(hash);
        self
    }

    /// Sets the workspace hash.
    pub fn workspace_hash(mut self, hash: Hash) -> Self {
        self.workspace_hash = Some(hash);
        self
    }

    /// Sets the environment hash.
    pub fn environment_hash(mut self, hash: Hash) -> Self {
        self.environment_hash = Some(hash);
        self
    }

    /// Sets the secrets hash.
    pub fn secrets_hash(mut self, hash: Hash) -> Self {
        self.secrets_hash = Some(hash);
        self
    }

    /// Sets the command that was executed.
    pub fn command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }

    /// Sets all inputs at once.
    pub fn set_inputs(
        mut self,
        manifest_hash: Hash,
        workspace_hash: Hash,
        environment_hash: Option<Hash>,
        secrets_hash: Option<Hash>,
        command: Vec<String>,
    ) -> Self {
        self.manifest_hash = Some(manifest_hash);
        self.workspace_hash = Some(workspace_hash);
        self.environment_hash = environment_hash;
        self.secrets_hash = secrets_hash;
        self.command = command;
        self
    }

    // ===== Execution setters =====

    /// Adds a capability call record.
    pub fn add_capability_call(mut self, call: CapabilityCall) -> Self {
        self.capability_calls.push(call);
        self
    }

    /// Adds multiple capability calls.
    pub fn capability_calls(mut self, calls: Vec<CapabilityCall>) -> Self {
        self.capability_calls = calls;
        self
    }

    /// Adds a network event.
    pub fn add_network_event(mut self, event: NetworkEvent) -> Self {
        self.network_events.push(event);
        self
    }

    /// Adds multiple network events.
    pub fn network_events(mut self, events: Vec<NetworkEvent>) -> Self {
        self.network_events = events;
        self
    }

    /// Sets the budget consumed.
    pub fn budget_consumed(mut self, budget: Budget) -> Self {
        self.budget_consumed = budget;
        self
    }

    /// Sets the execution duration in nanoseconds.
    pub fn duration_ns(mut self, duration_ns: u64) -> Self {
        self.duration_ns = Some(duration_ns);
        self
    }

    // ===== Output setters =====

    /// Sets the exit code.
    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = Some(exit_code);
        self
    }

    /// Sets the workspace diff hash.
    pub fn workspace_diff_hash(mut self, hash: Hash) -> Self {
        self.workspace_diff_hash = Some(hash);
        self
    }

    /// Adds an artifact.
    pub fn add_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Adds multiple artifacts.
    pub fn artifacts(mut self, artifacts: Vec<Artifact>) -> Self {
        self.artifacts = artifacts;
        self
    }

    /// Sets the stdout hash.
    pub fn stdout_hash(mut self, hash: Hash) -> Self {
        self.stdout_hash = Some(hash);
        self
    }

    /// Sets the stderr hash.
    pub fn stderr_hash(mut self, hash: Hash) -> Self {
        self.stderr_hash = Some(hash);
        self
    }

    /// Sets all outputs at once.
    pub fn set_outputs(
        mut self,
        exit_code: i32,
        workspace_diff_hash: Hash,
        artifacts: Vec<Artifact>,
    ) -> Self {
        self.exit_code = Some(exit_code);
        self.workspace_diff_hash = Some(workspace_diff_hash);
        self.artifacts = artifacts;
        self
    }

    // ===== Chain setters =====

    /// Sets the sequence number.
    pub fn sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    /// Sets the previous bundle hash.
    pub fn previous_hash(mut self, hash: Hash) -> Self {
        self.previous_hash = Some(hash);
        self
    }

    /// Sets the Merkle root.
    pub fn merkle_root(mut self, hash: Hash) -> Self {
        self.merkle_root = Some(hash);
        self
    }

    /// Sets the inclusion proof.
    pub fn inclusion_proof(mut self, proof: Vec<Hash>) -> Self {
        self.inclusion_proof = proof;
        self
    }

    /// Sets all chain info at once.
    pub fn set_chain(mut self, sequence: u64, previous_hash: Hash, merkle_root: Hash) -> Self {
        self.sequence = Some(sequence);
        self.previous_hash = Some(previous_hash);
        self.merkle_root = Some(merkle_root);
        self
    }

    // ===== Build =====

    /// Builds the evidence bundle.
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<EvidenceBundle, BundleError> {
        // Validate required fields
        let run_id = self.run_id.ok_or(BundleError::MissingField("run_id"))?;
        let capsule_id = self
            .capsule_id
            .ok_or(BundleError::MissingField("capsule_id"))?;
        let timestamp_ns = self
            .timestamp_ns
            .ok_or(BundleError::MissingField("timestamp_ns"))?;
        let manifest_hash = self
            .manifest_hash
            .ok_or(BundleError::MissingField("manifest_hash"))?;
        let workspace_hash = self
            .workspace_hash
            .ok_or(BundleError::MissingField("workspace_hash"))?;
        let exit_code = self
            .exit_code
            .ok_or(BundleError::MissingField("exit_code"))?;
        let workspace_diff_hash = self
            .workspace_diff_hash
            .ok_or(BundleError::MissingField("workspace_diff_hash"))?;

        // Build header
        let mut header = Header::new(format_uuid(&run_id), capsule_id, timestamp_ns);
        if let Some(version) = self.version {
            header = header.with_version(version);
        }
        if let Some(parent) = self.parent_run_id {
            header = header.with_parent(format_uuid(&parent));
        }

        // Build inputs
        let mut inputs =
            Inputs::new(crate::format_hash(&manifest_hash), crate::format_hash(&workspace_hash));
        if let Some(env_hash) = self.environment_hash {
            inputs = inputs.with_environment_hash(crate::format_hash(&env_hash));
        }
        if let Some(secrets_hash) = self.secrets_hash {
            inputs = inputs.with_secrets_hash(crate::format_hash(&secrets_hash));
        }
        if !self.command.is_empty() {
            inputs = inputs.with_command(self.command);
        }

        // Build execution
        let mut execution = ExecutionInfo::new(self.budget_consumed);
        for call in self.capability_calls {
            execution.add_capability_call(call);
        }
        for event in self.network_events {
            execution.add_network_event(event);
        }
        if let Some(duration) = self.duration_ns {
            execution = execution.with_duration_ns(duration);
        }

        // Build outputs
        let mut outputs = Outputs::new(exit_code, crate::format_hash(&workspace_diff_hash));
        for artifact in self.artifacts {
            outputs.add_artifact(artifact);
        }
        if let Some(hash) = self.stdout_hash {
            outputs = outputs.with_stdout_hash(crate::format_hash(&hash));
        }
        if let Some(hash) = self.stderr_hash {
            outputs = outputs.with_stderr_hash(crate::format_hash(&hash));
        }

        // Build chain info
        let sequence = self.sequence.unwrap_or(0);
        let previous_hash = self.previous_hash.unwrap_or([0u8; 32]);
        let merkle_root = self.merkle_root.unwrap_or([0u8; 32]);

        let chain = ChainInfo::new(
            sequence,
            crate::format_hash(&previous_hash),
            crate::format_hash(&merkle_root),
        )
        .with_inclusion_proof(self.inclusion_proof.iter().map(crate::format_hash).collect());

        // Create statement
        let statement = EvidenceStatement::new(header, inputs, execution, outputs, chain);

        let bundle = EvidenceBundle::new(statement);

        // Validate the built bundle
        bundle.validate()?;

        Ok(bundle)
    }
}

/// Formats a UUID as a hyphenated string.
fn format_uuid(bytes: &[u8; 16]) -> String {
    use alloc::format;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Errors that can occur when building or validating bundles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// A required field is missing
    MissingField(&'static str),
    /// Validation failed
    ValidationError(crate::statement::ValidationError),
    /// Serialization failed
    SerializationError(crate::statement::SerializationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bundle() -> EvidenceBundle {
        EvidenceBundleBuilder::new()
            .run_id([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0])
            .capsule_id("test-capsule")
            .timestamp_ns(1234567890000000000)
            .manifest_hash([0x01; 32])
            .workspace_hash([0x02; 32])
            .exit_code(0)
            .workspace_diff_hash([0x03; 32])
            .merkle_root([0x04; 32])
            .build()
            .expect("should build")
    }

    #[test]
    fn test_builder_basic() {
        let bundle = create_test_bundle();

        assert!(bundle.run_id().contains("12345678"));
        assert_eq!(bundle.capsule_id(), "test-capsule");
        assert_eq!(bundle.timestamp_ns(), 1234567890000000000);
        assert_eq!(bundle.exit_code(), 0);
    }

    #[test]
    fn test_builder_missing_run_id() {
        let result = EvidenceBundleBuilder::new()
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([0; 32])
            .workspace_hash([0; 32])
            .exit_code(0)
            .workspace_diff_hash([0; 32])
            .build();

        assert_eq!(result, Err(BundleError::MissingField("run_id")));
    }

    #[test]
    fn test_builder_missing_capsule_id() {
        let result = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .timestamp_ns(12345)
            .manifest_hash([0; 32])
            .workspace_hash([0; 32])
            .exit_code(0)
            .workspace_diff_hash([0; 32])
            .build();

        assert_eq!(result, Err(BundleError::MissingField("capsule_id")));
    }

    #[test]
    fn test_builder_with_capability_calls() {
        let call = CapabilityCall::new(0, 1000, "http", "sha256:000", "sha256:111");

        let bundle = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([0; 32])
            .workspace_hash([0; 32])
            .exit_code(0)
            .workspace_diff_hash([0; 32])
            .add_capability_call(call)
            .build()
            .expect("should build");

        assert_eq!(bundle.capability_calls().len(), 1);
        assert_eq!(bundle.capability_calls()[0].capability_type, "http");
    }

    #[test]
    fn test_builder_with_all_fields() {
        let bundle = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("full-test")
            .timestamp_ns(12345)
            .version("2.0.0")
            .parent_run_id([1; 16])
            .manifest_hash([1; 32])
            .workspace_hash([2; 32])
            .environment_hash([3; 32])
            .secrets_hash([4; 32])
            .command(alloc::vec![
                String::from("run"),
                String::from("--arg"),
            ])
            .budget_consumed(Budget::full(100, 200, 300, 400))
            .duration_ns(5000000)
            .exit_code(0)
            .workspace_diff_hash([5; 32])
            .stdout_hash([6; 32])
            .stderr_hash([7; 32])
            .add_artifact(Artifact::new("out.txt", "sha256:abc", 1024))
            .sequence(5)
            .previous_hash([8; 32])
            .merkle_root([9; 32])
            .inclusion_proof(alloc::vec![[10; 32], [11; 32]])
            .build()
            .expect("should build");

        assert_eq!(bundle.sequence(), 5);
        assert!(bundle.statement.header.parent_run_id.is_some());
        assert!(bundle.statement.inputs.environment_hash.is_some());
        assert!(bundle.statement.inputs.secrets_hash.is_some());
        assert_eq!(bundle.statement.inputs.command.len(), 2);
        assert!(bundle.statement.outputs.stdout_hash.is_some());
        assert_eq!(bundle.statement.outputs.artifacts.len(), 1);
        assert_eq!(bundle.statement.chain.inclusion_proof.len(), 2);
    }

    #[test]
    fn test_bundle_hash_deterministic() {
        let bundle1 = create_test_bundle();
        let bundle2 = create_test_bundle();

        assert_eq!(bundle1.compute_hash(), bundle2.compute_hash());
    }

    #[test]
    fn test_bundle_validate() {
        let bundle = create_test_bundle();
        assert!(bundle.validate().is_ok());
    }

    #[test]
    fn test_format_uuid() {
        let bytes = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0,
        ];
        let uuid = format_uuid(&bytes);
        assert_eq!(uuid, "12345678-9abc-def0-1234-56789abcdef0");
    }

    #[test]
    fn test_set_inputs() {
        let bundle = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .set_inputs(
                [1; 32],
                [2; 32],
                Some([3; 32]),
                None,
                alloc::vec![String::from("cmd")],
            )
            .exit_code(0)
            .workspace_diff_hash([0; 32])
            .build()
            .expect("should build");

        assert!(bundle.statement.inputs.environment_hash.is_some());
        assert!(bundle.statement.inputs.secrets_hash.is_none());
    }

    #[test]
    fn test_set_outputs() {
        let bundle = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([0; 32])
            .workspace_hash([0; 32])
            .set_outputs(
                42,
                [1; 32],
                alloc::vec![
                    Artifact::new("a.txt", "hash1", 100),
                    Artifact::new("b.txt", "hash2", 200),
                ],
            )
            .build()
            .expect("should build");

        assert_eq!(bundle.exit_code(), 42);
        assert_eq!(bundle.statement.outputs.artifacts.len(), 2);
    }

    #[test]
    fn test_set_chain() {
        let bundle = EvidenceBundleBuilder::new()
            .run_id([0; 16])
            .capsule_id("test")
            .timestamp_ns(12345)
            .manifest_hash([0; 32])
            .workspace_hash([0; 32])
            .exit_code(0)
            .workspace_diff_hash([0; 32])
            .set_chain(10, [1; 32], [2; 32])
            .build()
            .expect("should build");

        assert_eq!(bundle.sequence(), 10);
    }

    #[cfg(feature = "serde")]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_bundle_json_roundtrip() {
            let bundle = create_test_bundle();
            let json = bundle.to_json().unwrap();
            let parsed = EvidenceBundle::from_json(&json).unwrap();

            assert_eq!(bundle.run_id(), parsed.run_id());
            assert_eq!(bundle.capsule_id(), parsed.capsule_id());
            assert_eq!(bundle.timestamp_ns(), parsed.timestamp_ns());
            assert_eq!(bundle.exit_code(), parsed.exit_code());
        }
    }
}
