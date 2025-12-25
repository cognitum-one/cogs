//! Unit tests for CognitumSDK::step

use cognitum::sdk::{
    CognitumSDK, Error, MockEventHandler, MockSimulator, StepResult, TileId,
};

#[test]
fn should_execute_single_cycle() {
    let mut mock = MockSimulator::new();
    mock.expect_step().times(1).returning(|| {
        Ok(StepResult {
            cycle: 1,
            active_tiles: vec![TileId(0)],
            instructions_executed: 1,
        })
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.step().unwrap();

    assert_eq!(result.cycle, 1);
}

#[test]
fn should_fail_when_no_program_loaded() {
    let mock = MockSimulator::new();
    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let result = sdk.step();

    assert!(matches!(result, Err(Error::NoProgramLoaded)));
}

#[test]
fn should_emit_cycle_event_on_step() {
    let mut mock = MockSimulator::new();
    mock.expect_step().returning(|| Ok(StepResult::default()));

    let mut mock_handler = MockEventHandler::new();
    mock_handler
        .expect_on_cycle()
        .times(1)
        .returning(|_, _| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.add_handler(Box::new(mock_handler));
    sdk.mark_program_loaded();

    sdk.step().unwrap();
}

#[test]
fn should_increment_internal_cycle_counter() {
    let mut mock = MockSimulator::new();
    mock.expect_step().times(3).returning(|| {
        Ok(StepResult {
            cycle: 0, // Simulator returns its own cycle, SDK tracks separately
            active_tiles: vec![],
            instructions_executed: 1,
        })
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let step1 = sdk.step().unwrap();
    assert_eq!(step1.cycle, 1);

    let step2 = sdk.step().unwrap();
    assert_eq!(step2.cycle, 2);

    let step3 = sdk.step().unwrap();
    assert_eq!(step3.cycle, 3);
}

#[test]
fn should_return_active_tiles() {
    let mut mock = MockSimulator::new();
    mock.expect_step().returning(|| {
        Ok(StepResult {
            cycle: 1,
            active_tiles: vec![TileId(0), TileId(1), TileId(2)],
            instructions_executed: 3,
        })
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.step().unwrap();

    assert_eq!(result.active_tiles.len(), 3);
    assert_eq!(result.instructions_executed, 3);
}
