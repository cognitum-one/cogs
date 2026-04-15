//! Cross-tile communication tests
//!
//! Tests packet routing and multi-tile coordination

use cognitum_wasm_sim::{
    WasmSimulator,
    scale::{ScaleConfig, ScaleLevel},
    topology::{TopologyKind, LeafSpineConfig, HyperconvergedConfig},
    network::{Packet, NetworkStats},
    network::packet::{PacketCommand, QoS, Priority},
};

/// Test basic packet creation
#[test]
fn test_packet_creation() {
    let packet = Packet::new(0, 1, PacketCommand::Read);

    assert_eq!(packet.source, 0);
    assert_eq!(packet.destination, 1);
    assert!(packet.valid);
}

/// Test write packet creation
#[test]
fn test_write_packet() {
    let packet = Packet::write(5, 10, 0x1000, 0xDEADBEEF);

    assert_eq!(packet.source, 5);
    assert_eq!(packet.destination, 10);
    assert_eq!(packet.address(), 0x1000);
    assert_eq!(packet.data(), 0xDEADBEEF);
}

/// Test read packet creation
#[test]
fn test_read_packet() {
    let packet = Packet::read(0, 255, 0x2000);

    assert_eq!(packet.source, 0);
    assert_eq!(packet.destination, 255);
    assert_eq!(packet.address(), 0x2000);
}

/// Test packet response creation
#[test]
fn test_packet_response() {
    let request = Packet::read(0, 1, 0x1000);
    let response = request.response(PacketCommand::ReadData, Some(0x12345678));

    // Response should swap source and destination
    assert_eq!(response.source, 1);
    assert_eq!(response.destination, 0);
    assert_eq!(response.data(), 0x12345678);
}

/// Test network config from topology
#[test]
fn test_network_config_from_topology() {
    let scale = ScaleConfig::from_tiles(64);

    // Create simulators with different topologies
    let sim_raceway = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).unwrap();
    let sim_leafspine = WasmSimulator::new(
        scale.clone(),
        TopologyKind::LeafSpine(LeafSpineConfig::default())
    ).unwrap();

    // Verify different latencies
    let raceway_latency = sim_raceway.topology().base_latency_ns();
    let leafspine_latency = sim_leafspine.topology().base_latency_ns();

    assert!(raceway_latency < leafspine_latency);
}

/// Test multi-tile simulator initialization
#[test]
fn test_multi_tile_init() {
    let sim = WasmSimulator::with_scale(ScaleLevel::Medium).unwrap();

    // Should have 64 tiles
    assert_eq!(sim.tile_count(), 64);

    // All tiles should be accessible
    let stats = sim.stats();
    assert_eq!(stats.total_instructions, 0);
    assert_eq!(stats.total_cycles, 0);
}

/// Test loading bytecode to specific tiles
#[test]
fn test_load_to_tiles() {
    let mut sim = WasmSimulator::with_scale(ScaleLevel::Small).unwrap();

    // Load bytecode to tile 0
    let bytecode = vec![0x01, 0x01]; // nop, nop
    let result = sim.load_wasm(0, &bytecode);
    assert!(result.is_ok());

    // Load to tile 15 (last in Small scale)
    let result = sim.load_wasm(15, &bytecode);
    assert!(result.is_ok());

    // Load to invalid tile should fail
    let result = sim.load_wasm(16, &bytecode);
    assert!(result.is_err());
}

/// Test broadcast routing
#[test]
fn test_broadcast_packet() {
    let packet = Packet::broadcast(0, &[0xAB, 0xCD]);

    assert!(packet.is_broadcast());
    assert_eq!(packet.source, 0);
    assert_eq!(packet.destination, 0xFFFF);
}

/// Test packet serialization
#[test]
fn test_packet_serialization() {
    let original = Packet::write(5, 10, 0x1000, 0xDEADBEEF);
    let bytes = original.to_bytes();

    // Verify bytes are non-zero (packet has content)
    assert!(bytes.iter().any(|&b| b != 0));
}

/// Test network statistics default
#[test]
fn test_network_stats_default() {
    let stats = NetworkStats::default();

    assert_eq!(stats.packets_sent, 0);
    assert_eq!(stats.packets_received, 0);
    assert_eq!(stats.dropped_packets, 0);
}

/// Test network statistics merge
#[test]
fn test_network_stats_merge() {
    let mut stats1 = NetworkStats::default();
    let mut stats2 = NetworkStats::default();

    stats2.packets_sent = 100;
    stats2.packets_received = 90;

    stats1.merge(&stats2);

    assert_eq!(stats1.packets_sent, 100);
    assert_eq!(stats1.packets_received, 90);
}

/// Test packet QoS levels
#[test]
fn test_packet_qos() {
    let low = Packet::new(0, 1, PacketCommand::Read).with_qos(QoS::BestEffort);
    let high = Packet::new(0, 1, PacketCommand::Read).with_qos(QoS::RealTime);

    assert!(high.qos as u8 > low.qos as u8);
}

/// Test packet priority levels
#[test]
fn test_packet_priority() {
    let normal = Packet::new(0, 1, PacketCommand::Read).with_priority(Priority::Normal);
    let high = Packet::new(0, 1, PacketCommand::Read).with_priority(Priority::High);

    // Lower value = higher priority (Critical=0, High=1, Normal=2)
    assert!((high.priority as u8) < (normal.priority as u8));
}

/// Test payload handling
#[test]
fn test_payload_handling() {
    let mut packet = Packet::new(0, 1, PacketCommand::Write);
    packet.set_payload(&[0x01, 0x02, 0x03, 0x04, 0x05]);

    let slice = packet.payload_slice();
    assert_eq!(slice.len(), 5);
    assert_eq!(slice, &[0x01, 0x02, 0x03, 0x04, 0x05]);
}

/// Test all-to-all communication pattern
#[test]
fn test_all_to_all_pattern() {
    // Verify we can address all tile pairs
    for src in 0..16u16 {
        for dst in 0..16u16 {
            let packet = Packet::new(src, dst, PacketCommand::Read);
            assert_eq!(packet.source, src);
            assert_eq!(packet.destination, dst);
        }
    }
}

/// Test packet hop counting
#[test]
fn test_packet_hops() {
    let mut packet = Packet::new(0, 255, PacketCommand::Read);

    assert_eq!(packet.hops, 0);
    assert!(!packet.expired());

    for _ in 0..64 {
        packet.hop();
    }

    assert_eq!(packet.hops, 64);
    assert!(packet.expired());
}

/// Test response detection
#[test]
fn test_response_detection() {
    let request = Packet::new(0, 1, PacketCommand::Read);
    let response = Packet::new(1, 0, PacketCommand::ReadData);
    let error = Packet::new(1, 0, PacketCommand::Error);

    assert!(!request.is_response());
    assert!(response.is_response());
    assert!(error.is_response());
}

/// Test tag preservation in responses
#[test]
fn test_tag_preservation() {
    let request = Packet::new(0, 1, PacketCommand::Read).with_tag(42);
    let response = request.response(PacketCommand::ReadData, Some(0x1234));

    assert_eq!(response.tag, 42);
}

/// Test topology bandwidth comparison
#[test]
fn test_topology_bandwidth() {
    let scale = ScaleConfig::from_tiles(256);

    let sim_raceway = WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).unwrap();
    let sim_leafspine = WasmSimulator::new(
        scale.clone(),
        TopologyKind::LeafSpine(LeafSpineConfig::default())
    ).unwrap();

    // LeafSpine should have higher bandwidth
    assert!(sim_leafspine.topology().bandwidth_gbps() > sim_raceway.topology().bandwidth_gbps());
}

/// Test topology at different scales
#[test]
fn test_topologies_at_scales() {
    // Test RaceWay and LeafSpine at various scales
    for size in [16, 64, 256] {
        let scale = ScaleConfig::from_tiles(size);

        assert!(WasmSimulator::new(scale.clone(), TopologyKind::RaceWay).is_ok());
        assert!(WasmSimulator::new(
            scale.clone(),
            TopologyKind::LeafSpine(LeafSpineConfig::default())
        ).is_ok());
    }

    // Hyperconverged works best at scales suitable for storage clusters
    // Test with appropriate configurations (use for_nodes to scale config properly)
    let hc_scale = ScaleConfig::from_tiles(64);
    assert!(WasmSimulator::new(
        hc_scale,
        TopologyKind::Hyperconverged(HyperconvergedConfig::for_nodes(64))
    ).is_ok());
}
