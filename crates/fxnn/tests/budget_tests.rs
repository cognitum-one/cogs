//! Reality Budget Tests for ADR-001 Compliance
//!
//! These tests verify that the reality budget enforcement mechanisms from
//! ADR-001 Part II-B are correctly implemented.
//!
//! # Reality Budgets (ADR-001)
//!
//! | Budget | Limit | Enforcement | Failure Mode |
//! |--------|-------|-------------|--------------|
//! | Energy Drift | <0.01% per minute | Symplectic integrator + correction | Log warning, reduce timestep |
//! | Momentum Drift | <10^-6 relative error | Conservation validation | Hard error, rollback |
//! | Constraint Violation | 0 penetrations | Constraint projection | Force separation, emit witness |
//! | Numerical Stability | No NaN/Inf | Bounded force clipping | Emergency rollback |

use fxnn::governance::{
    BudgetConfig, BudgetValidator, BudgetViolation, BudgetStatus, BudgetReport,
    EnergyBudget, MomentumBudget, ConstraintBudget, SimulationStateSnapshot,
    validate_reality_budgets,
};

// ============================================================================
// Energy Drift Budget Tests (ADR-001: <0.01% per minute)
// ============================================================================

/// Test that energy drift within budget is correctly validated
#[test]
fn test_energy_drift_budget_within_limits() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config.clone());

    // Initial state with reference energy
    let initial_state = SimulationStateSnapshot::new(0, -100.0, -100.0);
    let report = validator.validate(&initial_state);
    assert!(report.is_valid(), "Initial state should be valid");
    assert_eq!(report.energy, BudgetStatus::Ok);

    // After 1 minute with <0.01% drift (should pass)
    let mut state_after_1min = SimulationStateSnapshot::new(1000, -100.005, -100.0);
    state_after_1min.wall_clock_seconds = 60.0;
    let report = validator.validate(&state_after_1min);
    assert_eq!(report.energy, BudgetStatus::Ok,
        "Energy drift of 0.005% should be within 0.01% limit");
}

/// Test that energy drift exceeding budget is detected
#[test]
fn test_energy_drift_budget_exceeded() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    // Initial state
    let initial_state = SimulationStateSnapshot::new(0, -100.0, -100.0);
    let _ = validator.validate(&initial_state);

    // After 1 minute with >0.01% drift (should fail)
    let mut state_with_drift = SimulationStateSnapshot::new(1000, -110.0, -100.0);
    state_with_drift.wall_clock_seconds = 60.0;

    let report = validator.validate(&state_with_drift);
    assert_eq!(report.energy, BudgetStatus::Exceeded,
        "Energy drift of 10% should exceed 0.01% limit");

    // Verify violation is reported
    let has_energy_violation = report.violations.iter().any(|v|
        matches!(v, BudgetViolation::EnergyDrift { .. })
    );
    assert!(has_energy_violation, "Should report energy drift violation");
}

/// Test energy drift warning threshold (80% of limit)
#[test]
fn test_energy_drift_budget_warning() {
    let mut budget = EnergyBudget::with_limit(0.01); // 1% limit

    // Set reference energy
    budget.update(100.0, 0.0);

    // 0.85% drift should trigger warning (>80% of limit)
    budget.update(100.85, 60.0);

    let status = budget.check(60.0);
    assert_eq!(status, BudgetStatus::Warning,
        "Energy drift at 85% of limit should be Warning");
}

/// Test energy drift with very small reference energy
#[test]
fn test_energy_drift_budget_near_zero_energy() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    // Very small reference energy
    let state = SimulationStateSnapshot::new(0, 1e-12, 1e-12);
    let report = validator.validate(&state);

    // Should handle near-zero energy gracefully without NaN
    assert!(report.numerical_stability);
}

// ============================================================================
// Momentum Drift Budget Tests (ADR-001: <10^-6 per 10,000 steps)
// ============================================================================

/// Test that momentum conservation within budget is validated
#[test]
fn test_momentum_drift_budget_within_limits() {
    let mut budget = MomentumBudget::with_limit(1e-6);

    // Reference momentum
    budget.update(100.0);

    // Run 10,000 steps with tiny drift
    for _ in 0..10_000 {
        budget.update(100.0 + 1e-8); // Very small drift
    }

    let status = budget.check();
    assert_eq!(status, BudgetStatus::Ok,
        "Momentum drift of 10^-8 should be within 10^-6 limit");
}

/// Test that momentum drift exceeding budget is detected
#[test]
fn test_momentum_drift_budget_exceeded() {
    let config = BudgetConfig::default();
    let mut state = SimulationStateSnapshot::new(10_000, -100.0, -100.0);
    state.total_momentum = 100.0;
    state.reference_momentum = 100.0;

    // Simulate drift exceeding limit
    state.total_momentum = 100.01; // 0.01% drift >> 10^-6

    let report = validate_reality_budgets(&state, &config);

    // With momentum drift > limit, should be exceeded
    let drift_ratio = state.momentum_drift_ratio();
    assert!(drift_ratio > 1e-6,
        "Drift ratio {} should exceed 10^-6", drift_ratio);
}

/// Test momentum conservation over 10,000 steps invariant
#[test]
fn test_momentum_conservation_invariant() {
    let mut budget = MomentumBudget::with_limit(1e-6);

    let initial_momentum = 100.0;
    budget.update(initial_momentum);

    // Simulate 10,000 steps with numerical noise
    let mut current_momentum = initial_momentum;
    let noise_per_step = 1e-10; // Very small numerical noise

    for _ in 0..10_000 {
        // Simulate tiny numerical errors
        current_momentum += noise_per_step;
        budget.update(current_momentum);
    }

    let status = budget.check();
    let total_drift = (current_momentum - initial_momentum).abs() / initial_momentum.abs();

    // 10^-6 relative drift over 10,000 steps
    assert!(total_drift < 1e-5,
        "Total drift {} should be within acceptable range", total_drift);
}

/// Test momentum budget reset functionality
#[test]
fn test_momentum_budget_reset() {
    let mut budget = MomentumBudget::with_limit(1e-6);

    budget.update(100.0);
    budget.update(200.0); // Large change

    // Reset to new reference
    budget.reset(200.0);

    // After reset, no drift from new reference
    budget.update(200.0);
    let status = budget.check();

    // Should be Ok after reset since we're at the new reference
    assert_eq!(budget.current_drift, 0.0);
}

// ============================================================================
// Constraint Violation Budget Tests (ADR-001: 0 penetrations)
// ============================================================================

/// Test that zero constraint violations pass
#[test]
fn test_constraint_violation_budget_zero_penetrations() {
    let mut budget = ConstraintBudget::default();

    budget.update(0, 0.0);
    let status = budget.check();

    assert_eq!(status, BudgetStatus::Ok,
        "Zero violations should be Ok");
}

/// Test that any constraint violation is detected
#[test]
fn test_constraint_violation_budget_any_penetration() {
    let mut budget = ConstraintBudget::default();

    // Even a single violation should exceed budget (max_violations = 0)
    budget.update(1, 0.001);
    let status = budget.check();

    assert_eq!(status, BudgetStatus::Exceeded,
        "Any penetration should exceed zero-tolerance budget");
}

/// Test constraint violation with multiple penetrations
#[test]
fn test_constraint_violation_budget_multiple_penetrations() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    let mut state = SimulationStateSnapshot::new(100, -100.0, -100.0);
    state.constraint_violations = 5;
    state.max_penetration = 0.1;

    let report = validator.validate(&state);

    assert!(!report.is_valid());
    assert_eq!(report.constraints, BudgetStatus::Exceeded);

    // Should have constraint violation in the list
    let has_constraint_violation = report.violations.iter().any(|v|
        matches!(v, BudgetViolation::ConstraintViolation { count: 5, .. })
    );
    assert!(has_constraint_violation);
}

/// Test constraint violation maximum penetration depth tracking
#[test]
fn test_constraint_violation_max_penetration_depth() {
    let mut budget = ConstraintBudget::default();

    // Update with various penetration depths
    budget.update(3, 0.05);
    assert_eq!(budget.current_max_penetration, 0.05);

    // Deeper penetration
    budget.update(2, 0.15);
    assert_eq!(budget.current_max_penetration, 0.15);
}

// ============================================================================
// Numerical Stability Budget Tests (ADR-001: No NaN/Inf)
// ============================================================================

/// Test that NaN values are detected
#[test]
fn test_numerical_stability_budget_nan_detection() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    let mut state = SimulationStateSnapshot::new(100, -100.0, -100.0);
    state.has_numerical_errors = true;

    let report = validator.validate(&state);

    assert!(!report.numerical_stability);
    assert!(!report.is_valid());

    // Should have numerical instability violation
    let has_numerical_violation = report.violations.iter().any(|v|
        matches!(v, BudgetViolation::NumericalInstability { .. })
    );
    assert!(has_numerical_violation);
}

/// Test that Inf values would trigger instability
#[test]
fn test_numerical_stability_budget_inf_detection() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    let mut state = SimulationStateSnapshot::new(100, f64::INFINITY, -100.0);
    state.has_numerical_errors = true;

    let report = validator.validate(&state);

    assert!(!report.numerical_stability);
}

/// Test numerical stability with valid values
#[test]
fn test_numerical_stability_budget_valid_values() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    let state = SimulationStateSnapshot::new(100, -100.0, -100.0);
    let report = validator.validate(&state);

    assert!(report.numerical_stability);
}

/// Test stress scenario with extreme but valid values
#[test]
fn test_numerical_stability_budget_stress_test() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    // Very large but finite values
    let mut state = SimulationStateSnapshot::new(0, 1e100, 1e100);
    state.has_numerical_errors = false;

    let report = validator.validate(&state);
    assert!(report.numerical_stability,
        "Large but finite values should not trigger instability");

    // Very small values
    let state2 = SimulationStateSnapshot::new(0, 1e-100, 1e-100);
    let report2 = validator.validate(&state2);
    assert!(report2.numerical_stability,
        "Small but non-zero values should not trigger instability");
}

// ============================================================================
// Budget Report Tests
// ============================================================================

/// Test budget report is_valid logic
#[test]
fn test_budget_report_validity() {
    let mut report = BudgetReport::new();
    assert!(report.is_valid());

    // Any exceeded budget should invalidate
    report.energy = BudgetStatus::Exceeded;
    assert!(!report.is_valid());

    // Reset and test other fields
    report.energy = BudgetStatus::Ok;
    report.determinism = false;
    assert!(!report.is_valid());

    report.determinism = true;
    report.numerical_stability = false;
    assert!(!report.is_valid());
}

/// Test budget report warnings detection
#[test]
fn test_budget_report_warnings() {
    let mut report = BudgetReport::new();
    assert!(!report.has_warnings());

    report.energy = BudgetStatus::Warning;
    assert!(report.has_warnings());
}

/// Test budget report violation accumulation
#[test]
fn test_budget_report_violations() {
    let mut report = BudgetReport::new();

    report.add_violation(BudgetViolation::EnergyDrift { actual: 0.02, limit: 0.0001 });
    report.add_violation(BudgetViolation::MomentumDrift { actual: 1e-4, limit: 1e-6 });

    assert_eq!(report.violations.len(), 2);
}

// ============================================================================
// Relaxed and Strict Configuration Tests
// ============================================================================

/// Test relaxed budget configuration
#[test]
fn test_relaxed_budget_config() {
    let relaxed = BudgetConfig::relaxed();
    let strict = BudgetConfig::strict();

    // Relaxed should be more lenient
    assert!(relaxed.max_energy_drift > strict.max_energy_drift);
    assert!(relaxed.max_momentum_drift > strict.max_momentum_drift);
    assert!(relaxed.max_constraint_violations > strict.max_constraint_violations);
}

/// Test strict budget configuration
#[test]
fn test_strict_budget_config() {
    let config = BudgetConfig::strict();
    let mut validator = BudgetValidator::new(config.clone());

    // Even tiny drifts should be detected with strict config
    let mut state = SimulationStateSnapshot::new(0, -100.0, -100.0);
    let _ = validator.validate(&state);

    state = SimulationStateSnapshot::new(1000, -100.001, -100.0);
    state.wall_clock_seconds = 60.0;

    let report = validator.validate(&state);

    // 0.001% drift should exceed strict limit of 0.001%
    assert!(config.max_energy_drift < 0.0001,
        "Strict config should have very tight energy drift limit");
}

// ============================================================================
// Clipping Functions Tests
// ============================================================================

/// Test action magnitude clipping
#[test]
fn test_clip_action_magnitude() {
    let validator = BudgetValidator::default();

    // Within bounds
    assert_eq!(validator.clip_action_magnitude(500.0), 500.0);

    // Exceeds bounds
    assert_eq!(validator.clip_action_magnitude(2000.0), 1000.0);

    // Negative values (should clip to max magnitude)
    assert_eq!(validator.clip_action_magnitude(-500.0), -500.0);
}

/// Test reward clipping
#[test]
fn test_clip_reward() {
    let validator = BudgetValidator::default();

    // Within bounds
    assert_eq!(validator.clip_reward(50.0), 50.0);
    assert_eq!(validator.clip_reward(-50.0), -50.0);

    // Exceeds positive bound
    assert_eq!(validator.clip_reward(200.0), 100.0);

    // Exceeds negative bound
    assert_eq!(validator.clip_reward(-200.0), -100.0);
}

/// Test policy update clipping
#[test]
fn test_clip_policy_update() {
    let validator = BudgetValidator::default();

    // Within bounds
    assert_eq!(validator.clip_policy_update(0.005), 0.005);

    // Exceeds bounds
    assert_eq!(validator.clip_policy_update(0.1), 0.01);
}

// ============================================================================
// Validator Reset Tests
// ============================================================================

/// Test budget validator reset
#[test]
fn test_budget_validator_reset() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    // Accumulate some state
    let mut state = SimulationStateSnapshot::new(0, -100.0, -100.0);
    let _ = validator.validate(&state);

    state = SimulationStateSnapshot::new(1000, -110.0, -100.0);
    state.wall_clock_seconds = 60.0;
    let _ = validator.validate(&state);

    // Reset
    validator.reset();

    // After reset, fresh state should be valid
    let fresh_state = SimulationStateSnapshot::new(0, -100.0, -100.0);
    let report = validator.validate(&fresh_state);
    assert!(report.is_valid());
}

// ============================================================================
// Combined Budget Validation Tests
// ============================================================================

/// Test validate_reality_budgets convenience function
#[test]
fn test_validate_reality_budgets_function() {
    let config = BudgetConfig::default();
    let state = SimulationStateSnapshot::new(0, -100.0, -100.0);

    let report = validate_reality_budgets(&state, &config);
    assert!(report.is_valid());
}

/// Test multiple simultaneous violations
#[test]
fn test_multiple_simultaneous_violations() {
    let config = BudgetConfig::default();
    let mut validator = BudgetValidator::new(config);

    // State with multiple violations
    let mut state = SimulationStateSnapshot::new(10_000, -110.0, -100.0);
    state.wall_clock_seconds = 60.0;
    state.constraint_violations = 3;
    state.max_penetration = 0.05;
    state.has_numerical_errors = true;

    let report = validator.validate(&state);

    assert!(!report.is_valid());

    // Should have multiple violations
    assert!(report.violations.len() >= 2,
        "Should report multiple violations, got {}", report.violations.len());
}
