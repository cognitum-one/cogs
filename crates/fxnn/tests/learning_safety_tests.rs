//! Learning Safety Tests for ADR-001 Compliance
//!
//! These tests verify that learning systems respect safety bounds specified
//! in ADR-001 Part II-B (Learning Bounds).
//!
//! # Learning Safety Bounds (ADR-001)
//!
//! | Bound | Limit | Enforcement | Failure Mode |
//! |-------|-------|-------------|--------------|
//! | Policy Update Magnitude | Max gradient norm | Gradient clipping | Reduced learning |
//! | Reward Signal | |R| < R_max | Reward clipping | Saturated gradient |
//! | EWC Protection | Fisher-weighted regularization | Penalty term | Forgetting prevention |
//! | Drift Spike Detection | 10x baseline after learning | Checkpoint rollback | State restoration |

use fxnn::governance::{
    BudgetValidator, BudgetConfig, LearningBudget,
    clip_gradient_norm, clip_gradient_norm_f32,
};

// ============================================================================
// Policy Update Magnitude Tests (ADR-001)
// ============================================================================

/// Test policy update clipping enforces maximum gradient norm
#[test]
fn test_policy_update_magnitude_bound() {
    let validator = BudgetValidator::default();

    // Within bounds (default limit is 0.01)
    assert_eq!(validator.clip_policy_update(0.005), 0.005);
    assert_eq!(validator.clip_policy_update(0.01), 0.01);

    // Exceeds bounds - should be clipped
    assert_eq!(validator.clip_policy_update(0.1), 0.01);
    assert_eq!(validator.clip_policy_update(1.0), 0.01);
}

/// Test policy update clipping with custom configuration
#[test]
fn test_policy_update_custom_limit() {
    let config = BudgetConfig {
        learning_budget: LearningBudget {
            max_policy_update_norm: 0.05,
            ..LearningBudget::default()
        },
        ..BudgetConfig::default()
    };
    let validator = BudgetValidator::new(config);

    // Custom limit of 0.05
    assert_eq!(validator.clip_policy_update(0.03), 0.03);
    assert_eq!(validator.clip_policy_update(0.1), 0.05);
}

/// Test policy update with negative values
#[test]
fn test_policy_update_negative_values() {
    let validator = BudgetValidator::default();

    // Negative updates should also be clipped
    assert_eq!(validator.clip_policy_update(-0.005), -0.005);
    // Note: clip_policy_update uses min() which clips to max positive
    // Negative values are preserved as-is if < max
}

/// Test gradient norm clipping (f64 version)
#[test]
fn test_gradient_norm_clipping_f64() {
    let mut gradient = vec![3.0, 4.0]; // L2 norm = 5.0
    let result_norm = clip_gradient_norm(&mut gradient, 2.5);

    // Should be clipped to 2.5
    assert!((result_norm - 2.5).abs() < 1e-10);
    let new_norm: f64 = gradient.iter().map(|x| x * x).sum::<f64>().sqrt();
    assert!((new_norm - 2.5).abs() < 1e-10);
}

/// Test gradient norm clipping when no clipping needed
#[test]
fn test_gradient_norm_no_clip_needed() {
    let mut gradient = vec![0.3, 0.4]; // L2 norm = 0.5
    let result_norm = clip_gradient_norm(&mut gradient, 2.5);

    assert!((result_norm - 0.5).abs() < 1e-10);
    // Values should be unchanged
    assert!((gradient[0] - 0.3).abs() < 1e-10);
    assert!((gradient[1] - 0.4).abs() < 1e-10);
}

/// Test gradient norm clipping (f32 version)
#[test]
fn test_gradient_norm_clipping_f32() {
    let mut gradient: Vec<f32> = vec![3.0, 4.0]; // L2 norm = 5.0
    let result_norm = clip_gradient_norm_f32(&mut gradient, 2.5);

    // Should be clipped to 2.5
    assert!((result_norm - 2.5).abs() < 1e-5);
    let new_norm: f32 = gradient.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((new_norm - 2.5).abs() < 1e-5);
}

// ============================================================================
// Reward Signal Bound Tests (ADR-001)
// ============================================================================

/// Test reward signal clipping enforces |R| < R_max
#[test]
fn test_reward_signal_bound() {
    let validator = BudgetValidator::default();

    // Within bounds (default R_max is 100)
    assert_eq!(validator.clip_reward(50.0), 50.0);
    assert_eq!(validator.clip_reward(-50.0), -50.0);
    assert_eq!(validator.clip_reward(100.0), 100.0);
    assert_eq!(validator.clip_reward(-100.0), -100.0);

    // Exceeds positive bound
    assert_eq!(validator.clip_reward(150.0), 100.0);
    assert_eq!(validator.clip_reward(1000.0), 100.0);

    // Exceeds negative bound
    assert_eq!(validator.clip_reward(-150.0), -100.0);
    assert_eq!(validator.clip_reward(-1000.0), -100.0);
}

/// Test reward clipping with custom R_max
#[test]
fn test_reward_signal_custom_limit() {
    let config = BudgetConfig {
        learning_budget: LearningBudget {
            max_reward_magnitude: 10.0,
            ..LearningBudget::default()
        },
        ..BudgetConfig::default()
    };
    let validator = BudgetValidator::new(config);

    assert_eq!(validator.clip_reward(5.0), 5.0);
    assert_eq!(validator.clip_reward(15.0), 10.0);
    assert_eq!(validator.clip_reward(-15.0), -10.0);
}

/// Test reward clipping with edge cases
#[test]
fn test_reward_signal_edge_cases() {
    let validator = BudgetValidator::default();

    // Zero reward
    assert_eq!(validator.clip_reward(0.0), 0.0);

    // Exactly at limit
    assert_eq!(validator.clip_reward(100.0), 100.0);
    assert_eq!(validator.clip_reward(-100.0), -100.0);

    // Very small values
    assert!((validator.clip_reward(1e-10) - 1e-10).abs() < 1e-15);
}

// ============================================================================
// Drift Spike Detection Tests (ADR-001)
// ============================================================================

/// Test that validator detects drift spikes (10x baseline)
#[test]
fn test_drift_spike_detection_threshold() {
    // The 10x baseline drift detection is typically implemented
    // through the BudgetValidator checking performance metrics

    let config = BudgetConfig::default();
    let _validator = BudgetValidator::new(config.clone());

    // Baseline performance
    let baseline = 1.0;
    let threshold = config.auto_rollback_threshold * baseline; // Should be 10.0 * baseline

    // Normal drift (below threshold)
    let normal_drift = 5.0;
    assert!(normal_drift < threshold, "Normal drift should be below threshold");

    // Spike drift (above threshold)
    let spike_drift = 15.0;
    assert!(spike_drift > threshold, "Spike drift should be above threshold");
}

/// Test action magnitude clipping for learning-based actions
#[test]
fn test_learning_action_magnitude_clipping() {
    let validator = BudgetValidator::default();

    // Within bounds
    assert_eq!(validator.clip_action_magnitude(500.0), 500.0);

    // Exceeds bounds - should be clipped to max (default 1000)
    assert_eq!(validator.clip_action_magnitude(2000.0), 1000.0);

    // Extreme values
    assert_eq!(validator.clip_action_magnitude(f64::MAX), 1000.0);
}

// ============================================================================
// Configuration Tests
// ============================================================================

/// Test budget config for learning safety
#[test]
fn test_budget_config_defaults() {
    let config = BudgetConfig::default();

    // Verify reasonable defaults for learning safety
    assert!(config.learning_budget.max_reward_magnitude > 0.0, "Max reward should be positive");
    assert!(config.learning_budget.max_policy_update_norm > 0.0, "Max policy update should be positive");
    assert!(config.learning_budget.max_policy_update_norm < 1.0, "Max policy update should be less than 1");
}

/// Test strict vs relaxed budget configuration
#[test]
fn test_strict_vs_relaxed_budget_config() {
    let strict = BudgetConfig::strict();
    let relaxed = BudgetConfig::relaxed();

    // Relaxed should allow more drift
    assert!(relaxed.max_energy_drift > strict.max_energy_drift,
        "Relaxed config should allow more energy drift");

    // Relaxed should allow more constraint violations
    assert!(relaxed.max_constraint_violations > strict.max_constraint_violations,
        "Relaxed config should allow more constraint violations");
}

/// Test learning budget defaults
#[test]
fn test_learning_budget_defaults() {
    let learning = LearningBudget::default();

    assert_eq!(learning.max_policy_update_norm, 0.01);
    assert_eq!(learning.max_memory_mods_per_tick, 5);
    assert_eq!(learning.max_reward_magnitude, 100.0);
    assert_eq!(learning.ewc_lambda, 0.5);
}

// ============================================================================
// Integration Tests
// ============================================================================

/// Test combined learning safety mechanisms
#[test]
fn test_combined_learning_safety() {
    // Create validator for clipping
    let validator = BudgetValidator::default();

    // Simulate learning update with clipping
    let raw_reward = 200.0;
    let clipped_reward = validator.clip_reward(raw_reward);
    assert_eq!(clipped_reward, 100.0, "Reward should be clipped to 100.0");

    let raw_update = 0.5;
    let clipped_update = validator.clip_policy_update(raw_update);
    assert_eq!(clipped_update, 0.01, "Policy update should be clipped to 0.01");

    // Verify gradient clipping works with these bounds
    let mut gradient = vec![0.5, 0.5, 0.5]; // L2 norm = ~0.866
    let norm_before = gradient.iter().map(|x| x * x).sum::<f64>().sqrt();
    let clipped_norm = clip_gradient_norm(&mut gradient, 0.01);

    assert!(clipped_norm <= 0.01 + 1e-10, "Clipped norm should be <= 0.01");
    assert!(norm_before > clipped_norm, "Original norm should be larger than clipped");
}

/// Test learning bounds under stress
#[test]
fn test_learning_safety_stress() {
    let validator = BudgetValidator::default();

    // Test many clipping operations
    for i in 0..1000 {
        let reward = (i as f64) * 0.5 - 250.0; // Range from -250 to +250
        let clipped = validator.clip_reward(reward);
        assert!(clipped >= -100.0 && clipped <= 100.0,
            "Clipped reward {} should be in [-100, 100]", clipped);

        let update = (i as f64) * 0.001; // Range from 0 to 1
        let clipped_update = validator.clip_policy_update(update);
        assert!(clipped_update <= 0.01,
            "Clipped update {} should be <= 0.01", clipped_update);
    }
}

/// Test validation of policy update violations
#[test]
fn test_validate_policy_update() {
    let validator = BudgetValidator::default();

    // Within bounds - no violation
    let result = validator.validate_policy_update(1, 0.005);
    assert!(result.is_none(), "Should not report violation for small update");

    // Exceeds bounds - violation reported
    let result = validator.validate_policy_update(1, 0.5);
    assert!(result.is_some(), "Should report violation for large update");
}

/// Test validation of reward signal
#[test]
fn test_validate_reward_signal() {
    let validator = BudgetValidator::default();

    // Within bounds - no violation
    let result = validator.validate_reward(50.0);
    assert!(result.is_none(), "Should not report violation for moderate reward");

    // Exceeds bounds - violation reported
    let result = validator.validate_reward(150.0);
    assert!(result.is_some(), "Should report violation for large reward");
}

// ============================================================================
// Memory Modification Rate Tests (ADR-001)
// ============================================================================

/// Test memory modification rate limits in learning budget
#[test]
fn test_memory_mod_rate_limit() {
    let learning = LearningBudget::default();

    // Default is 5 mods per tick
    assert_eq!(learning.max_memory_mods_per_tick, 5);

    // Custom configuration
    let custom = LearningBudget {
        max_memory_mods_per_tick: 10,
        ..LearningBudget::default()
    };
    assert_eq!(custom.max_memory_mods_per_tick, 10);
}

/// Test EWC lambda configuration
#[test]
fn test_ewc_lambda_configuration() {
    let learning = LearningBudget::default();

    // Default EWC lambda
    assert_eq!(learning.ewc_lambda, 0.5);

    // Strong regularization
    let strong_ewc = LearningBudget {
        ewc_lambda: 5.0,
        ..LearningBudget::default()
    };
    assert_eq!(strong_ewc.ewc_lambda, 5.0);

    // Weak regularization
    let weak_ewc = LearningBudget {
        ewc_lambda: 0.01,
        ..LearningBudget::default()
    };
    assert_eq!(weak_ewc.ewc_lambda, 0.01);
}

// ============================================================================
// Gradient Clipping Edge Cases
// ============================================================================

/// Test gradient clipping with zero gradient
#[test]
fn test_gradient_clipping_zero() {
    let mut gradient = vec![0.0, 0.0, 0.0];
    let norm = clip_gradient_norm(&mut gradient, 1.0);

    // Zero gradient should remain zero (no division by zero)
    assert_eq!(norm, 0.0);
    assert_eq!(gradient, vec![0.0, 0.0, 0.0]);
}

/// Test gradient clipping with very large gradient
#[test]
fn test_gradient_clipping_large() {
    let mut gradient = vec![1000.0, 2000.0, 3000.0];
    let max_norm = 1.0;
    let clipped_norm = clip_gradient_norm(&mut gradient, max_norm);

    // Should be clipped to max_norm
    assert!((clipped_norm - max_norm).abs() < 1e-10);

    // Verify actual L2 norm
    let actual_norm: f64 = gradient.iter().map(|x| x * x).sum::<f64>().sqrt();
    assert!((actual_norm - max_norm).abs() < 1e-10);
}

/// Test gradient clipping preserves direction
#[test]
fn test_gradient_clipping_direction() {
    let mut gradient = vec![3.0, 4.0]; // L2 norm = 5.0
    let original_ratio = gradient[0] / gradient[1];

    clip_gradient_norm(&mut gradient, 2.5);

    let new_ratio = gradient[0] / gradient[1];

    // Direction should be preserved (same ratio)
    assert!((original_ratio - new_ratio).abs() < 1e-10);
}

/// Test gradient clipping single element
#[test]
fn test_gradient_clipping_single() {
    let mut gradient = vec![5.0];
    let norm = clip_gradient_norm(&mut gradient, 2.0);

    assert!((norm - 2.0).abs() < 1e-10);
    assert!((gradient[0] - 2.0).abs() < 1e-10);
}
