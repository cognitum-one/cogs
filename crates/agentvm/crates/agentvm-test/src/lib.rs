//! Test utilities for Agentic VM
//!
//! This crate provides:
//! - Test fixtures (pre-built capabilities, capsules, budgets)
//! - Mock implementations
//! - Property test generators
//! - Assertion helpers

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use agentvm_types::{
    Budget, BudgetVector, Capability, CapabilityId, CapabilityProof, CapabilityScope, CapabilityType,
    CapsuleId, Quota, Rights,
};

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
        CapabilityId::from_bytes([0x01; 16])
    }

    /// Generate numbered capability ID
    pub fn capability_id_n(n: u8) -> CapabilityId {
        CapabilityId::from_bytes([n; 16])
    }

    // Budgets

    /// Standard test budget with generous limits
    pub fn budget() -> Budget {
        Budget::new(
            10000,        // 10s CPU
            60000,        // 60s wall
            1024 * 1024,  // 1MB memory
            1024 * 1024,  // 1MB disk
            1024 * 1024,  // 1MB network
            1000,         // 1000 requests
        )
    }

    /// Small budget for testing limits
    pub fn small_budget() -> Budget {
        Budget::new(
            100,   // 100ms CPU
            1000,  // 1s wall
            1024,  // 1KB memory
            1024,  // 1KB disk
            1024,  // 1KB network
            10,    // 10 requests
        )
    }

    /// Unlimited budget
    pub fn unlimited_budget() -> Budget {
        Budget::UNLIMITED
    }

    /// Zero budget (already exhausted)
    pub fn zero_budget() -> Budget {
        Budget::ZERO
    }

    // Capabilities

    /// Create a test capability
    pub fn capability(cap_type: CapabilityType) -> Capability {
        Capability {
            id: Self::capability_id(),
            cap_type,
            scope: CapabilityScope::Unrestricted,
            rights: Rights::ALL,
            quota: Quota::UNLIMITED,
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0x42u8; 64],
                issued_at: 0,
            },
            revoked: false,
        }
    }

    /// Create an expired capability
    pub fn expired_capability() -> Capability {
        let mut cap = Self::capability(CapabilityType::NetworkHttp);
        cap.expires_at = 0; // Already expired
        cap
    }

    /// Create a revoked capability
    pub fn revoked_capability() -> Capability {
        let mut cap = Self::capability(CapabilityType::NetworkHttp);
        cap.revoked = true;
        cap
    }

    /// Create a capability with exhausted quota
    pub fn exhausted_capability() -> Capability {
        let mut cap = Self::capability(CapabilityType::NetworkHttp);
        cap.quota = Quota {
            max_invocations: 10,
            used_invocations: 10,
            max_bytes: 1024,
            used_bytes: 1024,
            max_duration_ns: 1000,
            used_duration_ns: 1000,
        };
        cap
    }

    /// Signing key for tests
    pub fn signing_key() -> [u8; 32] {
        [0x42u8; 32]
    }
}

// ============================================================================
// Generators (deterministic RNG for property-like testing)
// ============================================================================

/// Simple linear congruential generator for deterministic random numbers
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    /// Create with seed
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Get next u64
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    /// Get next u8
    pub fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    /// Generate range
    pub fn gen_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min))
    }

    /// Generate bool
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
    let mut bytes = [0u8; 16];
    for byte in &mut bytes {
        *byte = rng.next_u8();
    }
    CapabilityId::from_bytes(bytes)
}

/// Generate random budget
pub fn gen_budget(rng: &mut SimpleRng) -> Budget {
    Budget::new(
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
    Rights::from_bits((rng.next_u64() as u32) & 0x3F)
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

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assert capability is valid
pub fn assert_capability_valid(cap: &Capability, current_time: u64) {
    assert!(
        !cap.is_expired(current_time),
        "Expected capability to not be expired at time {}",
        current_time
    );
    assert!(!cap.is_revoked(), "Expected capability to not be revoked");
    assert!(
        !cap.is_quota_exhausted(),
        "Expected capability quota to not be exhausted"
    );
}

/// Assert capability is expired
pub fn assert_capability_expired(cap: &Capability, current_time: u64) {
    assert!(
        cap.is_expired(current_time),
        "Expected capability to be expired at time {} (expires_at: {})",
        current_time,
        cap.expires_at
    );
}

/// Assert capability is revoked
pub fn assert_capability_revoked(cap: &Capability) {
    assert!(cap.is_revoked(), "Expected capability to be revoked");
}

/// Assert quota is exhausted
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

/// Assert budget can satisfy requirements
pub fn assert_budget_sufficient(budget: &Budget, required: &Budget) {
    assert!(
        budget.can_satisfy(required),
        "Expected budget to be sufficient. Available: {:?}, Required: {:?}",
        budget,
        required
    );
}

/// Assert rights contain specific right
pub fn assert_has_right(rights: Rights, right: u32) {
    assert!(
        rights.has(right),
        "Expected rights {:032b} to contain {:032b}",
        rights.bits(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures() {
        let capsule = TestFixtures::capsule_id();
        let cap_id = TestFixtures::capability_id();
        let budget = TestFixtures::budget();
        let cap = TestFixtures::capability(CapabilityType::NetworkHttp);

        assert!(!budget.is_exhausted());
        assert!(!cap.is_revoked());
    }

    #[test]
    fn test_simple_rng_determinism() {
        let mut rng1 = SimpleRng::new(12345);
        let mut rng2 = SimpleRng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_generators() {
        let mut rng = SimpleRng::new(42);

        for _ in 0..100 {
            let _ = gen_capsule_id(&mut rng);
            let _ = gen_capability_id(&mut rng);
            let _ = gen_budget(&mut rng);
            let _ = gen_rights(&mut rng);
            let _ = gen_capability_type(&mut rng);
        }
    }

    #[test]
    fn test_assertions() {
        let cap = TestFixtures::capability(CapabilityType::NetworkHttp);
        assert_capability_valid(&cap, 0);

        let expired = TestFixtures::expired_capability();
        assert_capability_expired(&expired, 1000);

        let revoked = TestFixtures::revoked_capability();
        assert_capability_revoked(&revoked);
    }
}
