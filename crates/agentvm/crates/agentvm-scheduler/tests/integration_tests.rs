//! Integration tests for the fabric scheduler.
//!
//! These tests verify the complete scheduling flow including
//! filter, score, select, and bind phases.

use std::sync::Arc;

use agentvm_scheduler::{
    CapsuleId, FabricScheduler, IsolationLevel, NodeHealth, NodeInfo, NodeRegistry,
    ResourceRequirements, ScheduleError, SchedulerConfig, TaskClass, TaskConstraints, TaskSpec,
    Tier,
};

/// Create a test node registry with nodes across all tiers.
async fn create_test_registry() -> Arc<NodeRegistry> {
    let registry = Arc::new(NodeRegistry::new());

    // Edge tier nodes
    let mut edge1 = NodeInfo::new("edge-stm32-1", Tier::Edge);
    edge1.power_draw_mw = 5;
    edge1.wake_latency_ms = 0;
    registry.add(edge1).await.unwrap();

    let mut edge2 = NodeInfo::new("edge-esp32-1", Tier::Edge);
    edge2.power_draw_mw = 10;
    edge2.wake_latency_ms = 1;
    registry.add(edge2).await.unwrap();

    // Host tier nodes
    let mut host1 = NodeInfo::new("host-pi-1", Tier::Host);
    host1.power_draw_mw = 5000;
    host1.wake_latency_ms = 100;
    host1.available_memory = 4 * 1024 * 1024 * 1024; // 4 GB
    registry.add(host1).await.unwrap();

    let mut host2 = NodeInfo::new("host-x86-1", Tier::Host);
    host2.power_draw_mw = 15000;
    host2.wake_latency_ms = 50;
    host2.available_memory = 16 * 1024 * 1024 * 1024; // 16 GB
    host2.current_load = 50; // Already loaded
    registry.add(host2).await.unwrap();

    // Accelerator tier node
    let mut accel1 = NodeInfo::new("accel-gpu-1", Tier::Accel);
    accel1.power_draw_mw = 150000;
    accel1.wake_latency_ms = 500;
    accel1.available_memory = 24 * 1024 * 1024 * 1024; // 24 GB
    registry.add(accel1).await.unwrap();

    registry
}

#[tokio::test]
async fn test_basic_scheduling() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Schedule a network task
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let placement = scheduler.schedule(task).await.unwrap();

    assert_eq!(placement.tier, Tier::Host);
    assert!(placement.score > 0);
}

#[tokio::test]
async fn test_task_class_routing() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Test routing for each task class
    let test_cases = vec![
        (TaskClass::Reflex, Tier::Edge),
        (TaskClass::Gating, Tier::Edge),
        (TaskClass::Sensor, Tier::Edge),
        (TaskClass::Network, Tier::Host),
        (TaskClass::Cli, Tier::Host),
        (TaskClass::Repository, Tier::Host),
        (TaskClass::HeavyCompute, Tier::Accel),
    ];

    for (class, expected_tier) in test_cases {
        let task = TaskSpec::new(CapsuleId::new(), class);
        let result = scheduler.schedule(task).await;

        match result {
            Ok(placement) => {
                assert_eq!(
                    placement.tier, expected_tier,
                    "Task class {:?} should be placed on {:?}, got {:?}",
                    class, expected_tier, placement.tier
                );
            }
            Err(e) => {
                panic!("Scheduling failed for {:?}: {:?}", class, e);
            }
        }
    }
}

#[tokio::test]
async fn test_tier_affinity_constraint() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Force a task that could run on host to run on accel
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Inference)
        .with_constraints(TaskConstraints::with_tier(Tier::Accel));

    let placement = scheduler.schedule(task).await.unwrap();
    assert_eq!(placement.tier, Tier::Accel);
}

#[tokio::test]
async fn test_power_budget_constraint() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Task with very low power budget should go to edge
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Anomaly)
        .with_constraints(TaskConstraints::with_power_budget(100)); // 100 mW max

    let placement = scheduler.schedule(task).await.unwrap();
    assert_eq!(placement.tier, Tier::Edge);
}

#[tokio::test]
async fn test_memory_constraint() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Task requiring lots of memory should go to the node with most memory
    let mut task = TaskSpec::new(CapsuleId::new(), TaskClass::Inference);
    task.resources.memory_bytes = 20 * 1024 * 1024 * 1024; // 20 GB

    let placement = scheduler.schedule(task).await.unwrap();
    // Should go to accel-gpu-1 which has 24 GB
    assert_eq!(placement.tier, Tier::Accel);
}

#[tokio::test]
async fn test_load_balancing() {
    // Create a registry with two identical host nodes, only differing in load
    let registry = Arc::new(NodeRegistry::new());

    // Less loaded node
    let mut host1 = NodeInfo::new("host-low-load", Tier::Host);
    host1.current_load = 10;
    host1.power_draw_mw = 5000;
    host1.wake_latency_ms = 100;
    host1.available_memory = 4 * 1024 * 1024 * 1024;
    registry.add(host1).await.unwrap();

    // More loaded node with same characteristics
    let mut host2 = NodeInfo::new("host-high-load", Tier::Host);
    host2.current_load = 90;
    host2.power_draw_mw = 5000;
    host2.wake_latency_ms = 100;
    host2.available_memory = 4 * 1024 * 1024 * 1024;
    registry.add(host2).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Schedule multiple tasks - they should prefer less loaded node
    let mut placements = Vec::new();

    for _ in 0..4 {
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let placement = scheduler.schedule(task).await.unwrap();
        placements.push(placement);
    }

    // Count placements per node
    let low_load_count = placements
        .iter()
        .filter(|p| p.reason.contains("host-low-load"))
        .count();
    let high_load_count = placements
        .iter()
        .filter(|p| p.reason.contains("host-high-load"))
        .count();

    // The less-loaded node should get at least as many tasks
    // (load balancing is one of several scoring factors)
    assert!(
        low_load_count >= high_load_count,
        "Load balancing should prefer less loaded nodes: low={}, high={}",
        low_load_count, high_load_count
    );
}

#[tokio::test]
async fn test_no_feasible_nodes() {
    let registry = Arc::new(NodeRegistry::new());

    // Only add edge nodes
    registry
        .add(NodeInfo::new("edge-1", Tier::Edge))
        .await
        .unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Try to schedule a task that requires host tier
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let result = scheduler.schedule(task).await;

    assert!(matches!(result, Err(ScheduleError::NoFeasibleNodes)));
}

#[tokio::test]
async fn test_unhealthy_node_filtering() {
    let registry = Arc::new(NodeRegistry::new());

    let mut healthy_host = NodeInfo::new("host-healthy", Tier::Host);
    healthy_host.health = NodeHealth::Healthy;
    registry.add(healthy_host.clone()).await.unwrap();

    let mut unhealthy_host = NodeInfo::new("host-unhealthy", Tier::Host);
    unhealthy_host.health = NodeHealth::Unhealthy;
    registry.add(unhealthy_host).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Schedule a task - should only go to healthy node
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let placement = scheduler.schedule(task).await.unwrap();

    assert_eq!(placement.node_id, healthy_host.id);
}

#[tokio::test]
async fn test_node_failure_handling() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry.clone());

    // Schedule a task
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let placement = scheduler.schedule(task).await.unwrap();
    let failed_node = placement.node_id;

    // Simulate node failure
    scheduler.handle_node_failure(failed_node).await.unwrap();

    // Node should be marked unhealthy
    let node = registry.get(failed_node).await.unwrap();
    assert_eq!(node.health, NodeHealth::Unhealthy);

    // New tasks should not go to the failed node
    let task2 = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let placement2 = scheduler.schedule(task2).await.unwrap();

    assert_ne!(placement2.node_id, failed_node);
}

#[tokio::test]
async fn test_task_completion() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Schedule a task
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let task_id = task.id;
    scheduler.schedule(task).await.unwrap();

    // Task should be in placement history
    let history = scheduler.placement_history();
    let active = history.get_current(task_id).await;
    assert!(active.is_some());
    assert!(!active.unwrap().completed);

    // Complete the task
    scheduler.complete_task(task_id).await.unwrap();

    // Task should no longer be active
    let active = history.get_current(task_id).await;
    assert!(active.is_none());
}

#[tokio::test]
async fn test_metrics_collection() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Schedule several tasks
    for _ in 0..5 {
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        scheduler.schedule(task).await.unwrap();
    }

    for _ in 0..3 {
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Reflex);
        scheduler.schedule(task).await.unwrap();
    }

    // Check metrics
    let metrics = scheduler.export_metrics().await;

    assert_eq!(*metrics.tasks_by_tier.get(&Tier::Host).unwrap(), 5);
    assert_eq!(*metrics.tasks_by_tier.get(&Tier::Edge).unwrap(), 3);
    assert_eq!(metrics.successful_schedules, 8);
    assert!(metrics.avg_scheduling_latency_ms >= 0.0);
}

#[tokio::test]
async fn test_locality_scoring() {
    let registry = Arc::new(NodeRegistry::new());

    let capsule_id = CapsuleId::new();

    // Node with local workspace
    let mut local_host = NodeInfo::new("host-local", Tier::Host);
    local_host.local_workspaces.push(capsule_id);
    registry.add(local_host.clone()).await.unwrap();

    // Node without local workspace
    let remote_host = NodeInfo::new("host-remote", Tier::Host);
    registry.add(remote_host).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Task should prefer the node with local workspace
    let task = TaskSpec::new(capsule_id, TaskClass::Network);
    let placement = scheduler.schedule(task).await.unwrap();

    assert_eq!(placement.node_id, local_host.id);
}

#[tokio::test]
async fn test_isolation_constraint() {
    let registry = Arc::new(NodeRegistry::new());

    // VM-isolated host
    let mut vm_host = NodeInfo::new("host-vm", Tier::Host);
    vm_host.isolation_level = IsolationLevel::Vm;
    registry.add(vm_host.clone()).await.unwrap();

    // Process-isolated host
    let mut proc_host = NodeInfo::new("host-proc", Tier::Host);
    proc_host.isolation_level = IsolationLevel::Process;
    registry.add(proc_host).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Task requiring VM isolation
    let mut task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    task.constraints.isolation = IsolationLevel::Vm;

    let placement = scheduler.schedule(task).await.unwrap();
    assert_eq!(placement.node_id, vm_host.id);
}

#[tokio::test]
async fn test_concurrent_scheduling() {
    let registry = create_test_registry().await;
    let scheduler = Arc::new(FabricScheduler::new(registry));

    // Schedule many tasks concurrently
    let mut handles = Vec::new();

    for i in 0..20 {
        let scheduler = Arc::clone(&scheduler);
        let handle = tokio::spawn(async move {
            let class = if i % 3 == 0 {
                TaskClass::Reflex
            } else if i % 3 == 1 {
                TaskClass::Network
            } else {
                TaskClass::Inference
            };

            let task = TaskSpec::new(CapsuleId::new(), class);
            scheduler.schedule(task).await
        });
        handles.push(handle);
    }

    // All scheduling operations should succeed
    let mut successes = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            successes += 1;
        }
    }

    // Most should succeed (some inference tasks might fail if accel is full)
    assert!(successes >= 15);
}

#[tokio::test]
async fn test_priority_ordering() {
    let registry = Arc::new(NodeRegistry::new());

    // Add a single host node with low capacity
    let mut host = NodeInfo::new("host-limited", Tier::Host);
    host.capabilities.max_concurrent_tasks = 2;
    registry.add(host).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Schedule low priority task
    let low_priority = TaskSpec::new(CapsuleId::new(), TaskClass::Network).with_priority(100);

    // Schedule high priority task
    let high_priority = TaskSpec::new(CapsuleId::new(), TaskClass::Network).with_priority(900);

    // Both should be scheduled (within capacity)
    let p1 = scheduler.schedule(low_priority).await.unwrap();
    let p2 = scheduler.schedule(high_priority).await.unwrap();

    assert!(p1.score > 0);
    assert!(p2.score > 0);
}

#[tokio::test]
async fn test_custom_scheduler_config() {
    let registry = create_test_registry().await;

    let config = SchedulerConfig {
        enable_preemption: false,
        max_attempts: 5,
        bind_timeout_ms: 10000,
    };

    let scheduler = FabricScheduler::with_config(registry, config);

    // Should still work normally
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
    let placement = scheduler.schedule(task).await.unwrap();
    assert_eq!(placement.tier, Tier::Host);
}

#[tokio::test]
async fn test_task_spec_builder() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    let capsule_id = CapsuleId::new();

    let task = TaskSpec::new(capsule_id, TaskClass::Inference)
        .with_priority(800)
        .with_resources(ResourceRequirements {
            cpu_ms: 5000,
            memory_bytes: 1024 * 1024 * 1024,
            network: true,
            accelerator: None,
            storage: Default::default(),
        })
        .with_constraints(TaskConstraints {
            tier_affinity: Some(Tier::Accel),
            power_budget_mw: Some(200000),
            ..Default::default()
        });

    let placement = scheduler.schedule(task).await.unwrap();
    assert_eq!(placement.tier, Tier::Accel);
}

#[tokio::test]
async fn test_placement_history_tracking() {
    let registry = create_test_registry().await;
    let scheduler = FabricScheduler::new(registry);

    // Schedule and complete several tasks
    for _ in 0..5 {
        let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
        let task_id = task.id;
        scheduler.schedule(task).await.unwrap();
        scheduler.complete_task(task_id).await.unwrap();
    }

    // Check history
    let history = scheduler.placement_history();
    let records = history.get_history(10).await;

    assert_eq!(records.len(), 5);
    for record in &records {
        assert!(record.completed);
        assert!(record.duration().is_some());
    }

    // Check stats
    let stats = history.get_stats().await;
    assert_eq!(stats.total_placements, 5);
    assert_eq!(stats.successful_completions, 5);
    assert_eq!(stats.reschedules, 0);
}

#[tokio::test]
async fn test_anomaly_task_fallback() {
    // Anomaly tasks can run on edge or host
    let registry = Arc::new(NodeRegistry::new());

    // Only add a host node (no edge)
    registry
        .add(NodeInfo::new("host-1", Tier::Host))
        .await
        .unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Anomaly task should fall back to host
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Anomaly);
    let placement = scheduler.schedule(task).await.unwrap();

    assert_eq!(placement.tier, Tier::Host);
}

#[tokio::test]
async fn test_inference_task_fallback() {
    // Inference tasks prefer accel but can run on host
    let registry = Arc::new(NodeRegistry::new());

    // Only add a host node (no accel)
    let mut host = NodeInfo::new("host-1", Tier::Host);
    host.available_memory = 8 * 1024 * 1024 * 1024;
    registry.add(host).await.unwrap();

    let scheduler = FabricScheduler::new(registry);

    // Inference task should fall back to host
    let task = TaskSpec::new(CapsuleId::new(), TaskClass::Inference);
    let placement = scheduler.schedule(task).await.unwrap();

    assert_eq!(placement.tier, Tier::Host);
}
