//! Configuration for the capability proxy.

use crate::error::ProxyError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// vsock configuration
    #[serde(default)]
    pub vsock: VsockConfig,

    /// Network executor configuration
    #[serde(default)]
    pub network: NetworkConfig,

    /// Filesystem executor configuration
    #[serde(default)]
    pub filesystem: FilesystemConfig,

    /// Secrets provider configuration
    #[serde(default)]
    pub secrets: SecretsConfig,

    /// Evidence logging configuration
    #[serde(default)]
    pub evidence: EvidenceConfig,

    /// General proxy settings
    #[serde(default)]
    pub general: GeneralConfig,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            vsock: VsockConfig::default(),
            network: NetworkConfig::default(),
            filesystem: FilesystemConfig::default(),
            secrets: SecretsConfig::default(),
            evidence: EvidenceConfig::default(),
            general: GeneralConfig::default(),
        }
    }
}

impl ProxyConfig {
    /// Load configuration from a file
    pub fn from_file(path: &std::path::Path) -> Result<Self, ProxyError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ProxyError::Config(format!("failed to read config file: {}", e)))?;

        let config: Self = if path.extension().map_or(false, |e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            // Assume TOML for other extensions
            toml::from_str(&content)
                .map_err(|e| ProxyError::Config(format!("failed to parse TOML: {}", e)))?
        };

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ProxyError> {
        let mut config = Self::default();

        // vsock settings
        if let Ok(cid) = std::env::var("AGENTVM_VSOCK_CID") {
            config.vsock.cid = cid
                .parse()
                .map_err(|_| ProxyError::Config("invalid VSOCK_CID".to_string()))?;
        }
        if let Ok(port) = std::env::var("AGENTVM_VSOCK_PORT") {
            config.vsock.port = port
                .parse()
                .map_err(|_| ProxyError::Config("invalid VSOCK_PORT".to_string()))?;
        }

        // Network settings
        if let Ok(domains) = std::env::var("AGENTVM_NETWORK_ALLOWLIST") {
            config.network.domain_allowlist = domains.split(',').map(String::from).collect();
        }

        // Filesystem settings
        if let Ok(workspace) = std::env::var("AGENTVM_WORKSPACE") {
            config.filesystem.workspace = PathBuf::from(workspace);
        }

        // Evidence settings
        if let Ok(path) = std::env::var("AGENTVM_EVIDENCE_PATH") {
            config.evidence.path = PathBuf::from(path);
        }

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ProxyError> {
        // Validation is lenient - we don't require paths to exist at config time
        // They will be created as needed or fail at runtime with clear errors
        // This allows for default configs to work without requiring specific directories

        // Basic sanity checks
        if self.general.max_capabilities_per_capsule == 0 {
            return Err(ProxyError::Config(
                "max_capabilities_per_capsule must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// vsock listener configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsockConfig {
    /// Context ID (CID) for vsock
    #[serde(default = "default_vsock_cid")]
    pub cid: u32,

    /// Port to listen on
    #[serde(default = "default_vsock_port")]
    pub port: u32,

    /// Maximum concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Connection timeout
    #[serde(default = "default_connection_timeout", with = "humantime_serde")]
    pub connection_timeout: Duration,

    /// Read timeout
    #[serde(default = "default_read_timeout", with = "humantime_serde")]
    pub read_timeout: Duration,

    /// Use TCP fallback instead of vsock (for testing)
    #[serde(default)]
    pub tcp_fallback: bool,

    /// TCP fallback address
    #[serde(default = "default_tcp_address")]
    pub tcp_address: String,
}

impl Default for VsockConfig {
    fn default() -> Self {
        Self {
            cid: default_vsock_cid(),
            port: default_vsock_port(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            read_timeout: default_read_timeout(),
            tcp_fallback: false,
            tcp_address: default_tcp_address(),
        }
    }
}

fn default_vsock_cid() -> u32 {
    2 // VMADDR_CID_HOST
}

fn default_vsock_port() -> u32 {
    9999
}

fn default_max_connections() -> usize {
    100
}

fn default_connection_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_read_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_tcp_address() -> String {
    "127.0.0.1:9999".to_string()
}

/// Network executor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Allowed domains (glob patterns)
    #[serde(default)]
    pub domain_allowlist: Vec<String>,

    /// Blocked domains (takes precedence over allowlist)
    #[serde(default)]
    pub domain_blocklist: Vec<String>,

    /// Maximum request body size
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize,

    /// Maximum response body size
    #[serde(default = "default_max_response_size")]
    pub max_response_size: usize,

    /// Request timeout
    #[serde(default = "default_request_timeout", with = "humantime_serde")]
    pub request_timeout: Duration,

    /// Enable connection pooling
    #[serde(default = "default_true")]
    pub connection_pooling: bool,

    /// Maximum idle connections per host
    #[serde(default = "default_max_idle_per_host")]
    pub max_idle_per_host: usize,

    /// Rate limit requests per second (0 = unlimited)
    #[serde(default)]
    pub rate_limit_rps: u32,

    /// User agent string
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            domain_allowlist: Vec::new(),
            domain_blocklist: Vec::new(),
            max_request_size: default_max_request_size(),
            max_response_size: default_max_response_size(),
            request_timeout: default_request_timeout(),
            connection_pooling: true,
            max_idle_per_host: default_max_idle_per_host(),
            rate_limit_rps: 0,
            user_agent: default_user_agent(),
        }
    }
}

fn default_max_request_size() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_max_response_size() -> usize {
    100 * 1024 * 1024 // 100 MB
}

fn default_request_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_max_idle_per_host() -> usize {
    10
}

fn default_user_agent() -> String {
    format!("agentvm-proxy/{}", env!("CARGO_PKG_VERSION"))
}

fn default_true() -> bool {
    true
}

/// Filesystem executor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    /// Workspace root path
    #[serde(default = "default_workspace")]
    pub workspace: PathBuf,

    /// Additional allowed paths outside workspace
    #[serde(default)]
    pub additional_paths: Vec<PathBuf>,

    /// Forbidden path patterns (even within allowed paths)
    #[serde(default = "default_forbidden_patterns")]
    pub forbidden_patterns: Vec<String>,

    /// Enable inotify for change tracking
    #[serde(default = "default_true")]
    pub enable_change_tracking: bool,

    /// Maximum file size for read operations
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,

    /// Maximum directory depth
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            additional_paths: Vec::new(),
            forbidden_patterns: default_forbidden_patterns(),
            enable_change_tracking: true,
            max_file_size: default_max_file_size(),
            max_depth: default_max_depth(),
        }
    }
}

fn default_workspace() -> PathBuf {
    PathBuf::from("/workspace")
}

fn default_forbidden_patterns() -> Vec<String> {
    vec![
        "**/.env".to_string(),
        "**/.env.*".to_string(),
        "**/credentials*".to_string(),
        "**/secrets*".to_string(),
        "**/*.key".to_string(),
        "**/*.pem".to_string(),
        "**/id_rsa*".to_string(),
        "**/.git/config".to_string(),
    ]
}

fn default_max_file_size() -> usize {
    100 * 1024 * 1024 // 100 MB
}

fn default_max_depth() -> usize {
    50
}

/// Secrets provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Provider type
    #[serde(default)]
    pub provider: SecretsProvider,

    /// Prefix for environment variable secrets
    #[serde(default = "default_env_prefix")]
    pub env_prefix: String,

    /// Path to secrets file (for file provider)
    #[serde(default)]
    pub secrets_file: Option<PathBuf>,

    /// Allowed secret names (empty = all)
    #[serde(default)]
    pub allowed_names: Vec<String>,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            provider: SecretsProvider::default(),
            env_prefix: default_env_prefix(),
            secrets_file: None,
            allowed_names: Vec::new(),
        }
    }
}

fn default_env_prefix() -> String {
    "AGENTVM_SECRET_".to_string()
}

/// Secrets provider type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretsProvider {
    /// Read from environment variables
    #[default]
    Environment,
    /// Read from a file
    File,
    /// No secrets provider
    None,
}

/// Evidence logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceConfig {
    /// Path to evidence log directory
    #[serde(default = "default_evidence_path")]
    pub path: PathBuf,

    /// Enable evidence logging
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Log rotation size (bytes)
    #[serde(default = "default_rotation_size")]
    pub rotation_size: usize,

    /// Maximum log files to keep
    #[serde(default = "default_max_log_files")]
    pub max_log_files: usize,

    /// Flush interval
    #[serde(default = "default_flush_interval", with = "humantime_serde")]
    pub flush_interval: Duration,

    /// Enable Merkle tree for integrity
    #[serde(default = "default_true")]
    pub merkle_enabled: bool,
}

impl Default for EvidenceConfig {
    fn default() -> Self {
        Self {
            path: default_evidence_path(),
            enabled: true,
            rotation_size: default_rotation_size(),
            max_log_files: default_max_log_files(),
            flush_interval: default_flush_interval(),
            merkle_enabled: true,
        }
    }
}

fn default_evidence_path() -> PathBuf {
    PathBuf::from("/var/log/agentvm/evidence")
}

fn default_rotation_size() -> usize {
    100 * 1024 * 1024 // 100 MB
}

fn default_max_log_files() -> usize {
    10
}

fn default_flush_interval() -> Duration {
    Duration::from_secs(1)
}

/// General proxy settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Maximum capabilities per capsule
    #[serde(default = "default_max_capabilities")]
    pub max_capabilities_per_capsule: usize,

    /// Default capability duration
    #[serde(default = "default_capability_duration", with = "humantime_serde")]
    pub default_capability_duration: Duration,

    /// Enable metrics collection
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Metrics collection interval
    #[serde(default = "default_metrics_interval", with = "humantime_serde")]
    pub metrics_interval: Duration,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            max_capabilities_per_capsule: default_max_capabilities(),
            default_capability_duration: default_capability_duration(),
            metrics_enabled: true,
            metrics_interval: default_metrics_interval(),
            log_level: default_log_level(),
        }
    }
}

fn default_max_capabilities() -> usize {
    1000
}

fn default_capability_duration() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

fn default_metrics_interval() -> Duration {
    Duration::from_secs(60)
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Helper module for humantime serialization
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&super::humantime::format_duration(*duration).to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        super::humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }
}

/// TOML parsing support
mod toml {
    use serde::de::DeserializeOwned;

    pub fn from_str<T: DeserializeOwned>(s: &str) -> Result<T, TomlError> {
        // Minimal TOML parsing - in production use the toml crate
        serde_json::from_str(s).map_err(|e| TomlError(e.to_string()))
    }

    #[derive(Debug)]
    pub struct TomlError(pub String);

    impl std::fmt::Display for TomlError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for TomlError {}
}

/// humantime parsing support
mod humantime {
    use std::time::Duration;

    pub fn parse_duration(s: &str) -> Result<Duration, ParseError> {
        // Parse durations like "30s", "5m", "1h"
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError("empty duration".to_string()));
        }

        let (num, unit) = if s.ends_with("ms") {
            (&s[..s.len() - 2], "ms")
        } else if s.ends_with('s') {
            (&s[..s.len() - 1], "s")
        } else if s.ends_with('m') {
            (&s[..s.len() - 1], "m")
        } else if s.ends_with('h') {
            (&s[..s.len() - 1], "h")
        } else if s.ends_with('d') {
            (&s[..s.len() - 1], "d")
        } else {
            // Assume seconds
            (s, "s")
        };

        let n: u64 = num.trim().parse().map_err(|_| ParseError(format!("invalid number: {}", num)))?;

        Ok(match unit {
            "ms" => Duration::from_millis(n),
            "s" => Duration::from_secs(n),
            "m" => Duration::from_secs(n * 60),
            "h" => Duration::from_secs(n * 3600),
            "d" => Duration::from_secs(n * 86400),
            _ => return Err(ParseError(format!("unknown unit: {}", unit))),
        })
    }

    pub fn format_duration(d: Duration) -> impl std::fmt::Display {
        let secs = d.as_secs();
        if secs == 0 {
            format!("{}ms", d.as_millis())
        } else if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else if secs < 86400 {
            format!("{}h", secs / 3600)
        } else {
            format!("{}d", secs / 86400)
        }
    }

    #[derive(Debug)]
    pub struct ParseError(String);

    impl std::fmt::Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for ParseError {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ProxyConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_duration_parsing() {
        assert_eq!(humantime::parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(humantime::parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(humantime::parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(humantime::parse_duration("100ms").unwrap(), Duration::from_millis(100));
    }

    #[test]
    fn test_env_config() {
        // This test verifies from_env doesn't panic with default env
        // In real tests, we'd set specific env vars
        let _ = ProxyConfig::from_env();
    }
}
