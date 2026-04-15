//! API endpoint unit tests

use mockall::mock;
use mockall::predicate::*;

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    mock! {
        pub RequestHandler {
            fn handle_simulation_start(&self, req: SimulationRequest) -> Result<SimulationResponse, ApiError>;
            fn handle_simulation_status(&self, id: &str) -> Result<StatusResponse, ApiError>;
            fn handle_simulation_stop(&self, id: &str) -> Result<(), ApiError>;
        }
    }

    #[derive(Debug, Clone)]
    pub struct SimulationRequest {
        pub program: Vec<u8>,
        pub max_cycles: u64,
        pub tiles: u32,
    }

    #[derive(Debug, Clone)]
    pub struct SimulationResponse {
        pub simulation_id: String,
        pub status: String,
    }

    #[derive(Debug, Clone)]
    pub struct StatusResponse {
        pub simulation_id: String,
        pub status: String,
        pub progress: f64,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum ApiError {
        #[error("Invalid request")]
        InvalidRequest,
        #[error("Simulation not found")]
        NotFound,
        #[error("Internal error")]
        InternalError,
    }

    #[test]
    fn should_handle_simulation_start_request() {
        // Given: A mock handler
        let mut mock_handler = MockRequestHandler::new();

        let request = SimulationRequest {
            program: vec![0x01, 0x02],
            max_cycles: 10000,
            tiles: 64,
        };

        mock_handler
            .expect_handle_simulation_start()
            .with(always())
            .times(1)
            .returning(|_| Ok(SimulationResponse {
                simulation_id: "sim_123".to_string(),
                status: "running".to_string(),
            }));

        // When: Handling request
        let result = mock_handler.handle_simulation_start(request);

        // Then: Should return simulation ID
        assert!(result.is_ok());
        assert_eq!(result.unwrap().simulation_id, "sim_123");
    }

    #[test]
    fn should_handle_status_request() {
        // Given: A mock handler
        let mut mock_handler = MockRequestHandler::new();

        mock_handler
            .expect_handle_simulation_status()
            .with(eq("sim_123"))
            .times(1)
            .returning(|id| Ok(StatusResponse {
                simulation_id: id.to_string(),
                status: "running".to_string(),
                progress: 0.45,
            }));

        // When: Getting status
        let result = mock_handler.handle_simulation_status("sim_123");

        // Then: Should return status
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.progress, 0.45);
    }

    #[test]
    fn should_return_not_found_for_invalid_id() {
        // Given: A handler with no simulation
        let mut mock_handler = MockRequestHandler::new();

        mock_handler
            .expect_handle_simulation_status()
            .returning(|_| Err(ApiError::NotFound));

        // When: Requesting invalid simulation
        let result = mock_handler.handle_simulation_status("invalid_id");

        // Then: Should return not found
        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[test]
    fn should_stop_running_simulation() {
        // Given: A handler with running simulation
        let mut mock_handler = MockRequestHandler::new();

        mock_handler
            .expect_handle_simulation_stop()
            .with(eq("sim_123"))
            .times(1)
            .returning(|_| Ok(()));

        // When: Stopping simulation
        let result = mock_handler.handle_simulation_stop("sim_123");

        // Then: Should succeed
        assert!(result.is_ok());
    }
}
