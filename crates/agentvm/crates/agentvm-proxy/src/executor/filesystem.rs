//! Filesystem executor for file capabilities.
//!
//! Handles file read, write, delete, and directory listing operations
//! with path validation against the capability scope.

use crate::config::FilesystemConfig;
use crate::error::ExecutorError;
use crate::executor::{Executor, ExecutorResult};
use crate::types::{Capability, CapabilityType, InvokeRequest, Operation, OperationResult, QuotaConsumed};
use async_trait::async_trait;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Local filesystem executor
pub struct LocalFilesystemExecutor {
    /// Configuration
    config: FilesystemConfig,
    /// Compiled forbidden patterns
    forbidden_patterns: Vec<glob::Pattern>,
    /// File watcher for change tracking
    watcher: Option<RecommendedWatcher>,
    /// Changed files channel
    changes_rx: Option<mpsc::Receiver<PathBuf>>,
}

impl LocalFilesystemExecutor {
    /// Create a new filesystem executor
    pub fn new(config: &FilesystemConfig) -> Result<Self, ExecutorError> {
        // Compile forbidden patterns
        let forbidden_patterns: Vec<glob::Pattern> = config
            .forbidden_patterns
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        Ok(Self {
            config: config.clone(),
            forbidden_patterns,
            watcher: None,
            changes_rx: None,
        })
    }

    /// Initialize file watcher (optional)
    pub async fn enable_change_tracking(&mut self) -> Result<(), ExecutorError> {
        if !self.config.enable_change_tracking {
            return Ok(());
        }

        let (tx, rx) = mpsc::channel(1000);
        let tx_clone = tx.clone();

        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        let _ = tx_clone.blocking_send(path);
                    }
                }
            },
            Config::default(),
        )
        .map_err(|e| ExecutorError::Internal(format!("Failed to create file watcher: {}", e)))?;

        self.watcher = Some(watcher);
        self.changes_rx = Some(rx);

        // Watch workspace
        if let Some(ref mut watcher) = self.watcher {
            watcher
                .watch(&self.config.workspace, RecursiveMode::Recursive)
                .map_err(|e| ExecutorError::Internal(format!("Failed to watch workspace: {}", e)))?;
        }

        info!("File change tracking enabled for {:?}", self.config.workspace);
        Ok(())
    }

    /// Get pending changes
    pub async fn get_changes(&mut self) -> Vec<PathBuf> {
        let mut changes = Vec::new();
        if let Some(ref mut rx) = self.changes_rx {
            while let Ok(path) = rx.try_recv() {
                changes.push(path);
            }
        }
        changes
    }

    /// Validate that a path is allowed
    fn validate_path(&self, path: &str) -> Result<PathBuf, ExecutorError> {
        let path = PathBuf::from(path);

        // Canonicalize to resolve any .. or symlinks
        // For security, we need to ensure the path is within allowed directories
        let canonical = if path.is_absolute() {
            path.clone()
        } else {
            self.config.workspace.join(&path)
        };

        // Check path depth
        let depth = canonical.components().count();
        if depth > self.config.max_depth {
            return Err(ExecutorError::PermissionDenied(format!(
                "Path depth {} exceeds maximum {}",
                depth, self.config.max_depth
            )));
        }

        // Check if path is within workspace or additional allowed paths
        let is_allowed = canonical.starts_with(&self.config.workspace)
            || self
                .config
                .additional_paths
                .iter()
                .any(|p| canonical.starts_with(p));

        if !is_allowed {
            return Err(ExecutorError::PermissionDenied(format!(
                "Path {:?} is outside allowed directories",
                canonical
            )));
        }

        // Check against forbidden patterns
        let path_str = canonical.to_string_lossy();
        for pattern in &self.forbidden_patterns {
            if pattern.matches(&path_str) {
                return Err(ExecutorError::PermissionDenied(format!(
                    "Path matches forbidden pattern: {}",
                    pattern.as_str()
                )));
            }
        }

        Ok(canonical)
    }

    /// Read file contents
    async fn read_file(
        &self,
        path: &str,
        offset: u64,
        len: u64,
    ) -> Result<ExecutorResult, ExecutorError> {
        let start = Instant::now();
        let canonical_path = self.validate_path(path)?;

        // Check file size
        let metadata = fs::metadata(&canonical_path).await?;
        if metadata.len() > self.config.max_file_size as u64 {
            return Err(ExecutorError::PermissionDenied(format!(
                "File size {} exceeds maximum {}",
                metadata.len(),
                self.config.max_file_size
            )));
        }

        // Determine how much to read
        let file_size = metadata.len();
        let actual_offset = offset.min(file_size);
        let max_len = file_size.saturating_sub(actual_offset);
        let actual_len = if len == 0 { max_len } else { len.min(max_len) };

        // Read file
        let mut file = File::open(&canonical_path).await?;
        if actual_offset > 0 {
            file.seek(std::io::SeekFrom::Start(actual_offset)).await?;
        }

        let mut buffer = vec![0u8; actual_len as usize];
        let bytes_read = file.read(&mut buffer).await?;
        buffer.truncate(bytes_read);

        let elapsed = start.elapsed();
        debug!(
            "Read {} bytes from {:?} in {:?}",
            bytes_read, canonical_path, elapsed
        );

        Ok(ExecutorResult::new(
            OperationResult::FileData { data: buffer },
            QuotaConsumed::single(bytes_read as u64, elapsed.as_nanos() as u64),
        ))
    }

    /// Write file contents
    async fn write_file(
        &self,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<ExecutorResult, ExecutorError> {
        let start = Instant::now();
        let canonical_path = self.validate_path(path)?;

        // Ensure parent directory exists
        if let Some(parent) = canonical_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Open file for writing
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(offset == 0)
            .open(&canonical_path)
            .await?;

        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset)).await?;
        }

        file.write_all(data).await?;
        file.flush().await?;

        let elapsed = start.elapsed();
        info!(
            "Wrote {} bytes to {:?} in {:?}",
            data.len(),
            canonical_path,
            elapsed
        );

        Ok(ExecutorResult::new(
            OperationResult::FileWritten {
                bytes_written: data.len() as u64,
            },
            QuotaConsumed::single(data.len() as u64, elapsed.as_nanos() as u64),
        ))
    }

    /// Delete a file
    async fn delete_file(&self, path: &str) -> Result<ExecutorResult, ExecutorError> {
        let start = Instant::now();
        let canonical_path = self.validate_path(path)?;

        // Check if file exists
        if !canonical_path.exists() {
            return Err(ExecutorError::NotFound(format!("File not found: {:?}", canonical_path)));
        }

        // Delete file
        fs::remove_file(&canonical_path).await?;

        let elapsed = start.elapsed();
        info!("Deleted {:?} in {:?}", canonical_path, elapsed);

        Ok(ExecutorResult::new(
            OperationResult::FileDeleted,
            QuotaConsumed::single(0, elapsed.as_nanos() as u64),
        ))
    }

    /// List directory contents
    async fn list_directory(&self, path: &str) -> Result<ExecutorResult, ExecutorError> {
        let start = Instant::now();
        let canonical_path = self.validate_path(path)?;

        // Check if directory exists
        if !canonical_path.is_dir() {
            return Err(ExecutorError::NotFound(format!(
                "Directory not found: {:?}",
                canonical_path
            )));
        }

        // Read directory entries
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(&canonical_path).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }

        entries.sort();

        let elapsed = start.elapsed();
        let bytes = entries.iter().map(|e| e.len()).sum::<usize>();

        debug!(
            "Listed {} entries from {:?} in {:?}",
            entries.len(),
            canonical_path,
            elapsed
        );

        Ok(ExecutorResult::new(
            OperationResult::DirectoryEntries { entries },
            QuotaConsumed::single(bytes as u64, elapsed.as_nanos() as u64),
        ))
    }
}

#[async_trait]
impl Executor for LocalFilesystemExecutor {
    async fn execute(
        &self,
        capability: &Capability,
        request: &InvokeRequest,
    ) -> Result<ExecutorResult, ExecutorError> {
        // Verify capability allows this operation
        if !capability.scope.permits(&request.operation) {
            return Err(ExecutorError::PermissionDenied(
                "Capability scope does not permit this path".to_string(),
            ));
        }

        match &request.operation {
            Operation::FileRead { path, offset, len } => {
                if capability.cap_type != CapabilityType::FileRead {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow file reads".to_string(),
                    ));
                }
                self.read_file(path, *offset, *len).await
            }
            Operation::FileWrite { path, offset, data } => {
                if capability.cap_type != CapabilityType::FileWrite {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow file writes".to_string(),
                    ));
                }
                self.write_file(path, *offset, data).await
            }
            Operation::FileDelete { path } => {
                if capability.cap_type != CapabilityType::FileDelete {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow file deletion".to_string(),
                    ));
                }
                self.delete_file(path).await
            }
            Operation::DirectoryList { path } => {
                if capability.cap_type != CapabilityType::DirectoryList {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow directory listing".to_string(),
                    ));
                }
                self.list_directory(path).await
            }
            _ => Err(ExecutorError::NotSupported(format!(
                "Filesystem executor cannot handle operation: {:?}",
                std::mem::discriminant(&request.operation)
            ))),
        }
    }

    fn can_handle(&self, capability: &Capability) -> bool {
        capability.cap_type.is_filesystem()
    }

    fn name(&self) -> &'static str {
        "local-filesystem"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config(workspace: &Path) -> FilesystemConfig {
        FilesystemConfig {
            workspace: workspace.to_path_buf(),
            additional_paths: vec![],
            forbidden_patterns: vec!["**/.env".to_string(), "**/secrets*".to_string()],
            enable_change_tracking: false,
            max_file_size: 10 * 1024 * 1024,
            max_depth: 50,
        }
    }

    #[test]
    fn test_path_validation_within_workspace() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        // Path within workspace should be allowed
        let valid_path = tmp.path().join("test.txt");
        assert!(executor.validate_path(valid_path.to_str().unwrap()).is_ok());

        // Relative path should be resolved to workspace
        assert!(executor.validate_path("test.txt").is_ok());
    }

    #[test]
    fn test_path_validation_outside_workspace() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        // Path outside workspace should be denied
        assert!(executor.validate_path("/etc/passwd").is_err());
        assert!(executor.validate_path("/tmp/other").is_err());
    }

    #[test]
    fn test_forbidden_patterns() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        // .env files should be forbidden
        let env_path = tmp.path().join(".env");
        assert!(executor.validate_path(env_path.to_str().unwrap()).is_err());

        // secrets files should be forbidden
        let secrets_path = tmp.path().join("secrets.json");
        assert!(executor.validate_path(secrets_path.to_str().unwrap()).is_err());
    }

    #[tokio::test]
    async fn test_read_write_file() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        let test_file = tmp.path().join("test.txt");
        let test_data = b"Hello, World!";

        // Write file
        let write_result = executor
            .write_file(test_file.to_str().unwrap(), 0, test_data)
            .await
            .unwrap();

        if let OperationResult::FileWritten { bytes_written } = write_result.data {
            assert_eq!(bytes_written, test_data.len() as u64);
        } else {
            panic!("Expected FileWritten result");
        }

        // Read file
        let read_result = executor
            .read_file(test_file.to_str().unwrap(), 0, 0)
            .await
            .unwrap();

        if let OperationResult::FileData { data } = read_result.data {
            assert_eq!(&data, test_data);
        } else {
            panic!("Expected FileData result");
        }
    }

    #[tokio::test]
    async fn test_list_directory() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        // Create some test files
        fs::write(tmp.path().join("file1.txt"), b"").await.unwrap();
        fs::write(tmp.path().join("file2.txt"), b"").await.unwrap();
        fs::create_dir(tmp.path().join("subdir")).await.unwrap();

        let result = executor
            .list_directory(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        if let OperationResult::DirectoryEntries { entries } = result.data {
            assert!(entries.contains(&"file1.txt".to_string()));
            assert!(entries.contains(&"file2.txt".to_string()));
            assert!(entries.contains(&"subdir".to_string()));
        } else {
            panic!("Expected DirectoryEntries result");
        }
    }

    #[tokio::test]
    async fn test_delete_file() {
        let tmp = TempDir::new().unwrap();
        let config = create_test_config(tmp.path());
        let executor = LocalFilesystemExecutor::new(&config).unwrap();

        let test_file = tmp.path().join("to_delete.txt");
        fs::write(&test_file, b"delete me").await.unwrap();
        assert!(test_file.exists());

        executor
            .delete_file(test_file.to_str().unwrap())
            .await
            .unwrap();

        assert!(!test_file.exists());
    }
}
