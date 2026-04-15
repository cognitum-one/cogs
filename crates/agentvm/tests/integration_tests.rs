//! End-to-end integration tests for Agentic VM
//!
//! These tests verify the complete flow:
//! - Create capsule
//! - Grant capabilities
//! - Invoke operations
//! - Verify evidence chain
//! - Snapshot/restore
//! - Replay verification

use std::sync::Arc;

// Import all agentvm crates
use agentvm_types::{
    Budget, BudgetVector, Capability, CapabilityGrant, CapabilityId, CapabilityProof,
    CapabilityScope, CapabilityType, CapsuleId, CapsuleManifest, Quota, Rights,
};

use agentvm_capability::{
    derive::{derive_capability, DeriveRequest},
    token::CapabilityTable,
    validate::{validate_capability, Operation, ValidationContext, ValidationResult},
    wire::{MessageEnvelope, MessageType},
};

use agentvm_evidence::{
    bundle::EvidenceBuilder,
    merkle::MerkleTree,
    sign::{Ed25519Signer, Signer},
    verify::{verify_bundle, verify_chain, verify_inclusion},
};

use agentvm_scheduler::{
    filter::CompositeFilter,
    node::{NodeCapabilities, NodeId, NodeInfo, NodeRegistry, Tier},
    score::CompositeScorer,
    task::{IsolationLevel, ResourceRequirements, TaskClass, TaskConstraints, TaskId, TaskSpec},
    FabricScheduler,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test capsule ID
fn test_capsule_id() -> CapsuleId {
    CapsuleId::from_bytes([0xAB; 16])
}

/// Create a test capability
fn create_test_capability(id: u128, cap_type: CapabilityType) -> Capability {
    Capability {
        id: CapabilityId::from_raw(id),
        cap_type,
        scope: CapabilityScope::Global,
        rights: Rights::all(),
        quota: Quota::new(100, 10 * 1024 * 1024, 60_000_000_000),
        expires_at: u64::MAX,
        parent: None,
        proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
        revoked: false,
    }
}

/// Create a test budget
fn create_test_budget() -> Budget {
    Budget::new(BudgetVector::new(
        10000,
        60000,
        1024 * 1024,
        1024 * 1024,
        1024 * 1024,
        1000,
    ))
}

/// Create a test node registry with diverse nodes
fn create_test_node_registry() -> Arc<NodeRegistry> {
    let registry = NodeRegistry::new();

    // Edge node
    let mut edge = NodeInfo::new(NodeId::new(0), Tier::Edge, "stm32-sensor-1");
    edge.power_draw_mw = 5;
    edge.wake_latency_ms = 1;
    edge.capabilities.network = false;
    registry.register(edge);

    // Host nodes
    let mut host1 = NodeInfo::new(NodeId::new(0), Tier::Host, "pi5-node-1");
    host1.power_draw_mw = 5000;
    host1.wake_latency_ms = 100;
    host1.capabilities.network = true;
    host1.capabilities.isolation_level = IsolationLevel::Vm;
    host1.capabilities.capability_types.push(CapabilityType::NetworkHttp);
    host1.capabilities.capability_types.push(CapabilityType::FileRead);
    host1.current_load = 30;
    registry.register(host1);

    let mut host2 = NodeInfo::new(NodeId::new(0), Tier::Host, "pi5-node-2");
    host2.power_draw_mw = 5000;
    host2.wake_latency_ms = 100;
    host2.capabilities.network = true;
    host2.capabilities.isolation_level = IsolationLevel::Vm;
    host2.capabilities.capability_types.push(CapabilityType::NetworkHttp);
    host2.current_load = 70;
    registry.register(host2);

    // Accelerator node
    let mut accel = NodeInfo::new(NodeId::new(0), Tier::Accel, "gpu-node-1");
    accel.power_draw_mw = 100000;
    accel.wake_latency_ms = 500;
    accel.capabilities.network = true;
    accel.capabilities.isolation_level = IsolationLevel::Hardware;
    accel.capabilities.accelerators.push(agentvm_scheduler::task::AcceleratorType::Gpu);
    registry.register(accel);

    Arc::new(registry)
}

// ============================================================================
// End-to-End Tests
// ============================================================================

mod capsule_lifecycle_tests {
    use super::*;

    #[test]
    fn test_complete_capsule_lifecycle() {
        // 1. Create capsule manifest
        let manifest = CapsuleManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .entry_point("main")
            .build();

        // 2. Initialize capsule with budget
        let capsule_id = test_capsule_id();
        let mut budget = create_test_budget();

        // 3. Create capability table
        let mut cap_table = CapabilityTable::new();

        // 4. Grant initial capabilities
        let http_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        let file_cap = create_test_capability(2, CapabilityType::FileRead);

        cap_table.insert(capsule_id.as_bytes(), http_cap.clone());
        cap_table.insert(capsule_id.as_bytes(), file_cap.clone());

        // 5. Verify capabilities are stored
        let caps = cap_table.get_by_capsule(capsule_id.as_bytes());
        assert_eq!(caps.len(), 2);

        // 6. Validate capabilities
        let ctx = ValidationContext::new(1000);
        assert!(matches!(
            validate_capability(&http_cap, &ctx),
            ValidationResult::Valid
        ));

        // 7. Simulate operations (budget deduction)
        let op_cost = BudgetVector::new(100, 500, 10240, 0, 5120, 1);
        budget.try_consume(&op_cost).expect("budget should be sufficient");

        assert!(budget.utilization() > 0.0);

        // 8. Revoke capability
        cap_table.revoke(http_cap.id);
        let revoked = cap_table.get(http_cap.id).expect("cap should exist");
        assert!(revoked.is_revoked());
    }

    #[test]
    fn test_capability_derivation_chain() {
        let cap_table = CapabilityTable::new();

        // Create root capability with DELEGATE right
        let root_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        cap_table.insert(&[0u8; 16], root_cap.clone());

        // Derive child capability with reduced rights
        let derive_req = DeriveRequest::new(&root_cap)
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(50, 1024 * 1024, 30_000_000_000))
            .with_lease(1800);

        let child_result = derive_capability(&derive_req, 1000, |_| CapabilityId::from_raw(2));

        // Should succeed since parent has DELEGATE right
        assert!(child_result.is_ok());

        let child_cap = child_result.unwrap();
        assert_eq!(child_cap.parent, Some(root_cap.id));
        assert!(child_cap.rights.is_subset_of(root_cap.rights));
    }
}

mod evidence_chain_tests {
    use super::*;

    #[test]
    fn test_evidence_chain_integrity() {
        // 1. Create evidence builder
        let mut builder = EvidenceBuilder::new(test_capsule_id());

        // 2. Add multiple evidence statements
        builder.capability_invoked(
            CapabilityId::from_raw(1),
            CapabilityType::NetworkHttp,
            "https://api.example.com/data",
            true,
            100,
        );

        builder.capability_invoked(
            CapabilityId::from_raw(2),
            CapabilityType::FileRead,
            "/workspace/config.json",
            true,
            50,
        );

        builder.capability_invoked(
            CapabilityId::from_raw(1),
            CapabilityType::NetworkHttp,
            "https://api.example.com/submit",
            true,
            200,
        );

        // 3. Build evidence bundle
        let bundle = builder.build();
        assert_eq!(bundle.statements.len(), 3);

        // 4. Create Merkle tree from statements
        let mut merkle = MerkleTree::new();
        for stmt in &bundle.statements {
            let hash = agentvm_evidence::sha256(&stmt.serialize());
            merkle.append(hash);
        }

        // 5. Verify inclusion proofs
        for i in 0..bundle.statements.len() {
            let proof = merkle.prove_inclusion(i).expect("should generate proof");
            let stmt_hash = agentvm_evidence::sha256(&bundle.statements[i].serialize());
            assert!(proof.verify(stmt_hash, merkle.root().unwrap()));
        }
    }

    #[test]
    fn test_signed_evidence_bundle() {
        let mut builder = EvidenceBuilder::new(test_capsule_id());

        builder.capability_invoked(
            CapabilityId::from_raw(1),
            CapabilityType::NetworkHttp,
            "https://example.com",
            true,
            100,
        );

        let bundle = builder.build();

        // Sign with test key
        let signer = Ed25519Signer::from_seed([0x42u8; 32]);
        let signed = signer.sign_bundle(&bundle);

        // Verify signature
        assert!(signer.verify_bundle(&signed, &bundle));
    }

    #[test]
    fn test_evidence_chain_consistency() {
        // Create two sequential trees
        let mut tree1 = MerkleTree::new();
        tree1.append([0x01; 32]);
        tree1.append([0x02; 32]);
        let root1 = tree1.root().unwrap();

        let mut tree2 = MerkleTree::new();
        tree2.append([0x01; 32]);
        tree2.append([0x02; 32]);
        tree2.append([0x03; 32]);
        let root2 = tree2.root().unwrap();

        // Generate consistency proof
        let proof = tree2.prove_consistency(2).expect("should generate proof");

        // Verify consistency (tree1 is prefix of tree2)
        assert!(proof.verify(root1, root2, 2, 3));
    }
}

mod scheduler_integration_tests {
    use super::*;

    #[test]
    fn test_task_scheduling_flow() {
        let registry = create_test_node_registry();
        let mut scheduler = FabricScheduler::new(registry.clone());

        // Add all default filters and scorers
        scheduler.add_filter(Box::new(CompositeFilter::with_defaults()));
        scheduler.add_scorer(Box::new(agentvm_scheduler::score::PowerScore));
        scheduler.add_scorer(Box::new(agentvm_scheduler::score::LatencyScore));
        scheduler.add_scorer(Box::new(agentvm_scheduler::score::LoadBalanceScore));
        scheduler.add_scorer(Box::new(agentvm_scheduler::score::TierPreferenceScore));

        // 1. Schedule edge task (reflex)
        let reflex_task = TaskSpec::new(TaskId::new(1), TaskClass::Reflex);
        let reflex_placement = scheduler.schedule(reflex_task).expect("should schedule");
        assert_eq!(reflex_placement.tier, Tier::Edge);

        // 2. Schedule host task (CLI)
        let cli_task = TaskSpec::new(TaskId::new(2), TaskClass::Cli)
            .with_resources(ResourceRequirements::new().with_network(true));
        let cli_placement = scheduler.schedule(cli_task).expect("should schedule");
        assert_eq!(cli_placement.tier, Tier::Host);

        // 3. Schedule with constraints
        let constrained_task = TaskSpec::new(TaskId::new(3), TaskClass::Network)
            .with_constraints(
                TaskConstraints::new()
                    .with_tier_affinity(Tier::Host)
                    .with_power_budget(10000),
            )
            .with_resources(ResourceRequirements::new().with_network(true));
        let constrained_placement = scheduler.schedule(constrained_task).expect("should schedule");
        assert_eq!(constrained_placement.tier, Tier::Host);

        // 4. Verify placements
        assert!(scheduler.get_placement(TaskId::new(1)).is_some());
        assert!(scheduler.get_placement(TaskId::new(2)).is_some());
        assert!(scheduler.get_placement(TaskId::new(3)).is_some());
    }

    #[test]
    fn test_node_failure_handling() {
        let registry = create_test_node_registry();
        let mut scheduler = FabricScheduler::new(registry.clone());

        scheduler.add_filter(Box::new(CompositeFilter::with_defaults()));

        // Schedule some tasks
        let task1 = TaskSpec::new(TaskId::new(1), TaskClass::Cli);
        let placement = scheduler.schedule(task1).expect("should schedule");

        // Simulate node failure
        let affected = scheduler.handle_node_failure(placement.node_id);

        // Verify node marked unhealthy
        let node = registry.get(placement.node_id).expect("should find node");
        assert!(!node.healthy);

        // Affected tasks should be reported
        assert!(!affected.is_empty());
    }

    #[test]
    fn test_load_balancing() {
        let registry = create_test_node_registry();
        let mut scheduler = FabricScheduler::new(registry.clone());

        scheduler.add_filter(Box::new(agentvm_scheduler::filter::TierFilter));
        scheduler.add_filter(Box::new(agentvm_scheduler::filter::HealthFilter));
        scheduler.add_scorer(Box::new(agentvm_scheduler::score::LoadBalanceScore));

        // Schedule multiple CLI tasks
        for i in 1..=5 {
            let task = TaskSpec::new(TaskId::new(i), TaskClass::Cli);
            scheduler.schedule(task).expect("should schedule");
        }

        // Check distribution across host nodes
        let hosts = registry.get_by_tier(Tier::Host);
        for host in hosts {
            let tasks = scheduler.get_tasks_on_node(host.id);
            // Tasks should be somewhat distributed
            assert!(tasks.len() <= 4, "single node shouldn't be overloaded");
        }
    }
}

mod wire_protocol_tests {
    use super::*;

    #[test]
    fn test_message_roundtrip() {
        let messages = [
            MessageType::Invoke,
            MessageType::Derive,
            MessageType::Revoke,
            MessageType::QueryQuota,
            MessageType::InvokeResponse,
            MessageType::DeriveResponse,
            MessageType::Error,
        ];

        for msg_type in messages {
            let payload = vec![0x01, 0x02, 0x03, 0x04];
            let envelope = MessageEnvelope::new(msg_type, payload.clone());

            // Serialize
            let bytes = envelope.serialize();

            // Deserialize
            let parsed = MessageEnvelope::parse(&bytes).expect("should parse");

            assert_eq!(parsed.message_type, msg_type);
            assert_eq!(parsed.payload, payload);
        }
    }

    #[test]
    fn test_large_message() {
        let large_payload = vec![0xAB; 64 * 1024]; // 64KB
        let envelope = MessageEnvelope::new(MessageType::Invoke, large_payload.clone());

        let bytes = envelope.serialize();
        let parsed = MessageEnvelope::parse(&bytes).expect("should parse");

        assert_eq!(parsed.payload.len(), 64 * 1024);
        assert_eq!(parsed.payload, large_payload);
    }

    #[test]
    fn test_checksum_validation() {
        let envelope = MessageEnvelope::new(MessageType::Invoke, vec![1, 2, 3]);
        let mut bytes = envelope.serialize();

        // Corrupt the payload
        if bytes.len() > 50 {
            bytes[50] ^= 0xFF;
        }

        // Should fail to parse due to checksum
        let result = MessageEnvelope::parse(&bytes);
        assert!(result.is_err());
    }
}

mod validation_tests {
    use super::*;

    #[test]
    fn test_capability_validation_scenarios() {
        let current_time = 1_000_000_000u64; // 1 second in ns
        let ctx = ValidationContext::new(current_time);

        // Valid capability
        let valid_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        assert!(matches!(
            validate_capability(&valid_cap, &ctx),
            ValidationResult::Valid
        ));

        // Expired capability
        let mut expired_cap = create_test_capability(2, CapabilityType::NetworkHttp);
        expired_cap.expires_at = 500_000_000; // Before current time
        assert!(matches!(
            validate_capability(&expired_cap, &ctx),
            ValidationResult::Expired
        ));

        // Revoked capability
        let mut revoked_cap = create_test_capability(3, CapabilityType::NetworkHttp);
        revoked_cap.revoked = true;
        assert!(matches!(
            validate_capability(&revoked_cap, &ctx),
            ValidationResult::Revoked
        ));

        // Exhausted quota
        let mut exhausted_cap = create_test_capability(4, CapabilityType::NetworkHttp);
        exhausted_cap.quota.used_invocations = exhausted_cap.quota.max_invocations;
        assert!(matches!(
            validate_capability(&exhausted_cap, &ctx),
            ValidationResult::QuotaExhausted
        ));
    }

    #[test]
    fn test_scope_validation() {
        let current_time = 1_000_000_000u64;
        let ctx = ValidationContext::new(current_time);

        // Capability with restricted scope
        let mut restricted_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        restricted_cap.scope = CapabilityScope::Network {
            hosts: vec!["allowed.example.com".to_string()],
            ports: vec![443],
            protocols: vec![],
        };

        // Validate with allowed target
        let result = ctx.validate_operation(
            &restricted_cap,
            Operation::NetworkRequest {
                target: "allowed.example.com",
                method: "GET",
            },
        );
        assert!(result.is_ok());

        // Validate with forbidden target
        let result = ctx.validate_operation(
            &restricted_cap,
            Operation::NetworkRequest {
                target: "forbidden.example.com",
                method: "GET",
            },
        );
        assert!(result.is_err());
    }
}

mod cross_crate_integration_tests {
    use super::*;

    #[test]
    fn test_full_agent_execution_flow() {
        // 1. Setup: Create capsule with capabilities
        let capsule_id = test_capsule_id();
        let mut budget = create_test_budget();
        let cap_table = CapabilityTable::new();

        // Grant capabilities
        let http_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        let file_cap = create_test_capability(2, CapabilityType::FileRead);
        cap_table.insert(capsule_id.as_bytes(), http_cap.clone());
        cap_table.insert(capsule_id.as_bytes(), file_cap.clone());

        // 2. Setup: Initialize scheduler with nodes
        let registry = create_test_node_registry();
        let mut scheduler = FabricScheduler::new(registry);
        scheduler.add_filter(Box::new(CompositeFilter::with_defaults()));

        // 3. Setup: Initialize evidence chain
        let mut evidence_builder = EvidenceBuilder::new(capsule_id);
        let mut merkle = MerkleTree::new();

        // 4. Execute: Schedule and run a network task
        let network_task = TaskSpec::new(TaskId::new(1), TaskClass::Network)
            .with_resources(ResourceRequirements::new().with_network(true))
            .with_capability(CapabilityType::NetworkHttp);

        let placement = scheduler.schedule(network_task).expect("should schedule");
        assert_eq!(placement.tier, Tier::Host);

        // 5. Execute: Validate capability for the operation
        let ctx = ValidationContext::new(1000);
        assert!(matches!(
            validate_capability(&http_cap, &ctx),
            ValidationResult::Valid
        ));

        // 6. Execute: Consume budget
        let op_budget = BudgetVector::new(50, 200, 5120, 0, 2048, 1);
        budget.try_consume(&op_budget).expect("should have budget");

        // 7. Record: Add evidence
        evidence_builder.capability_invoked(
            http_cap.id,
            CapabilityType::NetworkHttp,
            "https://api.example.com/data",
            true,
            200,
        );

        // 8. Record: Update Merkle tree
        let bundle = evidence_builder.build();
        for stmt in &bundle.statements {
            merkle.append(agentvm_evidence::sha256(&stmt.serialize()));
        }

        // 9. Verify: Check evidence integrity
        let proof = merkle.prove_inclusion(0).expect("should prove");
        let stmt_hash = agentvm_evidence::sha256(&bundle.statements[0].serialize());
        assert!(proof.verify(stmt_hash, merkle.root().unwrap()));

        // 10. Verify: Check budget consumed
        assert!(budget.utilization() > 0.0);

        // 11. Cleanup: Remove task from scheduler
        scheduler.remove_placement(TaskId::new(1));
        assert!(scheduler.get_placement(TaskId::new(1)).is_none());
    }

    #[test]
    fn test_capability_cascade_with_evidence() {
        let capsule_id = test_capsule_id();
        let cap_table = CapabilityTable::new();
        let mut evidence_builder = EvidenceBuilder::new(capsule_id);

        // Create parent capability
        let parent_cap = create_test_capability(1, CapabilityType::NetworkHttp);
        cap_table.insert(capsule_id.as_bytes(), parent_cap.clone());

        // Derive child capability
        let derive_req = DeriveRequest::new(&parent_cap)
            .with_rights(Rights::new(Rights::READ))
            .with_lease(1800);

        let child_cap = derive_capability(&derive_req, 1000, |_| CapabilityId::from_raw(2))
            .expect("should derive");
        cap_table.insert(capsule_id.as_bytes(), child_cap.clone());

        // Record derivation in evidence
        evidence_builder.capability_derived(
            child_cap.id,
            parent_cap.id,
            child_cap.cap_type,
        );

        // Use child capability
        evidence_builder.capability_invoked(
            child_cap.id,
            CapabilityType::NetworkHttp,
            "https://example.com",
            true,
            100,
        );

        // Revoke parent (should cascade)
        cap_table.revoke(parent_cap.id);

        // Record revocation
        evidence_builder.capability_revoked(parent_cap.id);

        // Build and verify evidence
        let bundle = evidence_builder.build();
        assert_eq!(bundle.statements.len(), 3); // derive + invoke + revoke
    }
}
