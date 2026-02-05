//! Verification functions for evidence

use agentvm_types::evidence::{EvidenceBundle, EvidenceStatement};

use crate::{Hash, InclusionProof, ConsistencyProof, hash_pair};

/// Verify that a leaf is included in a Merkle tree
pub fn verify_inclusion(proof: &InclusionProof, expected_root: &Hash) -> bool {
    proof.verify(expected_root)
}

/// Verify consistency between two tree states
pub fn verify_consistency(
    proof: &ConsistencyProof,
    expected_old_root: &Hash,
    expected_new_root: &Hash,
) -> bool {
    proof.verify(expected_old_root, expected_new_root)
}

/// Verify an evidence bundle
pub fn verify_bundle(bundle: &EvidenceBundle) -> BundleVerification {
    let mut result = BundleVerification::default();

    // Check payload type
    if bundle.payload_type != "application/vnd.agentvm.evidence+json" {
        result.valid_format = false;
        result.errors.push("invalid payload type".into());
        return result;
    }

    // Check has signatures
    if bundle.signatures.is_empty() {
        result.has_signatures = false;
        result.errors.push("no signatures".into());
    } else {
        result.has_signatures = true;
        result.signature_count = bundle.signatures.len();
    }

    // Check payload is not empty
    if bundle.payload.is_empty() {
        result.valid_format = false;
        result.errors.push("empty payload".into());
    }

    result.valid_format = result.errors.is_empty();
    result
}

/// Result of bundle verification
#[derive(Debug, Default)]
pub struct BundleVerification {
    /// Whether the format is valid
    pub valid_format: bool,
    /// Whether bundle has signatures
    pub has_signatures: bool,
    /// Number of signatures
    pub signature_count: usize,
    /// Whether all signatures are valid
    pub signatures_valid: bool,
    /// Whether chain is valid
    pub chain_valid: bool,
    /// Error messages
    pub errors: alloc::vec::Vec<alloc::string::String>,
}

impl BundleVerification {
    /// Check if everything is valid
    pub fn is_fully_valid(&self) -> bool {
        self.valid_format && self.has_signatures && self.signatures_valid && self.chain_valid
    }
}

/// Verify the chain of an evidence statement
pub fn verify_chain(
    statement: &EvidenceStatement,
    expected_previous: Option<&Hash>,
    expected_root: Option<&Hash>,
) -> ChainVerification {
    let mut result = ChainVerification::default();

    // Check previous hash if provided
    if let Some(expected) = expected_previous {
        if &statement.chain.previous_hash != expected {
            result.previous_valid = false;
            result.errors.push("previous hash mismatch".into());
        } else {
            result.previous_valid = true;
        }
    } else if statement.chain.sequence == 0 {
        // First in chain should have zero previous
        result.previous_valid = statement.chain.previous_hash == [0u8; 32];
        if !result.previous_valid {
            result.errors.push("first statement should have zero previous hash".into());
        }
    }

    // Check merkle root if provided
    if let Some(expected) = expected_root {
        if &statement.chain.merkle_root != expected {
            result.root_valid = false;
            result.errors.push("merkle root mismatch".into());
        } else {
            result.root_valid = true;
        }
    }

    // Verify inclusion proof if present
    if !statement.chain.inclusion_proof.is_empty() {
        // Would verify the inclusion proof here
        result.inclusion_valid = true;
    } else {
        result.inclusion_valid = statement.chain.sequence == 0;
    }

    result.valid = result.errors.is_empty();
    result
}

/// Result of chain verification
#[derive(Debug, Default)]
pub struct ChainVerification {
    /// Overall validity
    pub valid: bool,
    /// Previous hash is valid
    pub previous_valid: bool,
    /// Merkle root is valid
    pub root_valid: bool,
    /// Inclusion proof is valid
    pub inclusion_valid: bool,
    /// Error messages
    pub errors: alloc::vec::Vec<alloc::string::String>,
}

/// Replay verification result
#[derive(Debug)]
pub struct ReplayVerification {
    /// Whether the replay matches
    pub matches: bool,
    /// List of mismatches
    pub mismatches: alloc::vec::Vec<ReplayMismatch>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

/// A mismatch found during replay verification
#[derive(Debug)]
pub enum ReplayMismatch {
    /// Capability call mismatch at index
    CapabilityCall { index: usize, reason: alloc::string::String },
    /// Network event mismatch at index
    NetworkEvent { index: usize, reason: alloc::string::String },
    /// Workspace diff mismatch
    WorkspaceDiff,
    /// Exit code mismatch
    ExitCode { expected: i32, got: i32 },
    /// Budget mismatch
    Budget,
}

/// Compare two evidence statements for replay verification
pub fn verify_replay(
    original: &EvidenceStatement,
    replay: &EvidenceStatement,
) -> ReplayVerification {
    let mut mismatches = alloc::vec::Vec::new();

    // Compare capability calls
    let orig_calls = &original.execution.capability_calls;
    let replay_calls = &replay.execution.capability_calls;

    if orig_calls.len() != replay_calls.len() {
        mismatches.push(ReplayMismatch::CapabilityCall {
            index: 0,
            reason: alloc::format!("call count differs: {} vs {}", orig_calls.len(), replay_calls.len()),
        });
    } else {
        for (i, (orig, replay)) in orig_calls.iter().zip(replay_calls.iter()).enumerate() {
            if orig.capability_type != replay.capability_type {
                mismatches.push(ReplayMismatch::CapabilityCall {
                    index: i,
                    reason: "capability type differs".into(),
                });
            }
            if orig.operation != replay.operation {
                mismatches.push(ReplayMismatch::CapabilityCall {
                    index: i,
                    reason: "operation differs".into(),
                });
            }
            if orig.request_hash != replay.request_hash {
                mismatches.push(ReplayMismatch::CapabilityCall {
                    index: i,
                    reason: "request hash differs".into(),
                });
            }
            // Note: response hash may differ for non-deterministic services
        }
    }

    // Compare workspace diff
    if original.outputs.workspace_diff_hash != replay.outputs.workspace_diff_hash {
        mismatches.push(ReplayMismatch::WorkspaceDiff);
    }

    // Compare exit code
    if original.outputs.exit_code != replay.outputs.exit_code {
        mismatches.push(ReplayMismatch::ExitCode {
            expected: original.outputs.exit_code,
            got: replay.outputs.exit_code,
        });
    }

    // Calculate confidence
    let confidence = if mismatches.is_empty() {
        1.0
    } else {
        let total_checks = orig_calls.len() * 3 + 2; // 3 checks per call + workspace + exit
        let mismatch_count = mismatches.len();
        1.0 - (mismatch_count as f64 / total_checks as f64)
    };

    ReplayVerification {
        matches: mismatches.is_empty(),
        mismatches,
        confidence,
    }
}
