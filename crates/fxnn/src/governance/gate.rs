//! Action gating for the FXNN governance layer.
//!
//! This module provides mechanisms to control which actions agents can perform.
//! Each agent type/role has a whitelist of allowed actions, and all actions
//! are validated against bounds before execution.
//!
//! # Overview
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      ACTION GATING FLOW                         │
//! ├────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   Agent Request → ActionWhitelist → ActionValidator → Execute  │
//! │                          ↓                 ↓                    │
//! │                     Deny if not        Deny if bounds          │
//! │                     in whitelist       exceeded                 │
//! │                                                                 │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use fxnn::governance::{ActionWhitelist, ActionKind, ActionValidator, ActionBounds};
//!
//! // Create whitelist for role 0 (basic agent)
//! let mut whitelist = ActionWhitelist::default();
//! whitelist.allow(0, ActionKind::Move);
//! whitelist.allow(0, ActionKind::Observe);
//!
//! // Check if action is allowed
//! assert!(whitelist.is_allowed(0, &ActionKind::Move));
//! assert!(!whitelist.is_allowed(0, &ActionKind::Admin));
//! ```

use std::collections::{HashMap, HashSet};
use super::{GovernanceError, RoleId, ActionRequest};

/// Types of actions that agents can perform
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionKind {
    /// Movement action (apply force/velocity)
    Move,
    /// Apply force to another entity
    ApplyForce,
    /// Form or break bonds
    Bond,
    /// Observe environment
    Observe,
    /// Communicate with other agents
    Communicate,
    /// Write to memory
    MemoryWrite,
    /// Read from memory
    MemoryRead,
    /// Spawn new entity (privileged)
    Spawn,
    /// Destroy entity (privileged)
    Destroy,
    /// Modify simulation parameters (admin)
    Admin,
    /// Custom action with identifier
    Custom(String),
}

impl ActionKind {
    /// Check if this is a privileged action
    pub fn is_privileged(&self) -> bool {
        matches!(self, ActionKind::Spawn | ActionKind::Destroy | ActionKind::Admin)
    }

    /// Check if this is a physics-affecting action
    pub fn affects_physics(&self) -> bool {
        matches!(self, ActionKind::Move | ActionKind::ApplyForce | ActionKind::Bond)
    }
}

/// Trait for action gating implementations
pub trait ActionGate {
    /// Check if an action is authorized for the given context
    fn authorize(&self, role: RoleId, action: &ActionKind) -> bool;

    /// Deny an action and return the reason
    fn deny(&self, role: RoleId, action: &ActionKind) -> Option<String>;
}

/// Whitelist of allowed actions per role
#[derive(Debug, Clone)]
pub struct ActionWhitelist {
    /// Map from role ID to set of allowed actions
    allowed: HashMap<RoleId, HashSet<ActionKind>>,
    /// Global actions allowed for all roles
    global_allowed: HashSet<ActionKind>,
    /// Actions that are always denied
    global_denied: HashSet<ActionKind>,
}

impl Default for ActionWhitelist {
    fn default() -> Self {
        let mut whitelist = Self {
            allowed: HashMap::new(),
            global_allowed: HashSet::new(),
            global_denied: HashSet::new(),
        };

        // By default, Observe is allowed for all roles
        whitelist.global_allowed.insert(ActionKind::Observe);
        whitelist.global_allowed.insert(ActionKind::MemoryRead);

        whitelist
    }
}

impl ActionWhitelist {
    /// Create an empty whitelist
    pub fn new() -> Self {
        Self {
            allowed: HashMap::new(),
            global_allowed: HashSet::new(),
            global_denied: HashSet::new(),
        }
    }

    /// Allow an action for a specific role
    pub fn allow(&mut self, role: RoleId, action: ActionKind) {
        self.allowed
            .entry(role)
            .or_insert_with(HashSet::new)
            .insert(action);
    }

    /// Deny an action for a specific role
    pub fn deny(&mut self, role: RoleId, action: &ActionKind) {
        if let Some(actions) = self.allowed.get_mut(&role) {
            actions.remove(action);
        }
    }

    /// Allow an action globally for all roles
    pub fn allow_global(&mut self, action: ActionKind) {
        self.global_denied.remove(&action);
        self.global_allowed.insert(action);
    }

    /// Deny an action globally for all roles
    pub fn deny_global(&mut self, action: ActionKind) {
        self.global_allowed.remove(&action);
        self.global_denied.insert(action);
    }

    /// Check if an action is allowed for a role
    pub fn is_allowed(&self, role: RoleId, action: &ActionKind) -> bool {
        // Check global deny first
        if self.global_denied.contains(action) {
            return false;
        }

        // Check global allow
        if self.global_allowed.contains(action) {
            return true;
        }

        // Check role-specific whitelist
        self.allowed
            .get(&role)
            .map(|actions| actions.contains(action))
            .unwrap_or(false)
    }

    /// Get all allowed actions for a role
    pub fn get_allowed(&self, role: RoleId) -> Vec<ActionKind> {
        let mut actions: Vec<_> = self.global_allowed.iter().cloned().collect();

        if let Some(role_actions) = self.allowed.get(&role) {
            for action in role_actions {
                if !self.global_denied.contains(action) && !actions.contains(action) {
                    actions.push(action.clone());
                }
            }
        }

        actions
    }

    /// Create a preset for basic agents (limited actions)
    pub fn basic_agent_preset() -> Self {
        let mut whitelist = Self::default();
        let role = 0; // Basic agent role

        whitelist.allow(role, ActionKind::Move);
        whitelist.allow(role, ActionKind::ApplyForce);
        whitelist.allow(role, ActionKind::Communicate);

        whitelist
    }

    /// Create a preset for privileged agents
    pub fn privileged_agent_preset() -> Self {
        let mut whitelist = Self::basic_agent_preset();
        let role = 1; // Privileged role

        // Copy basic permissions
        for action in whitelist.get_allowed(0) {
            whitelist.allow(role, action);
        }

        // Add privileged actions
        whitelist.allow(role, ActionKind::Bond);
        whitelist.allow(role, ActionKind::MemoryWrite);

        whitelist
    }

    /// Create a preset for admin agents
    pub fn admin_preset() -> Self {
        let mut whitelist = Self::privileged_agent_preset();
        let role = 2; // Admin role

        // Copy privileged permissions
        for action in whitelist.get_allowed(1) {
            whitelist.allow(role, action);
        }

        // Add admin actions
        whitelist.allow(role, ActionKind::Spawn);
        whitelist.allow(role, ActionKind::Destroy);
        whitelist.allow(role, ActionKind::Admin);

        whitelist
    }
}

impl ActionGate for ActionWhitelist {
    fn authorize(&self, role: RoleId, action: &ActionKind) -> bool {
        self.is_allowed(role, action)
    }

    fn deny(&self, role: RoleId, action: &ActionKind) -> Option<String> {
        if !self.is_allowed(role, action) {
            Some(format!("Action {:?} not in whitelist for role {}", action, role))
        } else {
            None
        }
    }
}

/// Bounds for action parameters
#[derive(Debug, Clone)]
pub struct ActionBounds {
    /// Maximum force magnitude (for ApplyForce)
    pub max_force: f64,
    /// Maximum velocity magnitude (for Move)
    pub max_velocity: f64,
    /// Maximum communication distance
    pub max_comm_distance: f64,
    /// Maximum energy per action
    pub max_energy_per_action: f64,
    /// Maximum bond strength
    pub max_bond_strength: f64,
}

impl Default for ActionBounds {
    fn default() -> Self {
        Self {
            max_force: 1000.0,         // In simulation units
            max_velocity: 100.0,       // In simulation units
            max_comm_distance: 50.0,   // In simulation units
            max_energy_per_action: 100.0,
            max_bond_strength: 500.0,
        }
    }
}

/// Validator for action parameters
#[derive(Debug, Clone)]
pub struct ActionValidator {
    /// Bounds for action parameters
    bounds: ActionBounds,
    /// Whether to clamp values instead of rejecting
    clamp_instead_of_reject: bool,
}

impl Default for ActionValidator {
    fn default() -> Self {
        Self {
            bounds: ActionBounds::default(),
            clamp_instead_of_reject: false,
        }
    }
}

impl ActionValidator {
    /// Create a new validator with custom bounds
    pub fn new(bounds: ActionBounds) -> Self {
        Self {
            bounds,
            clamp_instead_of_reject: false,
        }
    }

    /// Enable clamping mode (values are clamped instead of rejected)
    pub fn with_clamping(mut self) -> Self {
        self.clamp_instead_of_reject = true;
        self
    }

    /// Validate an action request
    pub fn validate(&self, action: &ActionRequest) -> Result<(), GovernanceError> {
        // Check magnitude bounds for physics actions
        if let Some(magnitude) = action.magnitude {
            match &action.kind {
                ActionKind::ApplyForce => {
                    if magnitude > self.bounds.max_force {
                        return Err(GovernanceError::ActionBoundsExceeded {
                            action: action.kind.clone(),
                            bound: "max_force".to_string(),
                            actual: magnitude,
                            max: self.bounds.max_force,
                        });
                    }
                }
                ActionKind::Move => {
                    if magnitude > self.bounds.max_velocity {
                        return Err(GovernanceError::ActionBoundsExceeded {
                            action: action.kind.clone(),
                            bound: "max_velocity".to_string(),
                            actual: magnitude,
                            max: self.bounds.max_velocity,
                        });
                    }
                }
                ActionKind::Bond => {
                    if magnitude > self.bounds.max_bond_strength {
                        return Err(GovernanceError::ActionBoundsExceeded {
                            action: action.kind.clone(),
                            bound: "max_bond_strength".to_string(),
                            actual: magnitude,
                            max: self.bounds.max_bond_strength,
                        });
                    }
                }
                _ => {}
            }
        }

        // Check energy cost
        if action.energy_cost > self.bounds.max_energy_per_action {
            return Err(GovernanceError::ActionBoundsExceeded {
                action: action.kind.clone(),
                bound: "max_energy_per_action".to_string(),
                actual: action.energy_cost,
                max: self.bounds.max_energy_per_action,
            });
        }

        Ok(())
    }

    /// Clamp an action's magnitude to within bounds
    pub fn clamp_magnitude(&self, action: &mut ActionRequest) {
        if let Some(magnitude) = action.magnitude.as_mut() {
            *magnitude = match &action.kind {
                ActionKind::ApplyForce => magnitude.min(self.bounds.max_force),
                ActionKind::Move => magnitude.min(self.bounds.max_velocity),
                ActionKind::Bond => magnitude.min(self.bounds.max_bond_strength),
                _ => *magnitude,
            };
        }

        action.energy_cost = action.energy_cost.min(self.bounds.max_energy_per_action);
    }

    /// Get the bounds
    pub fn bounds(&self) -> &ActionBounds {
        &self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_whitelist_default() {
        let whitelist = ActionWhitelist::default();

        // Observe should be globally allowed
        assert!(whitelist.is_allowed(0, &ActionKind::Observe));
        assert!(whitelist.is_allowed(99, &ActionKind::Observe));

        // Move should not be allowed by default
        assert!(!whitelist.is_allowed(0, &ActionKind::Move));
    }

    #[test]
    fn test_action_whitelist_allow() {
        let mut whitelist = ActionWhitelist::default();
        whitelist.allow(0, ActionKind::Move);

        assert!(whitelist.is_allowed(0, &ActionKind::Move));
        assert!(!whitelist.is_allowed(1, &ActionKind::Move));
    }

    #[test]
    fn test_action_whitelist_global_deny() {
        let mut whitelist = ActionWhitelist::default();
        whitelist.allow(0, ActionKind::Move);
        whitelist.deny_global(ActionKind::Move);

        assert!(!whitelist.is_allowed(0, &ActionKind::Move));
    }

    #[test]
    fn test_action_validator_bounds() {
        let validator = ActionValidator::default();

        // Within bounds
        let action = ActionRequest::new(ActionKind::ApplyForce)
            .with_magnitude(500.0);
        assert!(validator.validate(&action).is_ok());

        // Exceeds bounds
        let action = ActionRequest::new(ActionKind::ApplyForce)
            .with_magnitude(2000.0);
        assert!(matches!(
            validator.validate(&action),
            Err(GovernanceError::ActionBoundsExceeded { .. })
        ));
    }

    #[test]
    fn test_action_validator_clamp() {
        let validator = ActionValidator::default();

        let mut action = ActionRequest::new(ActionKind::ApplyForce)
            .with_magnitude(2000.0);
        validator.clamp_magnitude(&mut action);

        assert_eq!(action.magnitude, Some(1000.0));
    }

    #[test]
    fn test_action_kind_privileged() {
        assert!(ActionKind::Admin.is_privileged());
        assert!(ActionKind::Spawn.is_privileged());
        assert!(!ActionKind::Move.is_privileged());
    }

    #[test]
    fn test_action_kind_physics() {
        assert!(ActionKind::Move.affects_physics());
        assert!(ActionKind::ApplyForce.affects_physics());
        assert!(!ActionKind::Observe.affects_physics());
    }

    #[test]
    fn test_basic_agent_preset() {
        let whitelist = ActionWhitelist::basic_agent_preset();

        assert!(whitelist.is_allowed(0, &ActionKind::Move));
        assert!(whitelist.is_allowed(0, &ActionKind::Observe));
        assert!(!whitelist.is_allowed(0, &ActionKind::Admin));
    }

    #[test]
    fn test_admin_preset() {
        let whitelist = ActionWhitelist::admin_preset();

        assert!(whitelist.is_allowed(2, &ActionKind::Move));
        assert!(whitelist.is_allowed(2, &ActionKind::Admin));
        assert!(whitelist.is_allowed(2, &ActionKind::Spawn));
    }
}
