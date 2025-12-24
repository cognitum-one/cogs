//! Scale validation tests
//!
//! Tests scaling from 1 to 1024+ tiles across different configurations

use cognitum_wasm_sim::{
    WasmSimulator,
    scale::{ScaleConfig, ScaleLevel},
    topology::TopologyKind,
};

/// Test all predefined scale levels
#[test]
fn test_all_scale_levels() {
    let levels = vec![
        (ScaleLevel::Development, 1),
        (ScaleLevel::Small, 16),
        (ScaleLevel::Medium, 64),
        (ScaleLevel::Large, 256),
        (ScaleLevel::Enterprise, 1024),
    ];

    for (level, expected_tiles) in levels {
        let config = ScaleConfig::from_level(level);
        assert_eq!(
            config.total_tiles(), expected_tiles,
            "Scale level {:?} should have {} tiles", level, expected_tiles
        );
    }
}

/// Test scale level detection from tile count
#[test]
fn test_scale_level_detection() {
    assert_eq!(ScaleConfig::from_tiles(1).level(), ScaleLevel::Development);
    assert_eq!(ScaleConfig::from_tiles(8).level(), ScaleLevel::Small);
    assert_eq!(ScaleConfig::from_tiles(16).level(), ScaleLevel::Small);
    assert_eq!(ScaleConfig::from_tiles(32).level(), ScaleLevel::Medium);
    assert_eq!(ScaleConfig::from_tiles(64).level(), ScaleLevel::Medium);
    assert_eq!(ScaleConfig::from_tiles(128).level(), ScaleLevel::Large);
    assert_eq!(ScaleConfig::from_tiles(256).level(), ScaleLevel::Large);
    assert_eq!(ScaleConfig::from_tiles(512).level(), ScaleLevel::Enterprise);
    assert_eq!(ScaleConfig::from_tiles(1024).level(), ScaleLevel::Enterprise);
}

/// Test memory calculation at different scales
#[test]
fn test_memory_scaling() {
    // 80KB per tile
    let small = ScaleConfig::from_level(ScaleLevel::Small);
    assert!(small.total_memory_mb() >= 1); // 16 * 80KB = 1.25MB

    let medium = ScaleConfig::from_level(ScaleLevel::Medium);
    assert_eq!(medium.total_memory_mb(), 5); // 64 * 80KB = 5MB

    let large = ScaleConfig::from_level(ScaleLevel::Large);
    assert_eq!(large.total_memory_mb(), 20); // 256 * 80KB = 20MB

    let enterprise = ScaleConfig::from_level(ScaleLevel::Enterprise);
    assert_eq!(enterprise.total_memory_mb(), 80); // 1024 * 80KB = 80MB
}

/// Test multi-chip configuration
#[test]
fn test_multi_chip_scaling() {
    // Single chip (up to 256 tiles)
    let single = ScaleConfig::from_tiles(256);
    assert!(!single.multi_chip_enabled);
    assert_eq!(single.num_chips, 1);

    // Multi-chip (> 256 tiles)
    let multi = ScaleConfig::from_tiles(512);
    assert!(multi.multi_chip_enabled);
    assert_eq!(multi.num_chips, 2);

    let quad = ScaleConfig::from_tiles(1024);
    assert!(quad.multi_chip_enabled);
    assert_eq!(quad.num_chips, 4);
}

/// Test compute GOPS scaling
#[test]
fn test_compute_scaling() {
    let dev = ScaleConfig::from_level(ScaleLevel::Development);
    let large = ScaleConfig::from_level(ScaleLevel::Large);

    // Large should have 256x the compute of Development
    assert_eq!(large.compute_gops() / dev.compute_gops(), 256.0);
}

/// Test simulator creation at various scales
#[test]
fn test_simulator_at_scales() {
    let test_sizes = vec![1, 4, 16, 32, 64, 128, 256];

    for size in test_sizes {
        let scale = ScaleConfig::from_tiles(size);
        let sim = WasmSimulator::new(scale, TopologyKind::RaceWay)
            .expect(&format!("Failed to create simulator with {} tiles", size));

        assert_eq!(sim.tile_count(), size, "Simulator should have {} tiles", size);
    }
}

/// Test large scale creation (stress test)
#[test]
fn test_large_scale_creation() {
    // 256 tiles - single chip max
    let sim256 = WasmSimulator::with_scale(ScaleLevel::Large)
        .expect("Failed to create 256-tile simulator");
    assert_eq!(sim256.tile_count(), 256);

    // Verify all tiles are accessible
    let info = sim256.scale_info();
    assert_eq!(info.total_tiles(), 256);
    assert!(!info.multi_chip_enabled);
}

/// Test scale config cloning and equality
#[test]
fn test_scale_config_clone() {
    let config1 = ScaleConfig::from_level(ScaleLevel::Large);
    let config2 = config1.clone();

    assert_eq!(config1.total_tiles(), config2.total_tiles());
    assert_eq!(config1.level(), config2.level());
    assert_eq!(config1.num_chips, config2.num_chips);
}

/// Test edge cases in scale configuration
#[test]
fn test_scale_edge_cases() {
    // Very large tile count
    let huge = ScaleConfig::from_tiles(4096);
    assert!(huge.multi_chip_enabled);
    assert_eq!(huge.num_chips, 16); // 4096 / 256 = 16 chips
}

/// Test scale with specific topology requirements
#[test]
fn test_scale_topology_combinations() {
    let scales = vec![16, 64, 256];

    for size in scales {
        let scale = ScaleConfig::from_tiles(size);

        // All topologies should work at all scales
        let _ = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay)
            .expect(&format!("RaceWay failed at {}", size));

        let _ = WasmSimulator::new(
            scale.clone(),
            TopologyKind::LeafSpine(cognitum_wasm_sim::topology::LeafSpineConfig::default())
        ).expect(&format!("LeafSpine failed at {}", size));

        let _ = WasmSimulator::new(
            scale.clone(),
            TopologyKind::Hyperconverged(cognitum_wasm_sim::topology::HyperconvergedConfig::for_nodes(size))
        ).expect(&format!("Hyperconverged failed at {}", size));
    }
}
