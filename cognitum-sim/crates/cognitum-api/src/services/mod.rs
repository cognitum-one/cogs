//! Service trait definitions

use async_trait::async_trait;
use crate::models::{
    error::{AuthError, RateLimitError, ServiceError},
    request::{Pagination, ProgramMetadata, RunRequest, SimulationConfig},
    response::{ExecutionResults, SimulationResponse, SimulationStatusResponse},
    JobId, ProgramId, SimulationId, UserId,
};
use std::time::Duration;

#[cfg(test)]
use mockall::automock;

/// Simulation service trait
#[cfg_attr(test, automock)]
#[async_trait]
pub trait SimulatorService: Send + Sync {
    /// Create a new simulation instance
    async fn create_simulation(
        &self,
        config: SimulationConfig,
        program_id: ProgramId,
        user_id: UserId,
    ) -> Result<SimulationId, ServiceError>;

    /// Get simulation by ID
    async fn get_simulation(&self, id: SimulationId) -> Result<SimulationResponse, ServiceError>;

    /// List simulations for user
    async fn list_simulations(
        &self,
        user_id: UserId,
        pagination: Pagination,
    ) -> Result<Vec<SimulationResponse>, ServiceError>;

    /// Start simulation execution
    async fn run_simulation(
        &self,
        id: SimulationId,
        options: RunRequest,
    ) -> Result<JobId, ServiceError>;

    /// Get simulation status
    async fn get_status(&self, id: SimulationId) -> Result<SimulationStatusResponse, ServiceError>;

    /// Get simulation results
    async fn get_results(&self, id: SimulationId) -> Result<ExecutionResults, ServiceError>;

    /// Delete simulation
    async fn delete_simulation(&self, id: SimulationId) -> Result<(), ServiceError>;
}

/// Storage service trait
#[cfg_attr(test, automock)]
#[async_trait]
pub trait StorageService: Send + Sync {
    /// Store program binary
    async fn store_program(
        &self,
        data: &[u8],
        metadata: ProgramMetadata,
        user_id: UserId,
    ) -> Result<ProgramId, ServiceError>;

    /// Retrieve program binary
    async fn get_program(&self, id: ProgramId) -> Result<(Vec<u8>, ProgramMetadata), ServiceError>;

    /// Delete program
    async fn delete_program(&self, id: ProgramId) -> Result<(), ServiceError>;
}

/// Authentication service trait
#[cfg_attr(test, automock)]
#[async_trait]
pub trait AuthService: Send + Sync {
    /// Validate API key and return user info
    async fn validate_api_key(&self, key: &str) -> Result<AuthenticatedUser, AuthError>;

    /// Check if user has permission for action
    async fn check_permission(&self, user: &AuthenticatedUser, action: Action) -> Result<bool, AuthError>;
}

/// Rate limiter trait
#[cfg_attr(test, automock)]
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Check if request is allowed
    async fn check(&self, key: &str) -> Result<RateLimitResult, RateLimitError>;

    /// Record request
    async fn record(&self, key: &str) -> Result<(), RateLimitError>;
}

/// Authenticated user information
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub api_key: String,
    pub tier: UserTier,
}

impl Default for AuthenticatedUser {
    fn default() -> Self {
        Self {
            user_id: "test_user".to_string(),
            api_key: "sk_test_xxx".to_string(),
            tier: UserTier::Free,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UserTier {
    Free,
    Pro,
    Enterprise,
}

#[derive(Debug, Clone)]
pub enum Action {
    CreateSimulation,
    RunSimulation,
    DeleteSimulation,
    UploadProgram,
}

#[derive(Debug, Clone)]
pub enum RateLimitResult {
    Allowed { remaining: u32 },
    Exceeded { retry_after: Duration },
}
