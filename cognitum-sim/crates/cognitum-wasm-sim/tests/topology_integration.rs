//! Topology integration tests
//!
//! Tests switching between topologies and verifying correct behavior

use cognitum_wasm_sim::{
    WasmSimulator,
    scale::{ScaleConfig, ScaleLevel},
    topology::{TopologyKind, LeafSpineConfig, HyperconvergedConfig, Topology},
};

/// Test RaceWay topology creation at all scale levels
#[test]
fn test_raceway_all_scales() {
    for level in [
        ScaleLevel::Development,
        ScaleLevel::Small,
        ScaleLevel::Medium,
        ScaleLevel::Large,
    ] {
        let scale = ScaleConfig::from_level(level);
        let sim = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay)
            .expect(&format!("Failed to create RaceWay at {:?}", level));

        assert_eq!(sim.tile_count(), scale.total_tiles());
        assert!(sim.topology().bandwidth_gbps() > 0.0);
        assert!(sim.topology().base_latency_ns() > 0);
    }
}

/// Test LeafSpine topology creation
#[test]
fn test_leafspine_creation() {
    let scale = ScaleConfig::from_tiles(256);
    let config = LeafSpineConfig::default();
    let sim = WasmSimulator::new(scale, TopologyKind::LeafSpine(config))
        .expect("Failed to create LeafSpine");

    assert_eq!(sim.tile_count(), 256);
    assert!(sim.topology().bandwidth_gbps() > 1000.0); // Should be high bandwidth
    assert_eq!(sim.topology().base_latency_ns(), 500); // 500ns typical
}

/// Test LeafSpine Arista preset
#[test]
fn test_leafspine_arista_preset() {
    let scale = ScaleConfig::from_tiles(2048);
    let config = LeafSpineConfig::arista_7060x6();
    let sim = WasmSimulator::new(scale, TopologyKind::LeafSpine(config))
        .expect("Failed to create Arista LeafSpine");

    assert_eq!(sim.tile_count(), 2048);
    // Arista 7060X6 should have very high bandwidth
    assert!(sim.topology().bandwidth_gbps() >= 100000.0);
}

/// Test Hyperconverged topology creation
#[test]
fn test_hyperconverged_creation() {
    let scale = ScaleConfig::from_tiles(64);
    let config = HyperconvergedConfig::for_nodes(64);  // Use for_nodes to scale config properly
    let sim = WasmSimulator::new(scale, TopologyKind::Hyperconverged(config))
        .expect("Failed to create Hyperconverged");

    assert_eq!(sim.tile_count(), 64);
    // Hyperconverged has higher latency (storage-dominated)
    assert!(sim.topology().base_latency_ns() >= 1000);
}

/// Test Hyperconverged Nutanix preset
#[test]
fn test_hyperconverged_nutanix_preset() {
    let scale = ScaleConfig::from_tiles(64);
    let config = HyperconvergedConfig::nutanix_style();
    let sim = WasmSimulator::new(scale, TopologyKind::Hyperconverged(config))
        .expect("Failed to create Nutanix Hyperconverged");

    assert_eq!(sim.tile_count(), 64);
    assert!(sim.topology().bandwidth_gbps() >= 1000.0);
}

/// Test topology switching - same scale, different topologies
#[test]
fn test_topology_switching() {
    let scale = ScaleConfig::from_tiles(64);

    // Create with RaceWay
    let sim1 = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay)
        .expect("Failed RaceWay");
    let raceway_latency = sim1.topology().base_latency_ns();
    let raceway_bw = sim1.topology().bandwidth_gbps();

    // Create with LeafSpine
    let sim2 = WasmSimulator::new(scale.clone(), TopologyKind::LeafSpine(LeafSpineConfig::default()))
        .expect("Failed LeafSpine");
    let leafspine_latency = sim2.topology().base_latency_ns();
    let leafspine_bw = sim2.topology().bandwidth_gbps();

    // Create with Hyperconverged
    let sim3 = WasmSimulator::new(scale.clone(), TopologyKind::Hyperconverged(HyperconvergedConfig::for_nodes(64)))
        .expect("Failed Hyperconverged");
    let hc_latency = sim3.topology().base_latency_ns();
    let hc_bw = sim3.topology().bandwidth_gbps();

    // Verify different characteristics
    assert!(raceway_latency < leafspine_latency, "RaceWay should have lower latency than LeafSpine");
    assert!(leafspine_latency < hc_latency, "LeafSpine should have lower latency than Hyperconverged");
    assert!(leafspine_bw > raceway_bw, "LeafSpine should have higher bandwidth than RaceWay");
}

/// Test topology description output
#[test]
fn test_topology_descriptions() {
    let scale = ScaleConfig::from_tiles(256);

    let sim1 = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).unwrap();
    assert!(sim1.topology_info().contains("RaceWay"));

    let sim2 = WasmSimulator::new(scale.clone(), TopologyKind::LeafSpine(LeafSpineConfig::default())).unwrap();
    assert!(sim2.topology_info().contains("Leaf-Spine"));

    let sim3 = WasmSimulator::new(scale.clone(), TopologyKind::Hyperconverged(HyperconvergedConfig::for_nodes(256))).unwrap();
    assert!(sim3.topology_info().contains("Hyperconverged"));
}

/// Test topology diameter (max hops)
#[test]
fn test_topology_diameter() {
    let scale = ScaleConfig::from_tiles(256);

    // RaceWay should have diameter ~6 for 256 tiles
    let sim1 = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).unwrap();
    assert!(sim1.topology().diameter() <= 10);

    // LeafSpine should have diameter 4 (server->leaf->spine->leaf->server)
    let sim2 = WasmSimulator::new(scale.clone(), TopologyKind::LeafSpine(LeafSpineConfig::default())).unwrap();
    assert_eq!(sim2.topology().diameter(), 4);

    // Hyperconverged should have diameter 4
    let sim3 = WasmSimulator::new(scale.clone(), TopologyKind::Hyperconverged(HyperconvergedConfig::for_nodes(256))).unwrap();
    assert!(sim3.topology().diameter() <= 6);
}

/// Test bisection bandwidth calculation
#[test]
fn test_bisection_bandwidth() {
    let scale = ScaleConfig::from_tiles(256);

    let sim1 = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).unwrap();
    let sim2 = WasmSimulator::new(scale.clone(), TopologyKind::LeafSpine(LeafSpineConfig::default())).unwrap();

    // LeafSpine should have higher bisection bandwidth (non-blocking fabric)
    assert!(sim2.topology().bisection_bandwidth() > sim1.topology().bisection_bandwidth());
}

/// Test small configurations (edge cases)
#[test]
fn test_small_configurations() {
    // Single tile
    let sim1 = WasmSimulator::new(
        ScaleConfig::from_tiles(1),
        TopologyKind::RaceWay
    ).expect("Failed single tile");
    assert_eq!(sim1.tile_count(), 1);

    // Small LeafSpine
    let sim2 = WasmSimulator::new(
        ScaleConfig::from_tiles(16),
        TopologyKind::LeafSpine(LeafSpineConfig::small())
    ).expect("Failed small LeafSpine");
    assert_eq!(sim2.tile_count(), 16);

    // Small Hyperconverged (3 nodes minimum for replication)
    let sim3 = WasmSimulator::new(
        ScaleConfig::from_tiles(3),
        TopologyKind::Hyperconverged(HyperconvergedConfig::small())
    ).expect("Failed small Hyperconverged");
    assert_eq!(sim3.tile_count(), 3);
}
