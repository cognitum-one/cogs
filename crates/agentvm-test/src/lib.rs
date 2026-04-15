//! Test utilities for Agentic VM
//!
//! This crate provides:
//! - Mock implementations (MockCapabilityProxy, MockExecutor, MockEvidenceLogger)
//! - Test fixtures (pre-built capabilities, capsules, budgets)
//! - Property test strategies (proptest generators)
//! - Assertion helpers

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

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
    CapabilityScope, CapabilityType, CapsuleId, CapsuleIdentity, CapsuleManifest, Quota, Rights,
};

// ============================================================================
// Mock Executor
// ============================================================================

/// Mock executor for testing capability invocations
pub struct MockExecutor {
    /// Executor name
    pub name: String,
    /// Response payload to return
    pub response_payload: Vec<u8>,
    /// Response budget to report
    pub response_budget: BudgetVector,
    /// Number of times execute was called
    call_count: AtomicU64,
    /// Whether to simulate failure
    pub should_fail: bool,
    /// Error message when failing
    pub fail_message: String,
    /// Recorded invocations
    invocations: Mutex<Vec<MockInvocation>>,
}

/// Recorded invocation for verification
#[derive(Debug, Clone)]
pub struct MockInvocation {
    pub cap_type: CapabilityType,
    pub target: String,
    pub payload: Vec<u8>,
}

impl MockExecutor {
    /// Create a new mock executor
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            response_payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
            response_budget: BudgetVector::new(10, 100, 1024, 0, 512, 1),
            call_count: AtomicU64::new(0),
            should_fail: false,
            fail_message: "mock failure".to_string(),
            invocations: Mutex::new(Vec::new()),
        }
    }

    /// Set response payload
    pub fn with_response(mut self, payload: Vec<u8>) -> Self {
        self.response_payload = payload;
        self
    }

    /// Set response budget
    pub fn with_budget(mut self, budget: BudgetVector) -> Self {
        self.response_budget = budget;
        self
    }

    /// Configure to fail
    pub fn failing(mut self) -> Self {
        self.should_fail = true;
        self
    }

    /// Set failure message
    pub fn with_fail_message(mut self, msg: impl Into<String>) -> Self {
        self.fail_message = msg.into();
        self.should_fail = true;
        self
    }

    /// Get call count
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Get recorded invocations
    pub fn invocations(&self) -> Vec<MockInvocation> {
        self.invocations.lock().unwrap().clone()
    }

    /// Clear recorded invocations
    pub fn clear_invocations(&self) {
        self.invocations.lock().unwrap().clear();
    }

    /// Record an invocation (for use by test frameworks)
    pub fn record_invocation(&self, cap_type: CapabilityType, target: String, payload: Vec<u8>) {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.invocations.lock().unwrap().push(MockInvocation {
            cap_type,
            target,
            payload,
        });
    }

    /// Execute and return result (simplified interface)
    pub fn execute_simple(&self, cap_type: CapabilityType, target: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
        self.record_invocation(cap_type, target.to_string(), payload.to_vec());

        if self.should_fail {
            Err(self.fail_message.clone())
        } else {
            Ok(self.response_payload.clone())
        }
    }
}

impl Default for MockExecutor {
    fn default() -> Self {
        Self::new("mock")
    }
}

// ============================================================================
// Mock Evidence Logger
// ============================================================================

/// Mock evidence logger for testing
pub struct MockEvidenceLogger {
    /// Number of grant events logged
    grant_count: AtomicU64,
    /// Number of revoke events logged
    revoke_count: AtomicU64,
    /// Number of invoke events logged
    invoke_count: AtomicU64,
    /// Next evidence ID to return
    next_evidence_id: AtomicU64,
    /// Recorded events
    events: Mutex<Vec<EvidenceEvent>>,
}

/// Recorded evidence event
#[derive(Debug, Clone)]
pub enum EvidenceEvent {
    Grant {
        capsule_id: CapsuleId,
        capability_id: CapabilityId,
        cap_type: CapabilityType,
    },
    Revoke {
        capsule_id: CapsuleId,
        capability_id: CapabilityId,
    },
    Invoke {
        capsule_id: CapsuleId,
        capability_id: CapabilityId,
        target: String,
        success: bool,
        evidence_id: u64,
    },
}

impl MockEvidenceLogger {
    /// Create a new mock evidence logger
    pub fn new() -> Self {
        Self {
            grant_count: AtomicU64::new(0),
            revoke_count: AtomicU64::new(0),
            invoke_count: AtomicU64::new(0),
            next_evidence_id: AtomicU64::new(1),
            events: Mutex::new(Vec::new()),
        }
    }

    /// Get grant count
    pub fn grant_count(&self) -> u64 {
        self.grant_count.load(Ordering::SeqCst)
    }

    /// Get revoke count
    pub fn revoke_count(&self) -> u64 {
        self.revoke_count.load(Ordering::SeqCst)
    }

    /// Get invoke count
    pub fn invoke_count(&self) -> u64 {
        self.invoke_count.load(Ordering::SeqCst)
    }

    /// Get total event count
    pub fn total_count(&self) -> u64 {
        self.grant_count() + self.revoke_count() + self.invoke_count()
    }

    /// Get all recorded events
    pub fn events(&self) -> Vec<EvidenceEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
        self.grant_count.store(0, Ordering::SeqCst);
        self.revoke_count.store(0, Ordering::SeqCst);
        self.invoke_count.store(0, Ordering::SeqCst);
    }

    /// Log a grant event
    pub fn log_grant(&self, capsule_id: CapsuleId, cap: &Capability) {
        self.grant_count.fetch_add(1, Ordering::SeqCst);
        self.events.lock().unwrap().push(EvidenceEvent::Grant {
            capsule_id,
            capability_id: cap.id,
            cap_type: cap.cap_type,
        });
    }

    /// Log a revoke event
    pub fn log_revoke(&self, capsule_id: CapsuleId, cap_id: CapabilityId) {
        self.revoke_count.fetch_add(1, Ordering::SeqCst);
        self.events.lock().unwrap().push(EvidenceEvent::Revoke {
            capsule_id,
            capability_id: cap_id,
        });
    }

    /// Log an invoke event
    pub fn log_invoke(
        &self,
        capsule_id: CapsuleId,
        cap_id: CapabilityId,
        target: &str,
        success: bool,
    ) -> u64 {
        self.invoke_count.fetch_add(1, Ordering::SeqCst);
        let evidence_id = self.next_evidence_id.fetch_add(1, Ordering::SeqCst);
        self.events.lock().unwrap().push(EvidenceEvent::Invoke {
            capsule_id,
            capability_id: cap_id,
            target: target.to_string(),
            success,
            evidence_id,
        });
        evidence_id
    }
}

impl Default for MockEvidenceLogger {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Test Fixtures
// ============================================================================

/// Pre-built test fixtures for common scenarios
pub struct TestFixtures;

impl TestFixtures {
    // Capsule IDs

    /// Standard test capsule ID
    pub fn capsule_id() -> CapsuleId {
        CapsuleId::from_bytes([0xAB; 16])
    }

    /// Alternative test capsule ID
    pub fn capsule_id_alt() -> CapsuleId {
        CapsuleId::from_bytes([0xCD; 16])
    }

    /// Generate numbered capsule ID
    pub fn capsule_id_n(n: u8) -> CapsuleId {
        CapsuleId::from_bytes([n; 16])
    }

    // Capability IDs

    /// Standard test capability ID
    pub fn capability_id() -> CapabilityId {
        CapabilityId::from_raw(1)
    }

    /// Generate numbered capability ID
    pub fn capability_id_n(n: u128) -> CapabilityId {
        CapabilityId::from_raw(n)
    }

    // Budgets

    /// Standard test budget with generous limits
    pub fn budget() -> Budget {
        Budget::new(BudgetVector::new(
            10000,        // 10s CPU
            60000,        // 60s wall
            1024 * 1024,  // 1MB memory
            1024 * 1024,  // 1MB disk
            1024 * 1024,  // 1MB network
            1000,         // 1000 requests
        ))
    }

    /// Small budget for testing limits
    pub fn small_budget() -> Budget {
        Budget::new(BudgetVector::new(
            100,   // 100ms CPU
            1000,  // 1s wall
            1024,  // 1KB memory
            1024,  // 1KB disk
            1024,  // 1KB network
            10,    // 10 requests
        ))
    }

    /// Unlimited budget
    pub fn unlimited_budget() -> Budget {
        Budget::new(BudgetVector::unlimited())
    }

    /// Zero budget (already exhausted)
    pub fn zero_budget() -> Budget {
        Budget::new(BudgetVector::zero())
    }

    // Budget Vectors

    /// Small budget vector for single operation
    pub fn small_budget_vector() -> BudgetVector {
        BudgetVector::new(10, 100, 1024, 0, 512, 1)
    }

    /// Medium budget vector
    pub fn medium_budget_vector() -> BudgetVector {
        BudgetVector::new(100, 1000, 10240, 1024, 5120, 10)
    }

    /// Large budget vector
    pub fn large_budget_vector() -> BudgetVector {
        BudgetVector::new(1000, 10000, 102400, 10240, 51200, 100)
    }

    // Capabilities

    /// Standard HTTP capability grant
    pub fn http_grant() -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_scope(CapabilityScope::Global)
            .with_rights(Rights::all())
            .with_quota(Quota::new(100, 1024 * 1024, 60_000_000_000))
            .with_lease(3600)
    }

    /// File read capability grant
    pub fn file_read_grant() -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::FileRead)
            .with_scope(CapabilityScope::Filesystem {
                paths: vec!["/workspace/**".to_string()],
                operations: agentvm_types::capability::FileOperations::read_only(),
            })
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(1000, 10 * 1024 * 1024, 300_000_000_000))
            .with_lease(7200)
    }

    /// File write capability grant
    pub fn file_write_grant() -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::FileWrite)
            .with_scope(CapabilityScope::Filesystem {
                paths: vec!["/workspace/**".to_string()],
                operations: agentvm_types::capability::FileOperations::all(),
            })
            .with_rights(Rights::new(Rights::READ | Rights::WRITE))
            .with_quota(Quota::new(500, 5 * 1024 * 1024, 300_000_000_000))
            .with_lease(3600)
    }

    /// Network capability with restricted scope
    pub fn restricted_http_grant(hosts: Vec<String>) -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_scope(CapabilityScope::Network {
                hosts,
                ports: vec![80, 443],
                protocols: vec![],
            })
            .with_rights(Rights::new(Rights::READ | Rights::WRITE))
            .with_quota(Quota::new(50, 512 * 1024, 30_000_000_000))
            .with_lease(1800)
    }

    /// Capability with limited quota
    pub fn limited_quota_grant(invocations: u64) -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_quota(Quota::new(invocations, 1024 * 1024, 60_000_000_000))
            .with_lease(3600)
    }

    /// Capability with short lease
    pub fn short_lease_grant(lease_secs: u64) -> CapabilityGrant {
        CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_quota(Quota::unlimited())
            .with_lease(lease_secs)
    }

    /// Create a test capability
    pub fn capability(id: CapabilityId, cap_type: CapabilityType) -> Capability {
        Capability {
            id,
            cap_type,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        }
    }

    /// Create an expired capability
    pub fn expired_capability(id: CapabilityId) -> Capability {
        Capability {
            id,
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: 0, // Already expired
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        }
    }

    /// Create a revoked capability
    pub fn revoked_capability(id: CapabilityId) -> Capability {
        Capability {
            id,
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: true,
        }
    }

    /// Create a capability with exhausted quota
    pub fn exhausted_capability(id: CapabilityId) -> Capability {
        let mut quota = Quota::new(10, 1024, 1_000_000);
        quota.used_invocations = 10; // Exhausted

        Capability {
            id,
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota,
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        }
    }

    // Capsule Manifests

    /// Standard test capsule manifest
    pub fn capsule_manifest() -> CapsuleManifest {
        CapsuleManifest::builder()
            .name("test-capsule")
            .version("1.0.0")
            .entry_point("main")
            .build()
    }

    // Signing keys

    /// Test signing key
    pub fn signing_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    /// Alternative signing key
    pub fn signing_key_alt() -> [u8; 32] {
        [0x24u8; 32]
    }
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assertion helpers for capability tests
pub mod assertions {
    use super::*;

    /// Assert that a capability is valid
    pub fn assert_capability_valid(cap: &Capability, current_time: u64) {
        assert!(
            cap.is_valid(current_time),
            "Expected capability to be valid at time {}, but it was not. Expired: {}, Revoked: {}, Quota exhausted: {}",
            current_time,
            cap.is_expired(current_time),
            cap.is_revoked(),
            cap.quota.is_exhausted()
        );
    }

    /// Assert that a capability is invalid
    pub fn assert_capability_invalid(cap: &Capability, current_time: u64) {
        assert!(
            !cap.is_valid(current_time),
            "Expected capability to be invalid at time {}, but it was valid",
            current_time
        );
    }

    /// Assert that a capability is expired
    pub fn assert_capability_expired(cap: &Capability, current_time: u64) {
        assert!(
            cap.is_expired(current_time),
            "Expected capability to be expired at time {} (expires_at: {})",
            current_time,
            cap.expires_at
        );
    }

    /// Assert that a capability is revoked
    pub fn assert_capability_revoked(cap: &Capability) {
        assert!(cap.is_revoked(), "Expected capability to be revoked");
    }

    /// Assert that quota is exhausted
    pub fn assert_quota_exhausted(quota: &Quota) {
        assert!(
            quota.is_exhausted(),
            "Expected quota to be exhausted: invocations {}/{}, bytes {}/{}, duration {}/{}",
            quota.used_invocations,
            quota.max_invocations,
            quota.used_bytes,
            quota.max_bytes,
            quota.used_duration_ns,
            quota.max_duration_ns
        );
    }

    /// Assert that budget can satisfy a request
    pub fn assert_budget_sufficient(budget: &Budget, request: &BudgetVector) {
        let remaining = budget.remaining();
        assert!(
            remaining.can_satisfy(request),
            "Expected budget to be sufficient. Remaining: {:?}, Requested: {:?}",
            remaining,
            request
        );
    }

    /// Assert that budget cannot satisfy a request
    pub fn assert_budget_insufficient(budget: &Budget, request: &BudgetVector) {
        let remaining = budget.remaining();
        assert!(
            !remaining.can_satisfy(request),
            "Expected budget to be insufficient. Remaining: {:?}, Requested: {:?}",
            remaining,
            request
        );
    }

    /// Assert rights contain specific right
    pub fn assert_has_right(rights: Rights, right: u32) {
        assert!(
            rights.has(right),
            "Expected rights {:032b} to contain {:032b}",
            rights.0,
            right
        );
    }

    /// Assert rights do not contain specific right
    pub fn assert_missing_right(rights: Rights, right: u32) {
        assert!(
            !rights.has(right),
            "Expected rights {:032b} to not contain {:032b}",
            rights.0,
            right
        );
    }

    /// Assert scope permits target
    pub fn assert_scope_permits(scope: &CapabilityScope, target: &str) {
        assert!(
            scope.permits(target),
            "Expected scope {:?} to permit '{}'",
            scope,
            target
        );
    }

    /// Assert scope denies target
    pub fn assert_scope_denies(scope: &CapabilityScope, target: &str) {
        assert!(
            !scope.permits(target),
            "Expected scope {:?} to deny '{}'",
            scope,
            target
        );
    }
}

// ============================================================================
// Property Test Generators
// ============================================================================

/// Generators for property-based testing
pub mod generators {
    use super::*;

    /// Simple linear congruential generator for deterministic random numbers
    pub struct SimpleRng {
        state: u64,
    }

    impl SimpleRng {
        pub fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        pub fn next_u64(&mut self) -> u64 {
            // LCG parameters from Numerical Recipes
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.state
        }

        pub fn next_u32(&mut self) -> u32 {
            (self.next_u64() >> 32) as u32
        }

        pub fn next_u8(&mut self) -> u8 {
            (self.next_u64() >> 56) as u8
        }

        pub fn gen_range(&mut self, min: u64, max: u64) -> u64 {
            if min >= max {
                return min;
            }
            min + (self.next_u64() % (max - min))
        }

        pub fn gen_bool(&mut self) -> bool {
            self.next_u64() & 1 == 1
        }
    }

    /// Generate random capsule ID
    pub fn gen_capsule_id(rng: &mut SimpleRng) -> CapsuleId {
        let mut bytes = [0u8; 16];
        for byte in &mut bytes {
            *byte = rng.next_u8();
        }
        CapsuleId::from_bytes(bytes)
    }

    /// Generate random capability ID
    pub fn gen_capability_id(rng: &mut SimpleRng) -> CapabilityId {
        CapabilityId::from_raw(rng.next_u64() as u128 | ((rng.next_u64() as u128) << 64))
    }

    /// Generate random budget vector
    pub fn gen_budget_vector(rng: &mut SimpleRng) -> BudgetVector {
        BudgetVector::new(
            rng.gen_range(0, 100000),
            rng.gen_range(0, 1000000),
            rng.gen_range(0, 100 * 1024 * 1024),
            rng.gen_range(0, 10 * 1024 * 1024),
            rng.gen_range(0, 10 * 1024 * 1024),
            rng.gen_range(0, 10000),
        )
    }

    /// Generate random rights
    pub fn gen_rights(rng: &mut SimpleRng) -> Rights {
        Rights::new(rng.next_u32() & Rights::ALL)
    }

    /// Generate random capability type
    pub fn gen_capability_type(rng: &mut SimpleRng) -> CapabilityType {
        let types = [
            CapabilityType::NetworkHttp,
            CapabilityType::NetworkTcp,
            CapabilityType::FileRead,
            CapabilityType::FileWrite,
            CapabilityType::ProcessSpawn,
            CapabilityType::SecretRead,
            CapabilityType::ClockRead,
            CapabilityType::RandomSecure,
            CapabilityType::EvidenceAppend,
        ];
        types[rng.gen_range(0, types.len() as u64) as usize]
    }

    /// Generate random quota
    pub fn gen_quota(rng: &mut SimpleRng) -> Quota {
        Quota::new(
            rng.gen_range(1, 1000),
            rng.gen_range(1024, 10 * 1024 * 1024),
            rng.gen_range(1_000_000, 1_000_000_000_000),
        )
    }

    /// Generate random capability
    pub fn gen_capability(rng: &mut SimpleRng) -> Capability {
        Capability {
            id: gen_capability_id(rng),
            cap_type: gen_capability_type(rng),
            scope: CapabilityScope::Global,
            rights: gen_rights(rng),
            quota: gen_quota(rng),
            expires_at: rng.gen_range(1_000_000_000, u64::MAX / 2),
            parent: if rng.gen_bool() {
                Some(gen_capability_id(rng))
            } else {
                None
            },
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        }
    }

    /// Generate random capability grant
    pub fn gen_capability_grant(rng: &mut SimpleRng) -> CapabilityGrant {
        CapabilityGrant::new(gen_capability_type(rng))
            .with_rights(gen_rights(rng))
            .with_quota(gen_quota(rng))
            .with_lease(rng.gen_range(60, 86400))
    }
}

// ============================================================================
// Tests for test utilities
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use assertions::*;
    use generators::*;

    #[test]
    fn test_mock_executor() {
        let executor = MockExecutor::new("test")
            .with_response(vec![1, 2, 3])
            .with_budget(BudgetVector::new(10, 20, 30, 40, 50, 60));

        let result = executor.execute_simple(CapabilityType::NetworkHttp, "target", &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn test_mock_executor_failure() {
        let executor = MockExecutor::new("test").failing();

        let result = executor.execute_simple(CapabilityType::NetworkHttp, "target", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_evidence_logger() {
        let logger = MockEvidenceLogger::new();
        let cap = TestFixtures::capability(
            TestFixtures::capability_id(),
            CapabilityType::NetworkHttp,
        );

        logger.log_grant(TestFixtures::capsule_id(), &cap);
        assert_eq!(logger.grant_count(), 1);

        logger.log_revoke(TestFixtures::capsule_id(), cap.id);
        assert_eq!(logger.revoke_count(), 1);

        let eid = logger.log_invoke(
            TestFixtures::capsule_id(),
            cap.id,
            "target",
            true,
        );
        assert_eq!(logger.invoke_count(), 1);
        assert!(eid > 0);

        assert_eq!(logger.total_count(), 3);
        assert_eq!(logger.events().len(), 3);

        logger.clear();
        assert_eq!(logger.total_count(), 0);
    }

    #[test]
    fn test_fixtures() {
        let capsule = TestFixtures::capsule_id();
        let cap_id = TestFixtures::capability_id();
        let budget = TestFixtures::budget();
        let grant = TestFixtures::http_grant();

        assert!(!capsule.is_null());
        assert!(!cap_id.is_null());
        assert!(!budget.is_exhausted());
        assert_eq!(grant.cap_type, CapabilityType::NetworkHttp);
    }

    #[test]
    fn test_assertions_capability_valid() {
        let cap = TestFixtures::capability(
            TestFixtures::capability_id(),
            CapabilityType::NetworkHttp,
        );
        assert_capability_valid(&cap, 1000);
    }

    #[test]
    fn test_assertions_capability_expired() {
        let cap = TestFixtures::expired_capability(TestFixtures::capability_id());
        assert_capability_expired(&cap, 1000);
        assert_capability_invalid(&cap, 1000);
    }

    #[test]
    fn test_assertions_capability_revoked() {
        let cap = TestFixtures::revoked_capability(TestFixtures::capability_id());
        assert_capability_revoked(&cap);
        assert_capability_invalid(&cap, 0);
    }

    #[test]
    fn test_assertions_rights() {
        let rights = Rights::new(Rights::READ | Rights::WRITE);
        assert_has_right(rights, Rights::READ);
        assert_has_right(rights, Rights::WRITE);
        assert_missing_right(rights, Rights::DELETE);
    }

    #[test]
    fn test_assertions_scope() {
        let scope = CapabilityScope::Network {
            hosts: vec!["example.com".to_string()],
            ports: vec![443],
            protocols: vec![],
        };
        assert_scope_permits(&scope, "example.com");
        assert_scope_denies(&scope, "other.com");
    }

    #[test]
    fn test_generators() {
        let mut rng = SimpleRng::new(12345);

        // Test determinism
        let cap1 = gen_capability(&mut rng);
        let mut rng2 = SimpleRng::new(12345);
        let cap2 = gen_capability(&mut rng2);
        assert_eq!(cap1.id.as_raw(), cap2.id.as_raw());

        // Generate various types
        for _ in 0..100 {
            let _ = gen_capsule_id(&mut rng);
            let _ = gen_capability_id(&mut rng);
            let _ = gen_budget_vector(&mut rng);
            let _ = gen_rights(&mut rng);
            let _ = gen_capability_type(&mut rng);
            let _ = gen_quota(&mut rng);
            let _ = gen_capability(&mut rng);
            let _ = gen_capability_grant(&mut rng);
        }
    }

    #[test]
    fn test_budget_assertions() {
        let budget = TestFixtures::budget();
        let small_request = TestFixtures::small_budget_vector();
        let large_request = BudgetVector::new(
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
        );

        assert_budget_sufficient(&budget, &small_request);
        assert_budget_insufficient(&budget, &large_request);
    }
}
