//! Unit tests for CognitumSDK::run

use cognitum::sdk::{
    CognitumSDK, Error, ExitReason, ExecutionResult, Metrics, MetricsSummary,
    MockMetricsCollector, MockSimulator,
};

#[test]
fn should_delegate_execution_to_simulator() {
    let mut mock = MockSimulator::new();
    mock.expect_execute()
        .with(mockall::predicate::eq(u64::MAX)) // Run until complete
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
fn should_fail_when_no_program_loaded() {
    let mock = MockSimulator::new();
    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    let result = sdk.run();

    assert!(matches!(result, Err(Error::NoProgramLoaded)));
}

#[test]
fn should_collect_metrics_after_execution() {
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
    assert_eq!(result.memory_reads, 100);
    assert_eq!(result.memory_writes, 50);
}

#[test]
fn should_return_correct_exit_reason() {
    let mut mock = MockSimulator::new();
    mock.expect_execute().returning(|_| {
        Ok(ExecutionResult {
            cycles: 100,
            exit_reason: ExitReason::ProgramComplete,
        })
    });
    mock.expect_get_metrics().returning(|| Metrics::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run().unwrap();
    assert_eq!(result.exit_reason, ExitReason::ProgramComplete);
}

#[test]
fn should_propagate_simulator_errors() {
    use cognitum::sdk::SimulatorError;

    let mut mock = MockSimulator::new();
    mock.expect_execute()
        .returning(|_| Err(SimulatorError::ExecutionFailed("test error".into())));

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));
    sdk.mark_program_loaded();

    let result = sdk.run();
    assert!(matches!(result, Err(Error::Simulator(_))));
}
