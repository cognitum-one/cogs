//! Unit tests for CognitumSDK::run_for

use cognitum::sdk::{CognitumSDK, Error, ExitReason, ExecutionResult, Metrics, MockSimulator};
use mockall::predicate::eq;

#[test]
fn should_pass_cycle_limit_to_simulator() {
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
    assert_eq!(result.exit_reason, ExitReason::CycleLimit);
}

#[test]
fn should_reject_zero_cycles() {
    let mock = MockSimulator::new();
    // No expectations - should not be called

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run_for(0);

    assert!(matches!(result, Err(Error::InvalidCycleCount)));
}

#[test]
fn should_fail_when_no_program_loaded() {
    let mock = MockSimulator::new();
    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let result = sdk.run_for(100);

    assert!(matches!(result, Err(Error::NoProgramLoaded)));
}

#[test]
fn should_return_metrics_from_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_execute().returning(|cycles| {
        Ok(ExecutionResult {
            cycles,
            exit_reason: ExitReason::CycleLimit,
        })
    });
    mock.expect_get_metrics().returning(|| Metrics {
        instructions: 500,
        memory_reads: 100,
        memory_writes: 50,
        messages_sent: 10,
    });

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run_for(500).unwrap();

    assert_eq!(result.instructions_executed, 500);
    assert_eq!(result.memory_reads, 100);
    assert_eq!(result.memory_writes, 50);
}
