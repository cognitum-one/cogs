//! Executor modules for different capability types.
//!
//! Each executor handles operations for a specific capability type,
//! validating scope and performing the actual external effects.

pub mod filesystem;
pub mod network;
pub mod secrets;

use crate::error::ExecutorError;
use crate::types::{Capability, InvokeRequest, OperationResult, QuotaConsumed};
use async_trait::async_trait;
use std::sync::Arc;

/// Result of an executor operation
#[derive(Debug, Clone)]
pub struct ExecutorResult {
    /// The operation result data
    pub data: OperationResult,
    /// Quota consumed by this operation
    pub quota_consumed: QuotaConsumed,
}

impl ExecutorResult {
    /// Create a new executor result
    pub fn new(data: OperationResult, quota_consumed: QuotaConsumed) -> Self {
        Self {
            data,
            quota_consumed,
        }
    }

    /// Create a result for a simple operation (1 invocation)
    pub fn simple(data: OperationResult, bytes: u64, duration_ns: u64) -> Self {
        Self {
            data,
            quota_consumed: QuotaConsumed::single(bytes, duration_ns),
        }
    }
}

/// Trait for capability executors
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute an operation with the given capability
    async fn execute(
        &self,
        capability: &Capability,
        request: &InvokeRequest,
    ) -> Result<ExecutorResult, ExecutorError>;

    /// Check if this executor can handle the given capability type
    fn can_handle(&self, capability: &Capability) -> bool;

    /// Get the name of this executor
    fn name(&self) -> &'static str;
}

/// Mock executor for testing
#[cfg(any(test, feature = "mock"))]
pub struct MockExecutor {
    /// Name of this mock
    pub name: &'static str,
    /// Result to return
    pub result: Option<ExecutorResult>,
    /// Error to return
    pub error: Option<ExecutorError>,
}

#[cfg(any(test, feature = "mock"))]
impl MockExecutor {
    /// Create a mock that returns success
    pub fn success(name: &'static str, result: ExecutorResult) -> Self {
        Self {
            name,
            result: Some(result),
            error: None,
        }
    }

    /// Create a mock that returns an error
    pub fn failure(name: &'static str, error: ExecutorError) -> Self {
        Self {
            name,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(any(test, feature = "mock"))]
#[async_trait]
impl Executor for MockExecutor {
    async fn execute(
        &self,
        _capability: &Capability,
        _request: &InvokeRequest,
    ) -> Result<ExecutorResult, ExecutorError> {
        if let Some(ref err) = self.error {
            return Err(ExecutorError::Internal(err.to_string()));
        }
        Ok(self.result.clone().unwrap())
    }

    fn can_handle(&self, _capability: &Capability) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

// Re-exports
pub use filesystem::LocalFilesystemExecutor;
pub use network::HttpNetworkExecutor;
pub use secrets::{EnvSecretsProvider, SecretsProvider};
