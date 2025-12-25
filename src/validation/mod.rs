//! Input Validation and Injection Prevention Module
//!
//! This module provides comprehensive security controls for preventing
//! SQL injection, command injection, path traversal, and other input-based attacks.
//!
//! # Components
//!
//! - `sql` - SQL and NoSQL injection prevention with parameterized queries
//! - `path` - Path traversal and directory escape prevention
//! - `program` - Secure program loading with whitelist and sandbox integration
//! - `sanitize` - General input sanitization utilities

pub mod sql;
pub mod path;
pub mod program;
pub mod sanitize;

pub use sql::{QueryParam, QueryBuilder, SqlValidator};
pub use path::{PathValidator, PathValidationError};
pub use program::{SecureProgramLoader, ProgramLoaderConfig, LoaderError};
pub use sanitize::{InputSanitizer, SanitizationError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_exports_are_accessible() {
        // Verify all public exports are accessible
        let _ = QueryParam::String("test".to_string());
        let _ = PathValidator::new();
        let _ = InputSanitizer::new();
    }
}
