//! # agentvm-types
//! 
//! Core types for Agentic VM - the accountable agent capsule runtime.
//! 
//! This crate provides shared types used across all Agentic VM components:
//! - Capsule identifiers and manifests
//! - Capability tokens and scopes
//! - Budget vectors and tracking
//! - Evidence bundle structures
//! 
//! ## Features
//! 
//! - `std` - Enable standard library support
//! - `serde` - Enable serialization/deserialization
//! - `alloc` - Enable alloc-only features (no_std compatible)

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod capsule;
pub mod capability;
pub mod budget;
pub mod evidence;
pub mod error;

// Re-exports for convenience
pub use capsule::{CapsuleId, CapsuleManifest, CapsuleIdentity, SignatureAlgorithm};
pub use capability::{
    CapabilityId, CapabilityType, Capability, CapabilityScope, CapabilityScopeType,
    Rights, Quota, CapabilityProof, CapabilityGrant,
};
pub use budget::{Budget, BudgetVector, QuotaConsumed};
pub use evidence::{
    EvidenceBundle, EvidenceStatement, EvidenceHeader,
    EvidenceInputs, EvidenceExecution, EvidenceOutputs,
    CapabilityCallRecord, NetworkEvent, MerkleProof,
};
pub use error::{AgentVmError, Result};

/// Hash type used throughout (SHA-256)
pub type Hash = [u8; 32];

/// Timestamp in nanoseconds since Unix epoch
pub type TimestampNs = u64;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::string::ToString;
    use capability::{NetworkScope, FilesystemScope, PathPattern, ProcessScope, SecretsScope, PortRange, Protocol};

    // ============================================================================
    // CapsuleId Tests
    // ============================================================================

    #[test]
    fn test_capsule_id_generation() {
        let id1 = CapsuleId::generate();
        let id2 = CapsuleId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_capsule_id_from_bytes() {
        let bytes = [0xAB; 16];
        let id = CapsuleId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn test_capsule_id_equality() {
        let id1 = CapsuleId::from_bytes([0x01; 16]);
        let id2 = CapsuleId::from_bytes([0x01; 16]);
        let id3 = CapsuleId::from_bytes([0x02; 16]);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    // ============================================================================
    // CapabilityId Tests
    // ============================================================================

    #[test]
    fn test_capability_id_generation() {
        let id1 = CapabilityId::generate();
        let id2 = CapabilityId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_capability_id_from_bytes() {
        let bytes = [0xCD; 16];
        let id = CapabilityId::from_bytes(bytes);
        assert_eq!(id.0, bytes);
    }

    // ============================================================================
    // CapabilityType Tests
    // ============================================================================

    #[test]
    fn test_capability_type_scope_type() {
        assert_eq!(CapabilityType::NetworkHttp.scope_type(), capability::CapabilityScopeType::Network);
        assert_eq!(CapabilityType::NetworkTcp.scope_type(), capability::CapabilityScopeType::Network);
        assert_eq!(CapabilityType::FileRead.scope_type(), capability::CapabilityScopeType::Filesystem);
        assert_eq!(CapabilityType::FileWrite.scope_type(), capability::CapabilityScopeType::Filesystem);
        assert_eq!(CapabilityType::ProcessSpawn.scope_type(), capability::CapabilityScopeType::Process);
        assert_eq!(CapabilityType::SecretRead.scope_type(), capability::CapabilityScopeType::Secrets);
        assert_eq!(CapabilityType::ClockRead.scope_type(), capability::CapabilityScopeType::Clock);
        assert_eq!(CapabilityType::RandomSecure.scope_type(), capability::CapabilityScopeType::Random);
        assert_eq!(CapabilityType::EvidenceAppend.scope_type(), capability::CapabilityScopeType::Evidence);
        assert_eq!(CapabilityType::CapsuleSend.scope_type(), capability::CapabilityScopeType::InterCapsule);
        assert_eq!(CapabilityType::MemoryAllocate.scope_type(), capability::CapabilityScopeType::Resource);
    }

    #[test]
    fn test_capability_type_name() {
        assert_eq!(CapabilityType::NetworkHttp.name(), "network.http");
        assert_eq!(CapabilityType::FileRead.name(), "filesystem.read");
        assert_eq!(CapabilityType::ProcessSpawn.name(), "process.spawn");
    }

    // ============================================================================
    // Rights Tests
    // ============================================================================

    #[test]
    fn test_rights_has() {
        let rights = Rights(Rights::READ | Rights::WRITE);
        assert!(rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));
        assert!(!rights.has(Rights::DELETE));
        assert!(!rights.has(Rights::EXECUTE));
    }

    #[test]
    fn test_rights_intersect() {
        let rw = Rights(Rights::READ | Rights::WRITE);
        let rd = Rights(Rights::READ | Rights::DELETE);
        let result = rw.intersect(rd);
        assert!(result.has(Rights::READ));
        assert!(!result.has(Rights::WRITE));
        assert!(!result.has(Rights::DELETE));
    }

    #[test]
    fn test_rights_is_subset_of() {
        let read_only = Rights(Rights::READ);
        let read_write = Rights(Rights::READ | Rights::WRITE);
        let all = Rights::ALL;

        assert!(read_only.is_subset_of(read_write));
        assert!(read_only.is_subset_of(all));
        assert!(read_write.is_subset_of(all));
        assert!(!read_write.is_subset_of(read_only));
    }

    #[test]
    fn test_rights_all_and_none() {
        assert_eq!(Rights::NONE.bits(), 0);
        assert!(Rights::ALL.has(Rights::READ));
        assert!(Rights::ALL.has(Rights::WRITE));
        assert!(Rights::ALL.has(Rights::EXECUTE));
        assert!(Rights::ALL.has(Rights::DELETE));
        assert!(Rights::ALL.has(Rights::DELEGATE));
        assert!(Rights::ALL.has(Rights::AUDIT));
    }

    // ============================================================================
    // Quota Tests
    // ============================================================================

    #[test]
    fn test_quota_is_exhausted() {
        let mut quota = Quota {
            max_invocations: 10,
            used_invocations: 5,
            max_bytes: 1000,
            used_bytes: 500,
            max_duration_ns: 1_000_000,
            used_duration_ns: 500_000,
        };
        assert!(!quota.is_exhausted());

        quota.used_invocations = 10;
        assert!(quota.is_exhausted());
    }

    #[test]
    fn test_quota_remaining() {
        let quota = Quota {
            max_invocations: 100,
            used_invocations: 30,
            max_bytes: 1000,
            used_bytes: 400,
            max_duration_ns: 1_000_000,
            used_duration_ns: 0,
        };
        assert_eq!(quota.remaining_invocations(), 70);
        assert_eq!(quota.remaining_bytes(), 600);
    }

    #[test]
    fn test_quota_unlimited() {
        let quota = Quota::UNLIMITED;
        assert!(!quota.is_exhausted());
        assert_eq!(quota.remaining_invocations(), u64::MAX);
    }

    // ============================================================================
    // Budget Tests
    // ============================================================================

    #[test]
    fn test_budget_creation() {
        let budget = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        assert_eq!(budget.cpu_time_ms, 1000);
        assert_eq!(budget.wall_time_ms, 2000);
        assert_eq!(budget.memory_bytes, 3000);
        assert_eq!(budget.disk_write_bytes, 4000);
        assert_eq!(budget.network_bytes, 5000);
        assert_eq!(budget.network_requests, 100);
    }

    #[test]
    fn test_budget_can_satisfy() {
        let available = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        let required_small = Budget::new(500, 1000, 1500, 2000, 2500, 50);
        let required_large = Budget::new(2000, 3000, 4000, 5000, 6000, 200);

        assert!(available.can_satisfy(&required_small));
        assert!(!available.can_satisfy(&required_large));
    }

    #[test]
    fn test_budget_saturating_sub() {
        let budget = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        let consumption = Budget::new(500, 1000, 1500, 2000, 2500, 50);

        let remaining = budget.saturating_sub(&consumption);
        assert_eq!(remaining.cpu_time_ms, 500);
        assert_eq!(remaining.wall_time_ms, 1000);
        assert_eq!(remaining.memory_bytes, 1500);
    }

    #[test]
    fn test_budget_saturating_sub_underflow() {
        let budget = Budget::new(100, 200, 300, 400, 500, 10);
        let consumption = Budget::new(200, 300, 400, 500, 600, 20);

        let remaining = budget.saturating_sub(&consumption);
        assert_eq!(remaining.cpu_time_ms, 0);
        assert_eq!(remaining.wall_time_ms, 0);
    }

    #[test]
    fn test_budget_is_exhausted() {
        let exhausted = Budget::ZERO;
        assert!(exhausted.is_exhausted());

        let unlimited = Budget::UNLIMITED;
        assert!(!unlimited.is_exhausted());
    }

    #[test]
    fn test_budget_add() {
        let a = Budget::new(100, 200, 300, 400, 500, 10);
        let b = Budget::new(50, 100, 150, 200, 250, 5);
        let sum = a + b;

        assert_eq!(sum.cpu_time_ms, 150);
        assert_eq!(sum.wall_time_ms, 300);
        assert_eq!(sum.network_requests, 15);
    }

    // ============================================================================
    // BudgetVector Tests
    // ============================================================================

    #[test]
    fn test_budget_vector_creation() {
        let initial = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        let vector = BudgetVector::new(initial);

        assert_eq!(vector.remaining(), initial);
        assert!(!vector.is_exhausted());
    }

    #[test]
    fn test_budget_vector_consume() {
        let initial = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        let mut vector = BudgetVector::new(initial);

        let consumption = Budget::new(500, 1000, 1500, 2000, 2500, 50);
        assert!(vector.consume(&consumption));

        let remaining = vector.remaining();
        assert_eq!(remaining.cpu_time_ms, 500);
    }

    #[test]
    fn test_budget_vector_consume_fails_insufficient() {
        let initial = Budget::new(100, 200, 300, 400, 500, 10);
        let mut vector = BudgetVector::new(initial);

        let large_consumption = Budget::new(200, 300, 400, 500, 600, 20);
        assert!(!vector.consume(&large_consumption));

        // Should remain unchanged
        assert_eq!(vector.remaining(), initial);
    }

    #[test]
    fn test_budget_vector_utilization() {
        let initial = Budget::new(1000, 2000, 3000, 4000, 5000, 100);
        let mut vector = BudgetVector::new(initial);

        assert_eq!(vector.utilization(), 0.0);

        let consumption = Budget::new(500, 1000, 1500, 2000, 2500, 50);
        vector.consume(&consumption);

        assert!((vector.utilization() - 0.5).abs() < 0.001);
    }

    // ============================================================================
    // CapabilityScope Tests
    // ============================================================================

    #[test]
    fn test_scope_unrestricted() {
        let scope = CapabilityScope::Unrestricted;
        assert!(scope.permits("anything"));
        assert!(scope.permits("https://example.com"));
        assert!(scope.permits("/any/path"));
    }

    #[test]
    fn test_network_scope_permits_wildcard() {
        let scope = NetworkScope {
            hosts: vec!["*.github.com".to_string()],
            ports: vec![],
            protocols: vec![],
        };

        assert!(scope.permits("api.github.com"));
        assert!(scope.permits("raw.github.com"));
        assert!(!scope.permits("example.com"));
    }

    #[test]
    fn test_network_scope_permits_exact() {
        let scope = NetworkScope {
            hosts: vec!["api.anthropic.com".to_string()],
            ports: vec![],
            protocols: vec![],
        };

        assert!(scope.permits("api.anthropic.com"));
        assert!(!scope.permits("other.anthropic.com"));
    }

    #[test]
    fn test_filesystem_scope_permits() {
        let scope = FilesystemScope {
            paths: vec![
                PathPattern { pattern: "/workspace/**".to_string(), exclude: false },
                PathPattern { pattern: "/workspace/.env".to_string(), exclude: true },
            ],
        };

        assert!(scope.permits("/workspace/src/main.rs"));
        assert!(scope.permits("/workspace/Cargo.toml"));
        assert!(!scope.permits("/workspace/.env")); // Excluded
        assert!(!scope.permits("/etc/passwd")); // Not in workspace
    }

    #[test]
    fn test_process_scope_permits() {
        let scope = ProcessScope {
            executables: vec!["npm".to_string(), "node".to_string(), "git".to_string()],
            env_allowlist: vec![],
            args_pattern: None,
        };

        assert!(scope.permits("npm"));
        assert!(scope.permits("node"));
        assert!(!scope.permits("rm"));
    }

    #[test]
    fn test_secrets_scope_permits() {
        let scope = SecretsScope {
            names: vec!["API_KEY".to_string(), "DATABASE_URL".to_string()],
        };

        assert!(scope.permits("API_KEY"));
        assert!(scope.permits("DATABASE_URL"));
        assert!(!scope.permits("PASSWORD"));
    }

    #[test]
    fn test_scope_intersect() {
        let scope1 = CapabilityScope::Unrestricted;
        let scope2 = CapabilityScope::Network(NetworkScope {
            hosts: vec!["example.com".to_string()],
            ports: vec![],
            protocols: vec![],
        });

        let result = scope1.intersect(&scope2);
        assert!(result.is_some());

        // Unrestricted intersect specific = specific
        let intersected = result.unwrap();
        assert!(intersected.permits("example.com"));
    }

    // ============================================================================
    // Port Range Tests
    // ============================================================================

    #[test]
    fn test_port_range_single() {
        let range = PortRange::single(443);
        assert!(range.contains(443));
        assert!(!range.contains(80));
    }

    #[test]
    fn test_port_range_range() {
        let range = PortRange::range(8000, 9000);
        assert!(range.contains(8000));
        assert!(range.contains(8500));
        assert!(range.contains(9000));
        assert!(!range.contains(7999));
        assert!(!range.contains(9001));
    }

    // ============================================================================
    // Capability Tests
    // ============================================================================

    #[test]
    fn test_capability_is_expired() {
        let cap = Capability {
            id: CapabilityId::generate(),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Unrestricted,
            rights: Rights::ALL,
            quota: Quota::UNLIMITED,
            expires_at: 1000,
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0u8; 64],
                issued_at: 0,
            },
            revoked: false,
        };

        assert!(!cap.is_expired(500));
        assert!(cap.is_expired(1000));
        assert!(cap.is_expired(2000));
    }

    #[test]
    fn test_capability_is_revoked() {
        let mut cap = Capability {
            id: CapabilityId::generate(),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Unrestricted,
            rights: Rights::ALL,
            quota: Quota::UNLIMITED,
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0u8; 64],
                issued_at: 0,
            },
            revoked: false,
        };

        assert!(!cap.is_revoked());
        cap.revoked = true;
        assert!(cap.is_revoked());
    }

    #[test]
    fn test_capability_is_quota_exhausted() {
        let mut cap = Capability {
            id: CapabilityId::generate(),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Unrestricted,
            rights: Rights::ALL,
            quota: Quota {
                max_invocations: 10,
                used_invocations: 5,
                max_bytes: u64::MAX,
                used_bytes: 0,
                max_duration_ns: u64::MAX,
                used_duration_ns: 0,
            },
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0u8; 64],
                issued_at: 0,
            },
            revoked: false,
        };

        assert!(!cap.is_quota_exhausted());
        cap.quota.used_invocations = 10;
        assert!(cap.is_quota_exhausted());
    }

    // ============================================================================
    // QuotaConsumed Tests
    // ============================================================================

    #[test]
    fn test_quota_consumed_single() {
        let consumed = QuotaConsumed::single(1024, 1_000_000);
        assert_eq!(consumed.invocations, 1);
        assert_eq!(consumed.bytes, 1024);
        assert_eq!(consumed.duration_ns, 1_000_000);
    }

    #[test]
    fn test_quota_consumed_zero() {
        let zero = QuotaConsumed::ZERO;
        assert_eq!(zero.invocations, 0);
        assert_eq!(zero.bytes, 0);
        assert_eq!(zero.duration_ns, 0);
    }
}
