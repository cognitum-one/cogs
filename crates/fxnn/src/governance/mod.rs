//! # Governance Layer (Layer 5) for FXNN Simulated Reality
//!
//! The governance module implements Layer 5 of the FXNN Reality Stack, providing
//! mechanisms to prevent **software violations** in addition to physics violations.
//!
//! ## Overview
//!
//! While Layers 1-4 ensure physical consistency (forces, conservation laws, etc.),
//! the governance layer ensures that agents and system components cannot bypass
//! rules through software means.
//!
//! ## Key Components
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`gate`] | Action gating and whitelisting per agent type |
//! | [`permissions`] | Role-based access control and capability tokens |
//! | [`audit`] | Append-only audit logging and witness records |
//! | [`budget`] | Reality budget enforcement (energy, momentum, constraints) |
//!
//! ## Governance Functions
//!
//! The governance layer intercepts all state modifications:
//!
//! ```rust,ignore
//! impl GovernanceLayer {
//!     fn authorize_action(&self, agent: &Agent, action: &Action) -> Result<(), GovernanceError>;
//!     fn authorize_memory_write(&self, agent: &Agent, target: &MemoryRegion) -> bool;
//! }
//! ```
//!
//! ## Integration with Reality Stack
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────┐
//! │  LAYER 5: GOVERNANCE                                         │
//! │  ├── Tool and action gating (what can be modified)           │
//! │  ├── Write permissions into memory                           │
//! │  ├── Policy for rollbacks and state recovery                 │
//! │  ├── Authority boundaries between agents                     │
//! │  ├── Audit log and witness records                           │
//! │  └── Budget enforcement (energy, momentum, constraints)      │
//! └───────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use fxnn::governance::{GovernanceLayer, ActionGate, BudgetValidator};
//!
//! let governance = GovernanceLayer::new()
//!     .with_budget_config(BudgetConfig::default())
//!     .with_action_whitelist(ActionWhitelist::default());
//!
//! // Authorize an action before execution
//! match governance.authorize_action(&agent, &action) {
//!     Ok(()) => execute_action(&action),
//!     Err(e) => governance.emit_witness(e),
//! }
//! ```
//!
//! ## Reality Budgets
//!
//! The governance layer enforces the following budgets from ADR-001:
//!
//! | Budget | Limit | Failure Mode |
//! |--------|-------|--------------|
//! | Energy Drift | <0.01% per minute | Log warning, reduce timestep |
//! | Momentum Drift | <10⁻⁶ relative error | Hard error, rollback |
//! | Constraint Violation | 0 penetrations | Force separation, emit witness |
//! | Numerical Stability | No NaN/Inf | Emergency state rollback |
//!
//! ## References
//!
//! - ADR-001: FXNN as Simulated Reality Substrate
//! - Part II-B: Reality Budgets
//! - Part VIII: Architecture (Layer 5 Details)

mod gate;
mod permissions;
mod audit;
mod budget;

pub use gate::{ActionGate, ActionKind, ActionWhitelist, ActionValidator, ActionBounds};
pub use permissions::{Permission, Role, MemoryRegion, CapabilityToken, MemoryPermissions};
pub use audit::{AuditLog, AuditEntry, AuditResult, WitnessEventType, WitnessRecord};
pub use budget::{
    BudgetConfig, BudgetValidator, BudgetViolation, BudgetStatus, BudgetReport,
    EnergyBudget, MomentumBudget, ConstraintBudget, AgentBudget, LearningBudget,
    validate_reality_budgets, validate_reality_budgets_full,
    ValidationProtocolResult, DriftCheckResult, ConstraintCheckResult, NumericalCheckResult,
    RecommendedAction, check_numerical_stability_f32, check_numerical_stability_f64,
    clip_gradient_norm, clip_gradient_norm_f32,
};

use crate::error::FxnnError;

/// Agent identifier type
pub type AgentId = u64;

/// Role identifier type
pub type RoleId = u32;

/// Error type for governance operations
#[derive(Debug, Clone)]
pub enum GovernanceError {
    /// Action is not in the agent's allowed set
    UnauthorizedAction {
        /// Agent attempting the action
        agent_id: AgentId,
        /// Role of the agent
        role: RoleId,
        /// Action that was denied
        action: ActionKind,
    },
    /// Action exceeds the agent's budget
    BudgetExceeded {
        /// Agent attempting the action
        agent_id: AgentId,
        /// Type of budget exceeded
        budget_type: String,
        /// Requested amount
        requested: f64,
        /// Available amount
        available: f64,
    },
    /// Agent lacks permission for memory region
    MemoryAccessDenied {
        /// Agent attempting the write
        agent_id: AgentId,
        /// Target memory region
        region_id: u64,
        /// Required permission
        required: Permission,
    },
    /// Invalid or expired capability token
    InvalidCapability {
        /// Description of the issue
        reason: String,
    },
    /// Reality budget violation
    RealityBudgetViolation {
        /// The specific violation
        violation: BudgetViolation,
    },
    /// Action bounds exceeded
    ActionBoundsExceeded {
        /// The action
        action: ActionKind,
        /// The bound that was exceeded
        bound: String,
        /// Actual value
        actual: f64,
        /// Maximum allowed
        max: f64,
    },
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnauthorizedAction { agent_id, role, action } => {
                write!(f, "Agent {} (role {}) not authorized for action {:?}", agent_id, role, action)
            }
            Self::BudgetExceeded { agent_id, budget_type, requested, available } => {
                write!(f, "Agent {} exceeded {} budget: requested {}, available {}",
                       agent_id, budget_type, requested, available)
            }
            Self::MemoryAccessDenied { agent_id, region_id, required } => {
                write!(f, "Agent {} denied {:?} access to memory region {}",
                       agent_id, required, region_id)
            }
            Self::InvalidCapability { reason } => {
                write!(f, "Invalid capability token: {}", reason)
            }
            Self::RealityBudgetViolation { violation } => {
                write!(f, "Reality budget violation: {:?}", violation)
            }
            Self::ActionBoundsExceeded { action, bound, actual, max } => {
                write!(f, "Action {:?} exceeded {} bound: {} > {}", action, bound, actual, max)
            }
        }
    }
}

impl std::error::Error for GovernanceError {}

impl From<GovernanceError> for FxnnError {
    fn from(e: GovernanceError) -> Self {
        FxnnError::InvalidParameter(e.to_string())
    }
}

/// Agent information required for governance decisions
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// Unique agent identifier
    pub id: AgentId,
    /// Agent's role (determines permissions)
    pub role: RoleId,
    /// Remaining energy budget for this tick
    pub remaining_energy_budget: f64,
    /// Remaining compute budget for this tick
    pub remaining_compute_budget: f64,
    /// Remaining memory write budget for this tick
    pub remaining_memory_writes: u32,
}

impl AgentInfo {
    /// Create a new agent with the given ID and role
    pub fn new(id: AgentId, role: RoleId) -> Self {
        Self {
            id,
            role,
            remaining_energy_budget: 100.0,
            remaining_compute_budget: 1000.0,
            remaining_memory_writes: 10,
        }
    }
}

/// Action request to be authorized
#[derive(Debug, Clone)]
pub struct ActionRequest {
    /// Type of action
    pub kind: ActionKind,
    /// Energy cost of the action
    pub energy_cost: f64,
    /// Compute cost of the action (FLOPs)
    pub compute_cost: f64,
    /// Target entity (if applicable)
    pub target_id: Option<u64>,
    /// Force/velocity magnitude (for physics actions)
    pub magnitude: Option<f64>,
}

impl ActionRequest {
    /// Create a new action request
    pub fn new(kind: ActionKind) -> Self {
        Self {
            kind,
            energy_cost: 0.0,
            compute_cost: 0.0,
            target_id: None,
            magnitude: None,
        }
    }

    /// Set the energy cost
    pub fn with_energy_cost(mut self, cost: f64) -> Self {
        self.energy_cost = cost;
        self
    }

    /// Set the compute cost
    pub fn with_compute_cost(mut self, cost: f64) -> Self {
        self.compute_cost = cost;
        self
    }

    /// Set the target entity
    pub fn with_target(mut self, target_id: u64) -> Self {
        self.target_id = Some(target_id);
        self
    }

    /// Set the magnitude
    pub fn with_magnitude(mut self, magnitude: f64) -> Self {
        self.magnitude = Some(magnitude);
        self
    }
}

/// The main governance layer that coordinates all governance functions
#[derive(Debug)]
pub struct GovernanceLayer {
    /// Action whitelist per role
    action_whitelist: ActionWhitelist,
    /// Action bounds validator
    action_validator: ActionValidator,
    /// Memory permissions
    memory_permissions: MemoryPermissions,
    /// Budget configuration
    budget_config: BudgetConfig,
    /// Budget validator
    budget_validator: BudgetValidator,
    /// Audit log
    audit_log: AuditLog,
}

impl Default for GovernanceLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl GovernanceLayer {
    /// Create a new governance layer with default configuration
    pub fn new() -> Self {
        Self {
            action_whitelist: ActionWhitelist::default(),
            action_validator: ActionValidator::default(),
            memory_permissions: MemoryPermissions::new(),
            budget_config: BudgetConfig::default(),
            budget_validator: BudgetValidator::default(),
            audit_log: AuditLog::new(),
        }
    }

    /// Set the budget configuration
    pub fn with_budget_config(mut self, config: BudgetConfig) -> Self {
        self.budget_config = config;
        self.budget_validator = BudgetValidator::new(self.budget_config.clone());
        self
    }

    /// Set the action whitelist
    pub fn with_action_whitelist(mut self, whitelist: ActionWhitelist) -> Self {
        self.action_whitelist = whitelist;
        self
    }

    /// Set the action validator
    pub fn with_action_validator(mut self, validator: ActionValidator) -> Self {
        self.action_validator = validator;
        self
    }

    /// Set the memory permissions
    pub fn with_memory_permissions(mut self, permissions: MemoryPermissions) -> Self {
        self.memory_permissions = permissions;
        self
    }

    /// Authorize an action before execution
    ///
    /// This is the main entry point for action authorization. It checks:
    /// 1. Action is in agent's allowed set (whitelist)
    /// 2. Action doesn't exceed budget
    /// 3. Action is within bounds (magnitude limits)
    ///
    /// If authorized, logs the action to the audit trail.
    pub fn authorize_action(
        &mut self,
        agent: &AgentInfo,
        action: &ActionRequest,
    ) -> std::result::Result<(), GovernanceError> {
        // Check action is in agent's allowed set
        if !self.action_whitelist.is_allowed(agent.role, &action.kind) {
            let error = GovernanceError::UnauthorizedAction {
                agent_id: agent.id,
                role: agent.role,
                action: action.kind.clone(),
            };
            self.emit_witness_for_error(&error);
            return Err(error);
        }

        // Check action bounds
        if let Err(bound_error) = self.action_validator.validate(action) {
            self.emit_witness_for_error(&bound_error);
            return Err(bound_error);
        }

        // Check energy budget
        if action.energy_cost > agent.remaining_energy_budget {
            let error = GovernanceError::BudgetExceeded {
                agent_id: agent.id,
                budget_type: "energy".to_string(),
                requested: action.energy_cost,
                available: agent.remaining_energy_budget,
            };
            self.emit_witness_for_error(&error);
            return Err(error);
        }

        // Check compute budget
        if action.compute_cost > agent.remaining_compute_budget {
            let error = GovernanceError::BudgetExceeded {
                agent_id: agent.id,
                budget_type: "compute".to_string(),
                requested: action.compute_cost,
                available: agent.remaining_compute_budget,
            };
            self.emit_witness_for_error(&error);
            return Err(error);
        }

        // Log the authorized action
        self.audit_log.append(AuditEntry::action_authorized(
            agent.id,
            action.kind.clone(),
            action.energy_cost,
        ));

        Ok(())
    }

    /// Check if an agent can write to a memory region
    pub fn authorize_memory_write(
        &mut self,
        agent: &AgentInfo,
        region: &MemoryRegion,
    ) -> std::result::Result<(), GovernanceError> {
        // Check remaining write budget
        if agent.remaining_memory_writes == 0 {
            let error = GovernanceError::BudgetExceeded {
                agent_id: agent.id,
                budget_type: "memory_writes".to_string(),
                requested: 1.0,
                available: 0.0,
            };
            self.emit_witness_for_error(&error);
            return Err(error);
        }

        // Check permissions
        if !self.memory_permissions.can_write(agent.id, region.id) {
            let error = GovernanceError::MemoryAccessDenied {
                agent_id: agent.id,
                region_id: region.id,
                required: Permission::Write,
            };
            self.emit_witness_for_error(&error);
            return Err(error);
        }

        // Log the memory access
        self.audit_log.append(AuditEntry::memory_write(
            agent.id,
            region.id,
        ));

        Ok(())
    }

    /// Validate reality budgets for the current simulation state
    pub fn validate_budgets(&mut self, state: &SimulationStateSnapshot) -> BudgetReport {
        let report = self.budget_validator.validate(state);

        // Emit witnesses for any violations
        for violation in &report.violations {
            self.audit_log.append(AuditEntry::budget_violation(violation.clone()));
        }

        report
    }

    /// Emit a witness record for an error
    fn emit_witness_for_error(&mut self, error: &GovernanceError) {
        let witness = WitnessRecord::from_governance_error(error);
        self.audit_log.append_witness(witness);
    }

    /// Emit a witness record for a custom event
    pub fn emit_witness(&mut self, witness: WitnessRecord) {
        self.audit_log.append_witness(witness);
    }

    /// Get the audit log for querying
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    /// Get the budget configuration
    pub fn budget_config(&self) -> &BudgetConfig {
        &self.budget_config
    }

    /// Get the action whitelist
    pub fn action_whitelist(&self) -> &ActionWhitelist {
        &self.action_whitelist
    }

    /// Get the memory permissions
    pub fn memory_permissions(&self) -> &MemoryPermissions {
        &self.memory_permissions
    }
}

/// Snapshot of simulation state for budget validation
#[derive(Debug, Clone)]
pub struct SimulationStateSnapshot {
    /// Current tick number
    pub tick: u64,
    /// Total energy of the system
    pub total_energy: f64,
    /// Reference energy (initial or equilibrium)
    pub reference_energy: f64,
    /// Total momentum vector (magnitude)
    pub total_momentum: f64,
    /// Reference momentum
    pub reference_momentum: f64,
    /// Number of constraint violations
    pub constraint_violations: u32,
    /// Maximum penetration depth
    pub max_penetration: f64,
    /// Whether any NaN or Inf values are present
    pub has_numerical_errors: bool,
    /// Wall clock time since start (seconds)
    pub wall_clock_seconds: f64,
}

impl SimulationStateSnapshot {
    /// Create a new state snapshot
    pub fn new(tick: u64, total_energy: f64, reference_energy: f64) -> Self {
        Self {
            tick,
            total_energy,
            reference_energy,
            total_momentum: 0.0,
            reference_momentum: 0.0,
            constraint_violations: 0,
            max_penetration: 0.0,
            has_numerical_errors: false,
            wall_clock_seconds: 0.0,
        }
    }

    /// Calculate the energy drift ratio
    pub fn energy_drift_ratio(&self) -> f64 {
        if self.reference_energy.abs() < 1e-10 {
            return 0.0;
        }
        (self.total_energy - self.reference_energy).abs() / self.reference_energy.abs()
    }

    /// Calculate the momentum drift ratio
    pub fn momentum_drift_ratio(&self) -> f64 {
        if self.reference_momentum.abs() < 1e-10 {
            return 0.0;
        }
        (self.total_momentum - self.reference_momentum).abs() / self.reference_momentum.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_layer_creation() {
        let governance = GovernanceLayer::new();
        assert!(governance.budget_config().max_energy_drift > 0.0);
    }

    #[test]
    fn test_action_authorization() {
        let mut governance = GovernanceLayer::new();

        // Add Move action to default role
        governance.action_whitelist.allow(0, ActionKind::Move);

        let agent = AgentInfo::new(1, 0);
        let action = ActionRequest::new(ActionKind::Move)
            .with_energy_cost(10.0);

        let result = governance.authorize_action(&agent, &action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unauthorized_action() {
        let mut governance = GovernanceLayer::new();

        let agent = AgentInfo::new(1, 0);
        let action = ActionRequest::new(ActionKind::Admin);

        let result = governance.authorize_action(&agent, &action);
        assert!(matches!(result, Err(GovernanceError::UnauthorizedAction { .. })));
    }

    #[test]
    fn test_budget_exceeded() {
        let mut governance = GovernanceLayer::new();
        governance.action_whitelist.allow(0, ActionKind::Move);

        let agent = AgentInfo {
            id: 1,
            role: 0,
            remaining_energy_budget: 5.0,
            remaining_compute_budget: 1000.0,
            remaining_memory_writes: 10,
        };

        let action = ActionRequest::new(ActionKind::Move)
            .with_energy_cost(10.0);

        let result = governance.authorize_action(&agent, &action);
        assert!(matches!(result, Err(GovernanceError::BudgetExceeded { .. })));
    }

    #[test]
    fn test_audit_logging() {
        let mut governance = GovernanceLayer::new();
        governance.action_whitelist.allow(0, ActionKind::Move);

        let agent = AgentInfo::new(1, 0);
        let action = ActionRequest::new(ActionKind::Move);

        let _ = governance.authorize_action(&agent, &action);

        let entries = governance.audit_log().query_by_agent(1);
        assert!(!entries.is_empty());
    }
}
