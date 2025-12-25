//! SDK Configuration

use super::errors::{Error, Result};
use super::results::TileId;

/// SDK Configuration
#[derive(Debug, Clone)]
pub struct CognitumConfig {
    /// Number of tiles
    pub tiles: usize,

    /// Maximum cycles to execute
    pub max_cycles: Option<u64>,

    /// Enable tracing
    pub trace_enabled: bool,

    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for CognitumConfig {
    fn default() -> Self {
        Self {
            tiles: 256,
            max_cycles: None,
            trace_enabled: false,
            metrics_enabled: true,
        }
    }
}

impl CognitumConfig {
    /// Create a new configuration builder
    pub fn builder() -> CognitumConfigBuilder {
        CognitumConfigBuilder::default()
    }

    /// Create configuration with specific number of tiles
    pub fn with_tiles(tiles: usize) -> Self {
        Self {
            tiles,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.tiles == 0 || self.tiles > 256 {
            return Err(Error::InvalidProgram(format!(
                "Tiles must be between 1 and 256, got {}",
                self.tiles
            )));
        }
        Ok(())
    }
}

/// Configuration builder
#[derive(Debug, Default)]
pub struct CognitumConfigBuilder {
    config: CognitumConfig,
}

impl CognitumConfigBuilder {
    /// Set number of tiles
    pub fn tiles(mut self, tiles: usize) -> Self {
        self.config.tiles = tiles;
        self
    }

    /// Set maximum cycles
    pub fn max_cycles(mut self, cycles: u64) -> Self {
        self.config.max_cycles = Some(cycles);
        self
    }

    /// Enable tracing
    pub fn trace(mut self, enabled: bool) -> Self {
        self.config.trace_enabled = enabled;
        self
    }

    /// Enable metrics
    pub fn metrics(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    /// Build configuration
    pub fn build(self) -> Result<CognitumConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

/// Tile configuration
#[derive(Debug, Clone)]
pub struct TileConfig {
    /// Number of tiles
    pub count: usize,

    /// Configuration per tile
    pub configs: Vec<TileSpecificConfig>,
}

/// Configuration for a specific tile
#[derive(Debug, Clone)]
pub struct TileSpecificConfig {
    /// Tile ID
    pub id: TileId,

    /// Enable/disable tile
    pub enabled: bool,
}
