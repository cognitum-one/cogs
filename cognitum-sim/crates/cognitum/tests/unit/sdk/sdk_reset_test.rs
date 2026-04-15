//! Unit tests for CognitumSDK::reset

use cognitum::sdk::{
    CognitumSDK, InternalState, MockMetricsCollector, MockSimulator, StepResult,
};

#[test]
fn should_delegate_reset_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    sdk.reset();
    // Mock verifies reset was called
}

#[test]
fn should_reset_internal_state() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_step().returning(|| Ok(StepResult::default()));
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();
    sdk.step().unwrap();
    sdk.step().unwrap();

    sdk.reset();

    let state = sdk.get_state();
    assert!(!state.program_loaded);
    assert_eq!(state.current_cycle, 0);
}

#[test]
fn should_reset_metrics_collector() {
    let mut mock_sim = MockSimulator::new();
    mock_sim.expect_reset().times(1).returning(|| ());

    let mut mock_metrics = MockMetricsCollector::new();
    mock_metrics.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock_sim))
        .with_metrics_collector(Box::new(mock_metrics));

    sdk.reset();
}

#[test]
fn should_clear_program_loaded_flag() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    assert!(sdk.get_state().program_loaded);

    sdk.reset();

    assert!(!sdk.get_state().program_loaded);
}

#[test]
fn should_reset_cycle_count() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_step().returning(|| Ok(StepResult::default()));
    mock.expect_get_state().returning(|| {
        let mut state = InternalState::default();
        state.cycle = 0; // Simulator reset its cycle too
        state
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    // Execute some steps
    sdk.step().unwrap();
    sdk.step().unwrap();

    sdk.reset();

    let state = sdk.get_state();
    assert_eq!(state.current_cycle, 0);
}
