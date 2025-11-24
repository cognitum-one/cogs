use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Cognitum CLI Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CognitumCliConfig {
    pub simulation: SimulationConfig,
    pub hardware: HardwareConfig,
    pub logging: LoggingConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulationConfig {
    /// Use event-driven simulation (faster)
    pub event_driven: bool,

    /// Cycle-accurate mode (slower but precise)
    pub cycle_accurate: bool,

    /// Maximum number of cycles before timeout
    pub max_cycles: Option<u64>,

    /// Performance mode (disables some checks)
    pub performance_mode: bool,

    /// Random seed for deterministic execution
    pub random_seed: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HardwareConfig {
    /// Number of tiles (1-256)
    pub tiles: u16,

    /// Clock frequency in MHz
    pub clock_freq_mhz: u32,

    /// Memory configuration
    pub code_memory_kb: u32,
    pub data_memory_kb: u32,
    pub work_memory_kb: u32,

    /// Enable ECC memory protection
    pub ecc_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Trace packet flow through RaceWay
    pub trace_packets: bool,

    /// Trace instruction execution
    pub trace_instructions: bool,

    /// Trace memory operations
    pub trace_memory: bool,

    /// Output file for trace
    pub trace_file: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    pub worker_threads: usize,

    /// Enable metrics collection
    pub metrics_enabled: bool,

    /// Packet timeout in milliseconds
    pub packet_timeout_ms: u64,

    /// Enable parallel tile execution
    pub parallel_execution: bool,
}

impl Default for CognitumCliConfig {
    fn default() -> Self {
        Self {
            simulation: SimulationConfig {
                event_driven: true,
                cycle_accurate: false,
                max_cycles: None,
                performance_mode: true,
                random_seed: None,
            },
            hardware: HardwareConfig {
                tiles: 256,
                clock_freq_mhz: 1000,
                code_memory_kb: 8,
                data_memory_kb: 8,
                work_memory_kb: 64,
                ecc_enabled: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                trace_packets: false,
                trace_instructions: false,
                trace_memory: false,
                trace_file: None,
            },
            performance: PerformanceConfig {
                worker_threads: 8,
                metrics_enabled: true,
                packet_timeout_ms: 100,
                parallel_execution: true,
            },
        }
    }
}

impl CognitumCliConfig {
    /// Load configuration from TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Self =
            toml::from_str(&content).with_context(|| "Failed to parse TOML configuration")?;

        config.validate()?;

        Ok(config)
    }

    /// Save configuration to TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize configuration")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.hardware.tiles >= 1 && self.hardware.tiles <= 256,
            "Tiles must be between 1 and 256, got {}",
            self.hardware.tiles
        );

        anyhow::ensure!(
            self.performance.worker_threads >= 1 && self.performance.worker_threads <= 128,
            "Worker threads must be between 1 and 128, got {}",
            self.performance.worker_threads
        );

        anyhow::ensure!(
            self.hardware.clock_freq_mhz > 0,
            "Clock frequency must be positive, got {}",
            self.hardware.clock_freq_mhz
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = CognitumCliConfig::default();
        assert_eq!(config.hardware.tiles, 256);
        assert_eq!(config.performance.worker_threads, 8);
        assert!(config.simulation.event_driven);
    }

    #[test]
    fn test_config_validation() {
        let mut config = CognitumCliConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid tiles
        config.hardware.tiles = 300;
        assert!(config.validate().is_err());

        config.hardware.tiles = 0;
        assert!(config.validate().is_err());

        // Valid tiles
        config.hardware.tiles = 128;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_load_save_config() -> Result<()> {
        let config = CognitumCliConfig::default();

        let mut temp_file = NamedTempFile::new()?;
        let toml_content = toml::to_string_pretty(&config)?;
        temp_file.write_all(toml_content.as_bytes())?;

        let loaded_config = CognitumCliConfig::load(temp_file.path())?;
        assert_eq!(loaded_config.hardware.tiles, config.hardware.tiles);
        assert_eq!(
            loaded_config.performance.worker_threads,
            config.performance.worker_threads
        );

        Ok(())
    }
}
