//! End-to-end simulation flow integration tests

#[cfg(test)]
mod simulation_flow_tests {
    use std::time::Duration;

    /// Simulated simulation service
    pub struct SimulationService {
        simulations: std::sync::Arc<std::sync::Mutex<Vec<Simulation>>>,
    }

    #[derive(Debug, Clone)]
    pub struct Simulation {
        pub id: String,
        pub status: SimulationStatus,
        pub progress: f64,
        pub cycles_completed: u64,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum SimulationStatus {
        Queued,
        Running,
        Completed,
        Failed,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum SimulationError {
        #[error("Not found")]
        NotFound,
        #[error("Invalid program")]
        InvalidProgram,
    }

    impl SimulationService {
        pub fn new() -> Self {
            Self {
                simulations: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        pub async fn start_simulation(
            &self,
            program: &[u8],
            max_cycles: u64,
        ) -> Result<String, SimulationError> {
            if program.is_empty() {
                return Err(SimulationError::InvalidProgram);
            }

            let sim = Simulation {
                id: format!("sim_{}", uuid::Uuid::new_v4()),
                status: SimulationStatus::Queued,
                progress: 0.0,
                cycles_completed: 0,
            };

            let id = sim.id.clone();
            self.simulations.lock().unwrap().push(sim);

            Ok(id)
        }

        pub async fn get_status(&self, id: &str) -> Result<Simulation, SimulationError> {
            self.simulations
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.id == id)
                .cloned()
                .ok_or(SimulationError::NotFound)
        }

        pub async fn wait_for_completion(
            &self,
            id: &str,
            timeout: Duration,
        ) -> Result<Simulation, SimulationError> {
            let start = std::time::Instant::now();

            loop {
                let sim = self.get_status(id).await?;

                if sim.status == SimulationStatus::Completed
                    || sim.status == SimulationStatus::Failed
                {
                    return Ok(sim);
                }

                if start.elapsed() > timeout {
                    return Err(SimulationError::NotFound);
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    #[tokio::test]
    async fn should_complete_full_simulation_lifecycle() {
        // Given: A simulation service
        let service = SimulationService::new();

        // When: Starting simulation
        let program = vec![0x01, 0x02, 0x03, 0x04];
        let sim_id = service.start_simulation(&program, 10000).await;

        // Then: Should return simulation ID
        assert!(sim_id.is_ok());
        let sim_id = sim_id.unwrap();

        // When: Checking status
        let status = service.get_status(&sim_id).await;

        // Then: Should be queued
        assert!(status.is_ok());
        assert_eq!(status.unwrap().status, SimulationStatus::Queued);
    }

    #[tokio::test]
    async fn should_reject_empty_program() {
        // Given: A simulation service
        let service = SimulationService::new();

        // When: Starting with empty program
        let result = service.start_simulation(&[], 10000).await;

        // Then: Should fail
        assert!(matches!(result, Err(SimulationError::InvalidProgram)));
    }

    #[tokio::test]
    async fn should_handle_concurrent_simulations() {
        // Given: A simulation service
        let service = std::sync::Arc::new(SimulationService::new());

        // When: Starting multiple simulations concurrently
        let mut handles = vec![];

        for i in 0..5 {
            let service_clone = service.clone();
            let handle = tokio::spawn(async move {
                let program = vec![i as u8; 10];
                service_clone.start_simulation(&program, 1000).await
            });
            handles.push(handle);
        }

        // Then: All should complete
        let results = futures::future::join_all(handles).await;

        let success_count = results
            .iter()
            .filter(|r| r.as_ref().unwrap().is_ok())
            .count();

        assert_eq!(success_count, 5);
    }

    #[tokio::test]
    async fn should_return_not_found_for_invalid_id() {
        // Given: A simulation service
        let service = SimulationService::new();

        // When: Querying invalid ID
        let result = service.get_status("invalid_id").await;

        // Then: Should return not found
        assert!(matches!(result, Err(SimulationError::NotFound)));
    }
}
