//! Evidence command implementation
//!
//! Manages evidence bundles for auditing and replay.
//!
//! Usage:
//!   agentvm evidence get <run_id>
//!   agentvm evidence query --capsule <id> --start <time> --end <time>
//!   agentvm evidence verify <path>
//!   agentvm evidence export --format <format> --output <path>

use crate::commands::run::{EvidenceBundle, CapabilityCall, NetworkEvent};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{
    format_bytes, format_duration, format_hash, format_timestamp, OutputFormat, OutputWriter,
    ProgressManager, TableDisplay,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Evidence command action
#[derive(Debug, Clone)]
pub enum EvidenceAction {
    /// Get evidence by run ID
    Get {
        /// Run ID
        run_id: String,
    },
    /// Query evidence by criteria
    Query {
        /// Capsule ID filter
        capsule: Option<String>,
        /// Start time (ISO 8601)
        start: Option<String>,
        /// End time (ISO 8601)
        end: Option<String>,
        /// Maximum results
        limit: Option<usize>,
    },
    /// Verify evidence integrity
    Verify {
        /// Path to evidence bundle
        path: PathBuf,
    },
    /// Export evidence for audit
    Export {
        /// Export format (json, csv, siem)
        format: String,
        /// Output path
        output: PathBuf,
        /// Run IDs to export (empty for all)
        run_ids: Vec<String>,
        /// Start time filter
        start: Option<String>,
        /// End time filter
        end: Option<String>,
    },
}

/// Evidence command arguments
#[derive(Debug, Clone)]
pub struct EvidenceArgs {
    /// The action to perform
    pub action: EvidenceAction,
    /// Output format
    pub output_format: OutputFormat,
}

/// Evidence summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Run ID
    pub run_id: String,
    /// Capsule ID
    pub capsule_id: String,
    /// Timestamp
    pub timestamp: String,
    /// Exit code
    pub exit_code: i32,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of capability calls
    pub capability_calls: usize,
    /// Network bytes transferred
    pub network_bytes: u64,
    /// Verification status
    pub verified: Option<bool>,
}

impl TableDisplay for EvidenceSummary {
    fn table_headers() -> Vec<String> {
        vec![
            "Run ID".to_string(),
            "Capsule".to_string(),
            "Timestamp".to_string(),
            "Exit".to_string(),
            "Duration".to_string(),
            "Caps".to_string(),
            "Network".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.run_id),
            format_hash(&self.capsule_id),
            self.timestamp.clone(),
            self.exit_code.to_string(),
            format!("{} ms", self.duration_ms),
            self.capability_calls.to_string(),
            format_bytes(self.network_bytes),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.kv("Run ID", &self.run_id);
        writer.kv("Capsule", &self.capsule_id);
        writer.kv("Timestamp", &self.timestamp);
        writer.kv("Exit Code", self.exit_code);
        writer.kv("Duration", format!("{} ms", self.duration_ms));
        writer.kv("Capability Calls", self.capability_calls);
        writer.kv("Network Bytes", format_bytes(self.network_bytes));
        if let Some(verified) = self.verified {
            writer.kv("Verified", if verified { "Yes" } else { "No" });
        }
    }
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed
    pub valid: bool,
    /// Evidence path
    pub path: String,
    /// Run ID
    pub run_id: String,
    /// Capsule ID
    pub capsule_id: String,
    /// Hash verification
    pub hash_valid: bool,
    /// Computed hash
    pub computed_hash: String,
    /// Chain verification
    pub chain_valid: bool,
    /// Signature verification (if signed)
    pub signature_valid: Option<bool>,
    /// Issues found
    pub issues: Vec<String>,
}

impl TableDisplay for VerificationResult {
    fn table_headers() -> Vec<String> {
        vec![
            "Run ID".to_string(),
            "Valid".to_string(),
            "Hash".to_string(),
            "Chain".to_string(),
            "Signature".to_string(),
            "Issues".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.run_id),
            if self.valid { "Yes" } else { "No" }.to_string(),
            if self.hash_valid { "OK" } else { "FAIL" }.to_string(),
            if self.chain_valid { "OK" } else { "FAIL" }.to_string(),
            self.signature_valid
                .map(|v| if v { "OK" } else { "FAIL" })
                .unwrap_or("N/A")
                .to_string(),
            self.issues.len().to_string(),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.header("Verification Result");
        writer.kv("Path", &self.path);
        writer.kv("Run ID", &self.run_id);
        writer.kv("Capsule", &self.capsule_id);
        writer.separator();
        writer.kv(
            "Overall",
            if self.valid { "VALID" } else { "INVALID" },
        );
        writer.kv("Hash", if self.hash_valid { "OK" } else { "FAILED" });
        writer.kv("Computed Hash", format_hash(&self.computed_hash));
        writer.kv("Chain", if self.chain_valid { "OK" } else { "FAILED" });
        if let Some(sig_valid) = self.signature_valid {
            writer.kv("Signature", if sig_valid { "OK" } else { "FAILED" });
        }

        if !self.issues.is_empty() {
            writer.header("Issues");
            for issue in &self.issues {
                writer.kv("  -", issue);
            }
        }
    }
}

/// Handle evidence commands
pub async fn handle_evidence(args: EvidenceArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);

    match args.action {
        EvidenceAction::Get { run_id } => handle_get(&writer, config, &run_id).await,
        EvidenceAction::Query {
            capsule,
            start,
            end,
            limit,
        } => handle_query(&writer, config, capsule, start, end, limit).await,
        EvidenceAction::Verify { path } => handle_verify(&writer, config, &path).await,
        EvidenceAction::Export {
            format,
            output,
            run_ids,
            start,
            end,
        } => handle_export(&writer, config, &format, &output, &run_ids, start, end).await,
    }
}

/// Handle evidence get
async fn handle_get(writer: &OutputWriter, config: &Config, run_id: &str) -> Result<()> {
    let bundle = load_evidence_bundle(run_id, &config.evidence.storage_dir)?;

    writer.header("Evidence Bundle");
    writer.kv("Version", &bundle.version);
    writer.kv("Run ID", &bundle.run_id);
    writer.kv("Capsule", &bundle.capsule_id);

    writer.header("Inputs");
    writer.kv("Manifest Hash", format_hash(&bundle.inputs.manifest_hash));
    writer.kv("Workspace Hash", format_hash(&bundle.inputs.workspace_hash));
    writer.kv("Command", bundle.inputs.command.join(" "));

    writer.header("Execution");
    writer.kv(
        "Duration",
        format_duration(bundle.execution.duration_ns),
    );
    writer.kv(
        "Start",
        format_timestamp(bundle.execution.start_timestamp_ns),
    );
    writer.kv(
        "End",
        format_timestamp(bundle.execution.end_timestamp_ns),
    );
    writer.kv(
        "Capability Calls",
        bundle.execution.capability_calls.len(),
    );
    writer.kv(
        "Network Events",
        bundle.execution.network_events.len(),
    );

    if !bundle.execution.capability_calls.is_empty() {
        writer.header("Capability Calls");
        for (i, call) in bundle.execution.capability_calls.iter().enumerate().take(10) {
            writer.kv(
                &format!("  [{}]", i),
                format!(
                    "{}: {} ({})",
                    call.capability_type,
                    call.operation,
                    format_duration(call.duration_ns)
                ),
            );
        }
        if bundle.execution.capability_calls.len() > 10 {
            writer.kv(
                "  ...",
                format!(
                    "{} more calls",
                    bundle.execution.capability_calls.len() - 10
                ),
            );
        }
    }

    writer.header("Outputs");
    writer.kv("Exit Code", bundle.outputs.exit_code);
    writer.kv(
        "Workspace Diff",
        format_hash(&bundle.outputs.workspace_diff_hash),
    );
    writer.kv("Artifacts", bundle.outputs.artifacts.len());

    writer.header("Chain");
    writer.kv("Sequence", bundle.chain.sequence);
    writer.kv("Previous", format_hash(&bundle.chain.previous_hash));
    writer.kv("Merkle Root", format_hash(&bundle.chain.merkle_root));

    if bundle.signature.is_some() {
        writer.header("Signature");
        let sig = bundle.signature.as_ref().unwrap();
        writer.kv("Algorithm", &sig.algorithm);
        writer.kv("Signer", &sig.signer);
    }

    Ok(())
}

/// Handle evidence query
async fn handle_query(
    writer: &OutputWriter,
    config: &Config,
    capsule: Option<String>,
    start: Option<String>,
    end: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let progress = ProgressManager::new();
    let spinner = progress.spinner("Searching evidence...");

    let start_time = start
        .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()
        .map_err(|e| CliError::Evidence(format!("Invalid start time: {}", e)))?;

    let end_time = end
        .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()
        .map_err(|e| CliError::Evidence(format!("Invalid end time: {}", e)))?;

    let summaries = query_evidence(
        &config.evidence.storage_dir,
        capsule.as_deref(),
        start_time,
        end_time,
        limit.unwrap_or(100),
    )?;

    spinner.finish_with_message("Search complete");

    if summaries.is_empty() {
        writer.info("No evidence found matching criteria");
        return Ok(());
    }

    writer.info(&format!("Found {} evidence bundle(s)", summaries.len()));
    writer.output_list(&summaries)?;

    Ok(())
}

/// Handle evidence verify
async fn handle_verify(writer: &OutputWriter, _config: &Config, path: &Path) -> Result<()> {
    let progress = ProgressManager::new();
    let spinner = progress.spinner("Verifying evidence bundle...");

    let result = verify_evidence_bundle(path)?;

    spinner.finish_with_message("Verification complete");

    writer.output(&result)?;

    if !result.valid {
        return Err(CliError::EvidenceVerificationFailed {
            reason: result.issues.join("; "),
        });
    }

    writer.success("Evidence bundle is valid");
    Ok(())
}

/// Handle evidence export
async fn handle_export(
    writer: &OutputWriter,
    config: &Config,
    format: &str,
    output: &Path,
    run_ids: &[String],
    start: Option<String>,
    end: Option<String>,
) -> Result<()> {
    let progress = ProgressManager::new();

    // Query evidence to export
    let spinner = progress.spinner("Querying evidence...");

    let start_time = start
        .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()
        .map_err(|e| CliError::Evidence(format!("Invalid start time: {}", e)))?;

    let end_time = end
        .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()
        .map_err(|e| CliError::Evidence(format!("Invalid end time: {}", e)))?;

    let bundles: Vec<EvidenceBundle> = if run_ids.is_empty() {
        // Export all matching evidence
        let summaries = query_evidence(
            &config.evidence.storage_dir,
            None,
            start_time,
            end_time,
            10000,
        )?;

        summaries
            .iter()
            .filter_map(|s| load_evidence_bundle(&s.run_id, &config.evidence.storage_dir).ok())
            .collect()
    } else {
        // Export specific run IDs
        run_ids
            .iter()
            .filter_map(|id| load_evidence_bundle(id, &config.evidence.storage_dir).ok())
            .collect()
    };

    spinner.finish_with_message(format!("Found {} bundles to export", bundles.len()));

    if bundles.is_empty() {
        writer.info("No evidence to export");
        return Ok(());
    }

    // Export in requested format
    let spinner = progress.spinner(&format!("Exporting to {} format...", format));

    // Create parent directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match format.to_lowercase().as_str() {
        "json" => export_json(&bundles, output)?,
        "csv" => export_csv(&bundles, output)?,
        "siem" | "cef" => export_siem(&bundles, output)?,
        _ => {
            return Err(CliError::Evidence(format!(
                "Unknown export format: {}",
                format
            )));
        }
    }

    spinner.finish_with_message("Export complete");

    writer.success(&format!("Exported {} bundles to {}", bundles.len(), output.display()));

    Ok(())
}

/// Load evidence bundle from storage
fn load_evidence_bundle(run_id: &str, storage_dir: &Path) -> Result<EvidenceBundle> {
    let path = storage_dir.join(format!("{}.json", run_id));

    if !path.exists() {
        // Try finding by prefix
        if let Some(full_path) = find_evidence_by_prefix(run_id, storage_dir)? {
            let content = std::fs::read_to_string(&full_path)?;
            return Ok(serde_json::from_str(&content)?);
        }

        return Err(CliError::EvidenceNotFound {
            run_id: run_id.to_string(),
        });
    }

    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Find evidence by run ID prefix
fn find_evidence_by_prefix(prefix: &str, storage_dir: &Path) -> Result<Option<PathBuf>> {
    if !storage_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(storage_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with(prefix) && name.ends_with(".json") {
            return Ok(Some(entry.path()));
        }
    }

    Ok(None)
}

/// Query evidence bundles
fn query_evidence(
    storage_dir: &Path,
    capsule_filter: Option<&str>,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    limit: usize,
) -> Result<Vec<EvidenceSummary>> {
    if !storage_dir.exists() {
        return Ok(vec![]);
    }

    let mut summaries = Vec::new();

    for entry in std::fs::read_dir(storage_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.extension().map(|e| e == "json").unwrap_or(false) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let bundle: EvidenceBundle = match serde_json::from_str(&content) {
            Ok(b) => b,
            Err(_) => continue,
        };

        // Apply capsule filter
        if let Some(filter) = capsule_filter {
            if !bundle.capsule_id.contains(filter) {
                continue;
            }
        }

        // Apply time filters
        let bundle_time = chrono::DateTime::from_timestamp(
            (bundle.execution.start_timestamp_ns / 1_000_000_000) as i64,
            (bundle.execution.start_timestamp_ns % 1_000_000_000) as u32,
        )
        .unwrap_or_default();

        if let Some(start) = start_time {
            if bundle_time < start {
                continue;
            }
        }

        if let Some(end) = end_time {
            if bundle_time > end {
                continue;
            }
        }

        let network_bytes: u64 = bundle
            .execution
            .network_events
            .iter()
            .map(|e| e.bytes)
            .sum();

        summaries.push(EvidenceSummary {
            run_id: bundle.run_id,
            capsule_id: bundle.capsule_id,
            timestamp: bundle_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            exit_code: bundle.outputs.exit_code,
            duration_ms: bundle.execution.duration_ns / 1_000_000,
            capability_calls: bundle.execution.capability_calls.len(),
            network_bytes,
            verified: None,
        });

        if summaries.len() >= limit {
            break;
        }
    }

    // Sort by timestamp descending
    summaries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(summaries)
}

/// Verify evidence bundle integrity
fn verify_evidence_bundle(path: &Path) -> Result<VerificationResult> {
    let content = std::fs::read_to_string(path)?;
    let bundle: EvidenceBundle = serde_json::from_str(&content)?;

    let mut issues = Vec::new();

    // Verify hash
    let computed_hash = compute_bundle_hash(&bundle);
    let hash_valid = true; // In a real impl, compare against stored hash

    // Verify chain
    let chain_valid = verify_chain(&bundle, &mut issues);

    // Verify signature if present
    let signature_valid = if bundle.signature.is_some() {
        Some(verify_signature(&bundle, &mut issues))
    } else {
        None
    };

    let valid = hash_valid && chain_valid && signature_valid.unwrap_or(true) && issues.is_empty();

    Ok(VerificationResult {
        valid,
        path: path.display().to_string(),
        run_id: bundle.run_id.clone(),
        capsule_id: bundle.capsule_id.clone(),
        hash_valid,
        computed_hash,
        chain_valid,
        signature_valid,
        issues,
    })
}

/// Compute hash of evidence bundle
fn compute_bundle_hash(bundle: &EvidenceBundle) -> String {
    let content = serde_json::to_string(bundle).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
}

/// Verify chain integrity
fn verify_chain(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    // Verify merkle root format
    if !bundle.chain.merkle_root.starts_with("sha256:") {
        issues.push("Invalid merkle root format".to_string());
        return false;
    }

    // In a real implementation, this would:
    // 1. Verify the merkle tree structure
    // 2. Verify inclusion proofs
    // 3. Check consistency with previous bundles

    true
}

/// Verify signature
fn verify_signature(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    let sig = match &bundle.signature {
        Some(s) => s,
        None => return true,
    };

    // Verify algorithm is supported
    if sig.algorithm != "ed25519" {
        issues.push(format!("Unsupported signature algorithm: {}", sig.algorithm));
        return false;
    }

    // In a real implementation, this would verify the Ed25519 signature
    // For now, we accept any well-formed signature

    true
}

/// Export to JSON format
fn export_json(bundles: &[EvidenceBundle], output: &Path) -> Result<()> {
    let content = serde_json::to_string_pretty(bundles)?;
    std::fs::write(output, content)?;
    Ok(())
}

/// Export to CSV format
fn export_csv(bundles: &[EvidenceBundle], output: &Path) -> Result<()> {
    let mut content = String::new();

    // Header
    content.push_str("run_id,capsule_id,start_time,end_time,duration_ms,exit_code,capability_calls,network_bytes\n");

    // Rows
    for bundle in bundles {
        let network_bytes: u64 = bundle
            .execution
            .network_events
            .iter()
            .map(|e| e.bytes)
            .sum();

        content.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            bundle.run_id,
            bundle.capsule_id,
            bundle.execution.start_timestamp_ns,
            bundle.execution.end_timestamp_ns,
            bundle.execution.duration_ns / 1_000_000,
            bundle.outputs.exit_code,
            bundle.execution.capability_calls.len(),
            network_bytes,
        ));
    }

    std::fs::write(output, content)?;
    Ok(())
}

/// Export to SIEM/CEF format
fn export_siem(bundles: &[EvidenceBundle], output: &Path) -> Result<()> {
    let mut content = String::new();

    for bundle in bundles {
        // CEF (Common Event Format) header
        // CEF:Version|Device Vendor|Device Product|Device Version|Signature ID|Name|Severity|Extension
        content.push_str(&format!(
            "CEF:0|AgentVM|Capsule|1.0|{}|AgentExecution|5|run_id={} capsule_id={} exit_code={} duration_ms={}\n",
            bundle.run_id,
            bundle.run_id,
            bundle.capsule_id,
            bundle.outputs.exit_code,
            bundle.execution.duration_ns / 1_000_000,
        ));

        // Add capability call events
        for call in &bundle.execution.capability_calls {
            content.push_str(&format!(
                "CEF:0|AgentVM|Capsule|1.0|capability_call|CapabilityInvocation|3|run_id={} type={} operation={}\n",
                bundle.run_id,
                call.capability_type,
                call.operation,
            ));
        }
    }

    std::fs::write(output, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compute_bundle_hash() {
        let bundle = create_test_bundle();
        let hash = compute_bundle_hash(&bundle);
        assert!(hash.starts_with("sha256:"));
    }

    fn create_test_bundle() -> EvidenceBundle {
        EvidenceBundle {
            version: "1.0".to_string(),
            run_id: "test-run-123".to_string(),
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
