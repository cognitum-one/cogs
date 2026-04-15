//! Capability Proxy for Agentic VM
//!
//! This crate implements the capability proxy layer that mediates all operations
//! from capsules to the underlying system. It provides:
//! - Capability grant and revocation
//! - Operation invocation with capability validation
//! - Budget tracking and deduction
//! - Evidence logging for all operations

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, RwLock};
#[cfg(not(feature = "std"))]
use spin::{Mutex, RwLock};

use agentvm_types::{
    Budget, BudgetVector, Capability, CapabilityGrant, CapabilityId, CapabilityProof,
    CapabilityScope, CapabilityType, CapsuleId, Quota, Rights,
};

#[cfg(test)]
mod tests;

/// Result type for proxy operations
pub type Result<T> = core::result::Result<T, ProxyError>;

/// Errors that can occur during proxy operations
#[derive(Debug, Clone)]
pub enum ProxyError {
    /// Capability not found
    CapabilityNotFound(CapabilityId),
    /// Capability has expired
    CapabilityExpired(CapabilityId),
    /// Capability has been revoked
    CapabilityRevoked(CapabilityId),
    /// Quota exhausted
    QuotaExhausted(CapabilityId),
    /// Scope violation - operation not permitted
    ScopeViolation { cap_id: CapabilityId, target: String },
    /// Rights violation - missing required right
    RightsViolation { cap_id: CapabilityId, required: Rights },
    /// Budget exceeded
    BudgetExceeded { capsule_id: CapsuleId },
    /// Invalid signature
    InvalidSignature,
    /// Executor error
    ExecutorError(String),
    /// Capsule not found
    CapsuleNotFound(CapsuleId),
    /// Operation not supported
    UnsupportedOperation,
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProxyError::CapabilityNotFound(id) => write!(f, "capability not found: {}", id),
            ProxyError::CapabilityExpired(id) => write!(f, "capability expired: {}", id),
            ProxyError::CapabilityRevoked(id) => write!(f, "capability revoked: {}", id),
            ProxyError::QuotaExhausted(id) => write!(f, "quota exhausted: {}", id),
            ProxyError::ScopeViolation { cap_id, target } => {
                write!(f, "scope violation: cap {} cannot access {}", cap_id, target)
            }
            ProxyError::RightsViolation { cap_id, required } => {
                write!(f, "rights violation: cap {} missing rights {:?}", cap_id, required)
            }
            ProxyError::BudgetExceeded { capsule_id } => {
                write!(f, "budget exceeded for capsule: {:?}", capsule_id)
            }
            ProxyError::InvalidSignature => write!(f, "invalid capability signature"),
            ProxyError::ExecutorError(msg) => write!(f, "executor error: {}", msg),
            ProxyError::CapsuleNotFound(id) => write!(f, "capsule not found: {:?}", id),
            ProxyError::UnsupportedOperation => write!(f, "operation not supported"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProxyError {}

/// Invocation request
#[derive(Debug, Clone)]
pub struct InvokeRequest {
    /// Capability to use
    pub capability_id: CapabilityId,
    /// Target resource (path, host, etc.)
    pub target: String,
    /// Required right for this operation
    pub required_right: u32,
    /// Operation-specific payload
    pub payload: Vec<u8>,
    /// Estimated resource usage
    pub estimated_budget: BudgetVector,
}

impl InvokeRequest {
    pub fn new(capability_id: CapabilityId, target: impl Into<String>) -> Self {
        Self {
            capability_id,
            target: target.into(),
            required_right: Rights::READ,
            payload: Vec::new(),
            estimated_budget: BudgetVector::zero(),
        }
    }

    pub fn with_right(mut self, right: u32) -> Self {
        self.required_right = right;
        self
    }

    pub fn with_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_budget(mut self, budget: BudgetVector) -> Self {
        self.estimated_budget = budget;
        self
    }
}

/// Invocation response
#[derive(Debug, Clone)]
pub struct InvokeResponse {
    /// Response payload
    pub payload: Vec<u8>,
    /// Actual resource usage
    pub actual_budget: BudgetVector,
    /// Evidence record ID
    pub evidence_id: u64,
}

/// Executor trait for actually performing operations
pub trait Executor: Send + Sync {
    /// Execute an operation
    fn execute(&self, cap_type: CapabilityType, request: &InvokeRequest) -> Result<InvokeResponse>;

    /// Get the executor name
    fn name(&self) -> &str;
}

/// Evidence logger trait
pub trait EvidenceLogger: Send + Sync {
    /// Log a capability grant
    fn log_grant(&self, capsule_id: CapsuleId, cap: &Capability);

    /// Log a capability revocation
    fn log_revoke(&self, capsule_id: CapsuleId, cap_id: CapabilityId);

    /// Log an invocation
    fn log_invoke(
        &self,
        capsule_id: CapsuleId,
        cap_id: CapabilityId,
        target: &str,
        success: bool,
        budget_used: &BudgetVector,
    ) -> u64;
}

/// Capsule context containing capabilities and budget
pub struct CapsuleContext {
    /// Capsule identifier
    pub id: CapsuleId,
    /// Granted capabilities indexed by ID
    capabilities: HashMap<CapabilityId, Capability>,
    /// Budget for this capsule
    budget: Budget,
    /// Current time (monotonic ns)
    current_time: u64,
}

impl CapsuleContext {
    /// Create a new capsule context
    pub fn new(id: CapsuleId, budget: Budget) -> Self {
        Self {
            id,
            capabilities: HashMap::new(),
            budget,
            current_time: 0,
        }
    }

    /// Set the current time
    pub fn set_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Grant a capability to this capsule
    pub fn grant(&mut self, cap: Capability) {
        self.capabilities.insert(cap.id, cap);
    }

    /// Revoke a capability from this capsule
    pub fn revoke(&mut self, cap_id: CapabilityId) -> Option<Capability> {
        if let Some(cap) = self.capabilities.get_mut(&cap_id) {
            cap.revoked = true;
            Some(cap.clone())
        } else {
            None
        }
    }

    /// Get a capability by ID
    pub fn get_capability(&self, cap_id: CapabilityId) -> Option<&Capability> {
        self.capabilities.get(&cap_id)
    }

    /// Get a mutable capability by ID
    pub fn get_capability_mut(&mut self, cap_id: CapabilityId) -> Option<&mut Capability> {
        self.capabilities.get_mut(&cap_id)
    }

    /// List all capabilities
    pub fn list_capabilities(&self) -> Vec<&Capability> {
        self.capabilities.values().collect()
    }

    /// List valid capabilities
    pub fn list_valid_capabilities(&self) -> Vec<&Capability> {
        self.capabilities
            .values()
            .filter(|c| c.is_valid(self.current_time))
            .collect()
    }

    /// Get remaining budget
    pub fn remaining_budget(&self) -> BudgetVector {
        self.budget.remaining()
    }

    /// Try to consume budget
    pub fn try_consume_budget(&mut self, amount: &BudgetVector) -> core::result::Result<(), ()> {
        self.budget.try_consume(amount).map_err(|_| ())
    }

    /// Get budget utilization
    pub fn budget_utilization(&self) -> f64 {
        self.budget.utilization()
    }
}

/// The capability proxy mediating all capsule operations
pub struct CapabilityProxy {
    /// Capsule contexts indexed by capsule ID
    contexts: RwLock<HashMap<CapsuleId, CapsuleContext>>,
    /// Executors indexed by capability type
    executors: RwLock<HashMap<CapabilityType, Box<dyn Executor>>>,
    /// Evidence logger
    evidence_logger: Option<Box<dyn EvidenceLogger>>,
    /// Global capability ID counter
    next_cap_id: Mutex<u128>,
    /// Signing key for capability proofs
    signing_key: [u8; 32],
}

impl CapabilityProxy {
    /// Create a new capability proxy
    pub fn new() -> Self {
        Self {
            contexts: RwLock::new(HashMap::new()),
            executors: RwLock::new(HashMap::new()),
            evidence_logger: None,
            next_cap_id: Mutex::new(1),
            signing_key: [0u8; 32],
        }
    }

    /// Create with signing key
    pub fn with_signing_key(signing_key: [u8; 32]) -> Self {
        Self {
            contexts: RwLock::new(HashMap::new()),
            executors: RwLock::new(HashMap::new()),
            evidence_logger: None,
            next_cap_id: Mutex::new(1),
            signing_key,
        }
    }

    /// Set evidence logger
    pub fn set_evidence_logger(&mut self, logger: Box<dyn EvidenceLogger>) {
        self.evidence_logger = Some(logger);
    }

    /// Register an executor for a capability type
    pub fn register_executor(&self, cap_type: CapabilityType, executor: Box<dyn Executor>) {
        let mut executors = self.executors.write().unwrap();
        executors.insert(cap_type, executor);
    }

    /// Register a capsule with the proxy
    pub fn register_capsule(&self, capsule_id: CapsuleId, budget: Budget) {
        let mut contexts = self.contexts.write().unwrap();
        contexts.insert(capsule_id, CapsuleContext::new(capsule_id, budget));
    }

    /// Unregister a capsule
    pub fn unregister_capsule(&self, capsule_id: CapsuleId) -> Option<CapsuleContext> {
        let mut contexts = self.contexts.write().unwrap();
        contexts.remove(&capsule_id)
    }

    /// Set current time for a capsule
    pub fn set_capsule_time(&self, capsule_id: CapsuleId, time: u64) -> Result<()> {
        let mut contexts = self.contexts.write().unwrap();
        let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;
        ctx.set_time(time);
        Ok(())
    }

    /// Grant a capability to a capsule
    pub fn grant(&self, capsule_id: CapsuleId, grant: CapabilityGrant) -> Result<CapabilityId> {
        let cap_id = self.generate_capability_id();

        // Create the capability
        let cap = self.create_capability(cap_id, grant);

        // Insert into capsule context
        {
            let mut contexts = self.contexts.write().unwrap();
            let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;
            ctx.grant(cap.clone());
        }

        // Log evidence
        if let Some(logger) = &self.evidence_logger {
            logger.log_grant(capsule_id, &cap);
        }

        Ok(cap_id)
    }

    /// Revoke a capability from a capsule
    pub fn revoke(&self, capsule_id: CapsuleId, cap_id: CapabilityId) -> Result<()> {
        let mut contexts = self.contexts.write().unwrap();
        let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;

        ctx.revoke(cap_id).ok_or(ProxyError::CapabilityNotFound(cap_id))?;

        // Log evidence
        if let Some(logger) = &self.evidence_logger {
            logger.log_revoke(capsule_id, cap_id);
        }

        Ok(())
    }

    /// Revoke all capabilities derived from a parent
    pub fn revoke_cascade(&self, capsule_id: CapsuleId, parent_cap_id: CapabilityId) -> Result<Vec<CapabilityId>> {
        let mut contexts = self.contexts.write().unwrap();
        let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;

        let mut revoked_ids = Vec::new();

        // Find and revoke all capabilities with this parent
        for cap in ctx.capabilities.values_mut() {
            if cap.parent == Some(parent_cap_id) || cap.id == parent_cap_id {
                cap.revoked = true;
                revoked_ids.push(cap.id);
            }
        }

        // Log evidence for each revocation
        if let Some(logger) = &self.evidence_logger {
            for cap_id in &revoked_ids {
                logger.log_revoke(capsule_id, *cap_id);
            }
        }

        Ok(revoked_ids)
    }

    /// Invoke an operation using a capability
    pub fn invoke(&self, capsule_id: CapsuleId, request: InvokeRequest) -> Result<InvokeResponse> {
        // Validate capability
        let (cap_type, actual_budget) = self.validate_and_prepare(capsule_id, &request)?;

        // Get executor
        let executors = self.executors.read().unwrap();
        let executor = executors.get(&cap_type).ok_or(ProxyError::UnsupportedOperation)?;

        // Execute the operation
        let response = executor.execute(cap_type, &request)?;

        // Deduct budget
        self.deduct_budget(capsule_id, &response.actual_budget)?;

        // Consume quota
        self.consume_quota(capsule_id, request.capability_id, &response.actual_budget)?;

        // Log evidence
        let evidence_id = if let Some(logger) = &self.evidence_logger {
            logger.log_invoke(capsule_id, request.capability_id, &request.target, true, &response.actual_budget)
        } else {
            0
        };

        Ok(InvokeResponse {
            payload: response.payload,
            actual_budget: response.actual_budget,
            evidence_id,
        })
    }

    /// Validate capability and prepare for invocation
    fn validate_and_prepare(
        &self,
        capsule_id: CapsuleId,
        request: &InvokeRequest,
    ) -> Result<(CapabilityType, BudgetVector)> {
        let contexts = self.contexts.read().unwrap();
        let ctx = contexts.get(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;

        // Get the capability
        let cap = ctx
            .get_capability(request.capability_id)
            .ok_or(ProxyError::CapabilityNotFound(request.capability_id))?;

        // Check if capability is valid
        if cap.is_expired(ctx.current_time) {
            return Err(ProxyError::CapabilityExpired(request.capability_id));
        }
        if cap.is_revoked() {
            return Err(ProxyError::CapabilityRevoked(request.capability_id));
        }
        if cap.quota.is_exhausted() {
            return Err(ProxyError::QuotaExhausted(request.capability_id));
        }

        // Check scope
        if !cap.scope.permits(&request.target) {
            return Err(ProxyError::ScopeViolation {
                cap_id: request.capability_id,
                target: request.target.clone(),
            });
        }

        // Check rights
        if !cap.rights.has(request.required_right) {
            return Err(ProxyError::RightsViolation {
                cap_id: request.capability_id,
                required: Rights::new(request.required_right),
            });
        }

        // Check budget
        if !ctx.remaining_budget().can_satisfy(&request.estimated_budget) {
            return Err(ProxyError::BudgetExceeded { capsule_id });
        }

        Ok((cap.cap_type, request.estimated_budget))
    }

    /// Deduct budget from capsule
    fn deduct_budget(&self, capsule_id: CapsuleId, amount: &BudgetVector) -> Result<()> {
        let mut contexts = self.contexts.write().unwrap();
        let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;

        ctx.try_consume_budget(amount).map_err(|_| ProxyError::BudgetExceeded { capsule_id })
    }

    /// Consume quota from capability
    fn consume_quota(
        &self,
        capsule_id: CapsuleId,
        cap_id: CapabilityId,
        budget: &BudgetVector,
    ) -> Result<()> {
        let mut contexts = self.contexts.write().unwrap();
        let ctx = contexts.get_mut(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;

        let cap = ctx.get_capability_mut(cap_id).ok_or(ProxyError::CapabilityNotFound(cap_id))?;

        // Consume quota based on actual usage
        cap.quota.consume(budget.network_bytes, budget.wall_time_ms * 1_000_000).ok();

        Ok(())
    }

    /// Get capabilities for a capsule
    pub fn get_capabilities(&self, capsule_id: CapsuleId) -> Result<Vec<Capability>> {
        let contexts = self.contexts.read().unwrap();
        let ctx = contexts.get(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;
        Ok(ctx.capabilities.values().cloned().collect())
    }

    /// Get valid capabilities for a capsule
    pub fn get_valid_capabilities(&self, capsule_id: CapsuleId) -> Result<Vec<Capability>> {
        let contexts = self.contexts.read().unwrap();
        let ctx = contexts.get(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;
        Ok(ctx.list_valid_capabilities().into_iter().cloned().collect())
    }

    /// Get remaining budget for a capsule
    pub fn get_remaining_budget(&self, capsule_id: CapsuleId) -> Result<BudgetVector> {
        let contexts = self.contexts.read().unwrap();
        let ctx = contexts.get(&capsule_id).ok_or(ProxyError::CapsuleNotFound(capsule_id))?;
        Ok(ctx.remaining_budget())
    }

    /// Generate a new capability ID
    fn generate_capability_id(&self) -> CapabilityId {
        let mut next = self.next_cap_id.lock().unwrap();
        let id = *next;
        *next += 1;
        CapabilityId::from_raw(id)
    }

    /// Create a capability from a grant
    fn create_capability(&self, id: CapabilityId, grant: CapabilityGrant) -> Capability {
        // Create proof (in real implementation, would sign with Ed25519)
        let proof = CapabilityProof::new(
            self.signing_key,
            [0x42u8; 64], // Placeholder signature
            self.get_current_time(),
        );

        let expires_at = self.get_current_time() + grant.lease_secs * 1_000_000_000;

        Capability {
            id,
            cap_type: grant.cap_type,
            scope: grant.scope,
            rights: grant.rights,
            quota: grant.quota,
            expires_at,
            parent: None,
            proof,
            revoked: false,
        }
    }

    /// Get current time (placeholder)
    fn get_current_time(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

impl Default for CapabilityProxy {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for capability proxy
pub struct CapabilityProxyBuilder {
    signing_key: [u8; 32],
    evidence_logger: Option<Box<dyn EvidenceLogger>>,
    executors: Vec<(CapabilityType, Box<dyn Executor>)>,
}

impl CapabilityProxyBuilder {
    pub fn new() -> Self {
        Self {
            signing_key: [0u8; 32],
            evidence_logger: None,
            executors: Vec::new(),
        }
    }

    pub fn with_signing_key(mut self, key: [u8; 32]) -> Self {
        self.signing_key = key;
        self
    }

    pub fn with_evidence_logger(mut self, logger: Box<dyn EvidenceLogger>) -> Self {
        self.evidence_logger = Some(logger);
        self
    }

    pub fn with_executor(mut self, cap_type: CapabilityType, executor: Box<dyn Executor>) -> Self {
        self.executors.push((cap_type, executor));
        self
    }

    pub fn build(self) -> CapabilityProxy {
        let mut proxy = CapabilityProxy::with_signing_key(self.signing_key);

        if let Some(logger) = self.evidence_logger {
            proxy.set_evidence_logger(logger);
        }

        for (cap_type, executor) in self.executors {
            proxy.register_executor(cap_type, executor);
        }

        proxy
    }
}

impl Default for CapabilityProxyBuilder {
    fn default() -> Self {
        Self::new()
    }
}
