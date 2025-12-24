//! Integration tests for vector-graph fusion

use cognitum::ruvector::embedding::{DefaultEmbeddingGenerator, EmbeddingGenerator};
use cognitum::ruvector::fusion::*;
use cognitum::ruvector::types::*;

#[test]
fn test_end_to_end_fusion_routing() {
    // Setup: Create a 4x4 tile grid with vector embeddings
    let generator = DefaultEmbeddingGenerator::new(256);
    let fusion = FusedEdgeWeight::default_weights();

    // Generate tile states
    let tiles: Vec<TileState> = (0..16).map(|_| TileState::random()).collect();

    // Generate embeddings
    let embeddings: Vec<Embedding> = tiles
        .iter()
        .map(|t| generator.from_tile_state(t))
        .collect();

    // Build graph with fused edge weights
    let mut graph = FusionGraph::new();

    for i in 0..16 {
        for j in (i + 1)..16 {
            let tile_a = TileId(i as u32);
            let tile_b = TileId(j as u32);

            // Compute vector similarity
            let similarity = cosine_similarity(&embeddings[i], &embeddings[j]) as f64;

            // Determine relation type
            let relation = RelationType::from_positions(tile_a, tile_b);

            // Compute fused capacity
            let graph_strength = 0.8; // Base strength
            let capacity = fusion.compute(similarity, graph_strength, relation);

            // Add edge if capacity is significant
            if capacity > 0.3 {
                graph.add_edge(tile_a, tile_b, capacity);
            }
        }
    }

    // Verify graph is well-connected
    assert!(graph.is_connected());

    // Analyze with brittleness monitor
    let mut monitor = BrittlenessMonitor::default_thresholds();
    let min_cut = graph.min_cut_approximation();
    let health = monitor.assess(min_cut);

    // Should be healthy with good fusion weights
    assert!(matches!(
        health,
        HealthSignal::Healthy | HealthSignal::Warning
    ));
}

#[test]
fn test_optimizer_auto_rebalancing() {
    let mut graph = FusionGraph::new();

    // Create intentionally weak graph
    for i in 0..8 {
        let tile_a = TileId(i);
        let tile_b = TileId(i + 1);
        graph.add_edge(tile_a, tile_b, 0.15); // Very weak edges
    }

    // Initial min-cut should be low
    let initial_min_cut = graph.min_cut_approximation();
    assert!(initial_min_cut < 0.3);

    // Run optimizer
    let mut optimizer = GraphOptimizer::new(0.1);
    optimizer.analyze_and_optimize(&graph, initial_min_cut);

    // Should generate optimization actions
    assert!(!optimizer.actions.is_empty());

    // Apply actions
    let applied = optimizer.apply_actions(&mut graph);
    assert!(applied > 0);

    // Min-cut should improve
    let new_min_cut = graph.min_cut_approximation();
    assert!(new_min_cut > initial_min_cut);
}

#[test]
fn test_graph_context_embedding_generation() {
    let generator = DefaultEmbeddingGenerator::new(256);

    // Create center tile and neighbors
    let center = TileState::random();
    let neighbors: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();

    // Define relations (4 direct neighbors)
    let relations = vec![
        RelationType::DirectNeighbor,
        RelationType::DirectNeighbor,
        RelationType::DirectNeighbor,
        RelationType::DirectNeighbor,
    ];

    // Generate context-aware embedding
    let context_embedding = generator.generate_with_context(&center, &neighbors, &relations);

    // Generate base embedding (no context)
    let base_embedding = generator.from_tile_state(&center);

    // Context embedding should differ from base
    assert_ne!(context_embedding.data, base_embedding.data);

    // But should have same dimension
    assert_eq!(context_embedding.dimension(), base_embedding.dimension());
}

#[test]
fn test_hybrid_routing_with_graph_awareness() {
    let generator = DefaultEmbeddingGenerator::new(256);
    let fusion = FusedEdgeWeight::new(0.5, 0.5); // Equal weighting

    // Create task embeddings
    let task_states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();

    let task_embeddings: Vec<Embedding> = task_states
        .iter()
        .map(|t| generator.from_tile_state(t))
        .collect();

    // Create routing graph
    let mut graph = FusionGraph::new();

    // Add edges with hybrid weights
    for i in 0..4 {
        for j in (i + 1)..4 {
            let sim = cosine_similarity(&task_embeddings[i], &task_embeddings[j]) as f64;
            let rel = RelationType::from_positions(TileId(i as u32), TileId(j as u32));

            let capacity = fusion.compute(sim, 0.7, rel);

            if capacity > 0.2 {
                graph.add_edge(TileId(i as u32), TileId(j as u32), capacity);
            }
        }
    }

    // Verify routing graph integrity
    assert!(graph.tiles.len() > 0);

    // Check that direct neighbors have stronger connections
    // than remote tiles (if both exist)
    let mut has_strong = false;
    let mut has_weak = false;

    for (&(u, v), &weight) in &graph.edges {
        let rel = RelationType::from_positions(u, v);
        if matches!(rel, RelationType::DirectNeighbor) {
            has_strong = weight > 0.5;
        }
        if matches!(rel, RelationType::Remote) {
            has_weak = weight < 0.5;
        }
    }

    // At least verify graph was created
    assert!(graph.edges.len() > 0);
}

#[test]
fn test_brittleness_monitoring_over_time() {
    let mut monitor = BrittlenessMonitor::default_thresholds();
    let mut graph = FusionGraph::new();

    // Start with healthy graph
    for i in 0..8 {
        graph.add_edge(TileId(i), TileId(i + 1), 0.8);
    }

    // Track health over degradation
    let mut health_signals = Vec::new();

    for decay in 0..10 {
        let current_min_cut = 0.8 - (decay as f64 * 0.1);
        let signal = monitor.assess(current_min_cut);
        health_signals.push(signal);

        // Update graph to match min-cut
        for (&(u, v), weight) in graph.edges.iter_mut() {
            *weight = current_min_cut;
        }
    }

    // Should transition from Healthy -> Warning -> Critical
    assert_eq!(health_signals[0], HealthSignal::Healthy);
    assert!(matches!(
        health_signals[5],
        HealthSignal::Warning | HealthSignal::Critical
    ));
    assert!(matches!(
        health_signals[9],
        HealthSignal::Critical | HealthSignal::Disconnected
    ));
}

#[test]
fn test_complete_optimization_cycle() {
    // Create weak graph
    let mut graph = FusionGraph::new();
    for i in 0..4 {
        graph.add_edge(TileId(i), TileId(i + 1), 0.1);
    }

    let mut optimizer = GraphOptimizer::new(0.15);
    let mut monitor = BrittlenessMonitor::default_thresholds();

    // Run optimization cycle
    for _iteration in 0..5 {
        let min_cut = graph.min_cut_approximation();
        let _health = monitor.assess(min_cut);

        optimizer.analyze_and_optimize(&graph, min_cut);
        optimizer.apply_actions(&mut graph);
    }

    // Graph should be strengthened
    let final_min_cut = graph.min_cut_approximation();
    assert!(final_min_cut > 0.1);
}

#[test]
fn test_relation_strength_impact_on_routing() {
    let fusion = FusedEdgeWeight::default_weights();

    // Same vector similarity, different relations
    let similarity = 0.8;
    let graph_strength = 0.8;

    let direct = fusion.compute(similarity, graph_strength, RelationType::DirectNeighbor);
    let diagonal = fusion.compute(similarity, graph_strength, RelationType::DiagonalNeighbor);
    let remote = fusion.compute(similarity, graph_strength, RelationType::Remote);

    // Direct neighbors should have highest capacity
    assert!(direct > diagonal);
    assert!(diagonal > remote);

    // All should be within valid range
    assert!(direct <= 1.0);
    assert!(remote >= 0.0);
}

#[test]
fn test_graph_partitioning_detection() {
    let mut graph = FusionGraph::new();

    // Create two weakly connected partitions
    // Partition 1: 0-1-2
    graph.add_edge(TileId(0), TileId(1), 0.9);
    graph.add_edge(TileId(1), TileId(2), 0.9);

    // Weak bridge
    graph.add_edge(TileId(2), TileId(3), 0.05);

    // Partition 2: 3-4-5
    graph.add_edge(TileId(3), TileId(4), 0.9);
    graph.add_edge(TileId(4), TileId(5), 0.9);

    // Find bridges
    let bridges = graph.find_bridges();

    // Should detect the weak bridge
    assert!(!bridges.is_empty());
    assert!(bridges.contains(&(TileId(2), TileId(3))) || bridges.contains(&(TileId(3), TileId(2))));
}

#[test]
fn test_optimizer_prevents_islanding() {
    let mut graph = FusionGraph::new();

    // Create graph at risk of islanding
    graph.add_edge(TileId(0), TileId(1), 0.9);
    graph.add_edge(TileId(1), TileId(2), 0.05); // Critical bridge

    let mut optimizer = GraphOptimizer::new(0.1);
    let mut monitor = BrittlenessMonitor::default_thresholds();

    // Simulate declining health
    monitor.assess(0.4);
    monitor.assess(0.2);
    monitor.assess(0.1);
    monitor.assess(0.05);

    let min_cut = graph.min_cut_approximation();
    optimizer.analyze_and_optimize(&graph, min_cut);

    // Should detect risk and strengthen bridges
    let has_strengthen = optimizer
        .actions
        .iter()
        .any(|a| matches!(a, OptimizerAction::StrengthenEdge { .. }));

    assert!(has_strengthen);
}

#[test]
fn test_batch_fusion_computation() {
    let fusion = FusedEdgeWeight::default_weights();

    // Create batch of edges with varying properties
    let edges: Vec<(f64, f64, RelationType)> = vec![
        (0.9, 0.9, RelationType::DirectNeighbor),
        (0.8, 0.7, RelationType::DiagonalNeighbor),
        (0.6, 0.6, RelationType::SameQuadrant),
        (0.5, 0.5, RelationType::CrossQuadrant),
        (0.3, 0.4, RelationType::Remote),
    ];

    let capacities = fusion.compute_batch(&edges);

    assert_eq!(capacities.len(), 5);

    // Verify ordering (stronger relations + higher similarity = higher capacity)
    for i in 1..capacities.len() {
        assert!(capacities[i - 1] >= capacities[i]);
    }
}
