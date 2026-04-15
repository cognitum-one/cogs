//! Replay command implementation
//!
//! Replays execution from an evidence bundle and optionally verifies
//! that the effects match the original execution.
//!
//! Usage:
//!   agentvm replay <evidence> --verify-effects

use crate::commands::run::{EvidenceBundle, ExecutionResult, WorkspaceChanges};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{
    format_bytes, format_duration, format_hash, OutputFormat, OutputWriter, ProgressManager,
    TableDisplay,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Replay command arguments
#[derive(Debug, Clone)]
pub struct ReplayArgs {
    /// Path to evidence bundle
    pub evidence: PathBuf,
    /// Verify that effects match original
    pub verify_effects: bool,
    /// Output format
    pub output_format: OutputFormat,
    /// Workspace path for replay (uses temp if not specified)
    pub workspace: Option<PathBuf>,
    /// Dry run (show what would be replayed without executing)
    pub dry_run: bool,
}

/// Replay result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// Original run ID
    pub original_run_id: String,
    /// Replay run ID
    pub replay_run_id: String,
    /// Capsule ID
    pub capsule_id: String,
    /// Original exit code
    pub original_exit_code: i32,
    /// Replay exit code
    pub replay_exit_code: i32,
    /// Original duration (ms)
    pub original_duration_ms: u64,
    /// Replay duration (ms)
    pub replay_duration_ms: u64,
    /// Effects match
    pub effects_match: bool,
    /// Verification performed
    pub verified: bool,
    /// Mismatches found
    pub mismatches: Vec<Mismatch>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// A mismatch between original and replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mismatch {
    /// Type of mismatch
    pub mismatch_type: String,
    /// Description
    pub description: String,
    /// Original value
    pub original: String,
    /// Replay value
    pub replay: String,
    /// Severity (info, warning, error)
    pub severity: String,
}

impl TableDisplay for ReplayResult {
    fn table_headers() -> Vec<String> {
        vec![
            "Original".to_string(),
            "Replay".to_string(),
            "Exit".to_string(),
            "Duration".to_string(),
            "Match".to_string(),
            "Mismatches".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.original_run_id),
            format_hash(&self.replay_run_id),
            format!(
                "{}/{}",
                self.original_exit_code, self.replay_exit_code
            ),
            format!(
                "{}/{} ms",
                self.original_duration_ms, self.replay_duration_ms
            ),
            if self.effects_match { "Yes" } else { "No" }.to_string(),
            self.mismatches.len().to_string(),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.header("Replay Result");
        writer.kv("Original Run", &self.original_run_id);
        writer.kv("Replay Run", &self.replay_run_id);
        writer.kv("Capsule", &self.capsule_id);

        writer.header("Comparison");
        writer.kv(
            "Exit Code",
            format!(
                "original={} replay={}",
                self.original_exit_code, self.replay_exit_code
            ),
        );
        writer.kv(
            "Duration",
            format!(
                "original={} ms replay={} ms",
                self.original_duration_ms, self.replay_duration_ms
            ),
        );
        writer.kv(
            "Effects Match",
            if self.effects_match { "Yes" } else { "No" },
        );
        writer.kv("Confidence", format!("{:.1}%", self.confidence * 100.0));

        if !self.mismatches.is_empty() {
            writer.header("Mismatches");
            for mismatch in &self.mismatches {
                writer.kv(
                    &format!("  [{}]", mismatch.severity.to_uppercase()),
                    &mismatch.description,
                );
                writer.kv("    Original", &mismatch.original);
                writer.kv("    Replay", &mismatch.replay);
            }
        }
    }
}

/// Handle the replay command
pub async fn handle_replay(args: ReplayArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);
    let progress = ProgressManager::new();

    // Load evidence bundle
    let spinner = progress.spinner("Loading evidence bundle...");
    let bundle = load_evidence_bundle(&args.evidence)?;
    spinner.finish_with_message("Evidence loaded");

    writer.header("Replay Information");
    writer.kv("Original Run", &bundle.run_id);
    writer.kv("Capsule", &bundle.capsule_id);
    writer.kv("Command", bundle.inputs.command.join(" "));
    writer.kv(
        "Original Duration",
        format_duration(bundle.execution.duration_ns),
    );
    writer.kv(
        "Capability Calls",
        bundle.execution.capability_calls.len(),
    );

    // Dry run mode
    if args.dry_run {
        writer.info("Dry run - would replay with the above configuration");
        return Ok(());
    }

    // Setup workspace
    let workspace = match &args.workspace {
        Some(w) => {
            if !w.exists() {
                std::fs::create_dir_all(w)?;
            }
            w.clone()
        }
        None => {
            let temp_dir = std::env::temp_dir().join(format!("agentvm-replay-{}", bundle.run_id));
            std::fs::create_dir_all(&temp_dir)?;
            temp_dir
        }
    };

    writer.kv("Workspace", workspace.display());

    // Execute replay
    let spinner = progress.spinner("Replaying execution...");
    let start_time = std::time::Instant::now();
    let replay_run_id = uuid::Uuid::now_v7().to_string();

    let replay_result = execute_replay(&bundle, &workspace).await?;

    let replay_duration = start_time.elapsed();
    spinner.finish_with_message("Replay complete");

    // Verify effects if requested
    let (effects_match, mismatches) = if args.verify_effects {
        let spinner = progress.spinner("Verifying effects...");
        let result = verify_effects(&bundle, &replay_result, &workspace)?;
        spinner.finish_with_message("Verification complete");
        result
    } else {
        (true, vec![])
    };

    // Calculate confidence score
    let confidence = calculate_confidence(&bundle, &replay_result, &mismatches);

    let result = ReplayResult {
        original_run_id: bundle.run_id.clone(),
        replay_run_id,
        capsule_id: bundle.capsule_id.clone(),
        original_exit_code: bundle.outputs.exit_code,
        replay_exit_code: replay_result.exit_code,
        original_duration_ms: bundle.execution.duration_ns / 1_000_000,
        replay_duration_ms: replay_duration.as_millis() as u64,
        effects_match,
        verified: args.verify_effects,
        mismatches: mismatches.clone(),
        confidence,
    };

    writer.output(&result)?;

    if !effects_match {
        return Err(CliError::ReplayVerificationFailed {
            mismatches: mismatches.len(),
        });
    }

    writer.success("Replay completed successfully");
    Ok(())
}

/// Load evidence bundle from file
fn load_evidence_bundle(path: &Path) -> Result<EvidenceBundle> {
    if !path.exists() {
        return Err(CliError::Evidence(format!(
            "Evidence file not found: {}",
            path.display()
        )));
    }

    let content = std::fs::read_to_string(path)?;
    let bundle: EvidenceBundle = serde_json::from_str(&content)?;
    Ok(bundle)
}

/// Replay execution result
struct ReplayExecutionResult {
    exit_code: i32,
    stdout_hash: String,
    stderr_hash: String,
    workspace_diff_hash: String,
    capability_calls: Vec<ReplayedCapabilityCall>,
}

/// A capability call that was replayed
struct ReplayedCapabilityCall {
    capability_type: String,
    operation: String,
    request_hash: String,
    response_hash: String,
}

/// Execute replay from evidence bundle
async fn execute_replay(
    bundle: &EvidenceBundle,
    workspace: &Path,
) -> Result<ReplayExecutionResult> {
    // In a real implementation, this would:
    // 1. Restore workspace to initial state (from workspace_hash)
    // 2. Execute the command in a capsule
    // 3. Intercept capability calls and compare against expected
    // 4. Record all outputs

    // For now, we simulate the replay by executing the command
    let command = &bundle.inputs.command;
    if command.is_empty() {
        return Err(CliError::Replay("Empty command in evidence".to_string()));
    }

    let output = tokio::process::Command::new(&command[0])
        .args(&command[1..])
        .current_dir(workspace)
        .output()
        .await
        .map_err(|e| CliError::Replay(format!("Failed to execute: {}", e)))?;

    let stdout_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(&output.stdout))
    );
    let stderr_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(&output.stderr))
    );

    // Compute workspace diff
    let workspace_diff_hash = compute_workspace_hash(workspace)?;

    Ok(ReplayExecutionResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout_hash,
        stderr_hash,
        workspace_diff_hash,
        capability_calls: vec![],
    })
}

/// Compute workspace hash
fn compute_workspace_hash(workspace: &Path) -> Result<String> {
    let mut hasher = Sha256::new();

    for entry in walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if let Ok(content) = std::fs::read(path) {
            hasher.update(&Sha256::digest(&content));
        }
    }

    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

/// Verify effects match between original and replay
fn verify_effects(
    bundle: &EvidenceBundle,
    replay: &ReplayExecutionResult,
    _workspace: &Path,
) -> Result<(bool, Vec<Mismatch>)> {
    let mut mismatches = Vec::new();

    // Check exit code
    if bundle.outputs.exit_code != replay.exit_code {
        mismatches.push(Mismatch {
            mismatch_type: "exit_code".to_string(),
            description: "Exit code mismatch".to_string(),
            original: bundle.outputs.exit_code.to_string(),
            replay: replay.exit_code.to_string(),
            severity: "error".to_string(),
        });
    }

    // Check stdout hash
    if let Some(original_stdout) = &bundle.outputs.stdout_hash {
        if original_stdout != &replay.stdout_hash {
            mismatches.push(Mismatch {
                mismatch_type: "stdout".to_string(),
                description: "Standard output differs".to_string(),
                original: format_hash(original_stdout),
                replay: format_hash(&replay.stdout_hash),
                severity: "warning".to_string(),
            });
        }
    }

    // Check stderr hash
    if let Some(original_stderr) = &bundle.outputs.stderr_hash {
        if original_stderr != &replay.stderr_hash {
            mismatches.push(Mismatch {
                mismatch_type: "stderr".to_string(),
                description: "Standard error differs".to_string(),
                original: format_hash(original_stderr),
                replay: format_hash(&replay.stderr_hash),
                severity: "info".to_string(),
            });
        }
    }

    // Check capability calls count
    if bundle.execution.capability_calls.len() != replay.capability_calls.len() {
        mismatches.push(Mismatch {
            mismatch_type: "capability_calls".to_string(),
            description: "Capability call count differs".to_string(),
            original: bundle.execution.capability_calls.len().to_string(),
            replay: replay.capability_calls.len().to_string(),
            severity: "error".to_string(),
        });
    }

    // Check individual capability calls
    for (i, (orig, replayed)) in bundle
        .execution
        .capability_calls
        .iter()
        .zip(replay.capability_calls.iter())
        .enumerate()
    {
        if orig.capability_type != replayed.capability_type {
            mismatches.push(Mismatch {
                mismatch_type: "capability_type".to_string(),
                description: format!("Capability type mismatch at index {}", i),
                original: orig.capability_type.clone(),
                replay: replayed.capability_type.clone(),
                severity: "error".to_string(),
            });
        }

        if orig.request_hash != replayed.request_hash {
            mismatches.push(Mismatch {
                mismatch_type: "request_hash".to_string(),
                description: format!("Request hash mismatch at index {}", i),
                original: format_hash(&orig.request_hash),
                replay: format_hash(&replayed.request_hash),
                severity: "warning".to_string(),
            });
        }
    }

    let effects_match = !mismatches
        .iter()
        .any(|m| m.severity == "error");

    Ok((effects_match, mismatches))
}

/// Calculate confidence score
fn calculate_confidence(
    bundle: &EvidenceBundle,
    replay: &ReplayExecutionResult,
    mismatches: &[Mismatch],
) -> f64 {
    let mut score: f64 = 1.0;

    // Reduce score based on mismatches
    for mismatch in mismatches {
        match mismatch.severity.as_str() {
            "error" => score -= 0.3,
            "warning" => score -= 0.1,
            "info" => score -= 0.02,
            _ => {}
        }
    }

    // Bonus for matching exit code
    if bundle.outputs.exit_code == replay.exit_code {
        score += 0.1;
    }

    // Bonus for matching capability call count
    if bundle.execution.capability_calls.len() == replay.capability_calls.len() {
        score += 0.1;
    }

    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_calculate_confidence() {
        let bundle = create_test_bundle();
        let replay = ReplayExecutionResult {
            exit_code: 0,
            stdout_hash: "sha256:abc".to_string(),
            stderr_hash: "sha256:def".to_string(),
            workspace_diff_hash: "sha256:ghi".to_string(),
            capability_calls: vec![],
        };
        let mismatches = vec![];

        let confidence = calculate_confidence(&bundle, &replay, &mismatches);
        assert!(confidence > 0.9);
    }

    #[test]
    fn test_confidence_with_errors() {
        let bundle = create_test_bundle();
        let replay = ReplayExecutionResult {
            exit_code: 1,
            stdout_hash: "sha256:abc".to_string(),
            stderr_hash: "sha256:def".to_string(),
            workspace_diff_hash: "sha256:ghi".to_string(),
            capability_calls: vec![],
        };
        let mismatches = vec![Mismatch {
            mismatch_type: "exit_code".to_string(),
            description: "Exit code mismatch".to_string(),
            original: "0".to_string(),
            replay: "1".to_string(),
            severity: "error".to_string(),
        }];

        let confidence = calculate_confidence(&bundle, &replay, &mismatches);
        assert!(confidence < 0.8);
    }

    fn create_test_bundle() -> EvidenceBundle {
        EvidenceBundle {
            version: "1.0".to_string(),
            run_id: "test-run".to_string(),
            capsule_id: "test-capsule".to_string(),
            inputs: crate::commands::run::InputsSection {
                manifest_hash: "sha256:abc".to_string(),
                workspace_hash: "sha256:def".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                environment_hash: None,
            },
            execution: crate::commands::run::ExecutionSection {
                capability_calls: vec![],
                network_events: vec![],
                budget_consumed: crate::commands::run::BudgetConsumed::default(),
                duration_ns: 1_000_000,
                start_timestamp_ns: 0,
                end_timestamp_ns: 1_000_000,
            },
            outputs: crate::commands::run::OutputsSection {
                exit_code: 0,
                workspace_diff_hash: "sha256:ghi".to_string(),
                artifacts: vec![],
                stdout_hash: None,
                stderr_hash: None,
            },
            chain: crate::commands::run::ChainSection {
                sequence: 1,
                previous_hash: "sha256:000".to_string(),
                merkle_root: "sha256:111".to_string(),
                inclusion_proof: vec![],
            },
            signature: None,
        }
    }
}
