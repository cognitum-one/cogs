//! Comprehensive tests for agentvm-types crate
//!
//! Test coverage:
//! - CapsuleId generation and comparison
//! - Capability serialization round-trip
//! - Budget arithmetic
//! - Rights bitflags operations
//! - Quota tracking and exhaustion

use super::*;

mod capsule_id_tests {
    use super::*;

    #[test]
    fn test_capsule_id_generation() {
        let id1 = CapsuleId::new_random();
        let id2 = CapsuleId::new_random();

        // Two random IDs should be different (with very high probability)
        assert_ne!(id1, id2);

        // Should not be null
        assert!(!id1.is_null());
        assert!(!id2.is_null());
    }

    #[test]
    fn test_capsule_id_null() {
        let null_id = CapsuleId::null();
        assert!(null_id.is_null());
        assert_eq!(null_id.as_bytes(), &[0u8; 16]);
    }

    #[test]
    fn test_capsule_id_from_bytes() {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let id = CapsuleId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
        assert!(!id.is_null());
    }

    #[test]
    fn test_capsule_id_comparison() {
        let id1 = CapsuleId::from_bytes([0u8; 16]);
        let id2 = CapsuleId::from_bytes([1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let id3 = CapsuleId::from_bytes([0u8; 16]);

        // Equality
        assert_eq!(id1, id3);
        assert_ne!(id1, id2);

        // Ordering
        assert!(id1 < id2);
        assert!(id2 > id1);
        assert!(id1 <= id3);
        assert!(id1 >= id3);
    }

    #[test]
    fn test_capsule_id_display() {
        let bytes = [0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let id = CapsuleId::from_bytes(bytes);
        let display = format!("{}", id);
        assert!(display.starts_with("deadbeef"));
    }

    #[test]
    fn test_capsule_id_debug() {
        let bytes = [0xab, 0xcd, 0xef, 0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let id = CapsuleId::from_bytes(bytes);
        let debug = format!("{:?}", id);
        assert!(debug.contains("CapsuleId"));
    }

    #[test]
    fn test_capsule_id_hash() {
        use alloc::collections::BTreeSet;

        let id1 = CapsuleId::from_bytes([1u8; 16]);
        let id2 = CapsuleId::from_bytes([2u8; 16]);
        let id3 = CapsuleId::from_bytes([1u8; 16]);

        let mut set = BTreeSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id3); // Duplicate of id1

        assert_eq!(set.len(), 2);
    }
}

mod capability_id_tests {
    use super::*;

    #[test]
    fn test_capability_id_generation() {
        let id1 = CapabilityId::generate();
        let id2 = CapabilityId::generate();

        // Should produce different IDs (with high probability)
        assert_ne!(id1, id2);
        assert!(!id1.is_null());
    }

    #[test]
    fn test_capability_id_null() {
        let null_id = CapabilityId::null();
        assert!(null_id.is_null());
        assert_eq!(null_id.as_raw(), 0);
    }

    #[test]
    fn test_capability_id_from_raw() {
        let id = CapabilityId::from_raw(0x123456789abcdef0);
        assert_eq!(id.as_raw(), 0x123456789abcdef0);
        assert!(!id.is_null());
    }
}

mod capability_type_tests {
    use super::*;

    #[test]
    fn test_capability_type_category() {
        assert_eq!(CapabilityType::NetworkHttp.category(), 0x01);
        assert_eq!(CapabilityType::NetworkTcp.category(), 0x01);
        assert_eq!(CapabilityType::FileRead.category(), 0x02);
        assert_eq!(CapabilityType::ProcessSpawn.category(), 0x03);
        assert_eq!(CapabilityType::SecretRead.category(), 0x04);
    }

    #[test]
    fn test_capability_type_is_network() {
        assert!(CapabilityType::NetworkHttp.is_network());
        assert!(CapabilityType::NetworkTcp.is_network());
        assert!(CapabilityType::NetworkDns.is_network());
        assert!(!CapabilityType::FileRead.is_network());
        assert!(!CapabilityType::ProcessSpawn.is_network());
    }

    #[test]
    fn test_capability_type_is_filesystem() {
        assert!(CapabilityType::FileRead.is_filesystem());
        assert!(CapabilityType::FileWrite.is_filesystem());
        assert!(CapabilityType::FileDelete.is_filesystem());
        assert!(CapabilityType::DirectoryList.is_filesystem());
        assert!(!CapabilityType::NetworkHttp.is_filesystem());
    }

    #[test]
    fn test_capability_type_is_process() {
        assert!(CapabilityType::ProcessSpawn.is_process());
        assert!(CapabilityType::ProcessSignal.is_process());
        assert!(!CapabilityType::FileRead.is_process());
    }
}

mod rights_tests {
    use super::*;

    #[test]
    fn test_rights_constants() {
        assert_eq!(Rights::READ, 1);
        assert_eq!(Rights::WRITE, 2);
        assert_eq!(Rights::EXECUTE, 4);
        assert_eq!(Rights::DELETE, 8);
        assert_eq!(Rights::DELEGATE, 16);
        assert_eq!(Rights::AUDIT, 32);
    }

    #[test]
    fn test_rights_has() {
        let rights = Rights::new(Rights::READ | Rights::WRITE);
        assert!(rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));
        assert!(!rights.has(Rights::EXECUTE));
        assert!(!rights.has(Rights::DELETE));
    }

    #[test]
    fn test_rights_add_remove() {
        let mut rights = Rights::new(Rights::READ);
        assert!(rights.has(Rights::READ));
        assert!(!rights.has(Rights::WRITE));

        rights.add(Rights::WRITE);
        assert!(rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));

        rights.remove(Rights::READ);
        assert!(!rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));
    }

    #[test]
    fn test_rights_intersect() {
        let r1 = Rights::new(Rights::READ | Rights::WRITE | Rights::EXECUTE);
        let r2 = Rights::new(Rights::READ | Rights::DELETE);

        let intersection = r1.intersect(r2);
        assert!(intersection.has(Rights::READ));
        assert!(!intersection.has(Rights::WRITE));
        assert!(!intersection.has(Rights::EXECUTE));
        assert!(!intersection.has(Rights::DELETE));
    }

    #[test]
    fn test_rights_union() {
        let r1 = Rights::new(Rights::READ);
        let r2 = Rights::new(Rights::WRITE);

        let union = r1.union(r2);
        assert!(union.has(Rights::READ));
        assert!(union.has(Rights::WRITE));
    }

    #[test]
    fn test_rights_is_subset_of() {
        let subset = Rights::new(Rights::READ);
        let superset = Rights::new(Rights::READ | Rights::WRITE);
        let disjoint = Rights::new(Rights::EXECUTE);

        assert!(subset.is_subset_of(superset));
        assert!(!superset.is_subset_of(subset));
        assert!(!disjoint.is_subset_of(subset));
    }

    #[test]
    fn test_rights_all_none() {
        let all = Rights::all();
        let none = Rights::none();

        assert!(all.has(Rights::READ));
        assert!(all.has(Rights::WRITE));
        assert!(all.has(Rights::EXECUTE));
        assert!(all.has(Rights::DELETE));
        assert!(all.has(Rights::DELEGATE));
        assert!(all.has(Rights::AUDIT));

        assert!(none.is_empty());
        assert!(!none.has(Rights::READ));
    }

    #[test]
    fn test_rights_count() {
        assert_eq!(Rights::none().count(), 0);
        assert_eq!(Rights::new(Rights::READ).count(), 1);
        assert_eq!(Rights::new(Rights::READ | Rights::WRITE).count(), 2);
        assert_eq!(Rights::all().count(), 6);
    }

    #[test]
    fn test_rights_bitops() {
        let r1 = Rights::new(Rights::READ);
        let r2 = Rights::new(Rights::WRITE);

        // BitOr
        let union = r1 | r2;
        assert!(union.has(Rights::READ));
        assert!(union.has(Rights::WRITE));

        // BitAnd
        let intersection = union & r1;
        assert!(intersection.has(Rights::READ));
        assert!(!intersection.has(Rights::WRITE));

        // Not
        let inverted = !r1;
        assert!(!inverted.has(Rights::READ));
        assert!(inverted.has(Rights::WRITE));
    }

    #[test]
    fn test_rights_assign_ops() {
        let mut rights = Rights::new(Rights::READ);
        rights |= Rights::new(Rights::WRITE);
        assert!(rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));

        rights &= Rights::new(Rights::READ | Rights::EXECUTE);
        assert!(rights.has(Rights::READ));
        assert!(!rights.has(Rights::WRITE));
    }
}

mod quota_tests {
    use super::*;

    #[test]
    fn test_quota_new() {
        let quota = Quota::new(100, 1000, 60_000_000_000);
        assert_eq!(quota.max_invocations, 100);
        assert_eq!(quota.max_bytes, 1000);
        assert_eq!(quota.max_duration_ns, 60_000_000_000);
        assert_eq!(quota.used_invocations, 0);
        assert_eq!(quota.used_bytes, 0);
        assert_eq!(quota.used_duration_ns, 0);
    }

    #[test]
    fn test_quota_unlimited() {
        let quota = Quota::unlimited();
        assert_eq!(quota.max_invocations, u64::MAX);
        assert_eq!(quota.max_bytes, u64::MAX);
        assert_eq!(quota.max_duration_ns, u64::MAX);
    }

    #[test]
    fn test_quota_exhaustion() {
        let mut quota = Quota::new(2, 100, 1000);
        assert!(!quota.is_exhausted());

        quota.used_invocations = 2;
        assert!(quota.is_exhausted());

        quota.used_invocations = 1;
        quota.used_bytes = 100;
        assert!(quota.is_exhausted());

        quota.used_bytes = 50;
        quota.used_duration_ns = 1000;
        assert!(quota.is_exhausted());
    }

    #[test]
    fn test_quota_remaining() {
        let quota = Quota {
            max_invocations: 100,
            used_invocations: 30,
            max_bytes: 1000,
            used_bytes: 250,
            max_duration_ns: 60_000,
            used_duration_ns: 10_000,
        };

        assert_eq!(quota.remaining_invocations(), 70);
        assert_eq!(quota.remaining_bytes(), 750);
        assert_eq!(quota.remaining_duration_ns(), 50_000);
    }

    #[test]
    fn test_quota_consume_success() {
        let mut quota = Quota::new(10, 1000, 100_000);

        // First consumption
        assert!(quota.consume(100, 10_000).is_ok());
        assert_eq!(quota.used_invocations, 1);
        assert_eq!(quota.used_bytes, 100);
        assert_eq!(quota.used_duration_ns, 10_000);

        // Second consumption
        assert!(quota.consume(200, 20_000).is_ok());
        assert_eq!(quota.used_invocations, 2);
        assert_eq!(quota.used_bytes, 300);
        assert_eq!(quota.used_duration_ns, 30_000);
    }

    #[test]
    fn test_quota_consume_exceeds_invocations() {
        let mut quota = Quota::new(1, 1000, 100_000);
        assert!(quota.consume(100, 10_000).is_ok());

        // Second invocation should fail
        let result = quota.consume(100, 10_000);
        assert!(matches!(result, Err(QuotaExceededError::Invocations)));
    }

    #[test]
    fn test_quota_consume_exceeds_bytes() {
        let mut quota = Quota::new(10, 100, 100_000);

        // This should fail - exceeds byte limit
        let result = quota.consume(150, 10_000);
        assert!(matches!(result, Err(QuotaExceededError::Bytes)));
    }

    #[test]
    fn test_quota_consume_exceeds_duration() {
        let mut quota = Quota::new(10, 1000, 100);

        // This should fail - exceeds duration limit
        let result = quota.consume(50, 150);
        assert!(matches!(result, Err(QuotaExceededError::Duration)));
    }

    #[test]
    fn test_quota_utilization() {
        let quota = Quota {
            max_invocations: 100,
            used_invocations: 50,
            max_bytes: 1000,
            used_bytes: 300, // 30%
            max_duration_ns: 100,
            used_duration_ns: 80, // 80%
        };

        // Should return highest utilization (80%)
        assert_eq!(quota.utilization(), 80);
    }
}

mod budget_vector_tests {
    use super::*;

    #[test]
    fn test_budget_vector_new() {
        let budget = BudgetVector::new(1000, 2000, 3000, 4000, 5000, 6000);
        assert_eq!(budget.cpu_time_ms, 1000);
        assert_eq!(budget.wall_time_ms, 2000);
        assert_eq!(budget.memory_bytes, 3000);
        assert_eq!(budget.disk_write_bytes, 4000);
        assert_eq!(budget.network_bytes, 5000);
        assert_eq!(budget.network_requests, 6000);
    }

    #[test]
    fn test_budget_vector_zero_unlimited() {
        let zero = BudgetVector::zero();
        assert_eq!(zero.cpu_time_ms, 0);
        assert!(!zero.has_remaining());
        assert!(zero.is_exhausted());

        let unlimited = BudgetVector::unlimited();
        assert_eq!(unlimited.cpu_time_ms, u64::MAX);
        assert!(unlimited.has_remaining());
        assert!(!unlimited.is_exhausted());
    }

    #[test]
    fn test_budget_vector_saturating_add() {
        let b1 = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let b2 = BudgetVector::new(10, 20, 30, 40, 50, 60);

        let sum = b1.saturating_add(&b2);
        assert_eq!(sum.cpu_time_ms, 110);
        assert_eq!(sum.wall_time_ms, 220);
        assert_eq!(sum.memory_bytes, 330);

        // Test overflow protection
        let max = BudgetVector::unlimited();
        let overflow = max.saturating_add(&b1);
        assert_eq!(overflow.cpu_time_ms, u64::MAX);
    }

    #[test]
    fn test_budget_vector_saturating_sub() {
        let b1 = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let b2 = BudgetVector::new(10, 20, 30, 40, 50, 60);

        let diff = b1.saturating_sub(&b2);
        assert_eq!(diff.cpu_time_ms, 90);
        assert_eq!(diff.wall_time_ms, 180);
        assert_eq!(diff.memory_bytes, 270);

        // Test underflow protection
        let underflow = b2.saturating_sub(&b1);
        assert_eq!(underflow.cpu_time_ms, 0);
        assert_eq!(underflow.wall_time_ms, 0);
    }

    #[test]
    fn test_budget_vector_can_satisfy() {
        let available = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let small_request = BudgetVector::new(50, 100, 150, 200, 250, 300);
        let large_request = BudgetVector::new(150, 100, 150, 200, 250, 300);

        assert!(available.can_satisfy(&small_request));
        assert!(!available.can_satisfy(&large_request));
    }

    #[test]
    fn test_budget_vector_min_max() {
        let b1 = BudgetVector::new(100, 50, 300, 150, 500, 250);
        let b2 = BudgetVector::new(50, 100, 150, 300, 250, 500);

        let min = b1.min(&b2);
        assert_eq!(min.cpu_time_ms, 50);
        assert_eq!(min.wall_time_ms, 50);
        assert_eq!(min.memory_bytes, 150);

        let max = b1.max(&b2);
        assert_eq!(max.cpu_time_ms, 100);
        assert_eq!(max.wall_time_ms, 100);
        assert_eq!(max.memory_bytes, 300);
    }

    #[test]
    fn test_budget_vector_utilization() {
        let max = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let used = BudgetVector::new(80, 100, 150, 200, 250, 300);

        let util = used.utilization(&max);
        // 80/100 = 0.8 is the highest ratio
        assert!((util - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_budget_vector_add_sub_ops() {
        let b1 = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let b2 = BudgetVector::new(10, 20, 30, 40, 50, 60);

        let sum = b1 + b2;
        assert_eq!(sum.cpu_time_ms, 110);

        let diff = sum - b2;
        assert_eq!(diff.cpu_time_ms, 100);
    }

    #[test]
    fn test_budget_vector_assign_ops() {
        let mut budget = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let delta = BudgetVector::new(10, 20, 30, 40, 50, 60);

        budget += delta;
        assert_eq!(budget.cpu_time_ms, 110);

        budget -= delta;
        assert_eq!(budget.cpu_time_ms, 100);
    }
}

mod budget_tests {
    use super::*;

    #[test]
    fn test_budget_new() {
        let initial = BudgetVector::new(1000, 2000, 3000, 4000, 5000, 6000);
        let budget = Budget::new(initial);

        assert_eq!(budget.initial.cpu_time_ms, 1000);
        assert_eq!(budget.consumed.cpu_time_ms, 0);
    }

    #[test]
    fn test_budget_remaining() {
        let initial = BudgetVector::new(1000, 2000, 3000, 4000, 5000, 6000);
        let mut budget = Budget::new(initial);

        budget.consume(&BudgetVector::new(100, 200, 300, 400, 500, 600));

        let remaining = budget.remaining();
        assert_eq!(remaining.cpu_time_ms, 900);
        assert_eq!(remaining.wall_time_ms, 1800);
    }

    #[test]
    fn test_budget_try_consume_success() {
        let initial = BudgetVector::new(1000, 2000, 3000, 4000, 5000, 6000);
        let mut budget = Budget::new(initial);

        let request = BudgetVector::new(500, 1000, 1500, 2000, 2500, 3000);
        assert!(budget.try_consume(&request).is_ok());

        assert_eq!(budget.consumed.cpu_time_ms, 500);
    }

    #[test]
    fn test_budget_try_consume_failure() {
        let initial = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let mut budget = Budget::new(initial);

        let large_request = BudgetVector::new(500, 1000, 1500, 2000, 2500, 3000);
        let result = budget.try_consume(&large_request);

        assert!(result.is_err());
        // Budget should remain unchanged
        assert_eq!(budget.consumed.cpu_time_ms, 0);
    }

    #[test]
    fn test_budget_utilization() {
        let initial = BudgetVector::new(100, 200, 300, 400, 500, 600);
        let mut budget = Budget::new(initial);

        budget.consume(&BudgetVector::new(80, 100, 150, 200, 250, 300));

        let util = budget.utilization();
        assert!((util - 0.8).abs() < 0.01); // 80/100 = 0.8
    }

    #[test]
    fn test_budget_exhaustion() {
        let initial = BudgetVector::new(100, 0, 0, 0, 0, 0);
        let mut budget = Budget::new(initial);

        assert!(!budget.is_exhausted());

        budget.consume(&BudgetVector::new(100, 0, 0, 0, 0, 0));

        assert!(budget.is_exhausted());
    }
}

mod capability_scope_tests {
    use super::*;

    #[test]
    fn test_scope_global_permits_all() {
        let scope = CapabilityScope::Global;
        assert!(scope.permits("anything"));
        assert!(scope.permits("example.com"));
        assert!(scope.permits("/any/path"));
    }

    #[test]
    fn test_scope_network_permits() {
        let scope = CapabilityScope::Network {
            hosts: vec!["api.example.com".into(), "*.github.com".into()],
            ports: vec![443, 80],
            protocols: vec![Protocol::Https],
        };

        assert!(scope.permits("api.example.com"));
        assert!(scope.permits("api.github.com")); // Matches wildcard
        assert!(!scope.permits("malicious.com"));
    }

    #[test]
    fn test_scope_filesystem_permits() {
        let scope = CapabilityScope::Filesystem {
            paths: vec!["/workspace".into(), "/tmp/**".into()],
            operations: FileOperations::all(),
        };

        assert!(scope.permits("/workspace/file.txt"));
        assert!(scope.permits("/tmp/any/nested/path")); // Matches **
        assert!(!scope.permits("/etc/passwd"));
    }

    #[test]
    fn test_scope_process_permits() {
        let scope = CapabilityScope::Process {
            executables: vec!["npm".into(), "node".into()],
            args_pattern: None,
            env_allowlist: vec![],
        };

        assert!(scope.permits("npm"));
        assert!(scope.permits("node"));
        assert!(!scope.permits("rm"));
    }

    #[test]
    fn test_scope_secrets_permits() {
        let scope = CapabilityScope::Secrets {
            names: vec!["GITHUB_TOKEN".into(), "API_KEY".into()],
        };

        assert!(scope.permits("GITHUB_TOKEN"));
        assert!(scope.permits("API_KEY"));
        assert!(!scope.permits("DATABASE_PASSWORD"));
    }

    #[test]
    fn test_scope_intersect_global() {
        let global = CapabilityScope::Global;
        let specific = CapabilityScope::Network {
            hosts: vec!["api.example.com".into()],
            ports: vec![443],
            protocols: vec![Protocol::Https],
        };

        let result = global.intersect(&specific);
        assert!(matches!(result, Some(CapabilityScope::Network { .. })));

        let result2 = specific.intersect(&global);
        assert!(matches!(result2, Some(CapabilityScope::Network { .. })));
    }
}

mod file_operations_tests {
    use super::*;

    #[test]
    fn test_file_operations_constants() {
        assert_eq!(FileOperations::READ, 1);
        assert_eq!(FileOperations::WRITE, 2);
        assert_eq!(FileOperations::DELETE, 4);
        assert_eq!(FileOperations::CREATE, 8);
    }

    #[test]
    fn test_file_operations_has() {
        let ops = FileOperations::new(FileOperations::READ | FileOperations::WRITE);
        assert!(ops.has(FileOperations::READ));
        assert!(ops.has(FileOperations::WRITE));
        assert!(!ops.has(FileOperations::DELETE));
    }

    #[test]
    fn test_file_operations_presets() {
        let all = FileOperations::all();
        assert!(all.has(FileOperations::READ));
        assert!(all.has(FileOperations::WRITE));
        assert!(all.has(FileOperations::DELETE));
        assert!(all.has(FileOperations::CREATE));

        let read_only = FileOperations::read_only();
        assert!(read_only.has(FileOperations::READ));
        assert!(!read_only.has(FileOperations::WRITE));
    }

    #[test]
    fn test_file_operations_intersect() {
        let ops1 = FileOperations::new(FileOperations::READ | FileOperations::WRITE);
        let ops2 = FileOperations::new(FileOperations::READ | FileOperations::DELETE);

        let intersection = ops1.intersect(ops2);
        assert!(intersection.has(FileOperations::READ));
        assert!(!intersection.has(FileOperations::WRITE));
        assert!(!intersection.has(FileOperations::DELETE));
    }
}

mod capability_tests {
    use super::*;

    fn create_test_capability() -> Capability {
        Capability {
            id: CapabilityId::from_raw(12345),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::new(100, 10000, 60_000_000_000),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        }
    }

    #[test]
    fn test_capability_is_valid() {
        let cap = create_test_capability();
        assert!(cap.is_valid(0));
        assert!(cap.is_valid(1000));
    }

    #[test]
    fn test_capability_expired() {
        let mut cap = create_test_capability();
        cap.expires_at = 1000;

        assert!(!cap.is_expired(999));
        assert!(cap.is_expired(1000));
        assert!(cap.is_expired(1001));
        assert!(!cap.is_valid(1001));
    }

    #[test]
    fn test_capability_revoked() {
        let mut cap = create_test_capability();
        assert!(!cap.is_revoked());
        assert!(cap.is_valid(0));

        cap.revoked = true;
        assert!(cap.is_revoked());
        assert!(!cap.is_valid(0));
    }

    #[test]
    fn test_capability_quota_exhausted() {
        let mut cap = create_test_capability();
        cap.quota.used_invocations = cap.quota.max_invocations;

        assert!(cap.quota.is_exhausted());
        assert!(!cap.is_valid(0));
    }

    #[test]
    fn test_capability_proof_verify() {
        let cap = create_test_capability();
        assert!(cap.proof.verify(&cap));

        // Zero signature should fail verification
        let mut cap2 = cap.clone();
        cap2.proof.signature = [0u8; 64];
        assert!(!cap2.proof.verify(&cap2));
    }
}

mod capability_grant_tests {
    use super::*;

    #[test]
    fn test_capability_grant_builder() {
        let grant = CapabilityGrant::new(CapabilityType::FileRead)
            .with_scope(CapabilityScope::Filesystem {
                paths: vec!["/workspace".into()],
                operations: FileOperations::read_only(),
            })
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(1000, 10_000_000, 3600_000_000_000))
            .with_lease(3600);

        assert_eq!(grant.cap_type, CapabilityType::FileRead);
        assert_eq!(grant.lease_secs, 3600);
        assert!(grant.rights.has(Rights::READ));
        assert!(!grant.rights.has(Rights::WRITE));
    }
}

mod capsule_manifest_tests {
    use super::*;

    #[test]
    fn test_capsule_manifest_builder() {
        let identity = CapsuleIdentity::new(SignatureAlgorithm::Ed25519, [0u8; 32]);
        let budget = BudgetVector::new(300_000, 3_600_000, 2_147_483_648, 1_073_741_824, 104_857_600, 1000);

        let manifest = CapsuleManifest::builder("test-agent")
            .version("1.0.0")
            .identity(identity)
            .budget(budget)
            .evidence_level(EvidenceLevel::Full)
            .build();

        assert!(manifest.is_ok());
        let manifest = manifest.unwrap();
        assert_eq!(manifest.name, "test-agent");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_capsule_manifest_builder_missing_identity() {
        let result = CapsuleManifest::builder("test-agent").build();
        assert!(result.is_err());
    }
}

mod evidence_tests {
    use super::*;

    #[test]
    fn test_evidence_statement_new() {
        let header = EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 1234567890,
            version: "1.0".into(),
            parent_run_id: None,
        };

        let statement = EvidenceStatement::new(header);
        assert_eq!(statement._type, "https://agentvm.io/EvidenceStatement/v1");
        assert_eq!(statement.header.run_id, [1u8; 16]);
    }

    #[test]
    fn test_evidence_statement_hash() {
        let header = EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 1234567890,
            version: "1.0".into(),
            parent_run_id: None,
        };

        let statement = EvidenceStatement::new(header);
        let hash = statement.hash();

        // Hash should be deterministic
        let hash2 = statement.hash();
        assert_eq!(hash, hash2);

        // Hash should not be all zeros
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_evidence_bundle_creation() {
        let header = EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 1234567890,
            version: "1.0".into(),
            parent_run_id: None,
        };

        let statement = EvidenceStatement::new(header);
        let bundle = EvidenceBundle::from_statement(&statement);

        assert_eq!(bundle.payload_type, "application/vnd.agentvm.evidence+json");
        assert!(bundle.signatures.is_empty());
    }

    #[test]
    fn test_evidence_bundle_signatures() {
        let header = EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 1234567890,
            version: "1.0".into(),
            parent_run_id: None,
        };

        let statement = EvidenceStatement::new(header);
        let mut bundle = EvidenceBundle::from_statement(&statement);

        assert!(!bundle.verify_signatures()); // No signatures

        bundle.add_signature("capsule:abc".into(), vec![1, 2, 3, 4]);
        assert!(bundle.verify_signatures()); // Has signatures

        assert_eq!(bundle.signatures.len(), 1);
        assert_eq!(bundle.signatures[0].keyid, "capsule:abc");
    }
}

mod error_tests {
    use super::*;

    #[test]
    fn test_capability_error_display() {
        let err = CapabilityError::Expired;
        assert_eq!(format!("{}", err), "capability expired");

        let err = CapabilityError::ScopeViolation("invalid host".into());
        assert!(format!("{}", err).contains("invalid host"));
    }

    #[test]
    fn test_agentvm_error_from() {
        let cap_err = CapabilityError::NotFound;
        let agent_err: AgentVmError = cap_err.into();
        assert!(matches!(agent_err, AgentVmError::Capability(_)));

        let budget_err = BudgetError::CpuExceeded;
        let agent_err: AgentVmError = budget_err.into();
        assert!(matches!(agent_err, AgentVmError::Budget(_)));
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;

    #[test]
    fn test_capsule_id_serde_roundtrip() {
        let id = CapsuleId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let json = serde_json::to_string(&id).unwrap();
        let recovered: CapsuleId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, recovered);
    }

    #[test]
    fn test_capability_serde_roundtrip() {
        let cap = Capability {
            id: CapabilityId::from_raw(12345),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::new(100, 10000, 60_000_000_000),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        };

        let json = serde_json::to_string(&cap).unwrap();
        let recovered: Capability = serde_json::from_str(&json).unwrap();

        assert_eq!(cap.id.as_raw(), recovered.id.as_raw());
        assert_eq!(cap.cap_type, recovered.cap_type);
    }

    #[test]
    fn test_budget_vector_serde_roundtrip() {
        let budget = BudgetVector::new(1000, 2000, 3000, 4000, 5000, 6000);
        let json = serde_json::to_string(&budget).unwrap();
        let recovered: BudgetVector = serde_json::from_str(&json).unwrap();

        assert_eq!(budget.cpu_time_ms, recovered.cpu_time_ms);
        assert_eq!(budget.wall_time_ms, recovered.wall_time_ms);
        assert_eq!(budget.memory_bytes, recovered.memory_bytes);
    }
}
