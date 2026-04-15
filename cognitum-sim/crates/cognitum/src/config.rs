//! Configuration builder for Cognitum SDK

use crate::error::{CognitumError, Result};
use std::path::PathBuf;
use std::time::Duration;

/// Cognitum SDK Configuration
#[derive(Debug, Clone)]
pub struct CognitumConfig {
    /// Number of tiles (1-256)
    pub tiles: usize,

    /// Enable execution tracing
    pub trace_enabled: bool,

    /// Trace output file
    pub trace_file: Option<PathBuf>,

    /// Maximum simulation cycles
    pub max_cycles: Option<u64>,

    /// RaceWay packet timeout
    pub packet_timeout: Duration,

    /// Enable performance metrics
    pub metrics_enabled: bool,

    /// Random seed for deterministic execution
    pub random_seed: Option<u64>,

    /// Number of worker threads
    pub worker_threads: usize,

    /// Enable parallel tile execution
    pub parallel_execution: bool,
}

impl Default for CognitumConfig {
    fn default() -> Self {
        Self {
            tiles: 256,
            trace_enabled: false,
            trace_file: None,
            max_cycles: None,
            packet_timeout: Duration::from_millis(100),
            metrics_enabled: true,
            random_seed: None,
            worker_threads: 8,
            parallel_execution: true,
        }
    }
}

impl CognitumConfig {
    /// Create a new configuration builder
    pub fn builder() -> CognitumConfigBuilder {
        CognitumConfigBuilder::default()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.tiles == 0 || self.tiles > 256 {
            return Err(CognitumError::config(format!(
                "Tiles must be between 1 and 256, got {}",
                self.tiles
            )));
        }

        if self.worker_threads == 0 {
            return Err(CognitumError::config("Worker threads must be at least 1"));
        }

        Ok(())
    }
}

/// Builder for CognitumConfig
#[derive(Debug, Default)]
pub struct CognitumConfigBuilder {
    config: CognitumConfig,
}

impl CognitumConfigBuilder {
    /// Set number of tiles (1-256)
    pub fn tiles(mut self, tiles: usize) -> Self {
        self.config.tiles = tiles;
        self
    }

    /// Enable or disable execution tracing
    pub fn trace(mut self, enabled: bool) -> Self {
        self.config.trace_enabled = enabled;
        self
    }

    /// Set trace output file
    pub fn trace_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.trace_file = Some(path.into());
        self.config.trace_enabled = true;
        self
    }

    /// Set maximum simulation cycles
    pub fn max_cycles(mut self, cycles: u64) -> Self {
        self.config.max_cycles = Some(cycles);
        self
    }

    /// Set RaceWay packet timeout
    pub fn packet_timeout(mut self, timeout: Duration) -> Self {
        self.config.packet_timeout = timeout;
        self
    }

    /// Enable or disable performance metrics
    pub fn metrics(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    /// Set random seed for deterministic execution
    pub fn random_seed(mut self, seed: u64) -> Self {
        self.config.random_seed = Some(seed);
        self
    }

    /// Set number of worker threads
    pub fn worker_threads(mut self, threads: usize) -> Self {
        self.config.worker_threads = threads;
        self
    }

    /// Enable or disable parallel tile execution
    pub fn parallel_execution(mut self, enabled: bool) -> Self {
        self.config.parallel_execution = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<CognitumConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CognitumConfig::default();
        assert_eq!(config.tiles, 256);
        assert_eq!(config.worker_threads, 8);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_builder() {
        let config = CognitumConfig::builder()
            .tiles(128)
            .trace(true)
            .max_cycles(1_000_000)
            .worker_threads(16)
            .build()
            .unwrap();

        assert_eq!(config.tiles, 128);
        assert!(config.trace_enabled);
        assert_eq!(config.max_cycles, Some(1_000_000));
        assert_eq!(config.worker_threads, 16);
    }

    #[test]
    fn test_validation() {
        // Too many tiles
        let result = CognitumConfig::builder().tiles(300).build();
        assert!(result.is_err());

        // Zero tiles
        let result = CognitumConfig::builder().tiles(0).build();
        assert!(result.is_err());

        // Valid configuration
        let result = CognitumConfig::builder().tiles(64).build();
        assert!(result.is_ok());
    }
}
