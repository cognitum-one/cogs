//! Witness Log Tests for ADR-001 Compliance
//!
//! These tests verify that the Witness Log system correctly captures all
//! corrections and violations as specified in ADR-001.

use fxnn::witness::{
    WitnessLog, WitnessRecord, WitnessEventType,
    StateSnapshot, CorrectionDetails, CheckpointManager, Checkpoint,
};

// ============================================================================
// Witness Log Capture Tests
// ============================================================================

/// Test that witness log captures correction events
#[test]
fn test_witness_log_captures_correction() {
    let mut log = WitnessLog::new();

    // Log a correction event using the new() constructor
    let record = WitnessRecord::new(
        100,
        WitnessEventType::OverlapCorrection,
        vec![1, 2],
        "overlap_resolved",
    );

    log.append(record);
    assert_eq!(log.len(), 1);
}

/// Test that witness log captures all ADR-001 event types
#[test]
fn test_witness_log_captures_all_event_types() {
    let mut log = WitnessLog::new();

    // Test all WitnessEventType variants from ADR-001
    let events = vec![
        WitnessEventType::OverlapCorrection,
        WitnessEventType::EnergyDriftCorrection,
        WitnessEventType::ConstraintViolation,
        WitnessEventType::ForceClipping,
        WitnessEventType::ActionRejected,
        WitnessEventType::RollbackTriggered,
    ];

    for (i, event) in events.into_iter().enumerate() {
        let record = WitnessRecord::new(
            i as u64,
            event,
            vec![i as u64],
            format!("event_{}", i),
        );
        log.append(record);
    }

    assert_eq!(log.len(), 6, "All 6 ADR-001 event types should be logged");
}

/// Test WitnessRecord complete fields per ADR-001
#[test]
fn test_witness_record_complete_fields() {
    // ADR-001 specifies these fields:
    // tick, event_type, entity_ids, constraint_fired,
    // before_state, after_state, correction_applied, invariant_improved, delta_magnitude

    let record = WitnessRecord::new(
        100,
        WitnessEventType::OverlapCorrection,
        vec![1, 2],
        "lj_repulsion",
    );

    assert_eq!(record.tick, 100);
    assert!(record.entity_ids.contains(&1));
    assert!(record.entity_ids.contains(&2));
    assert_eq!(record.constraint_fired, "lj_repulsion");
}

/// Test checkpoint manager for state rollback
#[test]
fn test_checkpoint_manager_rollback() {
    let mut manager = CheckpointManager::new(10);

    // Create checkpoints
    let state1 = StateSnapshot::default();
    let state2 = StateSnapshot::default();

    manager.save(Checkpoint::new(100, state1));
    manager.save(Checkpoint::new(200, state2));

    // Check IDs are ordered
    let cp1 = manager.get_by_tick(100);
    let cp2 = manager.get_by_tick(200);

    assert!(cp1.is_some(), "Should find checkpoint at tick 100");
    assert!(cp2.is_some(), "Should find checkpoint at tick 200");
    assert!(cp1.unwrap().tick < cp2.unwrap().tick, "Checkpoint ticks should be monotonically increasing");

    // Rollback to first checkpoint
    let restored = manager.get_by_tick(100);
    assert!(restored.is_some());
}

/// Test witness log capacity limits
#[test]
fn test_witness_log_capacity() {
    let mut log = WitnessLog::with_capacity(5);

    // Add more records than capacity
    for i in 0..10 {
        let record = WitnessRecord::new(
            i,
            WitnessEventType::OverlapCorrection,
            vec![i],
            "test",
        );
        log.append(record);
    }

    // Log should evict oldest entries
    assert!(log.len() <= 5, "Log should respect capacity limit");
}

/// Test witness log query by tick range
#[test]
fn test_witness_log_tick_range_query() {
    let mut log = WitnessLog::new();

    for i in 0..100 {
        let record = WitnessRecord::new(
            i,
            WitnessEventType::OverlapCorrection,
            vec![i],
            "test",
        );
        log.append(record);
    }

    let range_records = log.query_by_tick_range(10, 20);
    assert!(range_records.len() >= 10, "Should return records in tick range 10-20");
}

/// Test witness log entity query
#[test]
fn test_witness_log_entity_query() {
    let mut log = WitnessLog::new();

    // Add records involving different entities
    let record1 = WitnessRecord::new(
        1,
        WitnessEventType::OverlapCorrection,
        vec![1, 2],
        "test",
    );

    let record2 = WitnessRecord::new(
        2,
        WitnessEventType::OverlapCorrection,
        vec![3, 4],
        "test",
    );

    log.append(record1);
    log.append(record2);

    let entity1_records = log.query_by_entity(1);
    assert_eq!(entity1_records.len(), 1, "Should find exactly one record for entity 1");
}

/// Test witness event type severity levels
#[test]
fn test_witness_event_severity() {
    // ADR-001 specifies severity ordering
    assert!(WitnessEventType::OverlapCorrection.severity()
        < WitnessEventType::RollbackTriggered.severity(),
        "Rollback should be more severe than overlap correction");

    assert!(WitnessEventType::ForceClipping.severity()
        < WitnessEventType::RollbackTriggered.severity(),
        "Rollback should be most severe");
}

/// Test witness event type descriptions
#[test]
fn test_witness_event_descriptions() {
    // All event types should have non-empty descriptions
    let events = vec![
        WitnessEventType::OverlapCorrection,
        WitnessEventType::EnergyDriftCorrection,
        WitnessEventType::ConstraintViolation,
        WitnessEventType::ForceClipping,
        WitnessEventType::ActionRejected,
        WitnessEventType::RollbackTriggered,
    ];

    for event in events {
        assert!(!event.description().is_empty(),
            "Event {:?} should have description", event);
    }
}

/// Test state snapshot energy tracking
#[test]
fn test_state_snapshot_energy() {
    let mut snapshot = StateSnapshot::default();
    snapshot.total_energy = -100.0;
    snapshot.momentum = [0.0, 0.0, 0.0];

    assert_eq!(snapshot.total_energy, -100.0);
    assert_eq!(snapshot.momentum, [0.0, 0.0, 0.0]);
}

/// Test correction details magnitude tracking
#[test]
fn test_correction_details() {
    let correction = CorrectionDetails::new("force_separation", "push_apart")
        .with_note("Pushed atoms apart by 0.5 sigma");

    assert_eq!(correction.correction_type, "force_separation");
    assert!(!correction.notes.is_empty());
}

// ============================================================================
// Extended WitnessEventType Coverage Tests
// ============================================================================

/// Test all ADR-001 event types are captured
#[test]
fn test_all_adr001_event_types() {
    // Core physics events
    let physics_events = vec![
        WitnessEventType::OverlapCorrection,
        WitnessEventType::EnergyDriftCorrection,
        WitnessEventType::MomentumDriftCorrection,
        WitnessEventType::ConstraintViolation,
        WitnessEventType::ForceClipping,
        WitnessEventType::NumericalInstability,
    ];

    for event in &physics_events {
        assert!(!event.description().is_empty());
        assert!(event.severity() >= 1);
    }

    // Governance events
    let governance_events = vec![
        WitnessEventType::ActionRejected,
        WitnessEventType::RollbackTriggered,
    ];

    for event in &governance_events {
        assert!(!event.description().is_empty());
        assert!(event.severity() >= 3); // Governance events are more severe
    }

    // Learning safety events
    let learning_events = vec![
        WitnessEventType::GradientClipping,
        WitnessEventType::RewardClipping,
        WitnessEventType::MemoryRateLimitExceeded,
    ];

    for event in &learning_events {
        assert!(!event.description().is_empty());
    }

    // Resource budget events
    let resource_events = vec![
        WitnessEventType::ComputeBudgetExceeded,
        WitnessEventType::BandwidthLimitExceeded,
    ];

    for event in &resource_events {
        assert!(!event.description().is_empty());
    }
}

/// Test critical event detection
#[test]
fn test_critical_event_detection() {
    // Critical events (severity >= 4)
    assert!(WitnessEventType::RollbackTriggered.is_critical());
    assert!(WitnessEventType::NumericalInstability.is_critical());

    // Non-critical events
    assert!(!WitnessEventType::OverlapCorrection.is_critical());
    assert!(!WitnessEventType::ForceClipping.is_critical());
    assert!(!WitnessEventType::EnergyDriftCorrection.is_critical());
}

/// Test witness record builder pattern
#[test]
fn test_witness_record_builder() {
    let before_state = StateSnapshot::default();
    let after_state = StateSnapshot::default();
    let correction = CorrectionDetails::new("position_projection", "lagrange");

    let record = WitnessRecord::new(
        42,
        WitnessEventType::OverlapCorrection,
        vec![1, 2],
        "sigma_min",
    )
    .with_states(before_state, after_state)
    .with_correction(correction)
    .with_invariant("no_overlap")
    .with_delta(0.15);

    assert_eq!(record.tick, 42);
    assert_eq!(record.invariant_improved, "no_overlap");
    assert!((record.delta_magnitude - 0.15).abs() < 1e-10);
}

/// Test witness record entity involvement check
#[test]
fn test_witness_record_involves_entity() {
    let record = WitnessRecord::new(
        1,
        WitnessEventType::OverlapCorrection,
        vec![5, 10, 15],
        "test",
    );

    assert!(record.involves_entity(5));
    assert!(record.involves_entity(10));
    assert!(record.involves_entity(15));
    assert!(!record.involves_entity(1));
    assert!(!record.involves_entity(20));
}

/// Test witness log query by type
#[test]
fn test_witness_log_query_by_type() {
    let mut log = WitnessLog::new();

    // Add various event types
    log.append(WitnessRecord::new(1, WitnessEventType::OverlapCorrection, vec![1], "test"));
    log.append(WitnessRecord::new(2, WitnessEventType::ForceClipping, vec![2], "test"));
    log.append(WitnessRecord::new(3, WitnessEventType::OverlapCorrection, vec![3], "test"));
    log.append(WitnessRecord::new(4, WitnessEventType::EnergyDriftCorrection, vec![], "test"));
    log.append(WitnessRecord::new(5, WitnessEventType::OverlapCorrection, vec![5], "test"));

    let overlaps = log.query_by_type(WitnessEventType::OverlapCorrection);
    assert_eq!(overlaps.len(), 3);

    let force_clips = log.query_by_type(WitnessEventType::ForceClipping);
    assert_eq!(force_clips.len(), 1);

    let energy = log.query_by_type(WitnessEventType::EnergyDriftCorrection);
    assert_eq!(energy.len(), 1);
}

/// Test witness log critical event query
#[test]
fn test_witness_log_query_critical() {
    let mut log = WitnessLog::new();

    log.append(WitnessRecord::new(1, WitnessEventType::OverlapCorrection, vec![1], "test"));
    log.append(WitnessRecord::new(2, WitnessEventType::RollbackTriggered, vec![], "budget_exceeded"));
    log.append(WitnessRecord::new(3, WitnessEventType::ForceClipping, vec![2], "test"));
    log.append(WitnessRecord::new(4, WitnessEventType::NumericalInstability, vec![], "nan_detected"));

    let critical = log.query_critical();
    assert_eq!(critical.len(), 2, "Should have 2 critical events (rollback and NaN)");
}

/// Test witness log JSON export
#[test]
fn test_witness_log_json_export() {
    let mut log = WitnessLog::new();

    log.append(WitnessRecord::new(
        1,
        WitnessEventType::ForceClipping,
        vec![5],
        "F_max",
    ).with_invariant("bounded_force").with_delta(100.0));

    let json = log.export_json().expect("serialization should succeed");
    assert!(json.contains("ForceClipping"));
    assert!(json.contains("F_max"));
    assert!(json.contains("bounded_force"));
}

/// Test witness log statistics
#[test]
fn test_witness_log_statistics() {
    let mut log = WitnessLog::new();

    log.append(WitnessRecord::new(1, WitnessEventType::OverlapCorrection, vec![1], "test"));
    log.append(WitnessRecord::new(2, WitnessEventType::OverlapCorrection, vec![2], "test"));
    log.append(WitnessRecord::new(3, WitnessEventType::ForceClipping, vec![3], "test"));
    log.append(WitnessRecord::new(4, WitnessEventType::RollbackTriggered, vec![], "test"));

    let stats = log.statistics();

    assert_eq!(stats.total_records, 4);
    assert_eq!(stats.overlap_corrections, 2);
    assert_eq!(stats.force_clippings, 1);
    assert_eq!(stats.rollbacks, 1);
    assert_eq!(stats.critical_events, 1); // Only RollbackTriggered is critical
}

// ============================================================================
// Checkpoint Manager Tests
// ============================================================================

/// Test checkpoint manager capacity limits
#[test]
fn test_checkpoint_manager_capacity() {
    let mut manager = CheckpointManager::new(3);

    manager.save(Checkpoint::new(100, StateSnapshot::default()));
    manager.save(Checkpoint::new(200, StateSnapshot::default()));
    manager.save(Checkpoint::new(300, StateSnapshot::default()));

    assert_eq!(manager.len(), 3);
    assert_eq!(manager.evicted_count(), 0);

    // Adding a 4th should evict the oldest
    manager.save(Checkpoint::new(400, StateSnapshot::default()));

    assert_eq!(manager.len(), 3);
    assert_eq!(manager.evicted_count(), 1);
    assert!(manager.get_by_tick(100).is_none(), "Oldest checkpoint should be evicted");
    assert!(manager.get_by_tick(200).is_some());
}

/// Test checkpoint manager latest/oldest accessors
#[test]
fn test_checkpoint_manager_accessors() {
    let mut manager = CheckpointManager::new(10);

    manager.save(Checkpoint::new(100, StateSnapshot::default()));
    manager.save(Checkpoint::new(200, StateSnapshot::default()));
    manager.save(Checkpoint::new(300, StateSnapshot::default()));

    assert_eq!(manager.latest().unwrap().tick, 300);
    assert_eq!(manager.oldest().unwrap().tick, 100);
}

/// Test checkpoint manager get_before_tick
#[test]
fn test_checkpoint_manager_get_before_tick() {
    let mut manager = CheckpointManager::new(10);

    manager.save(Checkpoint::new(100, StateSnapshot::default()));
    manager.save(Checkpoint::new(200, StateSnapshot::default()));
    manager.save(Checkpoint::new(300, StateSnapshot::default()));

    let before_250 = manager.get_before_tick(250);
    assert!(before_250.is_some());
    assert_eq!(before_250.unwrap().tick, 200);

    let before_50 = manager.get_before_tick(50);
    assert!(before_50.is_none(), "No checkpoint before tick 50");
}

/// Test checkpoint pop for rollback
#[test]
fn test_checkpoint_pop_for_rollback() {
    let mut manager = CheckpointManager::new(10);

    manager.save(Checkpoint::new(100, StateSnapshot::default()));
    manager.save(Checkpoint::new(200, StateSnapshot::default()));

    assert_eq!(manager.rollback_count(), 0);

    let popped = manager.pop_for_rollback().unwrap();
    assert_eq!(popped.tick, 200);
    assert_eq!(manager.rollback_count(), 1);
    assert_eq!(manager.len(), 1);
}

/// Test labeled checkpoints
#[test]
fn test_labeled_checkpoint() {
    let checkpoint = Checkpoint::with_label(
        100,
        StateSnapshot::default(),
        "before_learning_update",
    );

    assert_eq!(checkpoint.tick, 100);
    assert_eq!(checkpoint.label, "before_learning_update");
}

// ============================================================================
// StateSnapshot Tests
// ============================================================================

/// Test state snapshot creation and modification
#[test]
fn test_state_snapshot_creation() {
    let mut snapshot = StateSnapshot::with_capacity(2);

    snapshot.add_entity([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    snapshot.add_entity([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);

    assert_eq!(snapshot.len(), 2);
    assert!(!snapshot.is_empty());
}

/// Test state snapshot energy calculations
#[test]
fn test_state_snapshot_energies() {
    let mut snapshot = StateSnapshot::new();
    snapshot.set_energies(0.5, -1.0);

    assert!((snapshot.kinetic_energy - 0.5).abs() < 1e-10);
    assert!((snapshot.potential_energy - (-1.0)).abs() < 1e-10);
    assert!((snapshot.total_energy - (-0.5)).abs() < 1e-10);
}

/// Test state snapshot momentum calculation
#[test]
fn test_state_snapshot_momentum_calculation() {
    let mut snapshot = StateSnapshot::new();
    snapshot.velocities = vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]];

    let masses = vec![1.0, 2.0];
    snapshot.calculate_momentum(&masses);

    // p = m1*v1 + m2*v2 = 1*1 + 2*(-1) = -1
    assert!((snapshot.momentum[0] - (-1.0)).abs() < 1e-10);
}

/// Test state snapshot comparison methods
#[test]
fn test_state_snapshot_comparison() {
    let s1 = StateSnapshot {
        positions: vec![[0.0, 0.0, 0.0]],
        total_energy: -1.0,
        ..Default::default()
    };

    let s2 = StateSnapshot {
        positions: vec![[1.0, 0.0, 0.0]],
        total_energy: -0.9,
        ..Default::default()
    };

    assert!((s1.max_position_delta(&s2) - 1.0).abs() < 1e-10);
    assert!((s1.energy_drift(&s2) - 0.1).abs() < 1e-10);
}

// ============================================================================
// CorrectionDetails Tests
// ============================================================================

/// Test correction details builder pattern
#[test]
fn test_correction_details_builder() {
    let correction = CorrectionDetails::new("position_projection", "SHAKE")
        .with_parameter("tolerance", 1e-6)
        .with_parameter("max_iterations", 100.0)
        .with_convergence(5, true, 1e-8)
        .with_note("Resolved overlap between atoms 1 and 2");

    assert_eq!(correction.correction_type, "position_projection");
    assert_eq!(correction.method, "SHAKE");
    assert_eq!(correction.parameters.len(), 2);
    assert_eq!(correction.iterations, 5);
    assert!(correction.converged);
    assert!(correction.is_successful());
}

/// Test correction success determination
#[test]
fn test_correction_success_determination() {
    // Successful correction
    let success = CorrectionDetails::new("test", "test")
        .with_convergence(3, true, 1e-8);
    assert!(success.is_successful());

    // Failed to converge
    let failed_converge = CorrectionDetails::new("test", "test")
        .with_convergence(100, false, 1e-3);
    assert!(!failed_converge.is_successful());

    // Converged but high residual
    let high_residual = CorrectionDetails::new("test", "test")
        .with_convergence(10, true, 1e-3);
    assert!(!high_residual.is_successful());
}
