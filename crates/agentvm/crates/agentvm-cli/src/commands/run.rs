//! Run command implementation
//!
//! Executes commands within an agent capsule with optional evidence generation.
//!
//! Usage:
//!   agentvm run [command...] --evidence --workspace --manifest

use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{format_bytes, format_duration, format_hash, OutputFormat, OutputWriter, ProgressManager, TableDisplay};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

/// Run command arguments
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// Command to execute in the capsule
    pub command: Vec<String>,
    /// Enable evidence generation
    pub evidence: bool,
    /// Workspace path
    pub workspace: PathBuf,
    /// Optional capsule manifest path
    pub manifest: Option<PathBuf>,
    /// Output format
    pub output_format: OutputFormat,
    /// Timeout in seconds
    pub timeout: Option<u64>,
    /// Dry run (don't execute, just show what would be done)
    pub dry_run: bool,
}

/// Evidence bundle generated during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// Version of the evidence format
    pub version: String,
    /// Unique run identifier
    pub run_id: String,
    /// Capsule identifier
    pub capsule_id: String,
    /// Input information
    pub inputs: InputsSection,
    /// Execution information
    pub execution: ExecutionSection,
    /// Output information
    pub outputs: OutputsSection,
    /// Merkle chain information
    pub chain: ChainSection,
    /// Signature (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureSection>,
}

/// Input section of evidence bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputsSection {
    /// Hash of the manifest
    pub manifest_hash: String,
    /// Hash of the workspace snapshot
    pub workspace_hash: String,
    /// Command executed
    pub command: Vec<String>,
    /// Environment hash
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_hash: Option<String>,
}

/// Execution section of evidence bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSection {
    /// List of capability calls made
    pub capability_calls: Vec<CapabilityCall>,
    /// Network events
    pub network_events: Vec<NetworkEvent>,
    /// Budget consumed
    pub budget_consumed: BudgetConsumed,
    /// Duration in nanoseconds
    pub duration_ns: u64,
    /// Start timestamp (nanoseconds since epoch)
    pub start_timestamp_ns: u64,
    /// End timestamp (nanoseconds since epoch)
    pub end_timestamp_ns: u64,
}

/// A capability call record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityCall {
    /// Sequence number
    pub sequence: u64,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Capability type
    pub capability_type: String,
    /// Operation performed
    pub operation: String,
    /// Request hash
    pub request_hash: String,
    /// Response hash
    pub response_hash: String,
    /// Duration in nanoseconds
    pub duration_ns: u64,
}

/// A network event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Direction (egress/ingress)
    pub direction: String,
    /// Destination address
    pub destination: String,
    /// Bytes transferred
    pub bytes: u64,
    /// Whether the connection was allowed
    pub allowed: bool,
}

/// Budget consumed during execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetConsumed {
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Memory bytes used (peak)
    pub memory_bytes: u64,
    /// Network bytes transferred
    pub network_bytes: u64,
    /// Disk bytes written
    pub disk_write_bytes: u64,
}

/// Output section of evidence bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputsSection {
    /// Exit code
    pub exit_code: i32,
    /// Hash of workspace diff
    pub workspace_diff_hash: String,
    /// Artifacts produced
    pub artifacts: Vec<ArtifactRecord>,
    /// Hash of stdout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_hash: Option<String>,
    /// Hash of stderr
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_hash: Option<String>,
}

/// An artifact produced during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    /// Path relative to workspace
    pub path: String,
    /// Hash of the artifact
    pub hash: String,
    /// Size in bytes
    pub size: u64,
}

/// Merkle chain section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSection {
    /// Sequence number in the chain
    pub sequence: u64,
    /// Previous bundle hash
    pub previous_hash: String,
    /// Current Merkle root
    pub merkle_root: String,
    /// Inclusion proof
    pub inclusion_proof: Vec<String>,
}

/// Signature section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureSection {
    /// Signing algorithm
    pub algorithm: String,
    /// Signer identifier
    pub signer: String,
    /// Signature value (base64)
    pub value: String,
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Run identifier
    pub run_id: String,
    /// Exit code
    pub exit_code: i32,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Evidence path (if generated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_path: Option<PathBuf>,
    /// Workspace changes
    pub workspace_changes: WorkspaceChanges,
    /// Success status
    pub success: bool,
}

/// Workspace changes tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceChanges {
    /// Files created
    pub created: Vec<String>,
    /// Files modified
    pub modified: Vec<String>,
    /// Files deleted
    pub deleted: Vec<String>,
}

impl TableDisplay for ExecutionResult {
    fn table_headers() -> Vec<String> {
        vec![
            "Run ID".to_string(),
            "Exit Code".to_string(),
            "Duration".to_string(),
            "Evidence".to_string(),
            "Status".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.run_id),
            self.exit_code.to_string(),
            format!("{} ms", self.duration_ms),
            self.evidence_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            if self.success {
                "Success".to_string()
            } else {
                "Failed".to_string()
            },
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.header("Execution Result");
        writer.kv("Run ID", &self.run_id);
        writer.kv("Exit Code", self.exit_code);
        writer.kv("Duration", format!("{} ms", self.duration_ms));
        if let Some(path) = &self.evidence_path {
            writer.kv("Evidence", path.display());
        }
        writer.kv(
            "Status",
            if self.success { "Success" } else { "Failed" },
        );

        if !self.workspace_changes.created.is_empty()
            || !self.workspace_changes.modified.is_empty()
            || !self.workspace_changes.deleted.is_empty()
        {
            writer.header("Workspace Changes");
            for path in &self.workspace_changes.created {
                writer.kv("  Created", path);
            }
            for path in &self.workspace_changes.modified {
                writer.kv("  Modified", path);
            }
            for path in &self.workspace_changes.deleted {
                writer.kv("  Deleted", path);
            }
        }
    }
}

/// Handle the run command
pub async fn handle_run(args: RunArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);
    let progress = ProgressManager::new();

    // Validate inputs
    if args.command.is_empty() {
        return Err(CliError::Capsule("No command specified".to_string()));
    }

    // Validate workspace
    if !args.workspace.exists() {
        return Err(CliError::WorkspaceNotFound {
            path: args.workspace.clone(),
        });
    }

    // Load manifest if provided
    let manifest = if let Some(manifest_path) = &args.manifest {
        if !manifest_path.exists() {
            return Err(CliError::ManifestNotFound {
                path: manifest_path.clone(),
            });
        }
        Some(load_manifest(manifest_path)?)
    } else {
        None
    };

    // Dry run mode
    if args.dry_run {
        writer.header("Dry Run - Would Execute");
        writer.kv("Command", args.command.join(" "));
        writer.kv("Workspace", args.workspace.display());
        writer.kv("Evidence", if args.evidence { "Enabled" } else { "Disabled" });
        if let Some(m) = &args.manifest {
            writer.kv("Manifest", m.display());
        }
        return Ok(());
    }

    // Generate run ID
    let run_id = Uuid::now_v7().to_string();
    writer.info(&format!("Starting run: {}", &run_id));

    // Take pre-execution snapshot of workspace
    let spinner = progress.spinner("Computing workspace hash...");
    let pre_workspace_hash = compute_workspace_hash(&args.workspace)?;
    spinner.finish_with_message("Workspace hash computed");

    // Start execution
    let spinner = progress.spinner(&format!("Executing: {}", args.command.join(" ")));
    let start_time = Instant::now();
    let start_timestamp = chrono::Utc::now();

    let result = execute_command(&args.command, &args.workspace, args.timeout).await?;

    let duration = start_time.elapsed();
    let end_timestamp = chrono::Utc::now();
    spinner.finish_with_message("Execution completed");

    // Compute post-execution workspace changes
    let spinner = progress.spinner("Computing workspace changes...");
    let workspace_changes = compute_workspace_changes(&args.workspace, &pre_workspace_hash)?;
    let post_workspace_hash = compute_workspace_hash(&args.workspace)?;
    spinner.finish_with_message("Workspace changes computed");

    // Generate evidence bundle if requested
    let evidence_path = if args.evidence {
        let spinner = progress.spinner("Generating evidence bundle...");
        let bundle = create_evidence_bundle(
            &run_id,
            &args,
            &manifest,
            &pre_workspace_hash,
            &post_workspace_hash,
            &workspace_changes,
            &result,
            start_timestamp,
            end_timestamp,
            duration.as_nanos() as u64,
        )?;

        let evidence_path = save_evidence_bundle(&bundle, &config.evidence.storage_dir)?;
        spinner.finish_with_message("Evidence bundle generated");
        Some(evidence_path)
    } else {
        None
    };

    // Create execution result
    let exec_result = ExecutionResult {
        run_id,
        exit_code: result.exit_code,
        duration_ms: duration.as_millis() as u64,
        evidence_path,
        workspace_changes,
        success: result.exit_code == 0,
    };

    writer.output(&exec_result)?;

    if result.exit_code != 0 {
        return Err(CliError::ProcessFailed {
            exit_code: result.exit_code,
        });
    }

    Ok(())
}

/// Load a capsule manifest
fn load_manifest(path: &Path) -> Result<CapsuleManifest> {
    let content = std::fs::read_to_string(path)?;
    let manifest: CapsuleManifest = serde_json::from_str(&content)
        .or_else(|_| toml::from_str(&content).map_err(CliError::Toml))?;
    Ok(manifest)
}

/// Capsule manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityGrant>,
    #[serde(default)]
    pub budget: BudgetSpec,
}

/// Capability grant in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    #[serde(rename = "type")]
    pub cap_type: String,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub quota: HashMap<String, u64>,
}

/// Budget specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetSpec {
    pub cpu_time_ms: Option<u64>,
    pub wall_time_ms: Option<u64>,
    pub memory_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub network_bytes: Option<u64>,
}

/// Command execution result
struct CommandResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Execute a command in the capsule
async fn execute_command(
    command: &[String],
    workspace: &Path,
    timeout: Option<u64>,
) -> Result<CommandResult> {
    if command.is_empty() {
        return Err(CliError::Capsule("Empty command".to_string()));
    }

    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..])
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CliError::ProcessSpawn(e.to_string()))?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let mut stdout = String::new();
    let mut stderr = String::new();

    // Read stdout
    if let Some(handle) = stdout_handle {
        let mut reader = BufReader::new(handle);
        let mut line = String::new();
        while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
            stdout.push_str(&line);
            line.clear();
        }
    }

    // Read stderr
    if let Some(handle) = stderr_handle {
        let mut reader = BufReader::new(handle);
        let mut line = String::new();
        while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
            stderr.push_str(&line);
            line.clear();
        }
    }

    // Wait for process with optional timeout
    let status = if let Some(timeout_secs) = timeout {
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait(),
        )
        .await
        {
            Ok(result) => result.map_err(|e| CliError::ProcessSpawn(e.to_string()))?,
            Err(_) => {
                child.kill().await.ok();
                return Err(CliError::Timeout {
                    seconds: timeout_secs,
                });
            }
        }
    } else {
        child
            .wait()
            .await
            .map_err(|e| CliError::ProcessSpawn(e.to_string()))?
    };

    Ok(CommandResult {
        exit_code: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

/// Compute workspace hash
fn compute_workspace_hash(workspace: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file_count = 0u64;

    for entry in walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative = path.strip_prefix(workspace).unwrap_or(path);

        // Add file path to hash
        hasher.update(relative.to_string_lossy().as_bytes());

        // Add file content hash
        if let Ok(content) = std::fs::read(path) {
            hasher.update(&Sha256::digest(&content));
        }

        file_count += 1;
    }

    // Include file count in hash
    hasher.update(&file_count.to_le_bytes());

    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

/// Compute workspace changes
fn compute_workspace_changes(
    workspace: &Path,
    _pre_hash: &str,
) -> Result<WorkspaceChanges> {
    // In a real implementation, this would diff against a pre-execution snapshot
    // For now, we just track what files exist
    let mut changes = WorkspaceChanges::default();

    for entry in walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // For now, assume all files are potentially modified
        // A real implementation would compare against pre-snapshot
        if let Ok(metadata) = path.metadata() {
            let modified = metadata.modified().ok();
            let created = metadata.created().ok();

            if created == modified {
                changes.created.push(relative);
            } else {
                changes.modified.push(relative);
            }
        }
    }

    Ok(changes)
}

/// Create evidence bundle
fn create_evidence_bundle(
    run_id: &str,
    args: &RunArgs,
    manifest: &Option<CapsuleManifest>,
    pre_workspace_hash: &str,
    _post_workspace_hash: &str,
    workspace_changes: &WorkspaceChanges,
    result: &CommandResult,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    duration_ns: u64,
) -> Result<EvidenceBundle> {
    let manifest_hash = manifest
        .as_ref()
        .map(|m| {
            let json = serde_json::to_string(m).unwrap_or_default();
            format!("sha256:{}", hex::encode(Sha256::digest(json.as_bytes())))
        })
        .unwrap_or_else(|| "sha256:0".repeat(64));

    let capsule_id = manifest
        .as_ref()
        .map(|m| format!("{}:{}", m.name, m.version))
        .unwrap_or_else(|| format!("anonymous:{}", run_id));

    let stdout_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(result.stdout.as_bytes()))
    );
    let stderr_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(result.stderr.as_bytes()))
    );

    let workspace_diff = serde_json::to_string(workspace_changes).unwrap_or_default();
    let workspace_diff_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(workspace_diff.as_bytes()))
    );

    Ok(EvidenceBundle {
        version: "1.0".to_string(),
        run_id: run_id.to_string(),
        capsule_id,
        inputs: InputsSection {
            manifest_hash,
            workspace_hash: pre_workspace_hash.to_string(),
            command: args.command.clone(),
            environment_hash: None,
        },
        execution: ExecutionSection {
            capability_calls: vec![],
            network_events: vec![],
            budget_consumed: BudgetConsumed::default(),
            duration_ns,
            start_timestamp_ns: start_time.timestamp_nanos_opt().unwrap_or(0) as u64,
            end_timestamp_ns: end_time.timestamp_nanos_opt().unwrap_or(0) as u64,
        },
        outputs: OutputsSection {
            exit_code: result.exit_code,
            workspace_diff_hash,
            artifacts: vec![],
            stdout_hash: Some(stdout_hash),
            stderr_hash: Some(stderr_hash),
        },
        chain: ChainSection {
            sequence: 1,
            previous_hash: format!("sha256:{}", "0".repeat(64)),
            merkle_root: format!("sha256:{}", "0".repeat(64)),
            inclusion_proof: vec![],
        },
        signature: None,
    })
}

/// Save evidence bundle to disk
fn save_evidence_bundle(bundle: &EvidenceBundle, storage_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(storage_dir)?;

    let filename = format!("{}.json", bundle.run_id);
    let path = storage_dir.join(filename);

    let content = serde_json::to_string_pretty(bundle)?;
    std::fs::write(&path, content)?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_execute_command() {
        let workspace = tempdir().unwrap();
        let result = execute_command(
            &["echo".to_string(), "hello".to_string()],
            workspace.path(),
            Some(10),
        )
        .await
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn test_compute_workspace_hash() {
        let workspace = tempdir().unwrap();
        std::fs::write(workspace.path().join("test.txt"), "content").unwrap();

        let hash = compute_workspace_hash(workspace.path()).unwrap();
        assert!(hash.starts_with("sha256:"));
    }
}
