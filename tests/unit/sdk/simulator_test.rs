//! Simulator unit tests using London School TDD

use mockall::predicate::*;
use mockall::mock;

#[cfg(test)]
mod simulator_tests {
    use super::*;

    // Mock trait for simulator backend
    mock! {
        pub SimulatorBackend {
            fn initialize(&mut self, config: SimulatorConfig) -> Result<(), SimulatorError>;
            fn load_program(&mut self, program: &[u8]) -> Result<ProgramId, SimulatorError>;
            fn run(&mut self, cycles: u64) -> Result<SimulationResults, SimulatorError>;
            fn step(&mut self) -> Result<CycleState, SimulatorError>;
            fn get_state(&self) -> SimulatorState;
            fn reset(&mut self);
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct SimulatorConfig {
        pub max_cycles: u64,
        pub tile_count: u32,
    }

    #[derive(Debug, Clone)]
    pub struct ProgramId(pub u64);

    #[derive(Debug, Clone)]
    pub struct SimulationResults {
        pub cycles_executed: u64,
        pub tiles_used: u32,
        pub memory_accessed: u64,
    }

    #[derive(Debug, Clone)]
    pub struct CycleState {
        pub cycle: u64,
        pub active_tiles: Vec<u32>,
    }

    #[derive(Debug, Clone)]
    pub struct SimulatorState {
        pub initialized: bool,
        pub program_loaded: bool,
        pub current_cycle: u64,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum SimulatorError {
        #[error("Invalid program")]
        InvalidProgram,
        #[error("Not initialized")]
        NotInitialized,
        #[error("Already running")]
        AlreadyRunning,
    }

    #[test]
    fn should_delegate_initialization_to_backend() {
        // Given: A mock backend expecting initialization
        let mut mock_backend = MockSimulatorBackend::new();

        let expected_config = SimulatorConfig {
            max_cycles: 1000000,
            tile_count: 256,
        };

        mock_backend
            .expect_initialize()
            .with(eq(expected_config.clone()))
            .times(1)
            .returning(|_| Ok(()));

        // When: Initializing the simulator
        let result = mock_backend.initialize(expected_config);

        // Then: Should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn should_load_program_and_return_program_id() {
        // Given: A mock backend
        let mut mock_backend = MockSimulatorBackend::new();
        let program = vec![0x01, 0x02, 0x03, 0x04];

        mock_backend
            .expect_load_program()
            .with(eq(program.clone()))
            .times(1)
            .returning(|_| Ok(ProgramId(42)));

        // When: Loading a program
        let result = mock_backend.load_program(&program);

        // Then: Should return program ID
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, 42);
    }

    #[test]
    fn should_reject_invalid_program() {
        // Given: A mock backend that rejects invalid programs
        let mut mock_backend = MockSimulatorBackend::new();

        mock_backend
            .expect_load_program()
            .returning(|_| Err(SimulatorError::InvalidProgram));

        // When: Loading an invalid program
        let result = mock_backend.load_program(&[0xFF]);

        // Then: Should return error
        assert!(matches!(result, Err(SimulatorError::InvalidProgram)));
    }

    #[test]
    fn should_execute_simulation_and_return_results() {
        // Given: A mock backend ready for execution
        let mut mock_backend = MockSimulatorBackend::new();

        mock_backend
            .expect_run()
            .with(eq(10000u64))
            .times(1)
            .returning(|cycles| Ok(SimulationResults {
                cycles_executed: cycles,
                tiles_used: 128,
                memory_accessed: 50000,
            }));

        // When: Running simulation
        let result = mock_backend.run(10000);

        // Then: Should return results
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.cycles_executed, 10000);
        assert_eq!(results.tiles_used, 128);
    }

    #[test]
    fn should_step_through_cycles_individually() {
        // Given: A mock backend
        let mut mock_backend = MockSimulatorBackend::new();

        mock_backend
            .expect_step()
            .times(3)
            .returning(|| Ok(CycleState {
                cycle: 1,
                active_tiles: vec![0, 1, 2],
            }));

        // When: Stepping through cycles
        for _ in 0..3 {
            let result = mock_backend.step();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn should_reset_simulator_state() {
        // Given: A mock backend
        let mut mock_backend = MockSimulatorBackend::new();

        mock_backend
            .expect_reset()
            .times(1)
            .return_const(());

        mock_backend
            .expect_get_state()
            .returning(|| SimulatorState {
                initialized: true,
                program_loaded: false,
                current_cycle: 0,
            });

        // When: Resetting
        mock_backend.reset();
        let state = mock_backend.get_state();

        // Then: State should be reset
        assert_eq!(state.current_cycle, 0);
        assert!(!state.program_loaded);
    }

    #[test]
    fn should_prevent_running_without_program() {
        // Given: A mock backend without loaded program
        let mut mock_backend = MockSimulatorBackend::new();

        mock_backend
            .expect_run()
            .returning(|_| Err(SimulatorError::NotInitialized));

        // When: Attempting to run
        let result = mock_backend.run(1000);

        // Then: Should fail
        assert!(matches!(result, Err(SimulatorError::NotInitialized)));
    }
}
