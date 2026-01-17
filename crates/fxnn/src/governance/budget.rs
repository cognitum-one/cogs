//! Reality budget enforcement for FXNN governance layer.
//!
//! This module implements the budget system from ADR-001, enforcing hard limits
//! on drift, violation, and resource consumption to maintain reality closure.
//!
//! # Core Reality Budgets (from ADR-001)
//!
//! | Budget | Limit | Enforcement | Failure Mode |
//! |--------|-------|-------------|--------------|
//! | Energy Drift | <0.01% per minute | Symplectic integrator + correction | Log warning, reduce timestep |
//! | Momentum Drift | <10⁻⁶ relative error | Conservation validation | Hard error, rollback |
//! | Constraint Violation | 0 penetrations | Constraint projection | Force separation, emit witness |
//! | Numerical Stability | No NaN/Inf | Bounded force clipping | Emergency rollback |
//!
//! # Agent Budgets
//!
//! | Budget | Limit | Enforcement | Failure Mode |
//! |--------|-------|-------------|--------------|
//! | Observation Bandwidth | Max bytes/second | Downsampling | Information loss |
//! | Compute per Tick | Max FLOPs | Policy size cap | Action timeout |
//! | Memory Write Rate | Max entries/second | Write throttling | Queue overflow |
//! | Action Magnitude | Max force/velocity | Clipping | Reduced effect |
//!
//! # Learning Budgets
//!
//! | Budget | Limit | Enforcement | Failure Mode |
//! |--------|-------|-------------|--------------|
//! | Policy Update Magnitude | Max ΔW (L2 norm) | Gradient clipping | Capped update |
//! | Memory Modification Rate | Max writes/tick | Write queue | Deferred writes |
//! | Reward Signal Bound | |R| < R_max | Reward clipping | Bounded signal |
//!
//! # Example
//!
//! ```rust
//! use fxnn::governance::{BudgetConfig, BudgetValidator, validate_reality_budgets};
//!
//! let config = BudgetConfig::default();
//! let validator = BudgetValidator::new(config);
//!
//! // Validate against a state snapshot
//! // let report = validator.validate(&state);
//! // if !report.is_valid() {
//! //     for violation in &report.violations {
//! //         println!("Violation: {:?}", violation);
//! //     }
//! // }
//! ```

use super::SimulationStateSnapshot;
use serde::{Serialize, Deserialize};

/// Status of a single budget check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetStatus {
    /// Budget is within limits
    Ok,
    /// Budget is approaching limit (warning threshold)
    Warning,
    /// Budget has exceeded limit
    Exceeded,
}

impl BudgetStatus {
    /// Check a value against a limit
    pub fn check(value: f64, limit: f64) -> Self {
        let ratio = value / limit;
        if ratio > 1.0 {
            BudgetStatus::Exceeded
        } else if ratio > 0.8 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        }
    }

    /// Check a count against a limit
    pub fn check_count(value: u32, limit: u32) -> Self {
        if value > limit {
            BudgetStatus::Exceeded
        } else if value > (limit as f64 * 0.8) as u32 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        }
    }
}

/// Types of budget violations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BudgetViolation {
    /// Energy drift exceeded limit
    EnergyDrift {
        /// Actual drift ratio
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Momentum drift exceeded limit
    MomentumDrift {
        /// Actual drift ratio
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Constraint violations detected
    ConstraintViolation {
        /// Number of violations
        count: u32,
        /// Maximum penetration depth
        max_penetration: f64,
    },
    /// Numerical stability issue (NaN/Inf)
    NumericalInstability {
        /// Description of the issue
        description: String,
    },
    /// Agent exceeded observation bandwidth
    ObservationBandwidth {
        /// Agent ID
        agent_id: u64,
        /// Actual bytes/second
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Agent exceeded compute budget
    ComputeBudget {
        /// Agent ID
        agent_id: u64,
        /// Actual FLOPs
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Agent exceeded memory write rate
    MemoryWriteRate {
        /// Agent ID
        agent_id: u64,
        /// Actual writes/tick
        actual: u32,
        /// Maximum allowed
        limit: u32,
    },
    /// Action magnitude exceeded
    ActionMagnitude {
        /// Agent ID
        agent_id: u64,
        /// Actual magnitude
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Policy update too large
    PolicyUpdateMagnitude {
        /// Agent ID
        agent_id: u64,
        /// Actual L2 norm
        actual: f64,
        /// Maximum allowed
        limit: f64,
    },
    /// Reward signal out of bounds
    RewardSignal {
        /// Actual reward
        actual: f64,
        /// Maximum allowed (absolute)
        limit: f64,
    },
}

/// Energy budget configuration and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyBudget {
    /// Maximum allowed drift ratio per wall-clock minute
    pub max_drift_per_minute: f64,
    /// Current accumulated drift
    pub current_drift: f64,
    /// Reference energy for comparison
    pub reference_energy: f64,
    /// Last check timestamp (seconds)
    pub last_check_time: f64,
}

impl Default for EnergyBudget {
    fn default() -> Self {
        Self {
            max_drift_per_minute: 0.0001, // 0.01%
            current_drift: 0.0,
            reference_energy: 0.0,
            last_check_time: 0.0,
        }
    }
}

impl EnergyBudget {
    /// Create with a custom drift limit
    pub fn with_limit(max_drift_per_minute: f64) -> Self {
        Self {
            max_drift_per_minute,
            ..Default::default()
        }
    }

    /// Update with new energy reading
    pub fn update(&mut self, energy: f64, wall_time_seconds: f64) {
        if self.reference_energy.abs() < 1e-10 {
            self.reference_energy = energy;
            self.last_check_time = wall_time_seconds;
            return;
        }

        self.current_drift = (energy - self.reference_energy).abs() / self.reference_energy.abs();
    }

    /// Check if budget is exceeded
    pub fn check(&self, wall_time_seconds: f64) -> BudgetStatus {
        let minutes_elapsed = (wall_time_seconds - self.last_check_time) / 60.0;
        if minutes_elapsed < 0.001 {
            return BudgetStatus::Ok;
        }

        let drift_per_minute = self.current_drift / minutes_elapsed;
        BudgetStatus::check(drift_per_minute, self.max_drift_per_minute)
    }
}

/// Momentum budget configuration and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumBudget {
    /// Maximum allowed relative error per 10,000 steps
    pub max_error_per_10k_steps: f64,
    /// Current step count since reference
    pub steps_since_reference: u64,
    /// Current accumulated drift
    pub current_drift: f64,
    /// Reference momentum magnitude
    pub reference_momentum: f64,
}

impl Default for MomentumBudget {
    fn default() -> Self {
        Self {
            max_error_per_10k_steps: 1e-6,
            steps_since_reference: 0,
            current_drift: 0.0,
            reference_momentum: 0.0,
        }
    }
}

impl MomentumBudget {
    /// Create with a custom error limit
    pub fn with_limit(max_error_per_10k_steps: f64) -> Self {
        Self {
            max_error_per_10k_steps,
            ..Default::default()
        }
    }

    /// Update with new momentum reading
    pub fn update(&mut self, momentum: f64) {
        if self.reference_momentum.abs() < 1e-10 {
            self.reference_momentum = momentum;
            return;
        }

        self.current_drift = (momentum - self.reference_momentum).abs() / self.reference_momentum.abs();
        self.steps_since_reference += 1;
    }

    /// Check if budget is exceeded
    pub fn check(&self) -> BudgetStatus {
        if self.steps_since_reference < 100 {
            return BudgetStatus::Ok;
        }

        // Extrapolate to 10,000 steps
        let drift_per_10k = self.current_drift * (10_000.0 / self.steps_since_reference as f64);
        BudgetStatus::check(drift_per_10k, self.max_error_per_10k_steps)
    }

    /// Reset the reference point
    pub fn reset(&mut self, momentum: f64) {
        self.reference_momentum = momentum;
        self.steps_since_reference = 0;
        self.current_drift = 0.0;
    }
}

/// Constraint budget for tracking violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintBudget {
    /// Maximum allowed violations (should be 0)
    pub max_violations: u32,
    /// Maximum allowed penetration depth
    pub max_penetration_depth: f64,
    /// Current violation count
    pub current_violations: u32,
    /// Current maximum penetration
    pub current_max_penetration: f64,
}

impl Default for ConstraintBudget {
    fn default() -> Self {
        Self {
            max_violations: 0,
            max_penetration_depth: 0.0,
            current_violations: 0,
            current_max_penetration: 0.0,
        }
    }
}

impl ConstraintBudget {
    /// Update with violation data
    pub fn update(&mut self, violations: u32, max_penetration: f64) {
        self.current_violations = violations;
        self.current_max_penetration = max_penetration;
    }

    /// Check if budget is exceeded
    pub fn check(&self) -> BudgetStatus {
        if self.current_violations > self.max_violations {
            BudgetStatus::Exceeded
        } else if self.current_max_penetration > 0.0 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        }
    }
}

/// Agent-specific budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudget {
    /// Maximum observation bandwidth (bytes/second)
    pub max_observation_bandwidth: f64,
    /// Maximum compute per tick (FLOPs)
    pub max_compute_per_tick: f64,
    /// Maximum memory writes per tick
    pub max_memory_writes_per_tick: u32,
    /// Maximum action magnitude (force/velocity)
    pub max_action_magnitude: f64,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_observation_bandwidth: 1_000_000.0, // 1 MB/s
            max_compute_per_tick: 1e9,              // 1 GFLOP
            max_memory_writes_per_tick: 10,
            max_action_magnitude: 1000.0,
        }
    }
}

/// Learning-specific budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningBudget {
    /// Maximum policy update magnitude (L2 norm)
    pub max_policy_update_norm: f64,
    /// Maximum memory modifications per tick
    pub max_memory_mods_per_tick: u32,
    /// Maximum absolute reward value
    pub max_reward_magnitude: f64,
    /// EWC lambda for catastrophic forgetting prevention
    pub ewc_lambda: f64,
}

impl Default for LearningBudget {
    fn default() -> Self {
        Self {
            max_policy_update_norm: 0.01,
            max_memory_mods_per_tick: 5,
            max_reward_magnitude: 100.0,
            ewc_lambda: 0.5,
        }
    }
}

/// Complete budget configuration (from ADR-001)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    // Core reality budgets
    /// Maximum energy drift ratio per wall-clock minute
    pub max_energy_drift: f64,
    /// Maximum momentum drift ratio per 10,000 steps
    pub max_momentum_drift: f64,
    /// Maximum allowed constraint violations (should be 0)
    pub max_constraint_violations: u32,
    /// Whether to allow NaN/Inf values
    pub allow_numerical_errors: bool,

    // Agent budgets
    /// Agent-specific budgets
    pub agent_budget: AgentBudget,

    // Learning budgets
    /// Learning-specific budgets
    pub learning_budget: LearningBudget,

    // Rollback policy
    /// Number of checkpoints to keep
    pub checkpoint_count: usize,
    /// Steps between checkpoints
    pub checkpoint_interval: u64,
    /// Maximum drift before automatic rollback
    pub auto_rollback_threshold: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            // From ADR-001 Part II-B
            max_energy_drift: 0.0001,      // <0.01% per minute
            max_momentum_drift: 1e-6,      // <10⁻⁶ per 10,000 steps
            max_constraint_violations: 0,  // Zero tolerance
            allow_numerical_errors: false, // No NaN/Inf allowed

            agent_budget: AgentBudget::default(),
            learning_budget: LearningBudget::default(),

            checkpoint_count: 10,
            checkpoint_interval: 1000,
            auto_rollback_threshold: 10.0, // 10x baseline drift triggers rollback
        }
    }
}

impl BudgetConfig {
    /// Create a relaxed configuration for testing
    pub fn relaxed() -> Self {
        Self {
            max_energy_drift: 0.01,        // 1% per minute
            max_momentum_drift: 1e-4,      // 0.01% per 10,000 steps
            max_constraint_violations: 10, // Allow some violations
            allow_numerical_errors: false,
            agent_budget: AgentBudget::default(),
            learning_budget: LearningBudget::default(),
            checkpoint_count: 5,
            checkpoint_interval: 5000,
            auto_rollback_threshold: 100.0,
        }
    }

    /// Create a strict configuration for production
    pub fn strict() -> Self {
        Self {
            max_energy_drift: 0.00001,     // <0.001% per minute
            max_momentum_drift: 1e-8,      // Very strict
            max_constraint_violations: 0,
            allow_numerical_errors: false,
            agent_budget: AgentBudget::default(),
            learning_budget: LearningBudget::default(),
            checkpoint_count: 20,
            checkpoint_interval: 500,
            auto_rollback_threshold: 5.0,
        }
    }
}

/// Report from budget validation
#[derive(Debug, Clone)]
pub struct BudgetReport {
    /// Energy budget status
    pub energy: BudgetStatus,
    /// Momentum budget status
    pub momentum: BudgetStatus,
    /// Constraint budget status
    pub constraints: BudgetStatus,
    /// Determinism check passed
    pub determinism: bool,
    /// Numerical stability check passed
    pub numerical_stability: bool,
    /// List of violations
    pub violations: Vec<BudgetViolation>,
}

impl BudgetReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            energy: BudgetStatus::Ok,
            momentum: BudgetStatus::Ok,
            constraints: BudgetStatus::Ok,
            determinism: true,
            numerical_stability: true,
            violations: Vec::new(),
        }
    }

    /// Check if all budgets are within limits
    pub fn is_valid(&self) -> bool {
        self.energy != BudgetStatus::Exceeded
            && self.momentum != BudgetStatus::Exceeded
            && self.constraints != BudgetStatus::Exceeded
            && self.determinism
            && self.numerical_stability
            && self.violations.is_empty()
    }

    /// Check if any budget is in warning state
    pub fn has_warnings(&self) -> bool {
        self.energy == BudgetStatus::Warning
            || self.momentum == BudgetStatus::Warning
            || self.constraints == BudgetStatus::Warning
    }

    /// Add a violation to the report
    pub fn add_violation(&mut self, violation: BudgetViolation) {
        self.violations.push(violation);
    }
}

impl Default for BudgetReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Budget validator for runtime enforcement
#[derive(Debug, Clone)]
pub struct BudgetValidator {
    config: BudgetConfig,
    energy_budget: EnergyBudget,
    momentum_budget: MomentumBudget,
    constraint_budget: ConstraintBudget,
}

impl Default for BudgetValidator {
    fn default() -> Self {
        Self::new(BudgetConfig::default())
    }
}

impl BudgetValidator {
    /// Create a new validator with the given configuration
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            energy_budget: EnergyBudget::with_limit(config.max_energy_drift),
            momentum_budget: MomentumBudget::with_limit(config.max_momentum_drift),
            constraint_budget: ConstraintBudget::default(),
            config,
        }
    }

    /// Validate a simulation state against all budgets
    pub fn validate(&mut self, state: &SimulationStateSnapshot) -> BudgetReport {
        let mut report = BudgetReport::new();

        // Update and check energy budget
        self.energy_budget.update(state.total_energy, state.wall_clock_seconds);
        report.energy = self.energy_budget.check(state.wall_clock_seconds);

        if report.energy == BudgetStatus::Exceeded {
            report.add_violation(BudgetViolation::EnergyDrift {
                actual: state.energy_drift_ratio(),
                limit: self.config.max_energy_drift,
            });
        }

        // Update and check momentum budget
        self.momentum_budget.update(state.total_momentum);
        report.momentum = self.momentum_budget.check();

        if report.momentum == BudgetStatus::Exceeded {
            report.add_violation(BudgetViolation::MomentumDrift {
                actual: state.momentum_drift_ratio(),
                limit: self.config.max_momentum_drift,
            });
        }

        // Update and check constraint budget
        self.constraint_budget.update(state.constraint_violations, state.max_penetration);
        report.constraints = self.constraint_budget.check();

        if report.constraints == BudgetStatus::Exceeded {
            report.add_violation(BudgetViolation::ConstraintViolation {
                count: state.constraint_violations,
                max_penetration: state.max_penetration,
            });
        }

        // Check numerical stability
        report.numerical_stability = !state.has_numerical_errors;
        if state.has_numerical_errors && !self.config.allow_numerical_errors {
            report.add_violation(BudgetViolation::NumericalInstability {
                description: "NaN or Inf detected in state".to_string(),
            });
        }

        report
    }

    /// Validate an agent's action magnitude
    pub fn validate_action_magnitude(&self, agent_id: u64, magnitude: f64) -> Option<BudgetViolation> {
        if magnitude > self.config.agent_budget.max_action_magnitude {
            Some(BudgetViolation::ActionMagnitude {
                agent_id,
                actual: magnitude,
                limit: self.config.agent_budget.max_action_magnitude,
            })
        } else {
            None
        }
    }

    /// Validate an agent's compute usage
    pub fn validate_compute(&self, agent_id: u64, flops: f64) -> Option<BudgetViolation> {
        if flops > self.config.agent_budget.max_compute_per_tick {
            Some(BudgetViolation::ComputeBudget {
                agent_id,
                actual: flops,
                limit: self.config.agent_budget.max_compute_per_tick,
            })
        } else {
            None
        }
    }

    /// Validate a policy update magnitude
    pub fn validate_policy_update(&self, agent_id: u64, update_norm: f64) -> Option<BudgetViolation> {
        if update_norm > self.config.learning_budget.max_policy_update_norm {
            Some(BudgetViolation::PolicyUpdateMagnitude {
                agent_id,
                actual: update_norm,
                limit: self.config.learning_budget.max_policy_update_norm,
            })
        } else {
            None
        }
    }

    /// Validate a reward signal
    pub fn validate_reward(&self, reward: f64) -> Option<BudgetViolation> {
        if reward.abs() > self.config.learning_budget.max_reward_magnitude {
            Some(BudgetViolation::RewardSignal {
                actual: reward,
                limit: self.config.learning_budget.max_reward_magnitude,
            })
        } else {
            None
        }
    }

    /// Clip a value to within bounds
    pub fn clip_action_magnitude(&self, magnitude: f64) -> f64 {
        magnitude.min(self.config.agent_budget.max_action_magnitude)
    }

    /// Clip a reward to within bounds
    pub fn clip_reward(&self, reward: f64) -> f64 {
        reward.clamp(
            -self.config.learning_budget.max_reward_magnitude,
            self.config.learning_budget.max_reward_magnitude,
        )
    }

    /// Clip a policy update to within bounds
    pub fn clip_policy_update(&self, update_norm: f64) -> f64 {
        update_norm.min(self.config.learning_budget.max_policy_update_norm)
    }

    /// Reset all tracking budgets
    pub fn reset(&mut self) {
        self.energy_budget = EnergyBudget::with_limit(self.config.max_energy_drift);
        self.momentum_budget = MomentumBudget::with_limit(self.config.max_momentum_drift);
        self.constraint_budget = ConstraintBudget::default();
    }

    /// Get the configuration
    pub fn config(&self) -> &BudgetConfig {
        &self.config
    }
}

// ============================================================================
// ADR-001 Part II-B: Reality Budget Validation Protocol
// ============================================================================

/// Full validation result from ADR-001 Budget Enforcement Protocol
#[derive(Debug, Clone)]
pub struct ValidationProtocolResult {
    /// Overall validation passed
    pub passed: bool,
    /// Energy drift check result
    pub energy_drift_check: DriftCheckResult,
    /// Momentum drift check result
    pub momentum_drift_check: DriftCheckResult,
    /// Constraint violation check result
    pub constraint_check: ConstraintCheckResult,
    /// Numerical stability check result
    pub numerical_check: NumericalCheckResult,
    /// Recommended action based on results
    pub recommended_action: RecommendedAction,
}

/// Result of a drift check (energy or momentum)
#[derive(Debug, Clone)]
pub struct DriftCheckResult {
    /// Check passed
    pub passed: bool,
    /// Actual drift value
    pub actual_drift: f64,
    /// Budget limit
    pub budget_limit: f64,
    /// Ratio of actual to limit
    pub utilization_ratio: f64,
}

/// Result of constraint violation check
#[derive(Debug, Clone)]
pub struct ConstraintCheckResult {
    /// Check passed (zero violations)
    pub passed: bool,
    /// Number of violations detected
    pub violation_count: u32,
    /// Maximum penetration depth
    pub max_penetration: f64,
    /// Violation entities (by ID)
    pub violation_entities: Vec<(u64, u64)>,
}

/// Result of numerical stability check
#[derive(Debug, Clone)]
pub struct NumericalCheckResult {
    /// Check passed (no NaN/Inf)
    pub passed: bool,
    /// Number of NaN values detected
    pub nan_count: u32,
    /// Number of Inf values detected
    pub inf_count: u32,
    /// Indices where issues were found
    pub issue_indices: Vec<usize>,
}

/// Recommended action after validation
#[derive(Debug, Clone, PartialEq)]
pub enum RecommendedAction {
    /// All good, continue simulation
    Continue,
    /// Log warning but continue
    LogWarning { reason: String },
    /// Reduce timestep to improve stability
    ReduceTimestep { factor: f64 },
    /// Force separation to resolve penetrations
    ForceSeparation { entity_pairs: Vec<(u64, u64)> },
    /// Hard error - must rollback
    Rollback { reason: String },
    /// Emergency state rollback due to numerical issues
    EmergencyRollback { reason: String },
}

/// Convenience function to validate reality budgets (from ADR-001)
///
/// This is the main entry point for budget validation as specified in ADR-001 Part II-B.
/// It performs the full validation protocol and returns detailed results.
///
/// # Budget Limits (ADR-001)
///
/// | Budget | Limit | Failure Mode |
/// |--------|-------|--------------|
/// | Energy Drift | <0.01% per minute | Log warning, reduce timestep |
/// | Momentum Drift | <10^-6 per 10,000 steps | Hard error, rollback |
/// | Constraint Violation | 0 penetrations | Force separation, emit witness |
/// | Numerical Stability | No NaN/Inf | Emergency state rollback |
///
/// # Example
///
/// ```rust,ignore
/// let result = validate_reality_budgets(&state, &config);
/// match result.recommended_action {
///     RecommendedAction::Continue => { /* all good */ }
///     RecommendedAction::Rollback { reason } => {
///         restore_from_checkpoint();
///     }
///     _ => handle_warning(),
/// }
/// ```
pub fn validate_reality_budgets(state: &SimulationStateSnapshot, config: &BudgetConfig) -> BudgetReport {
    let mut validator = BudgetValidator::new(config.clone());
    validator.validate(state)
}

/// Full ADR-001 validation protocol with detailed results
///
/// This implements the complete budget enforcement validation protocol from ADR-001 Part II-B.
pub fn validate_reality_budgets_full(
    state: &SimulationStateSnapshot,
    config: &BudgetConfig,
) -> ValidationProtocolResult {
    // Energy drift check (<0.01% per minute)
    let energy_drift = state.energy_drift_ratio();
    let energy_passed = energy_drift <= config.max_energy_drift;
    let energy_check = DriftCheckResult {
        passed: energy_passed,
        actual_drift: energy_drift,
        budget_limit: config.max_energy_drift,
        utilization_ratio: energy_drift / config.max_energy_drift,
    };

    // Momentum drift check (<10^-6 per 10,000 steps)
    let momentum_drift = state.momentum_drift_ratio();
    let momentum_passed = momentum_drift <= config.max_momentum_drift;
    let momentum_check = DriftCheckResult {
        passed: momentum_passed,
        actual_drift: momentum_drift,
        budget_limit: config.max_momentum_drift,
        utilization_ratio: momentum_drift / config.max_momentum_drift,
    };

    // Constraint violation check (0 penetrations)
    let constraint_passed = state.constraint_violations == 0;
    let constraint_check = ConstraintCheckResult {
        passed: constraint_passed,
        violation_count: state.constraint_violations,
        max_penetration: state.max_penetration,
        violation_entities: Vec::new(), // Would be populated by detailed check
    };

    // Numerical stability check (no NaN/Inf)
    let numerical_passed = !state.has_numerical_errors;
    let numerical_check = NumericalCheckResult {
        passed: numerical_passed,
        nan_count: 0,  // Would be populated by detailed check
        inf_count: 0,
        issue_indices: Vec::new(),
    };

    // Determine recommended action based on results
    let recommended_action = if !numerical_passed {
        RecommendedAction::EmergencyRollback {
            reason: "NaN or Inf detected - numerical instability".to_string(),
        }
    } else if !momentum_passed {
        RecommendedAction::Rollback {
            reason: format!(
                "Momentum drift {:.2e} exceeds limit {:.2e}",
                momentum_drift, config.max_momentum_drift
            ),
        }
    } else if !constraint_passed {
        RecommendedAction::ForceSeparation {
            entity_pairs: Vec::new(), // Would be populated
        }
    } else if !energy_passed {
        if energy_check.utilization_ratio > 10.0 {
            // Drift spike - trigger rollback
            RecommendedAction::Rollback {
                reason: format!(
                    "Energy drift spike: {:.4}% (>10x budget)",
                    energy_drift * 100.0
                ),
            }
        } else {
            RecommendedAction::ReduceTimestep { factor: 0.5 }
        }
    } else if energy_check.utilization_ratio > 0.8 {
        RecommendedAction::LogWarning {
            reason: format!(
                "Energy drift at {:.1}% of budget",
                energy_check.utilization_ratio * 100.0
            ),
        }
    } else {
        RecommendedAction::Continue
    };

    let passed = energy_passed && momentum_passed && constraint_passed && numerical_passed;

    ValidationProtocolResult {
        passed,
        energy_drift_check: energy_check,
        momentum_drift_check: momentum_check,
        constraint_check,
        numerical_check,
        recommended_action,
    }
}

/// Check for NaN/Inf in a slice of f32 values
///
/// Returns indices of problematic values.
pub fn check_numerical_stability_f32(values: &[f32]) -> Vec<usize> {
    values
        .iter()
        .enumerate()
        .filter(|(_, &v)| v.is_nan() || v.is_infinite())
        .map(|(i, _)| i)
        .collect()
}

/// Check for NaN/Inf in a slice of f64 values
///
/// Returns indices of problematic values.
pub fn check_numerical_stability_f64(values: &[f64]) -> Vec<usize> {
    values
        .iter()
        .enumerate()
        .filter(|(_, &v)| v.is_nan() || v.is_infinite())
        .map(|(i, _)| i)
        .collect()
}

/// Clip gradient to maximum L2 norm (for learning safety bounds)
///
/// ADR-001 Learning Safety: Max policy update magnitude.
pub fn clip_gradient_norm(gradient: &mut [f64], max_norm: f64) -> f64 {
    let norm_sq: f64 = gradient.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();

    if norm > max_norm {
        let scale = max_norm / norm;
        for g in gradient.iter_mut() {
            *g *= scale;
        }
    }

    norm.min(max_norm)
}

/// Clip gradient to maximum L2 norm (f32 version)
pub fn clip_gradient_norm_f32(gradient: &mut [f32], max_norm: f32) -> f32 {
    let norm_sq: f32 = gradient.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();

    if norm > max_norm {
        let scale = max_norm / norm;
        for g in gradient.iter_mut() {
            *g *= scale;
        }
    }

    norm.min(max_norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_status_check() {
        assert_eq!(BudgetStatus::check(0.5, 1.0), BudgetStatus::Ok);
        assert_eq!(BudgetStatus::check(0.9, 1.0), BudgetStatus::Warning);
        assert_eq!(BudgetStatus::check(1.5, 1.0), BudgetStatus::Exceeded);
    }

    #[test]
    fn test_energy_budget() {
        let mut budget = EnergyBudget::with_limit(0.01);

        budget.update(100.0, 0.0);  // Reference
        budget.update(100.0, 60.0); // 1 minute later, no drift

        assert_eq!(budget.check(60.0), BudgetStatus::Ok);
    }

    #[test]
    fn test_energy_budget_exceeded() {
        let mut budget = EnergyBudget::with_limit(0.01);

        budget.update(100.0, 0.0);  // Reference
        budget.update(110.0, 60.0); // 10% drift in 1 minute

        assert_eq!(budget.check(60.0), BudgetStatus::Exceeded);
    }

    #[test]
    fn test_momentum_budget() {
        let mut budget = MomentumBudget::with_limit(1e-6);

        budget.update(100.0); // Reference
        for _ in 0..1000 {
            budget.update(100.0); // No drift
        }

        assert_eq!(budget.check(), BudgetStatus::Ok);
    }

    #[test]
    fn test_constraint_budget() {
        let mut budget = ConstraintBudget::default();

        budget.update(0, 0.0);
        assert_eq!(budget.check(), BudgetStatus::Ok);

        budget.update(1, 0.1);
        assert_eq!(budget.check(), BudgetStatus::Exceeded);
    }

    #[test]
    fn test_budget_validator() {
        let config = BudgetConfig::default();
        let mut validator = BudgetValidator::new(config);

        let state = SimulationStateSnapshot::new(0, 100.0, 100.0);
        let report = validator.validate(&state);

        assert!(report.is_valid());
    }

    #[test]
    fn test_budget_validator_violation() {
        let config = BudgetConfig::default();
        let mut validator = BudgetValidator::new(config);

        let mut state = SimulationStateSnapshot::new(0, 100.0, 100.0);
        state.constraint_violations = 5;
        state.max_penetration = 0.5;

        let report = validator.validate(&state);

        assert!(!report.is_valid());
        assert!(matches!(report.violations[0], BudgetViolation::ConstraintViolation { .. }));
    }

    #[test]
    fn test_validate_reality_budgets_function() {
        let config = BudgetConfig::default();
        let state = SimulationStateSnapshot::new(0, 100.0, 100.0);

        let report = validate_reality_budgets(&state, &config);
        assert!(report.is_valid());
    }

    #[test]
    fn test_budget_clipping() {
        let validator = BudgetValidator::default();

        assert_eq!(validator.clip_action_magnitude(500.0), 500.0);
        assert_eq!(validator.clip_action_magnitude(2000.0), 1000.0);

        assert_eq!(validator.clip_reward(50.0), 50.0);
        assert_eq!(validator.clip_reward(200.0), 100.0);
        assert_eq!(validator.clip_reward(-200.0), -100.0);
    }

    #[test]
    fn test_relaxed_and_strict_configs() {
        let relaxed = BudgetConfig::relaxed();
        let strict = BudgetConfig::strict();

        assert!(relaxed.max_energy_drift > strict.max_energy_drift);
        assert!(relaxed.max_constraint_violations > strict.max_constraint_violations);
    }

    // =========================================================================
    // ADR-001 Validation Protocol Tests
    // =========================================================================

    #[test]
    fn test_validate_reality_budgets_full_passing() {
        let config = BudgetConfig::default();
        let state = SimulationStateSnapshot::new(0, 100.0, 100.0);

        let result = validate_reality_budgets_full(&state, &config);

        assert!(result.passed);
        assert!(result.energy_drift_check.passed);
        assert!(result.momentum_drift_check.passed);
        assert!(result.constraint_check.passed);
        assert!(result.numerical_check.passed);
        assert_eq!(result.recommended_action, RecommendedAction::Continue);
    }

    #[test]
    fn test_validate_reality_budgets_full_numerical_error() {
        let config = BudgetConfig::default();
        let mut state = SimulationStateSnapshot::new(0, 100.0, 100.0);
        state.has_numerical_errors = true;

        let result = validate_reality_budgets_full(&state, &config);

        assert!(!result.passed);
        assert!(!result.numerical_check.passed);
        assert!(matches!(
            result.recommended_action,
            RecommendedAction::EmergencyRollback { .. }
        ));
    }

    #[test]
    fn test_validate_reality_budgets_full_constraint_violation() {
        let config = BudgetConfig::default();
        let mut state = SimulationStateSnapshot::new(0, 100.0, 100.0);
        state.constraint_violations = 3;
        state.max_penetration = 0.5;

        let result = validate_reality_budgets_full(&state, &config);

        assert!(!result.passed);
        assert!(!result.constraint_check.passed);
        assert!(matches!(
            result.recommended_action,
            RecommendedAction::ForceSeparation { .. }
        ));
    }

    #[test]
    fn test_validate_reality_budgets_full_energy_drift() {
        let config = BudgetConfig::default();
        // 5% drift (way over 0.01% limit)
        let state = SimulationStateSnapshot::new(0, 105.0, 100.0);

        let result = validate_reality_budgets_full(&state, &config);

        // Energy drift is over limit but might not trigger full failure
        // depending on calculation
        assert!(!result.energy_drift_check.passed || result.energy_drift_check.utilization_ratio > 1.0);
    }

    #[test]
    fn test_check_numerical_stability() {
        let values_ok: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let issues = check_numerical_stability_f32(&values_ok);
        assert!(issues.is_empty());

        let values_bad: Vec<f32> = vec![1.0, f32::NAN, 3.0, f32::INFINITY, 5.0];
        let issues = check_numerical_stability_f32(&values_bad);
        assert_eq!(issues.len(), 2);
        assert!(issues.contains(&1));
        assert!(issues.contains(&3));
    }

    #[test]
    fn test_clip_gradient_norm() {
        let mut gradient = vec![3.0, 4.0];  // L2 norm = 5.0
        let norm = clip_gradient_norm(&mut gradient, 2.5);

        assert!((norm - 2.5).abs() < 1e-10);
        let new_norm: f64 = gradient.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((new_norm - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_clip_gradient_norm_no_clip_needed() {
        let mut gradient = vec![0.3, 0.4];  // L2 norm = 0.5
        let norm = clip_gradient_norm(&mut gradient, 2.5);

        assert!((norm - 0.5).abs() < 1e-10);
        // Values should be unchanged
        assert!((gradient[0] - 0.3).abs() < 1e-10);
        assert!((gradient[1] - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_drift_check_result() {
        let result = DriftCheckResult {
            passed: true,
            actual_drift: 0.00005,
            budget_limit: 0.0001,
            utilization_ratio: 0.5,
        };

        assert!(result.passed);
        assert!(result.utilization_ratio < 1.0);
    }
}
