//! Additional SDK-specific types and utilities

use serde::{Deserialize, Serialize};

/// Statistics for vector operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStats {
    /// Total number of insertions
    pub total_insertions: u64,

    /// Total number of searches
    pub total_searches: u64,

    /// Total number of deletions
    pub total_deletions: u64,

    /// Average search time in microseconds
    pub avg_search_time_us: f64,

    /// Average insertion time in microseconds
    pub avg_insert_time_us: f64,

    /// Current index size (number of vectors)
    pub current_size: usize,

    /// Index capacity utilization (0.0 to 1.0)
    pub utilization: f64,
}

impl Default for VectorStats {
    fn default() -> Self {
        Self {
            total_insertions: 0,
            total_searches: 0,
            total_deletions: 0,
            avg_search_time_us: 0.0,
            avg_insert_time_us: 0.0,
            current_size: 0,
            utilization: 0.0,
        }
    }
}

/// Statistics for neural routing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterStats {
    /// Total number of routing predictions
    pub total_predictions: u64,

    /// Total number of training iterations
    pub total_training_iterations: u64,

    /// Current model accuracy (0.0 to 1.0)
    pub current_accuracy: f64,

    /// Average prediction confidence
    pub avg_confidence: f64,

    /// Average prediction time in microseconds
    pub avg_prediction_time_us: f64,

    /// Number of execution traces collected
    pub traces_collected: usize,

    /// Last training timestamp (Unix epoch)
    pub last_training_timestamp: Option<u64>,
}

impl Default for RouterStats {
    fn default() -> Self {
        Self {
            total_predictions: 0,
            total_training_iterations: 0,
            current_accuracy: 0.0,
            avg_confidence: 0.0,
            avg_prediction_time_us: 0.0,
            traces_collected: 0,
            last_training_timestamp: None,
        }
    }
}

/// Overall client health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,

    /// Some degradation but functional
    Degraded,

    /// Critical errors present
    Unhealthy,
}

/// Comprehensive client health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    /// Overall health status
    pub status: HealthStatus,

    /// Vector index health
    pub index_healthy: bool,

    /// Router health
    pub router_healthy: bool,

    /// Router trained status
    pub router_trained: bool,

    /// Memory usage in bytes
    pub memory_usage_bytes: usize,

    /// Uptime in seconds
    pub uptime_seconds: u64,

    /// Error messages if any
    pub errors: Vec<String>,
}

impl Default for HealthInfo {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            index_healthy: true,
            router_healthy: true,
            router_trained: false,
            memory_usage_bytes: 0,
            uptime_seconds: 0,
            errors: Vec::new(),
        }
    }
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// Successful results
    pub successes: Vec<T>,

    /// Failed operations with error messages
    pub failures: Vec<(usize, String)>,

    /// Total operations attempted
    pub total: usize,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}

impl<T> BatchResult<T> {
    /// Create a new batch result
    pub fn new(successes: Vec<T>, failures: Vec<(usize, String)>, total: usize) -> Self {
        let success_rate = if total > 0 {
            successes.len() as f64 / total as f64
        } else {
            0.0
        };

        Self {
            successes,
            failures,
            total,
            success_rate,
        }
    }

    /// Check if all operations succeeded
    pub fn is_complete_success(&self) -> bool {
        self.failures.is_empty()
    }

    /// Get number of successful operations
    pub fn success_count(&self) -> usize {
        self.successes.len()
    }

    /// Get number of failed operations
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }
}

/// Configuration for batch operations
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_batch_size: usize,

    /// Continue on error
    pub continue_on_error: bool,

    /// Timeout per operation in milliseconds
    pub operation_timeout_ms: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            continue_on_error: true,
            operation_timeout_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_result_success_rate() {
        let result = BatchResult::new(
            vec![1, 2, 3],
            vec![(3, "error".to_string())],
            4,
        );

        assert_eq!(result.success_count(), 3);
        assert_eq!(result.failure_count(), 1);
        assert_eq!(result.success_rate, 0.75);
        assert!(!result.is_complete_success());
    }

    #[test]
    fn test_batch_result_complete_success() {
        let result: BatchResult<i32> = BatchResult::new(
            vec![1, 2, 3],
            vec![],
            3,
        );

        assert!(result.is_complete_success());
        assert_eq!(result.success_rate, 1.0);
    }

    #[test]
    fn test_health_status_default() {
        let health = HealthInfo::default();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.index_healthy);
        assert!(health.router_healthy);
    }

    #[test]
    fn test_stats_default() {
        let stats = VectorStats::default();
        assert_eq!(stats.total_insertions, 0);
        assert_eq!(stats.current_size, 0);

        let router_stats = RouterStats::default();
        assert_eq!(router_stats.total_predictions, 0);
    }
}
