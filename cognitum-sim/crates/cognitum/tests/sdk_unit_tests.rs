//! SDK Unit Tests
//!
//! Comprehensive unit tests for the SDK Core implementation

use cognitum::sdk::{
    CognitumSDK, Error, ExitReason, ExecutionResult, InternalState, Metrics, MetricsSummary,
    MockEventHandler, MockMetricsCollector, MockSimulator, ProgramId, SimulatorError, StepResult,
    TileId, TileState,
};
use mockall::predicate::*;
use mockall::Sequence;

// ============================================================================
// load_program Tests
// ============================================================================

#[test]
fn load_program_should_delegate_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_load_program()
        .with(eq(vec![0x01, 0x02, 0x03]))
        .times(1)
        .returning(|_| Ok(ProgramId(1)));
    mock.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    let result = sdk.load_program(&[0x01, 0x02, 0x03]);

    assert!(result.is_ok());
}

#[test]
fn load_program_should_validate_before_loading() {
    let mut mock = MockSimulator::new();
    mock.expect_load_program().times(0);
    mock.expect_reset().times(0);

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    let result = sdk.load_program(&[]);

    assert!(matches!(result, Err(Error::EmptyProgram)));
}

#[test]
fn load_program_should_propagate_simulator_errors() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().times(1).returning(|| ());
    mock.expect_load_program()
        .returning(|_| Err(SimulatorError::InvalidProgram("bad magic".into())));

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    let result = sdk.load_program(&[0x01, 0x02]);

    assert!(matches!(result, Err(Error::Simulator(_))));
}

#[test]
fn load_program_should_reset_before_loading() {
    let mut mock = MockSimulator::new();
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
fn load_program_should_update_internal_state() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_load_program().returning(|_| Ok(ProgramId(1)));
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let state1 = sdk.get_state();
    assert!(!state1.program_loaded);

    sdk.load_program(&[0x01, 0x02, 0x03]).unwrap();

    let state2 = sdk.get_state();
    assert!(state2.program_loaded);
    assert_eq!(state2.program_size, 3);
}

// ============================================================================
// run Tests
// ============================================================================

#[test]
fn run_should_delegate_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_execute()
        .with(eq(u64::MAX))
        .times(1)
        .returning(|_| {
            Ok(ExecutionResult {
                cycles: 100,
                exit_reason: ExitReason::ProgramComplete,
            })
        });
    mock.expect_get_metrics().returning(|| Metrics::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run().unwrap();
    assert_eq!(result.cycles_executed, 100);
}

#[test]
fn run_should_fail_without_program() {
    let mock = MockSimulator::new();
    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let result = sdk.run();
    assert!(matches!(result, Err(Error::NoProgramLoaded)));
}

#[test]
fn run_should_collect_metrics() {
    let mut mock = MockSimulator::new();
    mock.expect_execute()
        .returning(|_| Ok(ExecutionResult::default()));
    mock.expect_get_metrics().returning(|| Metrics::default());

    let mut mock_metrics = MockMetricsCollector::new();
    mock_metrics.expect_get_summary().times(1).returning(|| {
        MetricsSummary {
            total_instructions: 500,
            memory_reads: 100,
            memory_writes: 50,
            messages_sent: 0,
            avg_ipc: 0.0,
        }
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock))
        .with_metrics_collector(Box::new(mock_metrics));

    sdk.mark_program_loaded();
    let result = sdk.run().unwrap();

    assert_eq!(result.instructions_executed, 500);
}

// ============================================================================
// run_for Tests
// ============================================================================

#[test]
fn run_for_should_pass_cycle_limit() {
    let mut mock = MockSimulator::new();
    mock.expect_execute()
        .with(eq(1000u64))
        .times(1)
        .returning(|cycles| {
            Ok(ExecutionResult {
                cycles,
                exit_reason: ExitReason::CycleLimit,
            })
        });
    mock.expect_get_metrics().returning(|| Metrics::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run_for(1000).unwrap();
    assert_eq!(result.cycles_executed, 1000);
}

#[test]
fn run_for_should_reject_zero_cycles() {
    let mock = MockSimulator::new();
    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run_for(0);
    assert!(matches!(result, Err(Error::InvalidCycleCount)));
}

// ============================================================================
// step Tests
// ============================================================================

#[test]
fn step_should_execute_single_cycle() {
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
fn step_should_emit_events() {
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
fn step_should_increment_cycle_counter() {
    let mut mock = MockSimulator::new();
    mock.expect_step().times(3).returning(|| {
        Ok(StepResult {
            cycle: 0,
            active_tiles: vec![],
            instructions_executed: 1,
        })
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    assert_eq!(sdk.step().unwrap().cycle, 1);
    assert_eq!(sdk.step().unwrap().cycle, 2);
    assert_eq!(sdk.step().unwrap().cycle, 3);
}

// ============================================================================
// get_state Tests
// ============================================================================

#[test]
fn get_state_should_delegate_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state().times(1).returning(|| InternalState {
        tiles: vec![
            TileState {
                id: TileId(0),
                program_counter: 100,
                stack_pointer: 50,
            },
            TileState {
                id: TileId(1),
                program_counter: 200,
                stack_pointer: 60,
            },
        ],
        cycle: 500,
        memory: vec![],
    });

    let sdk = CognitumSDK::with_simulator(Box::new(mock));
    let state = sdk.get_state();

    assert_eq!(state.tiles.len(), 2);
    assert_eq!(state.current_cycle, 500);
}

#[test]
fn get_state_should_include_program_status() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let state1 = sdk.get_state();
    assert!(!state1.program_loaded);

    sdk.mark_program_loaded();
    let state2 = sdk.get_state();
    assert!(state2.program_loaded);
}

// ============================================================================
// reset Tests
// ============================================================================

#[test]
fn reset_should_delegate_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.reset();
}

#[test]
fn reset_should_clear_internal_state() {
    let mut mock = MockSimulator::new();
    mock.expect_reset().returning(|| ());
    mock.expect_step().returning(|| Ok(StepResult::default()));
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();
    sdk.step().unwrap();

    sdk.reset();

    let state = sdk.get_state();
    assert!(!state.program_loaded);
    assert_eq!(state.current_cycle, 0);
}

#[test]
fn reset_should_reset_metrics() {
    let mut mock_sim = MockSimulator::new();
    mock_sim.expect_reset().times(1).returning(|| ());

    let mut mock_metrics = MockMetricsCollector::new();
    mock_metrics.expect_reset().times(1).returning(|| ());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock_sim))
        .with_metrics_collector(Box::new(mock_metrics));

    sdk.reset();
}
