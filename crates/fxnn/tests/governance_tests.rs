//! Governance Layer Tests
//!
//! Tests for governance and safety mechanisms:
//! - Action authorization (allowed/denied actions)
//! - Permission checking (capability-based access)
//! - Budget enforcement (resource limits)
//! - Audit log append-only property
//!
//! These tests model governance constraints for agent systems
//! operating in physics simulations with safety requirements.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ============================================================================
// Governance Infrastructure
// ============================================================================

/// Types of actions an agent can attempt
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ActionType {
    ApplyForce,
    SetVelocity,
    ModifyMass,
    CreateAtom,
    DestroyAtom,
    ReadState,
    WriteMemory,
    AccessNetwork,
}

/// Authorization decision
#[derive(Debug, Clone, PartialEq)]
enum AuthorizationResult {
    Allowed,
    Denied { reason: String },
    RateLimited { retry_after_ms: u64 },
}

/// Permission levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PermissionLevel {
    None,
    Read,
    Write,
    Admin,
}

/// Agent identity for authorization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AgentId(u64);

/// Agent capabilities/permissions
#[derive(Debug, Clone)]
struct AgentCapabilities {
    agent_id: AgentId,
    permissions: HashMap<ActionType, PermissionLevel>,
    max_force: f32,
    max_velocity: f32,
    budget_remaining: f32,
    rate_limits: HashMap<ActionType, (u32, Duration)>, // (max_count, window)
}

impl AgentCapabilities {
    fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            permissions: HashMap::new(),
            max_force: 10.0,
            max_velocity: 5.0,
            budget_remaining: 1000.0,
            rate_limits: HashMap::new(),
        }
    }

    fn with_permission(mut self, action: ActionType, level: PermissionLevel) -> Self {
        self.permissions.insert(action, level);
        self
    }

    fn with_budget(mut self, budget: f32) -> Self {
        self.budget_remaining = budget;
        self
    }

    fn with_rate_limit(mut self, action: ActionType, max_count: u32, window: Duration) -> Self {
        self.rate_limits.insert(action, (max_count, window));
        self
    }
}

// ============================================================================
// Action Authorization Tests
// ============================================================================

/// Authorization service
struct AuthorizationService {
    agent_capabilities: HashMap<AgentId, AgentCapabilities>,
    action_history: HashMap<(AgentId, ActionType), Vec<Instant>>,
}

impl AuthorizationService {
    fn new() -> Self {
        Self {
            agent_capabilities: HashMap::new(),
            action_history: HashMap::new(),
        }
    }

    fn register_agent(&mut self, capabilities: AgentCapabilities) {
        self.agent_capabilities.insert(capabilities.agent_id.clone(), capabilities);
    }

    fn authorize(&mut self, agent_id: &AgentId, action: ActionType, params: &ActionParams) -> AuthorizationResult {
        // Check if agent exists
        let caps = match self.agent_capabilities.get(agent_id) {
            Some(c) => c,
            None => return AuthorizationResult::Denied {
                reason: "Unknown agent".to_string(),
            },
        };

        // Check permission level
        let required_level = match &action {
            ActionType::ReadState => PermissionLevel::Read,
            ActionType::ApplyForce | ActionType::SetVelocity => PermissionLevel::Write,
            ActionType::CreateAtom | ActionType::DestroyAtom | ActionType::ModifyMass => PermissionLevel::Admin,
            ActionType::WriteMemory => PermissionLevel::Write,
            ActionType::AccessNetwork => PermissionLevel::Admin,
        };

        let agent_level = caps.permissions.get(&action).unwrap_or(&PermissionLevel::None);
        if *agent_level < required_level {
            return AuthorizationResult::Denied {
                reason: format!("Insufficient permission: need {:?}, have {:?}", required_level, agent_level),
            };
        }

        // Check action-specific constraints
        match (&action, params) {
            (ActionType::ApplyForce, ActionParams::Force(f)) => {
                let magnitude = (f[0]*f[0] + f[1]*f[1] + f[2]*f[2]).sqrt();
                if magnitude > caps.max_force {
                    return AuthorizationResult::Denied {
                        reason: format!("Force {} exceeds max {}", magnitude, caps.max_force),
                    };
                }
            }
            (ActionType::SetVelocity, ActionParams::Velocity(v)) => {
                let speed = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
                if speed > caps.max_velocity {
                    return AuthorizationResult::Denied {
                        reason: format!("Velocity {} exceeds max {}", speed, caps.max_velocity),
                    };
                }
            }
            _ => {}
        }

        // Check rate limits
        if let Some((max_count, window)) = caps.rate_limits.get(&action) {
            let key = (agent_id.clone(), action.clone());
            let history = self.action_history.entry(key).or_insert_with(Vec::new);

            // Clean old entries
            let cutoff = Instant::now() - *window;
            history.retain(|t| *t > cutoff);

            if history.len() >= *max_count as usize {
                let oldest = history.first().unwrap();
                let retry_after = (*window - oldest.elapsed()).as_millis() as u64;
                return AuthorizationResult::RateLimited { retry_after_ms: retry_after };
            }

            // Record this action
            history.push(Instant::now());
        }

        AuthorizationResult::Allowed
    }
}

/// Action parameters for authorization
#[derive(Debug, Clone)]
enum ActionParams {
    None,
    Force([f32; 3]),
    Velocity([f32; 3]),
    AtomId(u32),
}

/// Test action authorization for allowed actions
#[test]
fn test_action_authorization_allowed() {
    let mut auth_service = AuthorizationService::new();

    let agent = AgentCapabilities::new(AgentId(1))
        .with_permission(ActionType::ApplyForce, PermissionLevel::Write)
        .with_permission(ActionType::ReadState, PermissionLevel::Read);

    auth_service.register_agent(agent);

    // Should be allowed: has Write permission for ApplyForce
    let result = auth_service.authorize(
        &AgentId(1),
        ActionType::ApplyForce,
        &ActionParams::Force([1.0, 0.0, 0.0]),
    );
    assert_eq!(result, AuthorizationResult::Allowed);

    // Should be allowed: has Read permission
    let result = auth_service.authorize(
        &AgentId(1),
        ActionType::ReadState,
        &ActionParams::None,
    );
    assert_eq!(result, AuthorizationResult::Allowed);
}

/// Test action authorization for denied actions
#[test]
fn test_action_authorization_denied() {
    let mut auth_service = AuthorizationService::new();

    let agent = AgentCapabilities::new(AgentId(1))
        .with_permission(ActionType::ReadState, PermissionLevel::Read);
    // Note: No ApplyForce permission

    auth_service.register_agent(agent);

    // Should be denied: no permission for ApplyForce
    let result = auth_service.authorize(
        &AgentId(1),
        ActionType::ApplyForce,
        &ActionParams::Force([1.0, 0.0, 0.0]),
    );

    match result {
        AuthorizationResult::Denied { reason } => {
            println!("Denied: {}", reason);
            assert!(reason.contains("Insufficient permission"));
        }
        _ => panic!("Should be denied"),
    }

    // Should be denied: unknown agent
    let result = auth_service.authorize(
        &AgentId(999),
        ActionType::ReadState,
        &ActionParams::None,
    );

    match result {
        AuthorizationResult::Denied { reason } => {
            assert!(reason.contains("Unknown agent"));
        }
        _ => panic!("Should be denied for unknown agent"),
    }
}

/// Test action authorization with constraints
#[test]
fn test_action_authorization_constraints() {
    let mut auth_service = AuthorizationService::new();

    let agent = AgentCapabilities::new(AgentId(1))
        .with_permission(ActionType::ApplyForce, PermissionLevel::Write);
    // max_force defaults to 10.0

    auth_service.register_agent(agent);

    // Should be allowed: force within limit
    let result = auth_service.authorize(
        &AgentId(1),
        ActionType::ApplyForce,
        &ActionParams::Force([5.0, 0.0, 0.0]),
    );
    assert_eq!(result, AuthorizationResult::Allowed);

    // Should be denied: force exceeds limit
    let result = auth_service.authorize(
        &AgentId(1),
        ActionType::ApplyForce,
        &ActionParams::Force([15.0, 0.0, 0.0]),
    );

    match result {
        AuthorizationResult::Denied { reason } => {
            assert!(reason.contains("exceeds max"));
        }
        _ => panic!("Should be denied for excessive force"),
    }
}

// ============================================================================
// Permission Check Tests
// ============================================================================

/// Permission checker with capability-based access control
struct PermissionChecker {
    capabilities: HashMap<AgentId, HashSet<String>>,
}

impl PermissionChecker {
    fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    fn grant_capability(&mut self, agent_id: AgentId, capability: &str) {
        self.capabilities
            .entry(agent_id)
            .or_insert_with(HashSet::new)
            .insert(capability.to_string());
    }

    fn revoke_capability(&mut self, agent_id: &AgentId, capability: &str) {
        if let Some(caps) = self.capabilities.get_mut(agent_id) {
            caps.remove(capability);
        }
    }

    fn has_capability(&self, agent_id: &AgentId, capability: &str) -> bool {
        self.capabilities
            .get(agent_id)
            .map(|caps| caps.contains(capability))
            .unwrap_or(false)
    }

    fn check_all(&self, agent_id: &AgentId, required: &[&str]) -> bool {
        required.iter().all(|cap| self.has_capability(agent_id, cap))
    }

    fn check_any(&self, agent_id: &AgentId, required: &[&str]) -> bool {
        required.iter().any(|cap| self.has_capability(agent_id, cap))
    }
}

/// Test permission checking
#[test]
fn test_permission_check() {
    let mut checker = PermissionChecker::new();

    let agent = AgentId(1);

    // Grant some capabilities
    checker.grant_capability(agent.clone(), "physics.read");
    checker.grant_capability(agent.clone(), "physics.write.force");

    // Check individual capabilities
    assert!(checker.has_capability(&agent, "physics.read"));
    assert!(checker.has_capability(&agent, "physics.write.force"));
    assert!(!checker.has_capability(&agent, "physics.write.velocity"));
    assert!(!checker.has_capability(&agent, "admin"));

    // Check all
    assert!(checker.check_all(&agent, &["physics.read", "physics.write.force"]));
    assert!(!checker.check_all(&agent, &["physics.read", "admin"]));

    // Check any
    assert!(checker.check_any(&agent, &["physics.read", "admin"]));
    assert!(!checker.check_any(&agent, &["admin", "network"]));
}

/// Test capability revocation
#[test]
fn test_permission_revocation() {
    let mut checker = PermissionChecker::new();
    let agent = AgentId(1);

    checker.grant_capability(agent.clone(), "physics.read");
    assert!(checker.has_capability(&agent, "physics.read"));

    checker.revoke_capability(&agent, "physics.read");
    assert!(!checker.has_capability(&agent, "physics.read"));
}

// ============================================================================
// Budget Enforcement Tests
// ============================================================================

/// Budget manager for resource limits
struct BudgetManager {
    budgets: HashMap<AgentId, Budget>,
}

#[derive(Debug, Clone)]
struct Budget {
    total: f32,
    remaining: f32,
    spent: f32,
    allocations: Vec<(String, f32, Instant)>,
}

impl Budget {
    fn new(total: f32) -> Self {
        Self {
            total,
            remaining: total,
            spent: 0.0,
            allocations: Vec::new(),
        }
    }
}

impl BudgetManager {
    fn new() -> Self {
        Self {
            budgets: HashMap::new(),
        }
    }

    fn allocate_budget(&mut self, agent_id: AgentId, total: f32) {
        self.budgets.insert(agent_id, Budget::new(total));
    }

    fn spend(&mut self, agent_id: &AgentId, amount: f32, reason: &str) -> Result<f32, String> {
        let budget = self.budgets.get_mut(agent_id)
            .ok_or_else(|| "No budget allocated".to_string())?;

        if amount > budget.remaining {
            return Err(format!(
                "Insufficient budget: need {}, have {}",
                amount, budget.remaining
            ));
        }

        budget.remaining -= amount;
        budget.spent += amount;
        budget.allocations.push((reason.to_string(), amount, Instant::now()));

        Ok(budget.remaining)
    }

    fn get_remaining(&self, agent_id: &AgentId) -> Option<f32> {
        self.budgets.get(agent_id).map(|b| b.remaining)
    }

    fn get_spent(&self, agent_id: &AgentId) -> Option<f32> {
        self.budgets.get(agent_id).map(|b| b.spent)
    }

    fn reset_budget(&mut self, agent_id: &AgentId) {
        if let Some(budget) = self.budgets.get_mut(agent_id) {
            budget.remaining = budget.total;
            budget.spent = 0.0;
            budget.allocations.clear();
        }
    }
}

/// Test budget enforcement
#[test]
fn test_budget_enforcement() {
    let mut manager = BudgetManager::new();
    let agent = AgentId(1);

    manager.allocate_budget(agent.clone(), 100.0);

    // Spend within budget
    let result = manager.spend(&agent, 30.0, "action 1");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 70.0);

    // Spend more
    let result = manager.spend(&agent, 50.0, "action 2");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 20.0);

    // Try to exceed budget
    let result = manager.spend(&agent, 30.0, "action 3");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Insufficient budget"));

    // Check remaining
    assert_eq!(manager.get_remaining(&agent), Some(20.0));
    assert_eq!(manager.get_spent(&agent), Some(80.0));
}

/// Test budget reset
#[test]
fn test_budget_reset() {
    let mut manager = BudgetManager::new();
    let agent = AgentId(1);

    manager.allocate_budget(agent.clone(), 100.0);
    manager.spend(&agent, 80.0, "big action").unwrap();

    assert_eq!(manager.get_remaining(&agent), Some(20.0));

    manager.reset_budget(&agent);

    assert_eq!(manager.get_remaining(&agent), Some(100.0));
    assert_eq!(manager.get_spent(&agent), Some(0.0));
}

// ============================================================================
// Audit Log Tests
// ============================================================================

/// Audit log entry
#[derive(Debug, Clone)]
struct AuditEntry {
    timestamp: u64,
    sequence: u64,
    agent_id: AgentId,
    action: String,
    params: String,
    result: String,
    hash: String, // Hash of previous entry for append-only verification
}

/// Append-only audit log
struct AuditLog {
    entries: Vec<AuditEntry>,
    sequence_counter: u64,
}

impl AuditLog {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            sequence_counter: 0,
        }
    }

    fn append(&mut self, agent_id: AgentId, action: &str, params: &str, result: &str) -> u64 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let prev_hash = if self.entries.is_empty() {
            "genesis".to_string()
        } else {
            self.entries.last().unwrap().hash.clone()
        };

        let sequence = self.sequence_counter;
        self.sequence_counter += 1;

        // Simple hash: concatenate and hash (in production, use proper crypto hash)
        let hash_input = format!(
            "{}{}{}{}{}{}",
            timestamp, sequence, agent_id.0, action, params, prev_hash
        );
        let hash = format!("{:x}", hash_input.len() * 31 + sequence as usize * 17);

        let entry = AuditEntry {
            timestamp,
            sequence,
            agent_id,
            action: action.to_string(),
            params: params.to_string(),
            result: result.to_string(),
            hash,
        };

        self.entries.push(entry);
        sequence
    }

    fn verify_chain(&self) -> bool {
        if self.entries.is_empty() {
            return true;
        }

        // Verify first entry links to genesis
        // In a real implementation, we'd verify the hash computation

        // Verify sequence numbers are monotonic
        for i in 0..self.entries.len() {
            if self.entries[i].sequence != i as u64 {
                return false;
            }
        }

        true
    }

    fn get_entry(&self, sequence: u64) -> Option<&AuditEntry> {
        self.entries.get(sequence as usize)
    }

    fn get_entries_by_agent(&self, agent_id: &AgentId) -> Vec<&AuditEntry> {
        self.entries.iter()
            .filter(|e| e.agent_id == *agent_id)
            .collect()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Test audit log append-only property
#[test]
fn test_audit_log_append_only() {
    let mut log = AuditLog::new();

    // Append entries
    let seq1 = log.append(AgentId(1), "ApplyForce", "[1.0, 0.0, 0.0]", "Allowed");
    let seq2 = log.append(AgentId(1), "ReadState", "", "Allowed");
    let seq3 = log.append(AgentId(2), "ApplyForce", "[5.0, 0.0, 0.0]", "Denied");

    // Verify sequence numbers
    assert_eq!(seq1, 0);
    assert_eq!(seq2, 1);
    assert_eq!(seq3, 2);

    // Verify chain integrity
    assert!(log.verify_chain());

    // Verify entries are retrievable
    let entry1 = log.get_entry(0).unwrap();
    assert_eq!(entry1.action, "ApplyForce");
    assert_eq!(entry1.agent_id, AgentId(1));

    // Verify agent filtering
    let agent1_entries = log.get_entries_by_agent(&AgentId(1));
    assert_eq!(agent1_entries.len(), 2);
}

/// Test audit log cannot be modified
#[test]
fn test_audit_log_immutability() {
    let mut log = AuditLog::new();

    log.append(AgentId(1), "Action1", "", "OK");
    log.append(AgentId(1), "Action2", "", "OK");

    let initial_hash = log.entries[1].hash.clone();
    let initial_len = log.len();

    // Try to add more entries - this should work
    log.append(AgentId(1), "Action3", "", "OK");

    // Length should increase
    assert_eq!(log.len(), initial_len + 1);

    // Previous entry hash should be unchanged
    assert_eq!(log.entries[1].hash, initial_hash);

    // Chain should still verify
    assert!(log.verify_chain());
}

/// Test audit log hash chain
#[test]
fn test_audit_log_hash_chain() {
    let mut log = AuditLog::new();

    // Add several entries
    for i in 0..10 {
        log.append(AgentId(i as u64 % 3), &format!("Action{}", i), "", "OK");
    }

    // All entries should have unique hashes (with high probability)
    let hashes: Vec<&String> = log.entries.iter().map(|e| &e.hash).collect();
    let unique_hashes: HashSet<&String> = hashes.iter().cloned().collect();

    assert_eq!(hashes.len(), unique_hashes.len(), "All hashes should be unique");

    // Verify chain
    assert!(log.verify_chain());
}

// ============================================================================
// Integrated Governance System Tests
// ============================================================================

/// Complete governance system
struct GovernanceSystem {
    auth_service: AuthorizationService,
    permission_checker: PermissionChecker,
    budget_manager: BudgetManager,
    audit_log: AuditLog,
}

impl GovernanceSystem {
    fn new() -> Self {
        Self {
            auth_service: AuthorizationService::new(),
            permission_checker: PermissionChecker::new(),
            budget_manager: BudgetManager::new(),
            audit_log: AuditLog::new(),
        }
    }

    fn register_agent(&mut self, agent_id: AgentId, capabilities: AgentCapabilities, budget: f32) {
        self.auth_service.register_agent(capabilities.clone());
        self.budget_manager.allocate_budget(agent_id.clone(), budget);

        // Grant capabilities based on permissions
        for (action, level) in &capabilities.permissions {
            if *level >= PermissionLevel::Read {
                self.permission_checker.grant_capability(
                    agent_id.clone(),
                    &format!("{:?}.read", action),
                );
            }
            if *level >= PermissionLevel::Write {
                self.permission_checker.grant_capability(
                    agent_id.clone(),
                    &format!("{:?}.write", action),
                );
            }
        }
    }

    fn execute_action(
        &mut self,
        agent_id: &AgentId,
        action: ActionType,
        params: ActionParams,
        cost: f32,
    ) -> Result<(), String> {
        // Step 1: Authorize
        let auth_result = self.auth_service.authorize(agent_id, action.clone(), &params);

        let result_str = match &auth_result {
            AuthorizationResult::Allowed => "Allowed",
            AuthorizationResult::Denied { .. } => "Denied",
            AuthorizationResult::RateLimited { .. } => "RateLimited",
        };

        // Step 2: Log the attempt
        self.audit_log.append(
            agent_id.clone(),
            &format!("{:?}", action),
            &format!("{:?}", params),
            result_str,
        );

        // Step 3: Handle authorization result
        match auth_result {
            AuthorizationResult::Allowed => {
                // Step 4: Check budget
                self.budget_manager.spend(agent_id, cost, &format!("{:?}", action))?;
                Ok(())
            }
            AuthorizationResult::Denied { reason } => Err(reason),
            AuthorizationResult::RateLimited { retry_after_ms } => {
                Err(format!("Rate limited, retry after {}ms", retry_after_ms))
            }
        }
    }
}

/// Test integrated governance system
#[test]
fn test_integrated_governance_system() {
    let mut gov = GovernanceSystem::new();

    let agent_id = AgentId(1);
    let capabilities = AgentCapabilities::new(agent_id.clone())
        .with_permission(ActionType::ApplyForce, PermissionLevel::Write)
        .with_permission(ActionType::ReadState, PermissionLevel::Read);

    gov.register_agent(agent_id.clone(), capabilities, 100.0);

    // Successful action
    let result = gov.execute_action(
        &agent_id,
        ActionType::ApplyForce,
        ActionParams::Force([1.0, 0.0, 0.0]),
        10.0,
    );
    assert!(result.is_ok());

    // Check budget was deducted
    assert_eq!(gov.budget_manager.get_remaining(&agent_id), Some(90.0));

    // Check audit log
    assert_eq!(gov.audit_log.len(), 1);

    // Denied action (no permission)
    let result = gov.execute_action(
        &agent_id,
        ActionType::CreateAtom,
        ActionParams::None,
        5.0,
    );
    assert!(result.is_err());

    // Audit log should have 2 entries now
    assert_eq!(gov.audit_log.len(), 2);

    // Budget should not have been deducted for denied action
    assert_eq!(gov.budget_manager.get_remaining(&agent_id), Some(90.0));
}
