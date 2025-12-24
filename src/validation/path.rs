//! Path Traversal Prevention
//!
//! Validates file paths to prevent directory traversal attacks and ensure
//! paths stay within authorized boundaries.

use std::path::{Path, PathBuf};
use std::fmt;

/// Path validation errors
#[derive(Debug, Clone, PartialEq)]
pub enum PathValidationError {
    PathTraversal,
    InvalidPath,
    NullByte,
    AbsolutePathNotAllowed,
    OutsideBasePath,
}

impl fmt::Display for PathValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathValidationError::PathTraversal => {
                write!(f, "Path traversal attempt detected")
            }
            PathValidationError::InvalidPath => {
                write!(f, "Invalid path format")
            }
            PathValidationError::NullByte => {
                write!(f, "Null byte in path")
            }
            PathValidationError::AbsolutePathNotAllowed => {
                write!(f, "Absolute paths not allowed")
            }
            PathValidationError::OutsideBasePath => {
                write!(f, "Path escapes base directory")
            }
        }
    }
}

impl std::error::Error for PathValidationError {}

/// Path validator for preventing traversal attacks
#[derive(Debug)]
pub struct PathValidator {
    base_path: Option<PathBuf>,
    allow_absolute: bool,
}

impl PathValidator {
    pub fn new() -> Self {
        Self {
            base_path: None,
            allow_absolute: false,
        }
    }

    pub fn with_base<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: Some(base.as_ref().to_path_buf()),
            allow_absolute: false,
        }
    }

    /// Validate a path string and return canonicalized path if safe
    pub fn validate(&self, path: &str) -> Result<PathBuf, PathValidationError> {
        // Check for null bytes (null byte injection attack)
        if path.contains('\0') {
            return Err(PathValidationError::NullByte);
        }

        // Check for obvious traversal patterns
        if path.contains("..") {
            return Err(PathValidationError::PathTraversal);
        }

        // Check for path traversal with URL encoding
        if path.contains("%2e%2e") || path.contains("%2E%2E") {
            return Err(PathValidationError::PathTraversal);
        }

        // Check for backslash traversal on Windows
        if cfg!(windows) && path.contains("..\\") {
            return Err(PathValidationError::PathTraversal);
        }

        let path_buf = PathBuf::from(path);

        // Check if absolute path when not allowed
        if !self.allow_absolute && path_buf.is_absolute() {
            return Err(PathValidationError::AbsolutePathNotAllowed);
        }

        // If base path is set, ensure the path doesn't escape it
        if let Some(base) = &self.base_path {
            let full_path = base.join(&path_buf);

            // Canonicalize to resolve any remaining .. or symlinks
            // Note: This would fail if path doesn't exist, so we manually check
            let normalized = self.normalize_path(&full_path);

            if !normalized.starts_with(base) {
                return Err(PathValidationError::OutsideBasePath);
            }

            return Ok(normalized);
        }

        Ok(path_buf)
    }

    /// Normalize path by resolving .. and . components
    fn normalize_path(&self, path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::CurDir => {
                    // Skip
                }
                _ => {
                    normalized.push(component);
                }
            }
        }

        normalized
    }

    /// Check if path contains traversal attempts
    pub fn contains_traversal(path: &str) -> bool {
        path.contains("..")
            || path.contains("%2e%2e")
            || path.contains("%2E%2E")
            || (cfg!(windows) && path.contains("..\\"))
    }
}

impl Default for PathValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_path_traversal_with_dotdot() {
        let validator = PathValidator::new();
        let result = validator.validate("../../../etc/passwd");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PathValidationError::PathTraversal);
    }

    #[test]
    fn detects_path_traversal_url_encoded() {
        let validator = PathValidator::new();
        let result = validator.validate("..%2f..%2fetc%2fpasswd");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PathValidationError::PathTraversal);
    }

    #[test]
    fn detects_null_byte_injection() {
        let validator = PathValidator::new();
        let result = validator.validate("program.bin\0.txt");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PathValidationError::NullByte);
    }

    #[test]
    fn allows_safe_relative_paths() {
        let validator = PathValidator::new();
        let result = validator.validate("documents/file.txt");

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_absolute_paths_by_default() {
        let validator = PathValidator::new();
        let result = validator.validate("/etc/passwd");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PathValidationError::AbsolutePathNotAllowed);
    }

    #[test]
    fn base_path_prevents_escape() {
        let validator = PathValidator::with_base("/var/app/uploads");
        let result = validator.validate("../../etc/passwd");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PathValidationError::PathTraversal);
    }

    #[test]
    fn base_path_allows_safe_paths() {
        let validator = PathValidator::with_base("/var/app/uploads");
        let result = validator.validate("user123/photo.jpg");

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_str().unwrap().contains("uploads"));
    }

    #[test]
    fn contains_traversal_detects_attacks() {
        assert!(PathValidator::contains_traversal("../../../etc/passwd"));
        assert!(PathValidator::contains_traversal("docs/../../../etc/passwd"));
        assert!(PathValidator::contains_traversal("%2e%2e/etc/passwd"));
        assert!(!PathValidator::contains_traversal("documents/file.txt"));
    }

    #[test]
    fn normalize_path_resolves_dotdot() {
        let validator = PathValidator::new();
        let path = PathBuf::from("/var/app/data/../uploads/file.txt");
        let normalized = validator.normalize_path(&path);

        // Should resolve to /var/app/uploads/file.txt
        assert!(!normalized.to_str().unwrap().contains(".."));
    }
}
