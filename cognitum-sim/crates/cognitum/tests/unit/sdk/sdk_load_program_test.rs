//! Unit tests for CognitumSDK::load_program

use cognitum::sdk::{CognitumSDK, Error, MockSimulator, ProgramId, SimulatorError};
use mockall::predicate::*;
use mockall::Sequence;

#[test]
fn should_delegate_to_simulator() {
    // Given: Mock simulator expecting load_program
    let mut mock = MockSimulator::new();
    mock.expect_load_program()
        .with(eq(vec![0x01, 0x02, 0x03]))
        .times(1)
        .returning(|_| Ok(ProgramId(1)));

    mock.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    // When: Loading program
    let result = sdk.load_program(&[0x01, 0x02, 0x03]);

    // Then: Succeeds (mock verifies delegation)
    assert!(result.is_ok());
}

#[test]
fn should_validate_program_before_loading() {
    // Given: Mock that should NOT be called for empty program
    let mut mock = MockSimulator::new();
    mock.expect_load_program().times(0);
    mock.expect_reset().times(0);

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    // When: Loading empty program
    let result = sdk.load_program(&[]);

    // Then: Fails with validation error (not simulator error)
    assert!(matches!(result, Err(Error::EmptyProgram)));
}

#[test]
fn should_propagate_simulator_errors() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().times(1).returning(|| ());
    mock.expect_load_program()
        .returning(|_| Err(SimulatorError::InvalidProgram("bad magic".into())));

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let result = sdk.load_program(&[0x01, 0x02]);

    assert!(matches!(result, Err(Error::Simulator(_))));
}

#[test]
fn should_reset_state_before_loading_new_program() {
    let mut mock = MockSimulator::new();

    // Expect reset THEN load_program
    let mut seq = Sequence::new();
    mock.expect_reset()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| ());
    mock.expect_load_program()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(ProgramId(1)));

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.load_program(&[0x01]).unwrap();
}

#[test]
fn should_update_internal_state_on_successful_load() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_load_program().returning(|_| Ok(ProgramId(1)));
    mock.expect_get_state()
        .returning(|| cognitum::sdk::InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    // Before load
    let state1 = sdk.get_state();
    assert!(!state1.program_loaded);

    // Load program
    sdk.load_program(&[0x01, 0x02, 0x03]).unwrap();

    // After load
    let state2 = sdk.get_state();
    assert!(state2.program_loaded);
    assert_eq!(state2.program_size, 3);
}
