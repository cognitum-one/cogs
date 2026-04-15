//! Snapshot command implementation
//!
//! Manages capsule snapshots for state preservation and restoration.
//!
//! Usage:
//!   agentvm snapshot create --name <name>
//!   agentvm snapshot list
//!   agentvm snapshot delete <id>

use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{format_bytes, format_hash, OutputFormat, OutputWriter, ProgressManager, TableDisplay};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Snapshot command action
#[derive(Debug, Clone)]
pub enum SnapshotAction {
    /// Create a new snapshot
    Create {
        /// Human-readable name for the snapshot
        name: Option<String>,
        /// Capsule ID to snapshot (uses current if not specified)
        capsule: Option<String>,
        /// Description
        description: Option<String>,
        /// Include memory snapshot
        include_memory: bool,
    },
    /// List all snapshots
    List {
        /// Filter by capsule ID
        capsule: Option<String>,
        /// Maximum number of results
        limit: Option<usize>,
    },
    /// Delete a snapshot
    Delete {
        /// Snapshot ID to delete
        id: String,
        /// Force deletion without confirmation
        force: bool,
    },
    /// Show snapshot details
    Show {
        /// Snapshot ID
        id: String,
    },
}

/// Snapshot command arguments
#[derive(Debug, Clone)]
pub struct SnapshotArgs {
    /// The action to perform
    pub action: SnapshotAction,
    /// Output format
    pub output_format: OutputFormat,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// Unique snapshot identifier
    pub id: String,
    /// Human-readable name
    #[serde(skip_serializing_if = "Option::is_none")]
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
    /// Number of files in snapshot
    pub file_count: u64,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parent snapshot ID (for incremental snapshots)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
}

impl TableDisplay for SnapshotInfo {
    fn table_headers() -> Vec<String> {
        vec![
            "ID".to_string(),
            "Name".to_string(),
            "Capsule".to_string(),
            "Type".to_string(),
            "Size".to_string(),
            "Created".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            format_hash(&self.id),
            self.name.clone().unwrap_or_else(|| "-".to_string()),
            format_hash(&self.capsule_id),
            self.snapshot_type.clone(),
            format_bytes(self.size_bytes),
            self.created_at.format("%Y-%m-%d %H:%M").to_string(),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.kv("ID", &self.id);
        if let Some(name) = &self.name {
            writer.kv("Name", name);
        }
        writer.kv("Capsule", &self.capsule_id);
        writer.kv("Type", &self.snapshot_type);
        writer.kv("Size", format_bytes(self.size_bytes));
        writer.kv("Files", self.file_count);
        writer.kv("Created", self.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
        writer.kv("State Hash", format_hash(&self.state_hash));
        if let Some(desc) = &self.description {
            writer.kv("Description", desc);
        }
        if let Some(parent) = &self.parent_id {
            writer.kv("Parent", format_hash(parent));
        }
        if !self.tags.is_empty() {
            writer.kv("Tags", self.tags.join(", "));
        }
    }
}

/// Handle snapshot commands
pub async fn handle_snapshot(args: SnapshotArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);

    match args.action {
        SnapshotAction::Create {
            name,
            capsule,
            description,
            include_memory,
        } => {
            handle_create(&writer, config, name, capsule, description, include_memory).await
        }
        SnapshotAction::List { capsule, limit } => {
            handle_list(&writer, config, capsule, limit).await
        }
        SnapshotAction::Delete { id, force } => {
            handle_delete(&writer, config, &id, force).await
        }
        SnapshotAction::Show { id } => {
            handle_show(&writer, config, &id).await
        }
    }
}

/// Handle snapshot create
async fn handle_create(
    writer: &OutputWriter,
    config: &Config,
    name: Option<String>,
    capsule: Option<String>,
    description: Option<String>,
    include_memory: bool,
) -> Result<()> {
    let progress = ProgressManager::new();

    // Generate snapshot ID
    let snapshot_id = Uuid::now_v7().to_string();

    // Determine capsule ID (use provided or default)
    let capsule_id = capsule.unwrap_or_else(|| "default".to_string());

    let snapshot_type = if include_memory { "full" } else { "disk" };

    writer.info(&format!(
        "Creating {} snapshot for capsule '{}'...",
        snapshot_type, capsule_id
    ));

    let start_time = std::time::Instant::now();

    // Create snapshot directory
    let spinner = progress.spinner("Preparing snapshot directory...");
    let snapshot_dir = config.snapshot.storage_dir.join(&snapshot_id);
    std::fs::create_dir_all(&snapshot_dir)?;
    spinner.finish_with_message("Snapshot directory ready");

    // In a real implementation, this would:
    // 1. Pause the capsule
    // 2. Create disk snapshot (QCOW2 CoW or similar)
    // 3. Optionally capture memory state
    // 4. Resume the capsule

    // Simulate capturing state
    let spinner = progress.spinner("Capturing capsule state...");
    let (size_bytes, file_count, state_hash) = capture_capsule_state(&capsule_id, &snapshot_dir)?;
    spinner.finish_with_message("State captured");

    let snapshot = SnapshotInfo {
        id: snapshot_id.clone(),
        name,
        capsule_id,
        created_at: chrono::Utc::now(),
        snapshot_type: snapshot_type.to_string(),
        size_bytes,
        state_hash,
        file_count,
        description,
        parent_id: None,
        tags: vec![],
    };

    // Save metadata
    let spinner = progress.spinner("Saving metadata...");
    save_snapshot_metadata(&snapshot, &config.snapshot.storage_dir)?;
    spinner.finish_with_message("Metadata saved");

    let duration = start_time.elapsed();

    writer.success(&format!(
        "Snapshot '{}' created in {:.2}s",
        snapshot_id,
        duration.as_secs_f64()
    ));
    writer.output(&snapshot)?;

    Ok(())
}

/// Handle snapshot list
async fn handle_list(
    writer: &OutputWriter,
    config: &Config,
    capsule_filter: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let snapshots = list_snapshots(&config.snapshot.storage_dir, capsule_filter.as_deref())?;

    let snapshots: Vec<_> = if let Some(limit) = limit {
        snapshots.into_iter().take(limit).collect()
    } else {
        snapshots
    };

    if snapshots.is_empty() {
        writer.info("No snapshots found");
        return Ok(());
    }

    writer.info(&format!("Found {} snapshot(s)", snapshots.len()));
    writer.output_list(&snapshots)?;

    Ok(())
}

/// Handle snapshot delete
async fn handle_delete(
    writer: &OutputWriter,
    config: &Config,
    id: &str,
    force: bool,
) -> Result<()> {
    // Load snapshot to verify it exists
    let snapshot = load_snapshot_info(id, &config.snapshot.storage_dir)?;

    writer.header("Snapshot to Delete");
    snapshot.print_text(writer);

    if !force {
        writer.warning("This will permanently delete the snapshot.");
        if !crate::output::confirm("Proceed with deletion?", false) {
            writer.info("Deletion cancelled");
            return Ok(());
        }
    }

    let progress = ProgressManager::new();
    let spinner = progress.spinner("Deleting snapshot...");

    // Delete snapshot directory
    let snapshot_dir = config.snapshot.storage_dir.join(&snapshot.id);
    if snapshot_dir.exists() {
        std::fs::remove_dir_all(&snapshot_dir)?;
    }

    // Delete metadata file
    let metadata_path = config.snapshot.storage_dir.join(format!("{}.meta.json", &snapshot.id));
    if metadata_path.exists() {
        std::fs::remove_file(&metadata_path)?;
    }

    spinner.finish_with_message("Snapshot deleted");
    writer.success(&format!("Snapshot '{}' deleted", snapshot.id));

    Ok(())
}

/// Handle snapshot show
async fn handle_show(
    writer: &OutputWriter,
    config: &Config,
    id: &str,
) -> Result<()> {
    let snapshot = load_snapshot_info(id, &config.snapshot.storage_dir)?;

    writer.header("Snapshot Details");
    writer.output(&snapshot)?;

    // Show additional details
    let snapshot_dir = config.snapshot.storage_dir.join(&snapshot.id);
    if snapshot_dir.exists() {
        writer.separator();
        writer.kv("Location", snapshot_dir.display());

        // List files in snapshot
        let mut files: Vec<_> = std::fs::read_dir(&snapshot_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files.sort();

        if !files.is_empty() {
            writer.header("Contents");
            for file in files.iter().take(10) {
                writer.kv("  ", file);
            }
            if files.len() > 10 {
                writer.kv("  ", format!("... and {} more files", files.len() - 10));
            }
        }
    }

    Ok(())
}

/// Capture capsule state (simulated)
fn capture_capsule_state(
    capsule_id: &str,
    snapshot_dir: &Path,
) -> Result<(u64, u64, String)> {
    // In a real implementation, this would capture actual VM state
    // For now, we create a placeholder

    let mut hasher = Sha256::new();
    hasher.update(capsule_id.as_bytes());
    hasher.update(&chrono::Utc::now().timestamp().to_le_bytes());

    // Create a state file
    let state_content = serde_json::json!({
        "capsule_id": capsule_id,
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "type": "placeholder"
    });

    let state_path = snapshot_dir.join("state.json");
    std::fs::write(&state_path, serde_json::to_string_pretty(&state_content)?)?;

    let size_bytes = std::fs::metadata(&state_path)?.len();
    let state_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

    Ok((size_bytes, 1, state_hash))
}

/// Save snapshot metadata
fn save_snapshot_metadata(snapshot: &SnapshotInfo, storage_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(storage_dir)?;

    let metadata_path = storage_dir.join(format!("{}.meta.json", &snapshot.id));
    let content = serde_json::to_string_pretty(snapshot)?;
    std::fs::write(&metadata_path, content)?;

    Ok(())
}

/// Load snapshot info
fn load_snapshot_info(id: &str, storage_dir: &Path) -> Result<SnapshotInfo> {
    let metadata_path = storage_dir.join(format!("{}.meta.json", id));

    if !metadata_path.exists() {
        // Try finding by prefix
        if let Some(full_id) = find_by_prefix(id, storage_dir)? {
            let metadata_path = storage_dir.join(format!("{}.meta.json", full_id));
            let content = std::fs::read_to_string(&metadata_path)?;
            return Ok(serde_json::from_str(&content)?);
        }

        return Err(CliError::SnapshotNotFound { id: id.to_string() });
    }

    let content = std::fs::read_to_string(&metadata_path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Find snapshot by ID prefix
fn find_by_prefix(prefix: &str, storage_dir: &Path) -> Result<Option<String>> {
    if !storage_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(storage_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with(prefix) && name.ends_with(".meta.json") {
            return Ok(Some(name.trim_end_matches(".meta.json").to_string()));
        }
    }

    Ok(None)
}

/// List all snapshots
fn list_snapshots(storage_dir: &Path, capsule_filter: Option<&str>) -> Result<Vec<SnapshotInfo>> {
    if !storage_dir.exists() {
        return Ok(vec![]);
    }

    let mut snapshots = Vec::new();

    for entry in std::fs::read_dir(storage_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.ends_with(".meta.json") {
            let content = std::fs::read_to_string(entry.path())?;
            if let Ok(snapshot) = serde_json::from_str::<SnapshotInfo>(&content) {
                // Apply capsule filter if specified
                if let Some(filter) = capsule_filter {
                    if !snapshot.capsule_id.contains(filter) {
                        continue;
                    }
                }
                snapshots.push(snapshot);
            }
        }
    }

    // Sort by creation time (newest first)
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load_snapshot() {
        let dir = tempdir().unwrap();
        let snapshot = SnapshotInfo {
            id: "test-snapshot-123".to_string(),
            name: Some("Test Snapshot".to_string()),
            capsule_id: "test-capsule".to_string(),
            created_at: chrono::Utc::now(),
            snapshot_type: "disk".to_string(),
            size_bytes: 1024,
            state_hash: "sha256:abc123".to_string(),
            file_count: 10,
            description: Some("Test description".to_string()),
            parent_id: None,
            tags: vec!["test".to_string()],
        };

        save_snapshot_metadata(&snapshot, dir.path()).unwrap();
        let loaded = load_snapshot_info(&snapshot.id, dir.path()).unwrap();

        assert_eq!(loaded.id, snapshot.id);
        assert_eq!(loaded.name, snapshot.name);
        assert_eq!(loaded.capsule_id, snapshot.capsule_id);
    }

    #[test]
    fn test_list_snapshots() {
        let dir = tempdir().unwrap();

        for i in 0..5 {
            let snapshot = SnapshotInfo {
                id: format!("snapshot-{}", i),
                name: Some(format!("Snapshot {}", i)),
                capsule_id: if i % 2 == 0 {
                    "capsule-a".to_string()
                } else {
                    "capsule-b".to_string()
                },
                created_at: chrono::Utc::now(),
                snapshot_type: "disk".to_string(),
                size_bytes: 1024,
                state_hash: format!("sha256:hash{}", i),
                file_count: 10,
                description: None,
                parent_id: None,
                tags: vec![],
            };
            save_snapshot_metadata(&snapshot, dir.path()).unwrap();
        }

        let all = list_snapshots(dir.path(), None).unwrap();
        assert_eq!(all.len(), 5);

        let filtered = list_snapshots(dir.path(), Some("capsule-a")).unwrap();
        assert_eq!(filtered.len(), 3);
    }
}
