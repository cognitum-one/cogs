//! Error types for the Agentic VM CLI

use std::path::PathBuf;
use thiserror::Error;

/// Result type for CLI operations
pub type Result<T> = std::result::Result<T, CliError>;

/// CLI error types
#[derive(Error, Debug)]
pub enum CliError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    /// Invalid configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    /// Capsule error
    #[error("Capsule error: {0}")]
    Capsule(String),

    /// Capsule not found
    #[error("Capsule not found: {id}")]
    CapsuleNotFound { id: String },

    /// Snapshot error
    #[error("Snapshot error: {0}")]
    Snapshot(String),

    /// Snapshot not found
    #[error("Snapshot not found: {id}")]
    SnapshotNotFound { id: String },

    /// Evidence error
    #[error("Evidence error: {0}")]
    Evidence(String),

    /// Evidence not found
    #[error("Evidence not found for run: {run_id}")]
    EvidenceNotFound { run_id: String },

    /// Evidence verification failed
    #[error("Evidence verification failed: {reason}")]
    EvidenceVerificationFailed { reason: String },

    /// Replay error
    #[error("Replay error: {0}")]
    Replay(String),

    /// Replay verification failed
    #[error("Replay verification failed: {mismatches} mismatches found")]
    ReplayVerificationFailed { mismatches: usize },

    /// Benchmark error
    #[error("Benchmark error: {0}")]
    Benchmark(String),

    /// Benchmark criteria not met
    #[error("Benchmark criteria not met: {criteria}")]
    BenchmarkCriteriaNotMet { criteria: String },

    /// Workspace error
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// Workspace not found
    #[error("Workspace not found: {path}")]
    WorkspaceNotFound { path: PathBuf },

    /// Manifest error
    #[error("Manifest error: {0}")]
    Manifest(String),

    /// Manifest not found
    #[error("Manifest not found: {path}")]
    ManifestNotFound { path: PathBuf },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// UUID parsing error
    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// Chrono parsing error
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(#[from] chrono::ParseError),

    /// Process spawn error
    #[error("Failed to spawn process: {0}")]
    ProcessSpawn(String),

    /// Process execution error
    #[error("Process execution failed: exit code {exit_code}")]
    ProcessFailed { exit_code: i32 },

    /// Timeout error
    #[error("Operation timed out after {seconds} seconds")]
    Timeout { seconds: u64 },

    /// VMM error
    #[error("VMM error: {0}")]
    Vmm(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Permission denied
    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    /// Resource exhausted
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Anyhow error wrapper
    #[error("{0}")]
    Anyhow(String),
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        CliError::Anyhow(err.to_string())
    }
}

/// Exit codes for the CLI
pub mod exit_codes {
    /// Successful execution
    pub const SUCCESS: i32 = 0;
    /// General error
    pub const ERROR: i32 = 1;
    /// Validation failure (evidence verification, benchmark criteria)
    pub const VALIDATION_FAILURE: i32 = 2;
    /// Configuration error
    pub const CONFIG_ERROR: i32 = 3;
    /// Resource not found
    pub const NOT_FOUND: i32 = 4;
}

impl CliError {
    /// Get the exit code for this error
    pub fn exit_code(&self) -> i32 {
        use exit_codes::*;
        match self {
            Self::EvidenceVerificationFailed { .. }
            | Self::ReplayVerificationFailed { .. }
            | Self::BenchmarkCriteriaNotMet { .. } => VALIDATION_FAILURE,

            Self::Config(_)
            | Self::ConfigNotFound { .. }
            | Self::InvalidConfig { .. } => CONFIG_ERROR,

            Self::CapsuleNotFound { .. }
            | Self::SnapshotNotFound { .. }
            | Self::EvidenceNotFound { .. }
            | Self::WorkspaceNotFound { .. }
            | Self::ManifestNotFound { .. } => NOT_FOUND,

            _ => ERROR,
        }
    }
}
