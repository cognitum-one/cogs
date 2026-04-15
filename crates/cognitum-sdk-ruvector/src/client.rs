//! High-level async client for Ruvector operations

use crate::config::RuvectorConfig;
use crate::error::{Result, RuvectorError};
use crate::types::*;
use cognitum::ruvector::{
    DefaultEmbeddingGenerator, Embedding, EmbeddingGenerator, EmbeddingId, ExecutionTrace,
    HnswVectorIndex, IndexStats, Metadata, SearchResult, TaskEmbedding, TaskRouter, TileId,
    TileState, TinyDancerRouter, TrainingMetrics, VectorIndex,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// High-level async client for Ruvector operations
pub struct RuvectorClient {
    /// Client configuration
    config: RuvectorConfig,

    /// Vector index for similarity search
    index: Arc<RwLock<HnswVectorIndex>>,

    /// Neural router for task routing
    router: Arc<RwLock<TinyDancerRouter>>,

    /// Embedding generator
    generator: Arc<DefaultEmbeddingGenerator>,

    /// Execution traces for training
    traces: Arc<RwLock<Vec<ExecutionTrace>>>,

    /// Vector operation statistics
    vector_stats: Arc<RwLock<VectorStats>>,

    /// Router statistics
    router_stats: Arc<RwLock<RouterStats>>,

    /// Client creation timestamp
    created_at: Instant,
}

impl RuvectorClient {
    /// Create a new builder for configuring the client
    pub fn builder() -> RuvectorClientBuilder {
        RuvectorClientBuilder::default()
    }

    /// Create a client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(RuvectorConfig::default())
    }

    /// Create a client with custom configuration
    pub fn with_config(config: RuvectorConfig) -> Result<Self> {
        config.validate()?;

        let index = Arc::new(RwLock::new(HnswVectorIndex::with_config(
            config.embedding_dimension,
            cognitum::ruvector::HnswConfig {
                m: config.hnsw_m,
                ef_construction: config.hnsw_ef_construction,
                ef_search: config.hnsw_ef_search,
            },
        )));

        let router = Arc::new(RwLock::new(TinyDancerRouter::new(
            config.num_tiles,
            config.embedding_dimension,
        )));

        let generator = Arc::new(DefaultEmbeddingGenerator::new(config.embedding_dimension));

        Ok(Self {
            config,
            index,
            router,
            generator,
            traces: Arc::new(RwLock::new(Vec::new())),
            vector_stats: Arc::new(RwLock::new(VectorStats::default())),
            router_stats: Arc::new(RwLock::new(RouterStats::default())),
            created_at: Instant::now(),
        })
    }

    /// Insert an embedding into the index
    pub async fn insert(
        &self,
        id: u64,
        embedding: &[f32],
        metadata: Metadata,
    ) -> Result<()> {
        let start = Instant::now();

        let embedding_obj = self.validate_embedding(embedding)?;

        let result = timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let index = self.index.clone();
                let embedding_obj = embedding_obj.clone();
                let metadata = metadata.clone();
                move || {
                    let mut idx = index.write();
                    idx.insert(EmbeddingId(id), &embedding_obj, &metadata)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        let elapsed = start.elapsed().as_micros() as f64;
        self.update_insert_stats(elapsed);

        Ok(result)
    }

    /// Search for k nearest neighbors
    pub async fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let start = Instant::now();

        let query_obj = self.validate_embedding(query)?;

        let results = timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let index = self.index.clone();
                let query_obj = query_obj.clone();
                move || {
                    let idx = index.read();
                    idx.search(&query_obj, k)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        let elapsed = start.elapsed().as_micros() as f64;
        self.update_search_stats(elapsed);

        Ok(results)
    }

    /// Delete an embedding by ID
    pub async fn delete(&self, id: u64) -> Result<()> {
        timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let index = self.index.clone();
                move || {
                    let mut idx = index.write();
                    idx.delete(EmbeddingId(id))
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        let mut stats = self.vector_stats.write();
        stats.total_deletions += 1;

        Ok(())
    }

    /// Predict optimal tile for task execution
    pub async fn predict_tile(&self, task_embedding: &[f32]) -> Result<TileId> {
        let start = Instant::now();

        let task_obj = self.validate_task_embedding(task_embedding)?;

        let tile_id = timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let router = self.router.clone();
                let task_obj = task_obj.clone();
                move || {
                    let rtr = router.read();
                    rtr.predict_tile(&task_obj)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))?;

        let elapsed = start.elapsed().as_micros() as f64;
        self.update_router_stats(elapsed);

        Ok(tile_id)
    }

    /// Get routing confidence score
    pub async fn routing_confidence(&self, task_embedding: &[f32]) -> Result<f32> {
        let task_obj = self.validate_task_embedding(task_embedding)?;

        timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let router = self.router.clone();
                let task_obj = task_obj.clone();
                move || {
                    let rtr = router.read();
                    rtr.confidence(&task_obj)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))
    }

    /// Record execution trace for training
    pub fn record_trace(&self, trace: ExecutionTrace) {
        let mut traces = self.traces.write();
        traces.push(trace);

        // Auto-train if enabled and enough traces collected
        if self.config.auto_train_router && traces.len() >= self.config.min_traces_for_training {
            // Training will happen on next train() call
        }
    }

    /// Train router from collected execution traces
    pub async fn train_router(&self) -> Result<TrainingMetrics> {
        let traces = {
            let t = self.traces.read();
            t.clone()
        };

        if traces.is_empty() {
            return Err(RuvectorError::ModelNotTrained(
                "No execution traces available".to_string(),
            ));
        }

        let metrics = timeout(
            Duration::from_millis(self.config.operation_timeout_ms * 10), // Longer timeout for training
            tokio::task::spawn_blocking({
                let router = self.router.clone();
                move || {
                    let mut rtr = router.write();
                    rtr.train(&traces)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms * 10))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        let mut stats = self.router_stats.write();
        stats.total_training_iterations += 1;
        stats.current_accuracy = metrics.accuracy as f64;
        stats.last_training_timestamp = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );

        Ok(metrics)
    }

    /// Load pre-trained router model
    pub async fn load_router_model(&self, path: &Path) -> Result<()> {
        timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let router = self.router.clone();
                let path = path.to_path_buf();
                move || {
                    let mut rtr = router.write();
                    rtr.load_model(&path)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Save router model
    pub async fn save_router_model(&self, path: &Path) -> Result<()> {
        timeout(
            Duration::from_millis(self.config.operation_timeout_ms),
            tokio::task::spawn_blocking({
                let router = self.router.clone();
                let path = path.to_path_buf();
                move || {
                    let rtr = router.read();
                    rtr.save_model(&path)
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Generate embedding from tile state
    pub fn embedding_from_tile_state(&self, state: &TileState) -> Embedding {
        self.generator.from_tile_state(state)
    }

    /// Get index statistics
    pub fn index_stats(&self) -> IndexStats {
        let idx = self.index.read();
        idx.stats()
    }

    /// Get vector operation statistics
    pub fn vector_stats(&self) -> VectorStats {
        self.vector_stats.read().clone()
    }

    /// Get router statistics
    pub fn router_stats(&self) -> RouterStats {
        self.router_stats.read().clone()
    }

    /// Get client health information
    pub fn health(&self) -> HealthInfo {
        let index_stats = self.index_stats();
        let router_stats = self.router_stats();

        let memory_usage = self.config.estimate_memory_bytes();
        let uptime = self.created_at.elapsed().as_secs();

        let router_trained = router_stats.last_training_timestamp.is_some();

        let mut errors = Vec::new();
        let mut status = HealthStatus::Healthy;

        // Check index utilization
        let utilization = index_stats.num_vectors as f64 / self.config.index_capacity as f64;
        if utilization > 0.9 {
            errors.push("Index near capacity".to_string());
            status = HealthStatus::Degraded;
        }

        // Check router training
        if !router_trained && self.config.auto_train_router {
            errors.push("Router not yet trained".to_string());
            if status == HealthStatus::Healthy {
                status = HealthStatus::Degraded;
            }
        }

        HealthInfo {
            status,
            index_healthy: true,
            router_healthy: true,
            router_trained,
            memory_usage_bytes: memory_usage,
            uptime_seconds: uptime,
            errors,
        }
    }

    /// Optimize index for better performance
    pub async fn optimize_index(&self) -> Result<()> {
        timeout(
            Duration::from_millis(self.config.operation_timeout_ms * 5), // Longer timeout
            tokio::task::spawn_blocking({
                let index = self.index.clone();
                move || {
                    let mut idx = index.write();
                    idx.optimize()
                }
            }),
        )
        .await
        .map_err(|_| RuvectorError::Timeout(self.config.operation_timeout_ms * 5))?
        .map_err(|e| RuvectorError::Internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Clear all collected execution traces
    pub fn clear_traces(&self) {
        let mut traces = self.traces.write();
        traces.clear();
    }

    /// Get number of collected traces
    pub fn trace_count(&self) -> usize {
        self.traces.read().len()
    }

    // Private helper methods

    fn validate_embedding(&self, data: &[f32]) -> Result<Embedding> {
        if data.len() != self.config.embedding_dimension {
            return Err(RuvectorError::InvalidDimension {
                expected: self.config.embedding_dimension,
                actual: data.len(),
            });
        }
        Ok(Embedding::new(data.to_vec()))
    }

    fn validate_task_embedding(&self, data: &[f32]) -> Result<TaskEmbedding> {
        if data.len() != self.config.embedding_dimension {
            return Err(RuvectorError::InvalidDimension {
                expected: self.config.embedding_dimension,
                actual: data.len(),
            });
        }
        Ok(TaskEmbedding::new(data.to_vec()))
    }

    fn update_insert_stats(&self, elapsed_us: f64) {
        let mut stats = self.vector_stats.write();
        stats.total_insertions += 1;

        // Update running average
        let n = stats.total_insertions as f64;
        stats.avg_insert_time_us = ((n - 1.0) * stats.avg_insert_time_us + elapsed_us) / n;

        // Update utilization
        let index_stats = self.index_stats();
        stats.current_size = index_stats.num_vectors;
        stats.utilization = stats.current_size as f64 / self.config.index_capacity as f64;
    }

    fn update_search_stats(&self, elapsed_us: f64) {
        let mut stats = self.vector_stats.write();
        stats.total_searches += 1;

        // Update running average
        let n = stats.total_searches as f64;
        stats.avg_search_time_us = ((n - 1.0) * stats.avg_search_time_us + elapsed_us) / n;
    }

    fn update_router_stats(&self, elapsed_us: f64) {
        let mut stats = self.router_stats.write();
        stats.total_predictions += 1;

        // Update running average
        let n = stats.total_predictions as f64;
        stats.avg_prediction_time_us = ((n - 1.0) * stats.avg_prediction_time_us + elapsed_us) / n;

        stats.traces_collected = self.trace_count();
    }
}

/// Builder for RuvectorClient
#[derive(Debug)]
pub struct RuvectorClientBuilder {
    config: RuvectorConfig,
}

impl Default for RuvectorClientBuilder {
    fn default() -> Self {
        Self {
            config: RuvectorConfig::default(),
        }
    }
}

impl RuvectorClientBuilder {
    /// Set embedding dimension
    pub fn embedding_dimension(mut self, dim: usize) -> Self {
        self.config.embedding_dimension = dim;
        self
    }

    /// Set number of tiles
    pub fn num_tiles(mut self, tiles: usize) -> Self {
        self.config.num_tiles = tiles;
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

    /// Enable automatic router training
    pub fn auto_train_router(mut self, enable: bool) -> Self {
        self.config.auto_train_router = enable;
        self
    }

    /// Set custom configuration
    pub fn config(mut self, config: RuvectorConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the client
    pub fn build(self) -> Result<RuvectorClient> {
        RuvectorClient::with_config(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = RuvectorClient::builder()
            .embedding_dimension(128)
            .num_tiles(4)
            .build()
            .unwrap();

        assert_eq!(client.config.embedding_dimension, 128);
        assert_eq!(client.config.num_tiles, 4);
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let client = RuvectorClient::builder()
            .embedding_dimension(128)
            .build()
            .unwrap();

        let embedding = vec![0.1; 128];
        client
            .insert(1, &embedding, Metadata::default())
            .await
            .unwrap();

        let results = client.search(&embedding, 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, EmbeddingId(1));
    }

    #[tokio::test]
    async fn test_dimension_validation() {
        let client = RuvectorClient::builder()
            .embedding_dimension(128)
            .build()
            .unwrap();

        let wrong_embedding = vec![0.1; 64];
        let result = client
            .insert(1, &wrong_embedding, Metadata::default())
            .await;

        assert!(matches!(result, Err(RuvectorError::InvalidDimension { .. })));
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let client = RuvectorClient::builder()
            .embedding_dimension(128)
            .build()
            .unwrap();

        let embedding = vec![0.1; 128];
        client
            .insert(1, &embedding, Metadata::default())
            .await
            .unwrap();

        let stats = client.vector_stats();
        assert_eq!(stats.total_insertions, 1);
        assert!(stats.avg_insert_time_us > 0.0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = RuvectorClient::new().unwrap();
        let health = client.health();

        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.index_healthy);
        assert!(health.router_healthy);
    }
}
