//! Unit tests for vector-graph fusion module

use cognitum::ruvector::fusion::*;
use cognitum::ruvector::types::*;

#[test]
fn test_relation_type_strengths_ordered() {
    // Verify strengths are properly ordered
    assert!(RelationType::DirectNeighbor.strength() > RelationType::DiagonalNeighbor.strength());
    assert!(RelationType::DiagonalNeighbor.strength() > RelationType::SameQuadrant.strength());
    assert!(RelationType::SameQuadrant.strength() > RelationType::CrossQuadrant.strength());
    assert!(RelationType::CrossQuadrant.strength() > RelationType::Remote.strength());
}

#[test]
fn test_relation_from_positions_grid_4x4() {
    // Test various positions in 4x4 grid
    // Layout:
    // 0  1  2  3
    // 4  5  6  7
    // 8  9  10 11
    // 12 13 14 15

    // Direct neighbors
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(4)),
        RelationType::DirectNeighbor
    );
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(6)),
        RelationType::DirectNeighbor
    );
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(1)),
        RelationType::DirectNeighbor
    );
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(9)),
        RelationType::DirectNeighbor
    );

    // Diagonal neighbors
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(0)),
        RelationType::DiagonalNeighbor
    );
    assert_eq!(
        RelationType::from_positions(TileId(5), TileId(2)),
        RelationType::DiagonalNeighbor
    );

    // Cross quadrant (0 is top-left quadrant, 3 is top-right quadrant)
    assert_eq!(
        RelationType::from_positions(TileId(0), TileId(3)),
        RelationType::CrossQuadrant
    );
}

#[test]
fn test_fused_edge_weight_default() {
    let fusion = FusedEdgeWeight::default_weights();
    assert_eq!(fusion.vector_weight, 0.6);
    assert_eq!(fusion.graph_weight, 0.4);
}

#[test]
fn test_fused_edge_weight_custom() {
    let fusion = FusedEdgeWeight::new(0.7, 0.3);
    assert_eq!(fusion.vector_weight, 0.7);
    assert_eq!(fusion.graph_weight, 0.3);
}

#[test]
#[should_panic(expected = "Weights must sum to 1.0")]
fn test_fused_edge_weight_invalid_sum() {
    FusedEdgeWeight::new(0.5, 0.3); // Sum is 0.8, should panic
}

#[test]
fn test_fused_edge_weight_compute_high_similarity() {
    let fusion = FusedEdgeWeight::default_weights();

    // High similarity, high graph strength, strong relation
    let capacity = fusion.compute(0.95, 0.90, RelationType::DirectNeighbor);

    // Expected: 0.6 * 0.95 + 0.4 * 0.90 * 1.0 = 0.57 + 0.36 = 0.93
    assert!((capacity - 0.93).abs() < 0.01);
}

#[test]
fn test_fused_edge_weight_compute_weak_relation() {
    let fusion = FusedEdgeWeight::default_weights();

    // High similarity but weak relation
    let capacity = fusion.compute(0.90, 0.90, RelationType::Remote);

    // Expected: 0.6 * 0.90 + 0.4 * 0.90 * 0.1 = 0.54 + 0.036 = 0.576
    assert!((capacity - 0.576).abs() < 0.01);
}

#[test]
fn test_fused_edge_weight_compute_clamped() {
    let fusion = FusedEdgeWeight::default_weights();

    // Values that might exceed 1.0
    let capacity = fusion.compute(1.0, 1.0, RelationType::DirectNeighbor);
    assert!(capacity <= 1.0);

    // Negative values should clamp to 0
    let capacity = fusion.compute(-0.5, -0.5, RelationType::DirectNeighbor);
    assert!(capacity >= 0.0);
}

#[test]
fn test_fused_edge_weight_batch() {
    let fusion = FusedEdgeWeight::default_weights();

    let edges = vec![
        (0.9, 0.9, RelationType::DirectNeighbor),
        (0.5, 0.5, RelationType::DiagonalNeighbor),
        (0.3, 0.3, RelationType::Remote),
    ];

    let capacities = fusion.compute_batch(&edges);

    assert_eq!(capacities.len(), 3);
    // Higher similarity and stronger relations should give higher capacity
    assert!(capacities[0] > capacities[1]);
    assert!(capacities[1] > capacities[2]);
}

#[test]
fn test_brittleness_monitor_states() {
    let mut monitor = BrittlenessMonitor::default_thresholds();

    // Test all states
    assert_eq!(monitor.assess(0.8), HealthSignal::Healthy);
    assert_eq!(monitor.assess(0.4), HealthSignal::Warning);
    assert_eq!(monitor.assess(0.2), HealthSignal::Warning);
    assert_eq!(monitor.assess(0.05), HealthSignal::Critical);
    assert_eq!(monitor.assess(0.0), HealthSignal::Disconnected);
}

#[test]
fn test_brittleness_monitor_history() {
    let mut monitor = BrittlenessMonitor::default_thresholds();

    // Add some history
    for i in 0..10 {
        monitor.assess(0.5 - (i as f64) * 0.01);
    }

    // History should be maintained
    assert_eq!(monitor.min_cut_history.len(), 10);
}

#[test]
fn test_brittleness_monitor_trend_improving() {
    let mut monitor = BrittlenessMonitor::default_thresholds();

    monitor.assess(0.3);
    monitor.assess(0.5);
    monitor.assess(0.7);

    assert_eq!(monitor.trend(), 1); // Improving
}

#[test]
fn test_brittleness_monitor_trend_declining() {
    let mut monitor = BrittlenessMonitor::default_thresholds();

    monitor.assess(0.7);
    monitor.assess(0.5);
    monitor.assess(0.3);

    assert_eq!(monitor.trend(), -1); // Declining
}

#[test]
fn test_brittleness_monitor_trend_stable() {
    let mut monitor = BrittlenessMonitor::default_thresholds();

    monitor.assess(0.5);
    monitor.assess(0.51);
    monitor.assess(0.50);

    assert_eq!(monitor.trend(), 0); // Stable
}

#[test]
fn test_fusion_graph_empty() {
    let graph = FusionGraph::new();
    assert!(graph.tiles.is_empty());
    assert!(graph.edges.is_empty());
    assert!(graph.is_connected()); // Empty graph is connected
}

#[test]
fn test_tile_graph_add_tile() {
    let mut graph = FusionGraph::new();
    graph.add_tile(TileId(0));
    graph.add_tile(TileId(1));

    assert_eq!(graph.tiles.len(), 2);
}

#[test]
fn test_tile_graph_add_edge() {
    let mut graph = FusionGraph::new();
    graph.add_edge(TileId(0), TileId(1), 0.8);

    assert_eq!(graph.tiles.len(), 2);
    assert_eq!(*graph.edges.get(&(TileId(0), TileId(1))).unwrap(), 0.8);
    assert_eq!(*graph.edges.get(&(TileId(1), TileId(0))).unwrap(), 0.8); // Symmetric
}

#[test]
fn test_tile_graph_connected() {
    let mut graph = FusionGraph::new();

    // Create connected path: 0 - 1 - 2 - 3
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_edge(TileId(1), TileId(2), 1.0);
    graph.add_edge(TileId(2), TileId(3), 1.0);

    assert!(graph.is_connected());
}

#[test]
fn test_tile_graph_disconnected() {
    let mut graph = FusionGraph::new();

    // Create two components: (0-1) and (2-3)
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_edge(TileId(2), TileId(3), 1.0);

    assert!(!graph.is_connected());
}

#[test]
fn test_tile_graph_find_bridges_linear() {
    let mut graph = FusionGraph::new();

    // Linear graph: 0 - 1 - 2
    // Both edges are bridges
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_edge(TileId(1), TileId(2), 1.0);

    let bridges = graph.find_bridges();
    assert_eq!(bridges.len(), 2);
}

#[test]
fn test_tile_graph_find_bridges_cycle() {
    let mut graph = FusionGraph::new();

    // Cycle: 0 - 1 - 2 - 0
    // No bridges in a cycle
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_edge(TileId(1), TileId(2), 1.0);
    graph.add_edge(TileId(2), TileId(0), 1.0);

    let bridges = graph.find_bridges();
    assert_eq!(bridges.len(), 0);
}

#[test]
fn test_tile_graph_min_cut_star() {
    let mut graph = FusionGraph::new();

    // Star topology: center (0) connected to 1, 2, 3
    graph.add_edge(TileId(0), TileId(1), 0.5);
    graph.add_edge(TileId(0), TileId(2), 0.5);
    graph.add_edge(TileId(0), TileId(3), 0.5);

    let min_cut = graph.min_cut_approximation();

    // Min-cut should be around 1.5 (sum of edges from center)
    assert!((min_cut - 1.5).abs() < 0.1);
}

#[test]
fn test_tile_graph_min_cut_path() {
    let mut graph = FusionGraph::new();

    // Path: 0 - 1 - 2
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_edge(TileId(1), TileId(2), 1.0);

    let min_cut = graph.min_cut_approximation();

    // Min-cut should be 1.0 (either edge)
    assert!((min_cut - 1.0).abs() < 0.1);
}

#[test]
fn test_graph_optimizer_healthy_no_action() {
    let mut optimizer = GraphOptimizer::new(0.1);
    let graph = FusionGraph::new();

    optimizer.analyze_and_optimize(&graph, 0.8);

    // Healthy graph should have no actions
    assert!(optimizer.actions.is_empty());
}

#[test]
fn test_graph_optimizer_warning_strengthens() {
    let mut optimizer = GraphOptimizer::new(0.1);
    let mut graph = FusionGraph::new();

    graph.add_edge(TileId(0), TileId(1), 0.2); // Weak edge

    optimizer.analyze_and_optimize(&graph, 0.4);

    // Should have strengthening actions
    assert!(!optimizer.actions.is_empty());
}

#[test]
fn test_graph_optimizer_critical_reindex() {
    let mut optimizer = GraphOptimizer::new(0.1);
    let graph = FusionGraph::new();

    optimizer.analyze_and_optimize(&graph, 0.05);

    // Should have reindex action
    let has_reindex = optimizer
        .actions
        .iter()
        .any(|a| matches!(a, OptimizerAction::Reindex { .. }));

    assert!(has_reindex);
}

#[test]
fn test_graph_optimizer_apply_strengthen() {
    let optimizer = GraphOptimizer::new(0.1);
    let mut graph = FusionGraph::new();

    graph.add_edge(TileId(0), TileId(1), 0.5);

    // Manually add strengthen action
    let mut optimizer = optimizer;
    optimizer.actions.push(OptimizerAction::StrengthenEdge {
        u: TileId(0),
        v: TileId(1),
        delta: 0.2,
    });

    let applied = optimizer.apply_actions(&mut graph);
    assert_eq!(applied, 1);

    // Check edge was strengthened
    let new_weight = graph.edges.get(&(TileId(0), TileId(1))).unwrap();
    assert!((new_weight - 0.7).abs() < 0.01);
}

#[test]
fn test_graph_optimizer_apply_weaken() {
    let optimizer = GraphOptimizer::new(0.1);
    let mut graph = FusionGraph::new();

    graph.add_edge(TileId(0), TileId(1), 0.8);

    let mut optimizer = optimizer;
    optimizer.actions.push(OptimizerAction::WeakenEdge {
        u: TileId(0),
        v: TileId(1),
        delta: 0.3,
    });

    let applied = optimizer.apply_actions(&mut graph);
    assert_eq!(applied, 1);

    // Check edge was weakened
    let new_weight = graph.edges.get(&(TileId(0), TileId(1))).unwrap();
    assert!((new_weight - 0.5).abs() < 0.01);
}

#[test]
fn test_brittleness_monitor_islanding_risk_disconnected() {
    let monitor = BrittlenessMonitor::default_thresholds();
    let mut graph = FusionGraph::new();

    // Disconnected graph
    graph.add_edge(TileId(0), TileId(1), 1.0);
    graph.add_tile(TileId(5)); // Isolated tile

    assert!(monitor.detect_islanding_risk(&graph));
}

#[test]
fn test_brittleness_monitor_islanding_risk_trend() {
    let mut monitor = BrittlenessMonitor::default_thresholds();
    let graph = FusionGraph::new();

    // Create declining trend below warning
    monitor.assess(0.25);
    monitor.assess(0.20);
    monitor.assess(0.15);
    monitor.assess(0.10);
    monitor.assess(0.05);

    assert!(monitor.detect_islanding_risk(&graph));
}

#[test]
fn test_partition_id_equality() {
    let p1 = PartitionId(1);
    let p2 = PartitionId(1);
    let p3 = PartitionId(2);

    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

#[test]
fn test_optimizer_action_equality() {
    let a1 = OptimizerAction::Reindex { new_threshold: 0.7 };
    let a2 = OptimizerAction::Reindex { new_threshold: 0.7 };
    let a3 = OptimizerAction::Reindex { new_threshold: 0.5 };

    assert_eq!(a1, a2);
    assert_ne!(a1, a3);
}

#[test]
fn test_health_signal_ordering() {
    use std::mem::discriminant;

    // Different variants should have different discriminants
    assert_ne!(
        discriminant(&HealthSignal::Healthy),
        discriminant(&HealthSignal::Warning)
    );
    assert_ne!(
        discriminant(&HealthSignal::Warning),
        discriminant(&HealthSignal::Critical)
    );
}
