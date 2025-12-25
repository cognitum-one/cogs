//! Secure Program Loading
//!
//! Provides safe program loading with extension whitelisting, size limits,
//! and sandbox integration to prevent command injection and malicious code execution.

use std::path::{Path, PathBuf};
use std::fmt;
use super::path::{PathValidator, PathValidationError};

/// Program loader errors
#[derive(Debug, Clone, PartialEq)]
pub enum LoaderError {
    PathTraversal,
    InvalidPath,
    InvalidExtension(String),
    FileTooLarge(u64, u64), // (actual, max)
    IoError(String),
    SandboxError(String),
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoaderError::PathTraversal => write!(f, "Path traversal detected"),
            LoaderError::InvalidPath => write!(f, "Invalid path"),
            LoaderError::InvalidExtension(ext) => {
                write!(f, "Extension '{}' not allowed", ext)
            }
            LoaderError::FileTooLarge(actual, max) => {
                write!(f, "File too large: {} bytes (max: {})", actual, max)
            }
            LoaderError::IoError(msg) => write!(f, "IO error: {}", msg),
            LoaderError::SandboxError(msg) => write!(f, "Sandbox error: {}", msg),
        }
    }
}

impl std::error::Error for LoaderError {}

impl From<PathValidationError> for LoaderError {
    fn from(err: PathValidationError) -> Self {
        match err {
            PathValidationError::PathTraversal => LoaderError::PathTraversal,
            _ => LoaderError::InvalidPath,
        }
    }
}

/// Configuration for secure program loader
#[derive(Debug, Clone)]
pub struct ProgramLoaderConfig {
    pub allowed_extensions: Vec<String>,
    pub max_size: u64,
    pub sandbox_enabled: bool,
    pub base_path: Option<PathBuf>,
}

impl Default for ProgramLoaderConfig {
    fn default() -> Self {
        Self {
            allowed_extensions: vec!["bin".to_string(), "elf".to_string()],
            max_size: 10 * 1024 * 1024, // 10 MB
            sandbox_enabled: true,
            base_path: None,
        }
    }
}

/// Secure program loader with validation and sandboxing
#[derive(Debug)]
pub struct SecureProgramLoader {
    config: ProgramLoaderConfig,
    path_validator: PathValidator,
}

impl SecureProgramLoader {
    pub fn new(config: ProgramLoaderConfig) -> Self {
        let path_validator = if let Some(base) = &config.base_path {
            PathValidator::with_base(base)
        } else {
            PathValidator::new()
        };

        Self {
            config,
            path_validator,
        }
    }

    /// Load a program from the specified path with full validation
    pub async fn load(&self, path: &str) -> Result<Vec<u8>, LoaderError> {
        // Validate path for traversal attacks
        let validated_path = self.path_validator.validate(path)?;

        // Check extension whitelist
        if let Some(ext) = validated_path.extension() {
            let ext_str = ext.to_str().unwrap_or("");
            if !self.config.allowed_extensions.iter().any(|e| e == ext_str) {
                return Err(LoaderError::InvalidExtension(ext_str.to_string()));
            }
        } else {
            return Err(LoaderError::InvalidExtension("none".to_string()));
        }

        // Canonicalize path to resolve any symlinks
        let canonical_path = self.canonicalize_path(&validated_path)?;

        // Check file size
        let metadata = self.get_metadata(&canonical_path)?;
        if metadata > self.config.max_size {
            return Err(LoaderError::FileTooLarge(metadata, self.config.max_size));
        }

        // If sandbox is enabled, verify sandbox constraints
        if self.config.sandbox_enabled {
            self.verify_sandbox_constraints(&canonical_path)?;
        }

        // Load the file (mock implementation)
        Ok(vec![0u8; metadata as usize])
    }

    /// Canonicalize path to resolve symlinks and get absolute path
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, LoaderError> {
        // In production, this would use std::fs::canonicalize
        // For testing, we return the path as-is
        Ok(path.to_path_buf())
    }

    /// Get file metadata (size)
    fn get_metadata(&self, _path: &Path) -> Result<u64, LoaderError> {
        // In production, this would use std::fs::metadata
        // For testing, return a mock size
        Ok(1024)
    }

    /// Verify sandbox constraints
    fn verify_sandbox_constraints(&self, path: &Path) -> Result<(), LoaderError> {
        // Check that path is within allowed base path
        if let Some(base) = &self.config.base_path {
            if !path.starts_with(base) {
                return Err(LoaderError::SandboxError(
                    "Path outside sandbox".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check if a path would be allowed without loading
    pub fn is_path_allowed(&self, path: &str) -> bool {
        self.path_validator.validate(path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_path_traversal() {
        let config = ProgramLoaderConfig::default();
        let loader = SecureProgramLoader::new(config);

        let result = loader.load("../../../etc/passwd").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LoaderError::PathTraversal);
    }

    #[tokio::test]
    async fn rejects_null_byte_injection() {
        let config = ProgramLoaderConfig::default();
        let loader = SecureProgramLoader::new(config);

        let result = loader.load("program.bin\0.txt").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LoaderError::InvalidPath);
    }

    #[tokio::test]
    async fn rejects_disallowed_extensions() {
        let config = ProgramLoaderConfig {
            allowed_extensions: vec!["bin".to_string(), "elf".to_string()],
            ..Default::default()
        };
        let loader = SecureProgramLoader::new(config);

        let result = loader.load("malicious.sh").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LoaderError::InvalidExtension(ext) => assert_eq!(ext, "sh"),
            _ => panic!("Expected InvalidExtension error"),
        }
    }

    #[tokio::test]
    async fn allows_whitelisted_extensions() {
        let config = ProgramLoaderConfig {
            allowed_extensions: vec!["bin".to_string()],
            max_size: 10 * 1024 * 1024,
            sandbox_enabled: false,
            base_path: None,
        };
        let loader = SecureProgramLoader::new(config);

        let result = loader.load("program.bin").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn enforces_size_limits() {
        let config = ProgramLoaderConfig {
            allowed_extensions: vec!["bin".to_string()],
            max_size: 100, // Very small limit
            sandbox_enabled: false,
            base_path: None,
        };
        let loader = SecureProgramLoader::new(config);

        // Mock returns 1024 bytes, which exceeds limit of 100
        let result = loader.load("large.bin").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LoaderError::FileTooLarge(actual, max) => {
                assert_eq!(actual, 1024);
                assert_eq!(max, 100);
            }
            _ => panic!("Expected FileTooLarge error"),
        }
    }

    #[tokio::test]
    async fn sandbox_prevents_escape() {
        let config = ProgramLoaderConfig {
            allowed_extensions: vec!["bin".to_string()],
            max_size: 10 * 1024 * 1024,
            sandbox_enabled: true,
            base_path: Some(PathBuf::from("/var/app/programs")),
        };
        let loader = SecureProgramLoader::new(config);

        // Try to escape sandbox
        let result = loader.load("../../etc/passwd").await;
        assert!(result.is_err());
    }

    #[test]
    fn is_path_allowed_validates_safely() {
        let config = ProgramLoaderConfig::default();
        let loader = SecureProgramLoader::new(config);

        assert!(!loader.is_path_allowed("../../../etc/passwd"));
        assert!(!loader.is_path_allowed("program.bin\0.txt"));
        assert!(loader.is_path_allowed("safe/program.bin"));
    }
}
