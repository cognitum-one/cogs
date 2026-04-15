//! Configuration management for Agentic VM CLI
//!
//! Supports loading from:
//! - Default config file (~/.agentvm/config.toml)
//! - Project-local config (.agentvm.toml)
//! - Environment variables (AGENTVM_*)
//! - Command-line arguments (highest priority)

use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General settings
    pub general: GeneralConfig,

    /// VMM settings
    pub vmm: VmmConfig,

    /// Evidence settings
    pub evidence: EvidenceConfig,

    /// Snapshot settings
    pub snapshot: SnapshotConfig,

    /// Benchmark settings
    pub benchmark: BenchmarkConfig,

    /// Network settings
    pub network: NetworkConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            vmm: VmmConfig::default(),
            evidence: EvidenceConfig::default(),
            snapshot: SnapshotConfig::default(),
            benchmark: BenchmarkConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

/// General configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Data directory for agentvm
    pub data_dir: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,

    /// Log format (text, json)
    pub log_format: String,

    /// Enable colored output
    pub color: bool,

    /// Default output format (text, json, table)
    pub output_format: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        let data_dir = dirs::home_dir()
            .map(|h| h.join(".agentvm"))
            .unwrap_or_else(|| PathBuf::from("/var/lib/agentvm"));

        Self {
            data_dir,
            log_level: "info".to_string(),
            log_format: "text".to_string(),
            color: true,
            output_format: "text".to_string(),
        }
    }
}

/// VMM configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VmmConfig {
    /// VMM backend (qemu, firecracker)
    pub backend: String,

    /// Path to VMM binary
    pub binary_path: Option<PathBuf>,

    /// Base image path
    pub base_image: Option<PathBuf>,

    /// Kernel path
    pub kernel_path: Option<PathBuf>,

    /// Default memory allocation (MB)
    pub default_memory_mb: u32,

    /// Default CPU count
    pub default_cpus: u32,

    /// Enable KVM acceleration
    pub enable_kvm: bool,

    /// Network mode (none, user, bridge)
    pub network_mode: String,

    /// Vsock CID base
    pub vsock_cid_base: u32,
}

impl Default for VmmConfig {
    fn default() -> Self {
        Self {
            backend: "qemu".to_string(),
            binary_path: None,
            base_image: None,
            kernel_path: None,
            default_memory_mb: 2048,
            default_cpus: 2,
            enable_kvm: true,
            network_mode: "user".to_string(),
            vsock_cid_base: 1000,
        }
    }
}

/// Evidence configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EvidenceConfig {
    /// Evidence storage directory
    pub storage_dir: PathBuf,

    /// Default evidence level (none, summary, full)
    pub default_level: String,

    /// Retention period in days
    pub retention_days: u32,

    /// Enable compression
    pub compress: bool,

    /// Signing mode (none, capsule, host, hsm)
    pub signing_mode: String,

    /// HSM configuration (if using HSM)
    pub hsm_slot: Option<u32>,

    /// Transparency log URL (optional)
    pub transparency_log_url: Option<String>,
}

impl Default for EvidenceConfig {
    fn default() -> Self {
        let storage_dir = dirs::home_dir()
            .map(|h| h.join(".agentvm").join("evidence"))
            .unwrap_or_else(|| PathBuf::from("/var/lib/agentvm/evidence"));

        Self {
            storage_dir,
            default_level: "full".to_string(),
            retention_days: 30,
            compress: true,
            signing_mode: "capsule".to_string(),
            hsm_slot: None,
            transparency_log_url: None,
        }
    }
}

/// Snapshot configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapshotConfig {
    /// Snapshot storage directory
    pub storage_dir: PathBuf,

    /// Maximum number of snapshots per capsule
    pub max_per_capsule: u32,

    /// Enable memory snapshots (in addition to disk)
    pub memory_snapshots: bool,

    /// Snapshot on capability calls
    pub snapshot_on_capability: bool,

    /// Compression level (0-9)
    pub compression_level: u32,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        let storage_dir = dirs::home_dir()
            .map(|h| h.join(".agentvm").join("snapshots"))
            .unwrap_or_else(|| PathBuf::from("/var/lib/agentvm/snapshots"));

        Self {
            storage_dir,
            max_per_capsule: 10,
            memory_snapshots: false,
            snapshot_on_capability: true,
            compression_level: 3,
        }
    }
}

/// Benchmark configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchmarkConfig {
    /// Output directory for benchmark reports
    pub output_dir: PathBuf,

    /// Default number of iterations
    pub default_iterations: u32,

    /// Warmup iterations
    pub warmup_iterations: u32,

    /// Required p95 improvement factor for pass
    pub p95_improvement_threshold: f64,

    /// Maximum coefficient of variation for pass
    pub cov_threshold: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        let output_dir = dirs::home_dir()
            .map(|h| h.join(".agentvm").join("benchmarks"))
            .unwrap_or_else(|| PathBuf::from("/var/lib/agentvm/benchmarks"));

        Self {
            output_dir,
            default_iterations: 30,
            warmup_iterations: 3,
            p95_improvement_threshold: 2.0,
            cov_threshold: 0.2,
        }
    }
}

/// Network configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    /// Capability proxy listen address
    pub proxy_address: String,

    /// Capability proxy port
    pub proxy_port: u16,

    /// Default network allowlist
    pub default_allowlist: Vec<String>,

    /// Enable request logging
    pub log_requests: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy_address: "127.0.0.1".to_string(),
            proxy_port: 8765,
            default_allowlist: vec![
                "api.anthropic.com".to_string(),
                "github.com".to_string(),
                "api.github.com".to_string(),
            ],
            log_requests: true,
        }
    }
}

impl Config {
    /// Load configuration from default locations
    ///
    /// Order of precedence (highest first):
    /// 1. Environment variables (AGENTVM_*)
    /// 2. Project-local config (.agentvm.toml)
    /// 3. User config (~/.agentvm/config.toml)
    /// 4. Default values
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load user config
        if let Some(user_config_path) = Self::user_config_path() {
            if user_config_path.exists() {
                let user_config = Self::load_from_file(&user_config_path)?;
                config = config.merge(user_config);
            }
        }

        // Load project-local config
        let local_config_path = PathBuf::from(".agentvm.toml");
        if local_config_path.exists() {
            let local_config = Self::load_from_file(&local_config_path)?;
            config = config.merge(local_config);
        }

        // Apply environment variable overrides
        config = config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::ConfigNotFound {
                    path: path.to_path_buf(),
                }
            } else {
                CliError::Io(e)
            }
        })?;

        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("Failed to serialize config: {}", e)))?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the user config path
    pub fn user_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".agentvm").join("config.toml"))
    }

    /// Merge another config into this one (other takes precedence)
    fn merge(self, other: Self) -> Self {
        // For simplicity, just use the other config's values
        // A more sophisticated merge would check for Option/default values
        other
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(mut self) -> Self {
        // General
        if let Ok(val) = std::env::var("AGENTVM_DATA_DIR") {
            self.general.data_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("AGENTVM_LOG_LEVEL") {
            self.general.log_level = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_LOG_FORMAT") {
            self.general.log_format = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_COLOR") {
            self.general.color = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("AGENTVM_OUTPUT_FORMAT") {
            self.general.output_format = val;
        }

        // VMM
        if let Ok(val) = std::env::var("AGENTVM_VMM_BACKEND") {
            self.vmm.backend = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_VMM_BINARY") {
            self.vmm.binary_path = Some(PathBuf::from(val));
        }
        if let Ok(val) = std::env::var("AGENTVM_BASE_IMAGE") {
            self.vmm.base_image = Some(PathBuf::from(val));
        }
        if let Ok(val) = std::env::var("AGENTVM_KERNEL") {
            self.vmm.kernel_path = Some(PathBuf::from(val));
        }
        if let Ok(val) = std::env::var("AGENTVM_DEFAULT_MEMORY_MB") {
            if let Ok(mem) = val.parse() {
                self.vmm.default_memory_mb = mem;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_DEFAULT_CPUS") {
            if let Ok(cpus) = val.parse() {
                self.vmm.default_cpus = cpus;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_ENABLE_KVM") {
            self.vmm.enable_kvm = val.parse().unwrap_or(true);
        }

        // Evidence
        if let Ok(val) = std::env::var("AGENTVM_EVIDENCE_DIR") {
            self.evidence.storage_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("AGENTVM_EVIDENCE_LEVEL") {
            self.evidence.default_level = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_EVIDENCE_RETENTION_DAYS") {
            if let Ok(days) = val.parse() {
                self.evidence.retention_days = days;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_SIGNING_MODE") {
            self.evidence.signing_mode = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_TRANSPARENCY_LOG_URL") {
            self.evidence.transparency_log_url = Some(val);
        }

        // Snapshot
        if let Ok(val) = std::env::var("AGENTVM_SNAPSHOT_DIR") {
            self.snapshot.storage_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("AGENTVM_MAX_SNAPSHOTS") {
            if let Ok(max) = val.parse() {
                self.snapshot.max_per_capsule = max;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_MEMORY_SNAPSHOTS") {
            self.snapshot.memory_snapshots = val.parse().unwrap_or(false);
        }

        // Benchmark
        if let Ok(val) = std::env::var("AGENTVM_BENCHMARK_DIR") {
            self.benchmark.output_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("AGENTVM_BENCHMARK_ITERATIONS") {
            if let Ok(iter) = val.parse() {
                self.benchmark.default_iterations = iter;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_P95_THRESHOLD") {
            if let Ok(threshold) = val.parse() {
                self.benchmark.p95_improvement_threshold = threshold;
            }
        }
        if let Ok(val) = std::env::var("AGENTVM_COV_THRESHOLD") {
            if let Ok(threshold) = val.parse() {
                self.benchmark.cov_threshold = threshold;
            }
        }

        // Network
        if let Ok(val) = std::env::var("AGENTVM_PROXY_ADDRESS") {
            self.network.proxy_address = val;
        }
        if let Ok(val) = std::env::var("AGENTVM_PROXY_PORT") {
            if let Ok(port) = val.parse() {
                self.network.proxy_port = port;
            }
        }

        self
    }

    /// Ensure all required directories exist
    pub fn ensure_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.general.data_dir)?;
        std::fs::create_dir_all(&self.evidence.storage_dir)?;
        std::fs::create_dir_all(&self.snapshot.storage_dir)?;
        std::fs::create_dir_all(&self.benchmark.output_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.vmm.default_memory_mb, 2048);
        assert_eq!(config.evidence.default_level, "full");
    }

    #[test]
    fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = Config::default();
        config.save_to_file(&path).unwrap();

        let loaded = Config::load_from_file(&path).unwrap();
        assert_eq!(loaded.general.log_level, config.general.log_level);
    }
}
