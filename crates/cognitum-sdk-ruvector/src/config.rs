//! Configuration types for Ruvector SDK

use crate::error::{Result, RuvectorError};
use serde::{Deserialize, Serialize};

/// Comprehensive configuration for Ruvector client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvectorConfig {
    /// Embedding vector dimension (typically 256, 512, or 1024)
    pub embedding_dimension: usize,

    /// Maximum number of embeddings to store
    pub index_capacity: usize,

    /// HNSW parameter: number of bi-directional links per node
    /// Higher values improve recall but increase memory usage
    /// Recommended: 16-48
    pub hnsw_m: usize,

    /// HNSW parameter: size of dynamic candidate list during construction
    /// Higher values improve index quality but slow construction
    /// Recommended: 200-400
    pub hnsw_ef_construction: usize,

    /// HNSW parameter: size of dynamic candidate list during search
    /// Higher values improve recall but slow search
    /// Recommended: 50-200
    pub hnsw_ef_search: usize,

    /// Number of chip tiles for routing
    pub num_tiles: usize,

    /// Enable automatic model training from execution traces
    pub auto_train_router: bool,

    /// Minimum traces required before training
    pub min_traces_for_training: usize,

    /// Training batch size
    pub training_batch_size: usize,

    /// Maximum concurrent operations
    pub max_concurrent_ops: usize,

    /// Operation timeout in milliseconds
    pub operation_timeout_ms: u64,

    /// Enable performance metrics collection
    pub enable_metrics: bool,
}

impl Default for RuvectorConfig {
    fn default() -> Self {
        Self {
            embedding_dimension: 256,
            index_capacity: 100_000,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 50,
            num_tiles: 16,
            auto_train_router: false,
            min_traces_for_training: 100,
            training_batch_size: 32,
            max_concurrent_ops: 1000,
            operation_timeout_ms: 5000,
            enable_metrics: false,
        }
    }
}

impl RuvectorConfig {
    /// Create a new configuration builder
    pub fn builder() -> RuvectorConfigBuilder {
        RuvectorConfigBuilder::default()
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.embedding_dimension == 0 {
            return Err(RuvectorError::Config(
                "Embedding dimension must be greater than 0".to_string(),
            ));
        }

        if self.embedding_dimension > 4096 {
            return Err(RuvectorError::Config(
                "Embedding dimension too large (max 4096)".to_string(),
            ));
        }

        if self.index_capacity == 0 {
            return Err(RuvectorError::Config(
                "Index capacity must be greater than 0".to_string(),
            ));
        }

        if self.hnsw_m < 4 || self.hnsw_m > 128 {
            return Err(RuvectorError::Config(
                "HNSW M parameter must be between 4 and 128".to_string(),
            ));
        }

        if self.hnsw_ef_construction < 8 {
            return Err(RuvectorError::Config(
                "HNSW ef_construction must be at least 8".to_string(),
            ));
        }

        if self.hnsw_ef_search < 1 {
            return Err(RuvectorError::Config(
                "HNSW ef_search must be at least 1".to_string(),
            ));
        }

        if self.num_tiles == 0 || self.num_tiles > 256 {
            return Err(RuvectorError::Config(
                "Number of tiles must be between 1 and 256".to_string(),
            ));
        }

        if self.min_traces_for_training == 0 {
            return Err(RuvectorError::Config(
                "Minimum traces for training must be greater than 0".to_string(),
            ));
        }

        if self.training_batch_size == 0 {
            return Err(RuvectorError::Config(
                "Training batch size must be greater than 0".to_string(),
            ));
        }

        if self.operation_timeout_ms == 0 {
            return Err(RuvectorError::Config(
                "Operation timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a configuration optimized for production use
    pub fn production() -> Self {
        Self {
            embedding_dimension: 512,
            index_capacity: 1_000_000,
            hnsw_m: 32,
            hnsw_ef_construction: 400,
            hnsw_ef_search: 100,
            num_tiles: 16,
            auto_train_router: true,
            min_traces_for_training: 1000,
            training_batch_size: 64,
            max_concurrent_ops: 10_000,
            operation_timeout_ms: 10_000,
            enable_metrics: true,
        }
    }

    /// Create a configuration optimized for development/testing
    pub fn development() -> Self {
        Self {
            embedding_dimension: 128,
            index_capacity: 10_000,
            hnsw_m: 8,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 25,
            num_tiles: 4,
            auto_train_router: false,
            min_traces_for_training: 10,
            training_batch_size: 8,
            max_concurrent_ops: 100,
            operation_timeout_ms: 1000,
            enable_metrics: true,
        }
    }

    /// Create a configuration optimized for low memory
    pub fn low_memory() -> Self {
        Self {
            embedding_dimension: 128,
            index_capacity: 10_000,
            hnsw_m: 8,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 25,
            num_tiles: 8,
            auto_train_router: false,
            min_traces_for_training: 50,
            training_batch_size: 16,
            max_concurrent_ops: 500,
            operation_timeout_ms: 5000,
            enable_metrics: false,
        }
    }

    /// Estimate memory usage in bytes
    pub fn estimate_memory_bytes(&self) -> usize {
        // Vector storage: capacity * dimension * size_of(f32)
        let vector_memory = self.index_capacity * self.embedding_dimension * 4;

        // HNSW graph overhead: approximately M * capacity * size_of(usize)
        let graph_memory = self.hnsw_m * self.index_capacity * 8;

        // Router model: num_tiles * embedding_dimension * size_of(f32)
        let router_memory = self.num_tiles * self.embedding_dimension * 4;

        vector_memory + graph_memory + router_memory
    }
}

/// Builder for RuvectorConfig
#[derive(Debug, Default)]
pub struct RuvectorConfigBuilder {
    config: RuvectorConfig,
}

impl RuvectorConfigBuilder {
    /// Set embedding dimension
    pub fn embedding_dimension(mut self, dim: usize) -> Self {
        self.config.embedding_dimension = dim;
        self
    }

    /// Set index capacity
    pub fn index_capacity(mut self, capacity: usize) -> Self {
        self.config.index_capacity = capacity;
        self
    }

    /// Set HNSW M parameter
    pub fn hnsw_m(mut self, m: usize) -> Self {
        self.config.hnsw_m = m;
        self
    }

    /// Set HNSW ef_construction parameter
    pub fn hnsw_ef_construction(mut self, ef: usize) -> Self {
        self.config.hnsw_ef_construction = ef;
        self
    }

    /// Set HNSW ef_search parameter
    pub fn hnsw_ef_search(mut self, ef: usize) -> Self {
        self.config.hnsw_ef_search = ef;
        self
    }

    /// Set number of tiles
    pub fn num_tiles(mut self, tiles: usize) -> Self {
        self.config.num_tiles = tiles;
        self
    }

    /// Enable automatic router training
    pub fn auto_train_router(mut self, enable: bool) -> Self {
        self.config.auto_train_router = enable;
        self
    }

    /// Set operation timeout
    pub fn operation_timeout_ms(mut self, timeout: u64) -> Self {
        self.config.operation_timeout_ms = timeout;
        self
    }

    /// Enable metrics collection
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }

    /// Build and validate configuration
    pub fn build(self) -> Result<RuvectorConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = RuvectorConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_production_config_valid() {
        let config = RuvectorConfig::production();
        assert!(config.validate().is_ok());
        assert_eq!(config.embedding_dimension, 512);
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_development_config_valid() {
        let config = RuvectorConfig::development();
        assert!(config.validate().is_ok());
        assert_eq!(config.embedding_dimension, 128);
    }

    #[test]
    fn test_invalid_dimension() {
        let mut config = RuvectorConfig::default();
        config.embedding_dimension = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_tiles() {
        let mut config = RuvectorConfig::default();
        config.num_tiles = 0;
        assert!(config.validate().is_err());

        config.num_tiles = 300;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_builder_pattern() {
        let config = RuvectorConfig::builder()
            .embedding_dimension(512)
            .num_tiles(8)
            .hnsw_m(24)
            .enable_metrics(true)
            .build()
            .unwrap();

        assert_eq!(config.embedding_dimension, 512);
        assert_eq!(config.num_tiles, 8);
        assert_eq!(config.hnsw_m, 24);
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_memory_estimation() {
        let config = RuvectorConfig::default();
        let memory = config.estimate_memory_bytes();

        // Should be reasonable estimate (not zero, not astronomically large)
        assert!(memory > 0);
        assert!(memory < 10_000_000_000); // Less than 10GB
    }
}
