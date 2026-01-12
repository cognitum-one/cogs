//! # Layer 5: GOVERNANCE
//!
//! The governance layer provides safety and control mechanisms for agent actions.
//! Key features:
//!
//! - **Action Gating**: Validate and approve/reject actions before execution
//! - **Permissions**: Define what actions agents are allowed to take
//! - **Audit Logging**: Record all actions for accountability
//! - **Budget Enforcement**: Limit resource usage (energy, compute, actions)
//!
//! ## Design Philosophy
//!
//! The governance layer ensures that agents operate within defined boundaries,
//! providing safety guarantees and enabling trust in autonomous systems.

use crate::error::{FxnnError, Result};
use super::agency::{AgentId, ProposedAction, ValidatedAction, ActionKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// ============================================================================
// Core Types
// ============================================================================

/// Error type for governance operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum GovernanceError {
    /// Action denied
    #[error("Action denied: {reason}")]
    ActionDenied { reason: String },

    /// Budget exceeded
    #[error("Budget exceeded: {resource} (used: {used}, limit: {limit})")]
    BudgetExceeded {
        resource: String,
        used: f32,
        limit: f32,
    },

    /// Permission denied
    #[error("Permission denied: {action} requires {permission}")]
    PermissionDenied {
        action: String,
        permission: String,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {limit} actions per {window:?}")]
    RateLimitExceeded {
        limit: usize,
        window: Duration,
    },
}

// ============================================================================
// Permissions
// ============================================================================

/// Permission levels for actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum PermissionLevel {
    /// No permission
    #[default]
    None = 0,
    /// Read-only observation
    Observe = 1,
    /// Low-impact actions
    Act = 2,
    /// Moderate-impact actions
    Modify = 3,
    /// High-impact actions
    Control = 4,
    /// Full access
    Admin = 5,
}

/// Permission for a specific action type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    /// Permission name
    pub name: String,
    /// Required level
    pub level: PermissionLevel,
    /// Description
    pub description: String,
    /// Action patterns this permission covers
    pub covers: Vec<String>,
}

impl Permission {
    /// Create a new permission
    pub fn new(name: &str, level: PermissionLevel) -> Self {
        Self {
            name: name.to_string(),
            level,
            description: String::new(),
            covers: Vec::new(),
        }
    }

    /// Set description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Add covered action pattern
    pub fn covers_action(mut self, pattern: &str) -> Self {
        self.covers.push(pattern.to_string());
        self
    }
}

/// Permission set for an agent
#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    /// Granted permissions
    permissions: HashMap<String, PermissionLevel>,
    /// Maximum allowed level
    max_level: PermissionLevel,
    /// Denied action patterns
    denials: Vec<String>,
}

impl PermissionSet {
    /// Create a new permission set
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
            max_level: PermissionLevel::Act,
            denials: Vec::new(),
        }
    }

    /// Grant a permission
    pub fn grant(&mut self, permission: &str, level: PermissionLevel) {
        self.permissions.insert(permission.to_string(), level);
    }

    /// Revoke a permission
    pub fn revoke(&mut self, permission: &str) {
        self.permissions.remove(permission);
    }

    /// Deny an action pattern
    pub fn deny(&mut self, pattern: &str) {
        self.denials.push(pattern.to_string());
    }

    /// Check if action is permitted
    pub fn check(&self, action: &str, required_level: PermissionLevel) -> bool {
        // Check denials first
        for denial in &self.denials {
            if action.contains(denial) {
                return false;
            }
        }

        // Check if we have required level
        if required_level > self.max_level {
            return false;
        }

        // Check specific permission
        if let Some(&level) = self.permissions.get(action) {
            return level >= required_level;
        }

        // Check wildcard permissions
        for (perm, &level) in &self.permissions {
            if perm.ends_with('*') && action.starts_with(&perm[..perm.len()-1]) {
                if level >= required_level {
                    return true;
                }
            }
        }

        false
    }

    /// Set maximum level
    pub fn with_max_level(mut self, level: PermissionLevel) -> Self {
        self.max_level = level;
        self
    }
}

// ============================================================================
// Budget Management
// ============================================================================

/// Resource budget for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Energy budget
    pub energy: ResourceBudget,
    /// Action count budget
    pub actions: ResourceBudget,
    /// Compute budget (in abstract units)
    pub compute: ResourceBudget,
    /// Force magnitude budget
    pub force: ResourceBudget,
}

/// Budget for a single resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// Maximum allowed
    pub limit: f32,
    /// Currently used
    pub used: f32,
    /// Regeneration rate per timestep
    pub regen_rate: f32,
    /// Warning threshold (0-1)
    pub warning_threshold: f32,
}

impl ResourceBudget {
    /// Create a new resource budget
    pub fn new(limit: f32) -> Self {
        Self {
            limit,
            used: 0.0,
            regen_rate: 0.0,
            warning_threshold: 0.8,
        }
    }

    /// Set regeneration rate
    pub fn with_regen(mut self, rate: f32) -> Self {
        self.regen_rate = rate;
        self
    }

    /// Check if can use amount
    pub fn can_use(&self, amount: f32) -> bool {
        self.used + amount <= self.limit
    }

    /// Use amount from budget
    pub fn use_amount(&mut self, amount: f32) -> bool {
        if self.can_use(amount) {
            self.used += amount;
            true
        } else {
            false
        }
    }

    /// Regenerate budget
    pub fn regenerate(&mut self) {
        self.used = (self.used - self.regen_rate).max(0.0);
    }

    /// Get remaining
    pub fn remaining(&self) -> f32 {
        (self.limit - self.used).max(0.0)
    }

    /// Check if at warning level
    pub fn is_warning(&self) -> bool {
        self.used / self.limit >= self.warning_threshold
    }

    /// Reset budget
    pub fn reset(&mut self) {
        self.used = 0.0;
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            energy: ResourceBudget::new(100.0).with_regen(0.1),
            actions: ResourceBudget::new(1000.0).with_regen(10.0),
            compute: ResourceBudget::new(10000.0).with_regen(100.0),
            force: ResourceBudget::new(100.0).with_regen(1.0),
        }
    }
}

impl Budget {
    /// Create a new budget
    pub fn new() -> Self {
        Self::default()
    }

    /// Set energy limit
    pub fn with_energy_limit(mut self, limit: f32) -> Self {
        self.energy.limit = limit;
        self
    }

    /// Set action limit
    pub fn with_action_limit(mut self, limit: f32) -> Self {
        self.actions.limit = limit;
        self
    }

    /// Regenerate all budgets
    pub fn regenerate_all(&mut self) {
        self.energy.regenerate();
        self.actions.regenerate();
        self.compute.regenerate();
        self.force.regenerate();
    }

    /// Check if action can be executed within budget
    pub fn check_action(&self, action: &ProposedAction) -> Result<()> {
        // Check action count
        if !self.actions.can_use(1.0) {
            return Err(FxnnError::invalid_parameter("Action budget exceeded"));
        }

        // Check energy
        if !self.energy.can_use(action.energy_cost) {
            return Err(FxnnError::invalid_parameter("Energy budget exceeded"));
        }

        // Check force budget for force actions
        if let ActionKind::ApplyForce { force, .. } = &action.kind {
            let magnitude = (force[0].powi(2) + force[1].powi(2) + force[2].powi(2)).sqrt();
            if !self.force.can_use(magnitude) {
                return Err(FxnnError::invalid_parameter("Force budget exceeded"));
            }
        }

        Ok(())
    }

    /// Deduct budget for action
    pub fn deduct_action(&mut self, action: &ProposedAction) {
        self.actions.use_amount(1.0);
        self.energy.use_amount(action.energy_cost);

        if let ActionKind::ApplyForce { force, .. } = &action.kind {
            let magnitude = (force[0].powi(2) + force[1].powi(2) + force[2].powi(2)).sqrt();
            self.force.use_amount(magnitude);
        }
    }
}

// ============================================================================
// Audit Logging
// ============================================================================

/// Entry in the audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Entry ID
    pub id: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Agent that performed action
    pub agent_id: AgentId,
    /// Action that was performed
    pub action: ActionKind,
    /// Whether action was approved
    pub approved: bool,
    /// Reason for approval/denial
    pub reason: String,
    /// Budget state at time of action
    pub budget_snapshot: Option<Budget>,
}

/// Audit log for tracking actions
#[derive(Debug)]
pub struct AuditLog {
    /// Log entries
    entries: VecDeque<AuditEntry>,
    /// Maximum entries to keep
    max_entries: usize,
    /// Entry counter
    counter: u64,
    /// Persistent storage path
    storage_path: Option<String>,
}

impl AuditLog {
    /// Create a new in-memory audit log
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100000,
            counter: 0,
            storage_path: None,
        }
    }

    /// Create a persistent audit log
    pub fn persistent(path: &str) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100000,
            counter: 0,
            storage_path: Some(path.to_string()),
        }
    }

    /// Set maximum entries
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Log an action
    pub fn log(&mut self, agent_id: AgentId, action: ActionKind, approved: bool, reason: &str) {
        let entry = AuditEntry {
            id: self.counter,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            agent_id,
            action,
            approved,
            reason: reason.to_string(),
            budget_snapshot: None,
        };

        self.counter += 1;

        // Remove oldest if at capacity
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }

        self.entries.push_back(entry);

        // Persist if configured
        if let Some(_path) = &self.storage_path {
            // Would write to file here
        }
    }

    /// Log with budget snapshot
    pub fn log_with_budget(
        &mut self,
        agent_id: AgentId,
        action: ActionKind,
        approved: bool,
        reason: &str,
        budget: &Budget,
    ) {
        let mut entry = AuditEntry {
            id: self.counter,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            agent_id,
            action,
            approved,
            reason: reason.to_string(),
            budget_snapshot: Some(budget.clone()),
        };

        self.counter += 1;

        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }

        self.entries.push_back(entry);
    }

    /// Get recent entries
    pub fn recent(&self, n: usize) -> Vec<&AuditEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Get entries for agent
    pub fn for_agent(&self, agent_id: AgentId) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.agent_id == agent_id).collect()
    }

    /// Get denied entries
    pub fn denied(&self) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| !e.approved).collect()
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Action Gate
// ============================================================================

/// Rate limiter for actions
#[derive(Debug)]
struct RateLimiter {
    /// Action timestamps
    timestamps: VecDeque<Instant>,
    /// Window size
    window: Duration,
    /// Maximum actions per window
    limit: usize,
}

impl RateLimiter {
    fn new(limit: usize, window: Duration) -> Self {
        Self {
            timestamps: VecDeque::new(),
            window,
            limit,
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();

        // Remove old timestamps
        while let Some(&ts) = self.timestamps.front() {
            if now.duration_since(ts) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check limit
        if self.timestamps.len() >= self.limit {
            false
        } else {
            self.timestamps.push_back(now);
            true
        }
    }
}

/// Main action gating system
pub struct ActionGate {
    /// Permissions per agent
    agent_permissions: HashMap<AgentId, PermissionSet>,
    /// Budgets per agent
    agent_budgets: HashMap<AgentId, Budget>,
    /// Rate limiters per agent
    rate_limiters: HashMap<AgentId, RateLimiter>,
    /// Audit log
    audit_log: AuditLog,
    /// Default permissions for new agents
    default_permissions: PermissionSet,
    /// Default budget for new agents
    default_budget: Budget,
    /// Rate limit settings
    rate_limit: usize,
    rate_window: Duration,
    /// Validators
    validators: Vec<Box<dyn ActionValidator>>,
}

impl ActionGate {
    /// Create a new action gate
    pub fn new() -> Self {
        Self {
            agent_permissions: HashMap::new(),
            agent_budgets: HashMap::new(),
            rate_limiters: HashMap::new(),
            audit_log: AuditLog::new(),
            default_permissions: PermissionSet::new(),
            default_budget: Budget::default(),
            rate_limit: 100,
            rate_window: Duration::from_secs(1),
            validators: Vec::new(),
        }
    }

    /// Set default permissions
    pub fn with_permissions(mut self, permissions: PermissionSet) -> Self {
        self.default_permissions = permissions;
        self
    }

    /// Set audit log
    pub fn with_audit(mut self, audit_log: AuditLog) -> Self {
        self.audit_log = audit_log;
        self
    }

    /// Set rate limit
    pub fn with_rate_limit(mut self, limit: usize, window: Duration) -> Self {
        self.rate_limit = limit;
        self.rate_window = window;
        self
    }

    /// Add a validator
    pub fn with_validator(mut self, validator: Box<dyn ActionValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Register an agent
    pub fn register_agent(&mut self, agent_id: AgentId) {
        self.agent_permissions.insert(agent_id, self.default_permissions.clone());
        self.agent_budgets.insert(agent_id, self.default_budget.clone());
        self.rate_limiters.insert(agent_id, RateLimiter::new(self.rate_limit, self.rate_window));
    }

    /// Set permissions for agent
    pub fn set_permissions(&mut self, agent_id: AgentId, permissions: PermissionSet) {
        self.agent_permissions.insert(agent_id, permissions);
    }

    /// Set budget for agent
    pub fn set_budget(&mut self, agent_id: AgentId, budget: Budget) {
        self.agent_budgets.insert(agent_id, budget);
    }

    /// Validate a single action
    fn validate_action(&mut self, action: &ProposedAction) -> Result<()> {
        let agent_id = action.agent_id;

        // Ensure agent is registered
        if !self.agent_permissions.contains_key(&agent_id) {
            self.register_agent(agent_id);
        }

        // Check rate limit
        let rate_limiter = self.rate_limiters.get_mut(&agent_id).unwrap();
        if !rate_limiter.check() {
            return Err(FxnnError::invalid_parameter("Rate limit exceeded"));
        }

        // Check budget
        let budget = self.agent_budgets.get(&agent_id).unwrap();
        budget.check_action(action)?;

        // Check custom validators
        for validator in &self.validators {
            if let Err(e) = validator.validate(action) {
                return Err(e);
            }
        }

        Ok(())
    }

    /// Validate and approve/reject actions
    pub fn validate(&mut self, actions: Vec<ProposedAction>) -> Result<Vec<ValidatedAction>> {
        let mut validated = Vec::new();

        for action in actions {
            let agent_id = action.agent_id;

            match self.validate_action(&action) {
                Ok(()) => {
                    // Deduct budget
                    if let Some(budget) = self.agent_budgets.get_mut(&agent_id) {
                        budget.deduct_action(&action);
                    }

                    // Log approval
                    self.audit_log.log(agent_id, action.kind.clone(), true, "Approved");

                    // Create validated action
                    let validated_action = ValidatedAction::from_proposed(
                        action,
                        "approved".to_string(),
                    );
                    validated.push(validated_action);
                }
                Err(e) => {
                    // Log denial
                    self.audit_log.log(agent_id, action.kind.clone(), false, &e.to_string());
                }
            }
        }

        Ok(validated)
    }

    /// Check if action is permitted
    pub fn is_permitted(&self, action: &ProposedAction) -> bool {
        let agent_id = action.agent_id;

        // Check budget
        if let Some(budget) = self.agent_budgets.get(&agent_id) {
            if budget.check_action(action).is_err() {
                return false;
            }
        }

        true
    }

    /// Get budget for agent
    pub fn budget(&self, agent_id: AgentId) -> Option<&Budget> {
        self.agent_budgets.get(&agent_id)
    }

    /// Get default budget
    pub fn default_budget(&self) -> &Budget {
        &self.default_budget
    }

    /// Get audit log
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    /// Regenerate all budgets
    pub fn regenerate_budgets(&mut self) {
        for budget in self.agent_budgets.values_mut() {
            budget.regenerate_all();
        }
    }
}

impl Default for ActionGate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Action Validators
// ============================================================================

/// Trait for custom action validation
pub trait ActionValidator: Send + Sync {
    /// Validate an action
    fn validate(&self, action: &ProposedAction) -> Result<()>;

    /// Get validator name
    fn name(&self) -> &str;
}

/// Validator that limits maximum force
pub struct MaxForceValidator {
    max_force: f32,
}

impl MaxForceValidator {
    /// Create a new max force validator
    pub fn new(max_force: f32) -> Self {
        Self { max_force }
    }
}

impl ActionValidator for MaxForceValidator {
    fn validate(&self, action: &ProposedAction) -> Result<()> {
        if let ActionKind::ApplyForce { force, .. } = &action.kind {
            let magnitude = (force[0].powi(2) + force[1].powi(2) + force[2].powi(2)).sqrt();
            if magnitude > self.max_force {
                return Err(FxnnError::invalid_parameter(
                    format!("Force magnitude {} exceeds limit {}", magnitude, self.max_force)
                ));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "MaxForceValidator"
    }
}

/// Validator that limits maximum velocity changes
pub struct MaxVelocityValidator {
    max_velocity: f32,
}

impl MaxVelocityValidator {
    /// Create a new max velocity validator
    pub fn new(max_velocity: f32) -> Self {
        Self { max_velocity }
    }
}

impl ActionValidator for MaxVelocityValidator {
    fn validate(&self, action: &ProposedAction) -> Result<()> {
        if let ActionKind::SetVelocity { velocity, .. } = &action.kind {
            let magnitude = (velocity[0].powi(2) + velocity[1].powi(2) + velocity[2].powi(2)).sqrt();
            if magnitude > self.max_velocity {
                return Err(FxnnError::invalid_parameter(
                    format!("Velocity magnitude {} exceeds limit {}", magnitude, self.max_velocity)
                ));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "MaxVelocityValidator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_set() {
        let mut perms = PermissionSet::new();
        perms.grant("action.force", PermissionLevel::Act);
        perms.grant("action.*", PermissionLevel::Observe);

        assert!(perms.check("action.force", PermissionLevel::Act));
        assert!(perms.check("action.move", PermissionLevel::Observe));
        assert!(!perms.check("action.move", PermissionLevel::Act));
    }

    #[test]
    fn test_resource_budget() {
        let mut budget = ResourceBudget::new(100.0);

        assert!(budget.can_use(50.0));
        assert!(budget.use_amount(50.0));
        assert_eq!(budget.remaining(), 50.0);
        assert!(!budget.can_use(60.0));
    }

    #[test]
    fn test_action_gate() {
        let mut gate = ActionGate::new();

        let action = ProposedAction::new(
            AgentId(0),
            ActionKind::ApplyForce { atom_id: 0, force: [1.0, 0.0, 0.0] },
        );

        let validated = gate.validate(vec![action]).unwrap();
        assert_eq!(validated.len(), 1);
    }

    #[test]
    fn test_audit_log() {
        let mut log = AuditLog::new();

        log.log(AgentId(0), ActionKind::Noop, true, "Test");
        log.log(AgentId(0), ActionKind::Noop, false, "Denied");

        assert_eq!(log.len(), 2);
        assert_eq!(log.denied().len(), 1);
    }
}
