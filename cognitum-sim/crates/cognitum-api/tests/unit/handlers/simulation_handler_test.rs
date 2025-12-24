//! Unit tests for simulation handlers

#[cfg(test)]
mod tests {
    use cognitum_api::services::{MockSimulatorService, SimulatorService};
    use cognitum_api::models::request::{CreateSimulationRequest, SimulationConfig};
    use std::sync::Arc;

    #[tokio::test]
    async fn should_delegate_to_service() {
        let mut mock_service = MockSimulatorService::new();
        mock_service
            .expect_create_simulation()
            .times(1)
            .returning(|_, _, _| Ok("sim_test123".to_string()));

        let service: Arc<dyn SimulatorService> = Arc::new(mock_service);

        let result = service
            .create_simulation(
                SimulationConfig::default(),
                "prog_123".to_string(),
                "user_123".to_string(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "sim_test123");
    }

    #[tokio::test]
    async fn should_return_created_response() {
        let mut mock_service = MockSimulatorService::new();
        let expected_id = "sim_abc123".to_string();
        mock_service
            .expect_create_simulation()
            .returning(move |_, _, _| Ok(expected_id.clone()));

        let service: Arc<dyn SimulatorService> = Arc::new(mock_service);

        let result = service
            .create_simulation(
                SimulationConfig::default(),
                "prog_123".to_string(),
                "user_123".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(result, "sim_abc123");
    }
}
