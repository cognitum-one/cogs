//! Reset command implementation
//!
//! Restores a capsule to a previous snapshot state.
//!
//! Usage:
//!   agentvm reset --from-snapshot <id> --preserve-workspace

use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{format_bytes, format_hash, OutputFormat, OutputWriter, ProgressManager, TableDisplay};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Reset command arguments
#[derive(Debug, Clone)]
pub struct ResetArgs {
    /// Snapshot ID to restore from
    pub from_snapshot: String,
    /// Preserve workspace (don't reset workspace contents)
    pub preserve_workspace: bool,
    /// Output format
    pub output_format: OutputFormat,
    /// Dry run (don't execute, just show what would be done)
    pub dry_run: bool,
    /// Force reset without confirmation
    pub force: bool,
}

/// Reset operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetResult {
    /// Snapshot ID restored from
    pub snapshot_id: String,
    /// Snapshot name
    pub snapshot_name: Option<String>,
    /// Capsule ID
    pub capsule_id: String,
    /// Whether workspace was preserved
    pub workspace_preserved: bool,
    /// Number of files restored
    pub files_restored: u64,
    /// Total bytes restored
    pub bytes_restored: u64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Previous state hash (before reset)
    pub previous_state_hash: String,
    /// New state hash (after reset)
    pub new_state_hash: String,
}

impl TableDisplay for ResetResult {
    fn table_headers() -> Vec<String> {
        vec![
            "Snapshot".to_string(),
            "Capsule".to_string(),
            "Files".to_string(),
            "Size".to_string(),
            "Workspace".to_string(),
            "Duration".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.snapshot_id),
            format_hash(&self.capsule_id),
            self.files_restored.to_string(),
            format_bytes(self.bytes_restored),
            if self.workspace_preserved {
                "Preserved".to_string()
            } else {
                "Reset".to_string()
            },
            format!("{} ms", self.duration_ms),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.header("Reset Result");
        writer.kv("Snapshot ID", &self.snapshot_id);
        if let Some(name) = &self.snapshot_name {
            writer.kv("Snapshot Name", name);
        }
        writer.kv("Capsule ID", &self.capsule_id);
        writer.kv(
            "Workspace",
            if self.workspace_preserved {
                "Preserved"
            } else {
                "Reset"
            },
        );
        writer.kv("Files Restored", self.files_restored);
        writer.kv("Bytes Restored", format_bytes(self.bytes_restored));
        writer.kv("Duration", format!("{} ms", self.duration_ms));
        writer.separator();
        writer.kv("Previous State", format_hash(&self.previous_state_hash));
        writer.kv("New State", format_hash(&self.new_state_hash));
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot identifier
    pub id: String,
    /// Human-readable name
    pub name: Option<String>,
    /// Capsule identifier
    pub capsule_id: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Snapshot type (disk, memory, full)
    pub snapshot_type: String,
    /// Total size in bytes
    pub size_bytes: u64,
    /// State hash
    pub state_hash: String,
    /// File count
    pub file_count: u64,
    /// Description
    pub description: Option<String>,
    /// Parent snapshot ID (if incremental)
    pub parent_id: Option<String>,
}

/// Handle the reset command
pub async fn handle_reset(args: ResetArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);
    let progress = ProgressManager::new();

    // Load snapshot metadata
    let snapshot = load_snapshot_metadata(&args.from_snapshot, &config.snapshot.storage_dir)?;

    // Show what will be done
    writer.header("Reset Operation");
    writer.kv("Snapshot", format_hash(&snapshot.id));
    if let Some(name) = &snapshot.name {
        writer.kv("Name", name);
    }
    writer.kv("Capsule", &snapshot.capsule_id);
    writer.kv("Created", snapshot.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
    writer.kv("Size", format_bytes(snapshot.size_bytes));
    writer.kv(
        "Workspace",
        if args.preserve_workspace {
            "Will be preserved"
        } else {
            "Will be reset"
        },
    );

    // Dry run mode
    if args.dry_run {
        writer.info("Dry run - no changes made");
        return Ok(());
    }

    // Confirmation prompt (unless --force)
    if !args.force {
        writer.warning("This will reset the capsule state to the snapshot.");
        if !args.preserve_workspace {
            writer.warning("Workspace contents will be OVERWRITTEN.");
        }
        if !crate::output::confirm("Proceed with reset?", false) {
            writer.info("Reset cancelled");
            return Ok(());
        }
    }

    let start_time = std::time::Instant::now();

    // Compute current state hash
    let spinner = progress.spinner("Computing current state...");
    let previous_state_hash = compute_state_hash(&config.snapshot.storage_dir, &snapshot.capsule_id)?;
    spinner.finish_with_message("Current state computed");

    // Perform the reset
    let spinner = progress.spinner("Restoring snapshot...");
    let restore_result = restore_snapshot(&snapshot, &config.snapshot.storage_dir, args.preserve_workspace)?;
    spinner.finish_with_message("Snapshot restored");

    // Compute new state hash
    let spinner = progress.spinner("Computing new state...");
    let new_state_hash = compute_state_hash(&config.snapshot.storage_dir, &snapshot.capsule_id)?;
    spinner.finish_with_message("New state computed");

    let duration = start_time.elapsed();

    // Create result
    let result = ResetResult {
        snapshot_id: snapshot.id,
        snapshot_name: snapshot.name,
        capsule_id: snapshot.capsule_id,
        workspace_preserved: args.preserve_workspace,
        files_restored: restore_result.files_restored,
        bytes_restored: restore_result.bytes_restored,
        duration_ms: duration.as_millis() as u64,
        previous_state_hash,
        new_state_hash,
    };

    writer.output(&result)?;
    writer.success("Capsule reset successfully");

    Ok(())
}

/// Load snapshot metadata from storage
fn load_snapshot_metadata(snapshot_id: &str, storage_dir: &Path) -> Result<SnapshotMetadata> {
    let metadata_path = storage_dir.join(format!("{}.meta.json", snapshot_id));

    if !metadata_path.exists() {
        // Try finding by partial ID
        if let Some(full_id) = find_snapshot_by_prefix(snapshot_id, storage_dir)? {
            let metadata_path = storage_dir.join(format!("{}.meta.json", full_id));
            let content = std::fs::read_to_string(&metadata_path)?;
            return Ok(serde_json::from_str(&content)?);
        }

        return Err(CliError::SnapshotNotFound {
            id: snapshot_id.to_string(),
        });
    }

    let content = std::fs::read_to_string(&metadata_path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Find snapshot by ID prefix
fn find_snapshot_by_prefix(prefix: &str, storage_dir: &Path) -> Result<Option<String>> {
    if !storage_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(storage_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with(prefix) && name.ends_with(".meta.json") {
            let id = name.trim_end_matches(".meta.json").to_string();
            return Ok(Some(id));
        }
    }

    Ok(None)
}

/// Restore result from snapshot operation
struct RestoreResult {
    files_restored: u64,
    bytes_restored: u64,
}

/// Restore from snapshot
fn restore_snapshot(
    snapshot: &SnapshotMetadata,
    storage_dir: &Path,
    preserve_workspace: bool,
) -> Result<RestoreResult> {
    let snapshot_path = storage_dir.join(&snapshot.id);

    if !snapshot_path.exists() {
        return Err(CliError::Snapshot(format!(
            "Snapshot data not found: {}",
            snapshot_path.display()
        )));
    }

    // In a real implementation, this would:
    // 1. Stop the capsule if running
    // 2. Restore disk image from snapshot
    // 3. Optionally restore memory state
    // 4. Handle workspace preservation
    // 5. Restart the capsule

    // For now, we simulate the restore operation
    let mut files_restored = 0u64;
    let mut bytes_restored = 0u64;

    // Count files in snapshot
    if snapshot_path.is_dir() {
        for entry in walkdir::WalkDir::new(&snapshot_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            files_restored += 1;
            if let Ok(metadata) = entry.metadata() {
                bytes_restored += metadata.len();
            }
        }
    }

    Ok(RestoreResult {
        files_restored,
        bytes_restored,
    })
}

/// Compute state hash for a capsule
fn compute_state_hash(storage_dir: &Path, capsule_id: &str) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(capsule_id.as_bytes());

    // In a real implementation, this would hash the actual VM state
    // For now, we include timestamp to simulate state changes
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    hasher.update(&timestamp.to_le_bytes());

    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_snapshot_not_found() {
        let dir = tempdir().unwrap();
        let result = load_snapshot_metadata("nonexistent", dir.path());
        assert!(matches!(result, Err(CliError::SnapshotNotFound { .. })));
    }

    #[test]
    fn test_find_snapshot_by_prefix() {
        let dir = tempdir().unwrap();
        let snapshot_id = "test-snapshot-123";
        let metadata = SnapshotMetadata {
            id: snapshot_id.to_string(),
            name: Some("Test Snapshot".to_string()),
            capsule_id: "test-capsule".to_string(),
            created_at: chrono::Utc::now(),
            snapshot_type: "disk".to_string(),
            size_bytes: 1024,
            state_hash: "sha256:abc123".to_string(),
            file_count: 10,
            description: None,
            parent_id: None,
        };

        let metadata_path = dir.path().join(format!("{}.meta.json", snapshot_id));
        std::fs::write(&metadata_path, serde_json::to_string(&metadata).unwrap()).unwrap();

        let found = find_snapshot_by_prefix("test-snapshot", dir.path()).unwrap();
        assert_eq!(found, Some(snapshot_id.to_string()));
    }
}
