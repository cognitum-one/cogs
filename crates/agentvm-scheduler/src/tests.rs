//! Comprehensive tests for agentvm-scheduler crate
//!
//! Test coverage:
//! - Task classification to tier mapping
//! - Filter plugins (each filter type)
//! - Score plugins (each score type)
//! - Scheduling with various constraints
//! - Node failure handling
//! - Preemption logic

use super::*;
use std::sync::Arc;

mod task_class_tests {
    use super::*;
    use crate::task::TaskClass;
    use crate::node::Tier;

    #[test]
    fn test_task_class_preferred_tier() {
        assert_eq!(TaskClass::Reflex.preferred_tier(), Tier::Edge);
        assert_eq!(TaskClass::Gating.preferred_tier(), Tier::Edge);
        assert_eq!(TaskClass::Anomaly.preferred_tier(), Tier::Edge);
        assert_eq!(TaskClass::Sensor.preferred_tier(), Tier::Edge);

        assert_eq!(TaskClass::Network.preferred_tier(), Tier::Host);
        assert_eq!(TaskClass::Cli.preferred_tier(), Tier::Host);
        assert_eq!(TaskClass::Repository.preferred_tier(), Tier::Host);

        assert_eq!(TaskClass::Inference.preferred_tier(), Tier::Accel);
        assert_eq!(TaskClass::HeavyCompute.preferred_tier(), Tier::Accel);
    }

    #[test]
    fn test_task_class_can_run_on_edge() {
        // Edge-only tasks
        assert!(TaskClass::Reflex.can_run_on(Tier::Edge));
        assert!(!TaskClass::Reflex.can_run_on(Tier::Host));
        assert!(!TaskClass::Reflex.can_run_on(Tier::Accel));

        assert!(TaskClass::Gating.can_run_on(Tier::Edge));
        assert!(!TaskClass::Gating.can_run_on(Tier::Host));

        assert!(TaskClass::Sensor.can_run_on(Tier::Edge));
        assert!(!TaskClass::Sensor.can_run_on(Tier::Host));
    }

    #[test]
    fn test_task_class_can_run_on_anomaly() {
        // Anomaly can run on edge or host
        assert!(TaskClass::Anomaly.can_run_on(Tier::Edge));
        assert!(TaskClass::Anomaly.can_run_on(Tier::Host));
        assert!(!TaskClass::Anomaly.can_run_on(Tier::Accel));
    }

    #[test]
    fn test_task_class_can_run_on_host() {
        // Host tasks can run on host or accel
        assert!(!TaskClass::Network.can_run_on(Tier::Edge));
        assert!(TaskClass::Network.can_run_on(Tier::Host));
        assert!(TaskClass::Network.can_run_on(Tier::Accel));

        assert!(TaskClass::Cli.can_run_on(Tier::Host));
        assert!(TaskClass::Cli.can_run_on(Tier::Accel));
    }

    #[test]
    fn test_task_class_can_run_on_accel() {
        // Inference can fall back to host
        assert!(!TaskClass::Inference.can_run_on(Tier::Edge));
        assert!(TaskClass::Inference.can_run_on(Tier::Host));
        assert!(TaskClass::Inference.can_run_on(Tier::Accel));

        // Heavy compute requires accel
        assert!(!TaskClass::HeavyCompute.can_run_on(Tier::Edge));
        assert!(!TaskClass::HeavyCompute.can_run_on(Tier::Host));
        assert!(TaskClass::HeavyCompute.can_run_on(Tier::Accel));
    }
}

mod task_spec_tests {
    use super::*;
    use crate::task::*;
    use agentvm_types::CapabilityType;

    #[test]
    fn test_task_spec_builder() {
        let task = TaskSpec::new(TaskId::new(1), TaskClass::Cli)
            .with_capsule([0xAB; 16])
            .with_resources(ResourceRequirements::new()
                .with_cpu(1000)
                .with_memory(1024 * 1024))
            .with_capability(CapabilityType::NetworkHttp)
            .with_priority(10)
            .with_deadline(60_000_000_000);

        assert_eq!(task.id.0, 1);
        assert_eq!(task.capsule_id, [0xAB; 16]);
        assert_eq!(task.resources.cpu_ms, 1000);
        assert_eq!(task.resources.memory_bytes, 1024 * 1024);
        assert_eq!(task.capabilities.len(), 1);
        assert_eq!(task.priority, 10);
        assert_eq!(task.deadline_ns, Some(60_000_000_000));
    }

    #[test]
    fn test_task_constraints_builder() {
        let constraints = TaskConstraints::new()
            .with_tier_affinity(Tier::Host)
            .with_tier_anti_affinity(Tier::Edge)
            .with_node_affinity(NodeId::new(1))
            .with_node_anti_affinity(NodeId::new(2))
            .with_power_budget(5000)
            .with_isolation(IsolationLevel::Vm);

        assert_eq!(constraints.tier_affinity, Some(Tier::Host));
        assert_eq!(constraints.tier_anti_affinity, vec![Tier::Edge]);
        assert_eq!(constraints.node_affinity, vec![NodeId::new(1)]);
        assert_eq!(constraints.node_anti_affinity, vec![NodeId::new(2)]);
        assert_eq!(constraints.power_budget_mw, Some(5000));
        assert_eq!(constraints.isolation, IsolationLevel::Vm);
    }

    #[test]
    fn test_resource_requirements_satisfied_by() {
        let required = ResourceRequirements::new()
            .with_cpu(1000)
            .with_memory(1024)
            .with_network(true);

        let available = ResourceRequirements {
            cpu_ms: 2000,
            memory_bytes: 2048,
            network: true,
            accelerator: None,
            storage: StorageRequirements::default(),
        };

        assert!(required.satisfied_by(&available));

        let insufficient = ResourceRequirements {
            cpu_ms: 500, // Not enough CPU
            memory_bytes: 2048,
            network: true,
            accelerator: None,
            storage: StorageRequirements::default(),
        };

        assert!(!required.satisfied_by(&insufficient));
    }
}

mod node_tests {
    use super::*;
    use crate::node::*;

    #[test]
    fn test_node_info_creation() {
        let node = NodeInfo::new(NodeId::new(1), Tier::Host, "test-node");

        assert_eq!(node.id.0, 1);
        assert_eq!(node.tier, Tier::Host);
        assert_eq!(node.name, "test-node");
        assert!(node.healthy);
        assert_eq!(node.current_load, 0);
    }

    #[test]
    fn test_node_tier_defaults() {
        let edge_node = NodeInfo::new(NodeId::new(1), Tier::Edge, "edge");
        assert_eq!(edge_node.available_memory, 256 * 1024); // 256 KB

        let host_node = NodeInfo::new(NodeId::new(2), Tier::Host, "host");
        assert_eq!(host_node.available_memory, 4 * 1024 * 1024 * 1024); // 4 GB

        let accel_node = NodeInfo::new(NodeId::new(3), Tier::Accel, "accel");
        assert_eq!(accel_node.available_memory, 16 * 1024 * 1024 * 1024); // 16 GB
    }

    #[test]
    fn test_node_has_local_workspace() {
        let mut node = NodeInfo::new(NodeId::new(1), Tier::Host, "test");
        let workspace_id = [0xAB; 16];

        assert!(!node.has_local_workspace(&workspace_id));

        node.local_workspaces.push(workspace_id);
        assert!(node.has_local_workspace(&workspace_id));

        assert!(!node.has_local_workspace(&[0xCD; 16]));
    }

    #[test]
    fn test_node_memory_utilization() {
        let mut node = NodeInfo::new(NodeId::new(1), Tier::Host, "test");
        node.total_memory = 1000;
        node.available_memory = 750;

        assert_eq!(node.memory_utilization(), 25); // 25% used
    }

    #[test]
    fn test_tier_typical_values() {
        assert_eq!(Tier::Edge.typical_power_mw(), 5);
        assert_eq!(Tier::Host.typical_power_mw(), 5000);
        assert_eq!(Tier::Accel.typical_power_mw(), 100000);

        assert_eq!(Tier::Edge.typical_wake_latency_ms(), 1);
        assert_eq!(Tier::Host.typical_wake_latency_ms(), 100);
        assert_eq!(Tier::Accel.typical_wake_latency_ms(), 500);
    }
}

mod node_registry_tests {
    use super::*;
    use crate::node::*;

    #[test]
    fn test_registry_register() {
        let registry = NodeRegistry::new();
        let node = NodeInfo::new(NodeId::new(0), Tier::Host, "test");

        let id = registry.register(node);
        assert_eq!(id.0, 1); // First ID is 1

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_get() {
        let registry = NodeRegistry::new();
        let node = NodeInfo::new(NodeId::new(0), Tier::Host, "test");
        let id = registry.register(node);

        let retrieved = registry.get(id).expect("should find node");
        assert_eq!(retrieved.name, "test");
    }

    #[test]
    fn test_registry_unregister() {
        let registry = NodeRegistry::new();
        let node = NodeInfo::new(NodeId::new(0), Tier::Host, "test");
        let id = registry.register(node);

        let removed = registry.unregister(id).expect("should remove node");
        assert_eq!(removed.name, "test");

        assert!(registry.get(id).is_none());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_get_all() {
        let registry = NodeRegistry::new();

        registry.register(NodeInfo::new(NodeId::new(0), Tier::Edge, "edge"));
        registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "host"));
        registry.register(NodeInfo::new(NodeId::new(0), Tier::Accel, "accel"));

        let all = registry.get_all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_registry_get_by_tier() {
        let registry = NodeRegistry::new();

        registry.register(NodeInfo::new(NodeId::new(0), Tier::Edge, "edge1"));
        registry.register(NodeInfo::new(NodeId::new(0), Tier::Edge, "edge2"));
        registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "host"));

        let edge_nodes = registry.get_by_tier(Tier::Edge);
        assert_eq!(edge_nodes.len(), 2);

        let host_nodes = registry.get_by_tier(Tier::Host);
        assert_eq!(host_nodes.len(), 1);

        let accel_nodes = registry.get_by_tier(Tier::Accel);
        assert_eq!(accel_nodes.len(), 0);
    }

    #[test]
    fn test_registry_health_management() {
        let registry = NodeRegistry::new();
        let id = registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "test"));

        assert!(registry.get(id).unwrap().healthy);

        registry.mark_unhealthy(id);
        assert!(!registry.get(id).unwrap().healthy);

        registry.mark_healthy(id);
        assert!(registry.get(id).unwrap().healthy);
    }

    #[test]
    fn test_registry_get_healthy() {
        let registry = NodeRegistry::new();

        let id1 = registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "healthy"));
        let id2 = registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "unhealthy"));

        registry.mark_unhealthy(id2);

        let healthy = registry.get_healthy();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].name, "healthy");
    }

    #[test]
    fn test_registry_update_load() {
        let registry = NodeRegistry::new();
        let id = registry.register(NodeInfo::new(NodeId::new(0), Tier::Host, "test"));

        registry.update_load(id, 75);
        assert_eq!(registry.get(id).unwrap().current_load, 75);

        // Should cap at 100
        registry.update_load(id, 150);
        assert_eq!(registry.get(id).unwrap().current_load, 100);
    }
}

mod filter_tests {
    use super::*;
    use crate::filter::*;
    use crate::task::*;
    use crate::node::*;
    use agentvm_types::CapabilityType;

    fn create_test_task(class: TaskClass) -> TaskSpec {
        TaskSpec::new(TaskId::new(1), class)
    }

    fn create_test_node(tier: Tier) -> NodeInfo {
        NodeInfo::new(NodeId::new(1), tier, "test")
    }

    #[test]
    fn test_tier_filter_affinity() {
        let filter = TierFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.tier_affinity = Some(Tier::Host);

        let host_node = create_test_node(Tier::Host);
        let edge_node = create_test_node(Tier::Edge);

        assert!(filter.filter(&task, &host_node).is_accept());
        assert!(filter.filter(&task, &edge_node).is_reject());
    }

    #[test]
    fn test_tier_filter_anti_affinity() {
        let filter = TierFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.tier_anti_affinity.push(Tier::Edge);

        let host_node = create_test_node(Tier::Host);
        let edge_node = create_test_node(Tier::Edge);

        assert!(filter.filter(&task, &host_node).is_accept());
        assert!(filter.filter(&task, &edge_node).is_reject());
    }

    #[test]
    fn test_tier_filter_task_class_compatibility() {
        let filter = TierFilter;

        // Reflex can only run on edge
        let reflex_task = create_test_task(TaskClass::Reflex);
        assert!(filter.filter(&reflex_task, &create_test_node(Tier::Edge)).is_accept());
        assert!(filter.filter(&reflex_task, &create_test_node(Tier::Host)).is_reject());

        // CLI can run on host or accel
        let cli_task = create_test_task(TaskClass::Cli);
        assert!(filter.filter(&cli_task, &create_test_node(Tier::Host)).is_accept());
        assert!(filter.filter(&cli_task, &create_test_node(Tier::Accel)).is_accept());
        assert!(filter.filter(&cli_task, &create_test_node(Tier::Edge)).is_reject());
    }

    #[test]
    fn test_resource_filter_memory() {
        let filter = ResourceFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.resources.memory_bytes = 1024;

        let mut node = create_test_node(Tier::Host);
        node.available_memory = 2048;
        assert!(filter.filter(&task, &node).is_accept());

        node.available_memory = 512;
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_resource_filter_network() {
        let filter = ResourceFilter;

        let mut task = create_test_task(TaskClass::Network);
        task.resources.network = true;

        let mut node = create_test_node(Tier::Host);
        node.capabilities.network = true;
        assert!(filter.filter(&task, &node).is_accept());

        node.capabilities.network = false;
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_resource_filter_accelerator() {
        let filter = ResourceFilter;

        let mut task = create_test_task(TaskClass::Inference);
        task.resources.accelerator = Some(AcceleratorType::Gpu);

        let mut node = create_test_node(Tier::Accel);
        node.capabilities.accelerators.push(AcceleratorType::Gpu);
        assert!(filter.filter(&task, &node).is_accept());

        let mut node_no_gpu = create_test_node(Tier::Accel);
        node_no_gpu.capabilities.accelerators.push(AcceleratorType::Tpu);
        assert!(filter.filter(&task, &node_no_gpu).is_reject());
    }

    #[test]
    fn test_power_filter() {
        let filter = PowerFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.power_budget_mw = Some(10000);

        let mut low_power_node = create_test_node(Tier::Host);
        low_power_node.power_draw_mw = 5000;
        assert!(filter.filter(&task, &low_power_node).is_accept());

        let mut high_power_node = create_test_node(Tier::Accel);
        high_power_node.power_draw_mw = 100000;
        assert!(filter.filter(&task, &high_power_node).is_reject());
    }

    #[test]
    fn test_capability_filter() {
        let filter = CapabilityFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.capabilities.push(CapabilityType::NetworkHttp);
        task.capabilities.push(CapabilityType::FileRead);

        let mut node_with_caps = create_test_node(Tier::Host);
        node_with_caps.capabilities.capability_types.push(CapabilityType::NetworkHttp);
        node_with_caps.capabilities.capability_types.push(CapabilityType::FileRead);
        assert!(filter.filter(&task, &node_with_caps).is_accept());

        let mut node_missing_cap = create_test_node(Tier::Host);
        node_missing_cap.capabilities.capability_types.push(CapabilityType::NetworkHttp);
        // Missing FileRead
        assert!(filter.filter(&task, &node_missing_cap).is_reject());
    }

    #[test]
    fn test_node_affinity_filter() {
        let filter = NodeAffinityFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.node_affinity.push(NodeId::new(1));

        let mut node1 = create_test_node(Tier::Host);
        node1.id = NodeId::new(1);
        assert!(filter.filter(&task, &node1).is_accept());

        let mut node2 = create_test_node(Tier::Host);
        node2.id = NodeId::new(2);
        assert!(filter.filter(&task, &node2).is_reject());
    }

    #[test]
    fn test_node_anti_affinity_filter() {
        let filter = NodeAffinityFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.node_anti_affinity.push(NodeId::new(2));

        let mut node1 = create_test_node(Tier::Host);
        node1.id = NodeId::new(1);
        assert!(filter.filter(&task, &node1).is_accept());

        let mut node2 = create_test_node(Tier::Host);
        node2.id = NodeId::new(2);
        assert!(filter.filter(&task, &node2).is_reject());
    }

    #[test]
    fn test_isolation_filter() {
        let filter = IsolationFilter;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.isolation = IsolationLevel::Vm;

        let mut vm_node = create_test_node(Tier::Host);
        vm_node.capabilities.isolation_level = IsolationLevel::Vm;
        assert!(filter.filter(&task, &vm_node).is_accept());

        let mut hw_node = create_test_node(Tier::Host);
        hw_node.capabilities.isolation_level = IsolationLevel::Hardware;
        assert!(filter.filter(&task, &hw_node).is_accept()); // Hardware > Vm

        let mut proc_node = create_test_node(Tier::Host);
        proc_node.capabilities.isolation_level = IsolationLevel::Process;
        assert!(filter.filter(&task, &proc_node).is_reject()); // Process < Vm
    }

    #[test]
    fn test_health_filter() {
        let filter = HealthFilter;
        let task = create_test_task(TaskClass::Cli);

        let healthy_node = create_test_node(Tier::Host);
        assert!(filter.filter(&task, &healthy_node).is_accept());

        let mut unhealthy_node = create_test_node(Tier::Host);
        unhealthy_node.healthy = false;
        assert!(filter.filter(&task, &unhealthy_node).is_reject());
    }

    #[test]
    fn test_composite_filter() {
        let composite = CompositeFilter::with_defaults();
        let task = create_test_task(TaskClass::Cli);
        let node = create_test_node(Tier::Host);

        // Should pass all default filters
        assert!(composite.filter(&task, &node).is_accept());

        // Unhealthy node should fail
        let mut unhealthy = create_test_node(Tier::Host);
        unhealthy.healthy = false;
        assert!(composite.filter(&task, &unhealthy).is_reject());
    }
}

mod score_tests {
    use super::*;
    use crate::score::*;
    use crate::task::*;
    use crate::node::*;

    fn create_test_task(class: TaskClass) -> TaskSpec {
        TaskSpec::new(TaskId::new(1), class)
    }

    fn create_test_node(tier: Tier) -> NodeInfo {
        NodeInfo::new(NodeId::new(1), tier, "test")
    }

    #[test]
    fn test_power_score() {
        let scorer = PowerScore;
        let task = create_test_task(TaskClass::Cli);

        let mut low_power = create_test_node(Tier::Edge);
        low_power.power_draw_mw = 5;

        let mut high_power = create_test_node(Tier::Accel);
        high_power.power_draw_mw = 100000;

        let low_score = scorer.score(&task, &low_power);
        let high_score = scorer.score(&task, &high_power);

        // Lower power should have higher score
        assert!(low_score.0 > high_score.0);
    }

    #[test]
    fn test_latency_score() {
        let scorer = LatencyScore;

        // Reflex tasks should penalize latency more
        let reflex_task = create_test_task(TaskClass::Reflex);

        let mut fast_node = create_test_node(Tier::Edge);
        fast_node.wake_latency_ms = 1;

        let mut slow_node = create_test_node(Tier::Host);
        slow_node.wake_latency_ms = 100;

        let fast_score = scorer.score(&reflex_task, &fast_node);
        let slow_score = scorer.score(&reflex_task, &slow_node);

        assert!(fast_score.0 > slow_score.0);
    }

    #[test]
    fn test_load_balance_score() {
        let scorer = LoadBalanceScore;
        let task = create_test_task(TaskClass::Cli);

        let mut idle_node = create_test_node(Tier::Host);
        idle_node.current_load = 0;

        let mut loaded_node = create_test_node(Tier::Host);
        loaded_node.current_load = 80;

        let idle_score = scorer.score(&task, &idle_node);
        let loaded_score = scorer.score(&task, &loaded_node);

        // Idle node should have higher score
        assert!(idle_score.0 > loaded_score.0);
        assert_eq!(idle_score.0, 100);
        assert_eq!(loaded_score.0, 20);
    }

    #[test]
    fn test_locality_score() {
        let scorer = LocalityScore;

        let mut task = create_test_task(TaskClass::Cli);
        task.capsule_id = [0xAB; 16];

        let mut local_node = create_test_node(Tier::Host);
        local_node.local_workspaces.push([0xAB; 16]);

        let remote_node = create_test_node(Tier::Host);

        let local_score = scorer.score(&task, &local_node);
        let remote_score = scorer.score(&task, &remote_node);

        assert_eq!(local_score.0, 100);
        assert_eq!(remote_score.0, 0);
    }

    #[test]
    fn test_risk_score() {
        let scorer = RiskScore;

        let mut task = create_test_task(TaskClass::Cli);
        task.constraints.isolation = IsolationLevel::Hardware;

        let mut hw_node = create_test_node(Tier::Host);
        hw_node.capabilities.isolation_level = IsolationLevel::Hardware;

        let mut vm_node = create_test_node(Tier::Host);
        vm_node.capabilities.isolation_level = IsolationLevel::Vm;

        let hw_score = scorer.score(&task, &hw_node);
        let vm_score = scorer.score(&task, &vm_node);

        assert_eq!(hw_score.0, 100); // Perfect match
        assert_eq!(vm_score.0, 0);   // Insufficient isolation
    }

    #[test]
    fn test_tier_preference_score() {
        let scorer = TierPreferenceScore;
        let task = create_test_task(TaskClass::Cli); // Prefers Host

        let host_node = create_test_node(Tier::Host);
        let accel_node = create_test_node(Tier::Accel); // Can run CLI but not preferred
        let edge_node = create_test_node(Tier::Edge);   // Cannot run CLI

        let host_score = scorer.score(&task, &host_node);
        let accel_score = scorer.score(&task, &accel_node);
        let edge_score = scorer.score(&task, &edge_node);

        assert_eq!(host_score.0, 100); // Preferred tier
        assert_eq!(accel_score.0, 50); // Can run but not preferred
        assert_eq!(edge_score.0, 0);   // Cannot run
    }

    #[test]
    fn test_composite_scorer() {
        let scorer = CompositeScorer::with_defaults();
        let task = create_test_task(TaskClass::Cli);

        let mut good_node = create_test_node(Tier::Host);
        good_node.power_draw_mw = 5000;
        good_node.wake_latency_ms = 10;
        good_node.current_load = 10;
        good_node.capabilities.isolation_level = IsolationLevel::Vm;

        let mut bad_node = create_test_node(Tier::Accel);
        bad_node.power_draw_mw = 100000;
        bad_node.wake_latency_ms = 500;
        bad_node.current_load = 90;

        let good_score = scorer.score(&task, &good_node);
        let bad_score = scorer.score(&task, &bad_node);

        // Good node should score higher
        assert!(good_score.0 > bad_score.0);
    }

    #[test]
    fn test_score_arithmetic() {
        let s1 = Score(50);
        let s2 = Score(30);

        let sum = s1 + s2;
        assert_eq!(sum.0, 80);

        // Test saturation
        let max = Score(i64::MAX - 10);
        let overflow = max + Score(20);
        assert_eq!(overflow.0, i64::MAX);
    }
}

mod scheduler_tests {
    use super::*;
    use crate::filter::*;
    use crate::score::*;

    fn create_test_registry() -> Arc<NodeRegistry> {
        let registry = NodeRegistry::new();

        // Add some test nodes
        let mut edge = NodeInfo::new(NodeId::new(0), Tier::Edge, "edge-1");
        edge.capabilities.network = false;
        registry.register(edge);

        let mut host1 = NodeInfo::new(NodeId::new(0), Tier::Host, "host-1");
        host1.capabilities.network = true;
        host1.capabilities.capability_types.push(agentvm_types::CapabilityType::NetworkHttp);
        host1.capabilities.isolation_level = IsolationLevel::Vm;
        host1.current_load = 50;
        registry.register(host1);

        let mut host2 = NodeInfo::new(NodeId::new(0), Tier::Host, "host-2");
        host2.capabilities.network = true;
        host2.capabilities.capability_types.push(agentvm_types::CapabilityType::NetworkHttp);
        host2.capabilities.isolation_level = IsolationLevel::Vm;
        host2.current_load = 10;
        registry.register(host2);

        Arc::new(registry)
    }

    #[test]
    fn test_scheduler_basic() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry);

        // Add default filters and scorers
        scheduler.add_filter(Box::new(HealthFilter));
        scheduler.add_filter(Box::new(TierFilter));
        scheduler.add_filter(Box::new(ResourceFilter));

        scheduler.add_scorer(Box::new(LoadBalanceScore));

        let task = TaskSpec::new(TaskId::new(1), TaskClass::Cli)
            .with_resources(ResourceRequirements::new().with_network(true));

        let placement = scheduler.schedule(task).expect("should schedule");

        assert_eq!(placement.task_id.0, 1);
        assert_eq!(placement.tier, Tier::Host);
    }

    #[test]
    fn test_scheduler_prefers_less_loaded_node() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry);

        scheduler.add_filter(Box::new(HealthFilter));
        scheduler.add_filter(Box::new(TierFilter));
        scheduler.add_scorer(Box::new(LoadBalanceScore));

        let task = TaskSpec::new(TaskId::new(1), TaskClass::Cli);

        let placement = scheduler.schedule(task).expect("should schedule");

        // Should pick host-2 which has load=10 vs host-1 with load=50
        assert_eq!(placement.tier, Tier::Host);
    }

    #[test]
    fn test_scheduler_no_feasible_nodes() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry);

        scheduler.add_filter(Box::new(TierFilter));

        // Heavy compute requires Accel tier, which we don't have
        let task = TaskSpec::new(TaskId::new(1), TaskClass::HeavyCompute);

        let result = scheduler.schedule(task);
        assert!(matches!(result, Err(ScheduleError::NoFeasibleNodes)));
    }

    #[test]
    fn test_scheduler_get_placement() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry);

        scheduler.add_filter(Box::new(TierFilter));

        let task_id = TaskId::new(1);
        let task = TaskSpec::new(task_id, TaskClass::Cli);

        scheduler.schedule(task).expect("should schedule");

        let placement = scheduler.get_placement(task_id).expect("should have placement");
        assert_eq!(placement.task_id, task_id);
    }

    #[test]
    fn test_scheduler_remove_placement() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry);

        scheduler.add_filter(Box::new(TierFilter));

        let task_id = TaskId::new(1);
        let task = TaskSpec::new(task_id, TaskClass::Cli);

        scheduler.schedule(task).expect("should schedule");

        let removed = scheduler.remove_placement(task_id).expect("should remove");
        assert_eq!(removed.task_id, task_id);

        assert!(scheduler.get_placement(task_id).is_none());
    }

    #[test]
    fn test_scheduler_get_tasks_on_node() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry.clone());

        scheduler.add_filter(Box::new(TierFilter));
        scheduler.add_filter(Box::new(HealthFilter));

        // Schedule multiple tasks
        for i in 1..=3 {
            let task = TaskSpec::new(TaskId::new(i), TaskClass::Cli);
            scheduler.schedule(task).expect("should schedule");
        }

        // Get all host nodes
        let hosts = registry.get_by_tier(Tier::Host);
        let total_tasks: usize = hosts
            .iter()
            .map(|h| scheduler.get_tasks_on_node(h.id).len())
            .sum();

        assert_eq!(total_tasks, 3);
    }

    #[test]
    fn test_scheduler_node_failure() {
        let registry = create_test_registry();
        let mut scheduler = FabricScheduler::new(registry.clone());

        scheduler.add_filter(Box::new(TierFilter));
        scheduler.add_filter(Box::new(HealthFilter));

        // Schedule a task
        let task = TaskSpec::new(TaskId::new(1), TaskClass::Cli);
        let placement = scheduler.schedule(task).expect("should schedule");

        // Fail the node
        let results = scheduler.handle_node_failure(placement.node_id);

        // Task should have been affected
        assert_eq!(results.len(), 1);

        // Node should be marked unhealthy
        let node = registry.get(placement.node_id).expect("should find node");
        assert!(!node.healthy);
    }
}

mod integration_tests {
    use super::*;
    use crate::filter::CompositeFilter;
    use crate::score::CompositeScorer;

    #[test]
    fn test_full_scheduling_flow() {
        let registry = Arc::new(NodeRegistry::new());

        // Register diverse nodes
        let mut edge1 = NodeInfo::new(NodeId::new(0), Tier::Edge, "stm32-1");
        edge1.power_draw_mw = 5;
        edge1.wake_latency_ms = 1;
        registry.register(edge1);

        let mut host1 = NodeInfo::new(NodeId::new(0), Tier::Host, "pi5-1");
        host1.power_draw_mw = 5000;
        host1.wake_latency_ms = 100;
        host1.capabilities.network = true;
        host1.capabilities.isolation_level = IsolationLevel::Vm;
        host1.current_load = 20;
        registry.register(host1);

        let mut host2 = NodeInfo::new(NodeId::new(0), Tier::Host, "pi5-2");
        host2.power_draw_mw = 5000;
        host2.wake_latency_ms = 100;
        host2.capabilities.network = true;
        host2.capabilities.isolation_level = IsolationLevel::Vm;
        host2.current_load = 80;
        registry.register(host2);

        let mut accel = NodeInfo::new(NodeId::new(0), Tier::Accel, "gpu-1");
        accel.power_draw_mw = 100000;
        accel.wake_latency_ms = 500;
        accel.capabilities.network = true;
        accel.capabilities.accelerators.push(AcceleratorType::Gpu);
        accel.capabilities.isolation_level = IsolationLevel::Hardware;
        registry.register(accel);

        let mut scheduler = FabricScheduler::new(registry);

        // Add all default filters
        let composite_filter = CompositeFilter::with_defaults();
        scheduler.add_filter(Box::new(composite_filter));

        // Add all default scorers
        scheduler.add_scorer(Box::new(PowerScore));
        scheduler.add_scorer(Box::new(LatencyScore));
        scheduler.add_scorer(Box::new(LoadBalanceScore));
        scheduler.add_scorer(Box::new(TierPreferenceScore));

        // Test scheduling different task types
        // 1. Reflex task should go to edge
        let reflex = TaskSpec::new(TaskId::new(1), TaskClass::Reflex);
        let reflex_placement = scheduler.schedule(reflex).expect("should schedule reflex");
        assert_eq!(reflex_placement.tier, Tier::Edge);

        // 2. CLI task should go to host (prefer less loaded)
        let cli = TaskSpec::new(TaskId::new(2), TaskClass::Cli)
            .with_resources(ResourceRequirements::new().with_network(true));
        let cli_placement = scheduler.schedule(cli).expect("should schedule cli");
        assert_eq!(cli_placement.tier, Tier::Host);

        // 3. Inference task should go to accel or host
        let inference = TaskSpec::new(TaskId::new(3), TaskClass::Inference)
            .with_resources(ResourceRequirements::new().with_accelerator(AcceleratorType::Gpu));
        let inference_placement = scheduler.schedule(inference).expect("should schedule inference");
        assert!(inference_placement.tier == Tier::Accel || inference_placement.tier == Tier::Host);
    }
}
