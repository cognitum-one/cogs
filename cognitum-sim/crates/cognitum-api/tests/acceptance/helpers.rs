//! Test helpers for acceptance tests

use cognitum_api::models::AppState;
use cognitum_api::services::*;
use std::sync::Arc;

#[allow(dead_code)]
pub fn create_test_state() -> AppState {
    AppState::new(
        Arc::new(MockSimulatorServiceImpl),
        Arc::new(MockStorageServiceImpl),
        Arc::new(MockAuthServiceImpl),
        Arc::new(MockRateLimiterImpl),
    )
}

// Mock implementations for testing
struct MockSimulatorServiceImpl;
struct MockStorageServiceImpl;
struct MockAuthServiceImpl;
struct MockRateLimiterImpl;

#[async_trait::async_trait]
impl SimulatorService for MockSimulatorServiceImpl {
    async fn create_simulation(
        &self,
        _config: cognitum_api::models::request::SimulationConfig,
        _program_id: String,
        _user_id: String,
    ) -> Result<String, cognitum_api::models::error::ServiceError> {
        Ok("sim_test123".to_string())
    }

    async fn get_simulation(
        &self,
        id: String,
    ) -> Result<cognitum_api::models::response::SimulationResponse, cognitum_api::models::error::ServiceError> {
        if id.starts_with("sim_") {
            Ok(cognitum_api::models::response::SimulationResponse {
                id,
                status: "created".to_string(),
                config: cognitum_api::models::request::SimulationConfig::default(),
                owner: "test_user".to_string(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        } else {
            Err(cognitum_api::models::error::ServiceError::SimulationNotFound)
        }
    }

    async fn list_simulations(
        &self,
        _user_id: String,
        _pagination: cognitum_api::models::request::Pagination,
    ) -> Result<Vec<cognitum_api::models::response::SimulationResponse>, cognitum_api::models::error::ServiceError> {
        Ok(vec![])
    }

    async fn run_simulation(
        &self,
        _id: String,
        _options: cognitum_api::models::request::RunRequest,
    ) -> Result<String, cognitum_api::models::error::ServiceError> {
        Ok("job_test123".to_string())
    }

    async fn get_status(
        &self,
        id: String,
    ) -> Result<cognitum_api::models::response::SimulationStatusResponse, cognitum_api::models::error::ServiceError> {
        Ok(cognitum_api::models::response::SimulationStatusResponse {
            simulation_id: id,
            status: "running".to_string(),
            progress: Some(0.5),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn get_results(
        &self,
        _id: String,
    ) -> Result<cognitum_api::models::response::ExecutionResults, cognitum_api::models::error::ServiceError> {
        Ok(cognitum_api::models::response::ExecutionResults {
            cycles_executed: 10000,
            instructions_executed: 45000,
            tiles_used: 16,
            memory_bytes_accessed: 1024000,
            execution_time_ms: 1523,
            exit_reason: "cycle_limit".to_string(),
        })
    }

    async fn delete_simulation(&self, _id: String) -> Result<(), cognitum_api::models::error::ServiceError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageService for MockStorageServiceImpl {
    async fn store_program(
        &self,
        _data: &[u8],
        metadata: cognitum_api::models::request::ProgramMetadata,
        _user_id: String,
    ) -> Result<String, cognitum_api::models::error::ServiceError> {
        Ok(format!("prog_{}", uuid::Uuid::new_v4()))
    }

    async fn get_program(
        &self,
        id: String,
    ) -> Result<(Vec<u8>, cognitum_api::models::request::ProgramMetadata), cognitum_api::models::error::ServiceError> {
        if id.starts_with("prog_") {
            Ok((
                vec![0x01, 0x02, 0x03],
                cognitum_api::models::request::ProgramMetadata::default(),
            ))
        } else {
            Err(cognitum_api::models::error::ServiceError::ProgramNotFound)
        }
    }

    async fn delete_program(&self, _id: String) -> Result<(), cognitum_api::models::error::ServiceError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AuthService for MockAuthServiceImpl {
    async fn validate_api_key(&self, key: &str) -> Result<AuthenticatedUser, cognitum_api::models::error::AuthError> {
        if key == "sk_test_xxx" {
            Ok(AuthenticatedUser {
                user_id: "test_user".to_string(),
                api_key: key.to_string(),
                tier: UserTier::Free,
            })
        } else {
            Err(cognitum_api::models::error::AuthError::InvalidApiKey)
        }
    }

    async fn check_permission(&self, _user: &AuthenticatedUser, _action: Action) -> Result<bool, cognitum_api::models::error::AuthError> {
        Ok(true)
    }
}

#[async_trait::async_trait]
impl RateLimiter for MockRateLimiterImpl {
    async fn check(&self, _key: &str) -> Result<RateLimitResult, cognitum_api::models::error::RateLimitError> {
        Ok(RateLimitResult::Allowed { remaining: 99 })
    }

    async fn record(&self, _key: &str) -> Result<(), cognitum_api::models::error::RateLimitError> {
        Ok(())
    }
}
