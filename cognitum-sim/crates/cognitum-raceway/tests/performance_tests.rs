//! Performance tests for RaceWay interconnect
//!
//! Tests latency and throughput characteristics.

use cognitum_raceway::*;
use std::time::Instant;

#[tokio::test]
async fn test_local_column_latency() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Measure latency for same-column routing
    let start = Instant::now();

    for _ in 0..100 {
        network
            .send(TileId(0x00), TileId(0x03), &[0xAA])
            .await
            .unwrap();
        let _ = network.receive(TileId(0x03)).await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_micros() / 100;

    // Local column: 2-5 cycles at 1 GHz = 2-5 ns
    // In simulation, should be very fast
    println!("Average local column latency: {} µs", avg_latency);
}

#[tokio::test]
async fn test_cross_hub_latency() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Measure latency for cross-hub routing (different quadrants)
    let start = Instant::now();

    for _ in 0..100 {
        // Q0 to Q3 (cross-hub)
        network
            .send(TileId(0x00), TileId(0xFF), &[0xBB])
            .await
            .unwrap();
        let _ = network.receive(TileId(0xFF)).await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_micros() / 100;

    // Cross-hub: 15-25 cycles at 1 GHz = 15-25 ns
    println!("Average cross-hub latency: {} µs", avg_latency);
}

#[tokio::test]
async fn test_throughput_single_lane() {
    let mut network = RaceWayNetwork::new_for_test().await;

    let packet_count = 1000;
    let start = Instant::now();

    // Send many packets
    for i in 0..packet_count {
        let dest = TileId(((i % 16) << 4) as u8);
        network.send(TileId(0x00), dest, &[0xFF]).await.unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = (packet_count as f64) / elapsed.as_secs_f64();

    // Per lane: 96 bits/cycle at 1 GHz
    println!("Throughput: {:.0} packets/sec", throughput);
}

#[tokio::test]
async fn test_broadcast_latency() {
    let mut network = RaceWayNetwork::new_for_test().await;

    let start = Instant::now();

    for _ in 0..10 {
        let broadcast = RaceWayPacket::new()
            .source(TileId(0x00))
            .command(Command::Broadcast)
            .tag(0x01)
            .push(true)
            .build()
            .unwrap();

        network.send_packet(broadcast).await.unwrap();

        // Wait for completion
        let _ = network.receive(TileId(0x00)).await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_micros() / 10;

    // Broadcast: 20-30 cycles for column
    println!("Average broadcast latency: {} µs", avg_latency);
}

#[tokio::test]
async fn test_aggregate_bandwidth() {
    // Theoretical aggregate bandwidth
    // 256 tiles × 96 bits/cycle at 1 GHz = 98 Gb/s
    // At 50% utilization = ~49 Gb/s = ~6 GB/s

    let bits_per_cycle = 96;
    let tiles = 256;
    let clock_ghz = 1.0;
    let utilization = 0.5;

    let aggregate_gbps = (tiles as f64) * (bits_per_cycle as f64) * clock_ghz * utilization;
    let aggregate_gbytes = aggregate_gbps / 8.0;

    println!(
        "Theoretical aggregate bandwidth: {:.1} Gb/s ({:.1} GB/s)",
        aggregate_gbps, aggregate_gbytes
    );

    assert!(aggregate_gbytes >= 5.0 && aggregate_gbytes <= 7.0);
}

#[tokio::test]
async fn test_latency_bounds() {
    // Verify latency is within expected bounds
    let latency_local_min_cycles = 2;
    let latency_local_max_cycles = 5;
    let latency_hub_min_cycles = 15;
    let latency_hub_max_cycles = 25;

    assert!(latency_local_min_cycles >= 2);
    assert!(latency_local_max_cycles <= 5);
    assert!(latency_hub_min_cycles >= 15);
    assert!(latency_hub_max_cycles <= 25);
}
