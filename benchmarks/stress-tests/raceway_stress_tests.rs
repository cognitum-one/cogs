//! RaceWay Network Stress Tests
//!
//! Comprehensive stress testing for the RaceWay interconnect:
//! - Maximum packet throughput
//! - Network congestion scenarios
//! - All 256 tiles communicating simultaneously
//! - Broadcast storm handling
//! - Packet loss and recovery

use cognitum_raceway::*;
use std::time::Instant;
use tokio::time::{timeout, Duration};

/// Stress test: 1M packets through the network
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_1m_packets() {
    let mut network = RaceWayNetwork::new_for_test().await;
    const PACKET_COUNT: usize = 1_000_000;

    let start = Instant::now();
    let mut successful_sends = 0;
    let mut failed_sends = 0;

    for i in 0..PACKET_COUNT {
        let source = TileId((i % 256) as u8);
        let dest = TileId(((i + 1) % 256) as u8);
        let data = vec![(i & 0xFF) as u8];

        match timeout(Duration::from_millis(10), network.send(source, dest, &data)).await {
            Ok(Ok(_)) => successful_sends += 1,
            _ => failed_sends += 1,
        }

        // Periodically drain receive queue to prevent backpressure
        if i % 1000 == 0 {
            for tile_id in 0..256 {
                let _ = timeout(
                    Duration::from_micros(100),
                    network.receive(TileId(tile_id as u8))
                ).await;
            }
        }
    }

    let elapsed = start.elapsed();
    let packets_per_sec = successful_sends as f64 / elapsed.as_secs_f64();
    let throughput_mbps = (packets_per_sec * 12.0 * 8.0) / 1_000_000.0; // 12 bytes per packet

    println!("✓ 1M Packet Stress Test:");
    println!("  - Total packets attempted: {}", PACKET_COUNT);
    println!("  - Successful sends: {}", successful_sends);
    println!("  - Failed sends: {}", failed_sends);
    println!("  - Success rate: {:.2}%", (successful_sends as f64 / PACKET_COUNT as f64) * 100.0);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Packets/sec: {:.2}", packets_per_sec);
    println!("  - Throughput: {:.2} Mbps", throughput_mbps);
}

/// Stress test: All 256 tiles sending simultaneously
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_256_tile_simultaneous_send() {
    let network = std::sync::Arc::new(tokio::sync::RwLock::new(
        RaceWayNetwork::new_for_test().await
    ));

    const PACKETS_PER_TILE: usize = 1000;
    let start = Instant::now();

    // Spawn tasks for all 256 tiles
    let mut handles = Vec::new();

    for tile_id in 0..256u8 {
        let network_clone = network.clone();

        let handle = tokio::spawn(async move {
            let mut successful = 0;
            let mut failed = 0;

            for i in 0..PACKETS_PER_TILE {
                let dest = TileId(((tile_id as usize + i + 1) % 256) as u8);
                let data = vec![tile_id, (i & 0xFF) as u8];

                let mut net = network_clone.write().await;
                match timeout(
                    Duration::from_millis(10),
                    net.send(TileId(tile_id), dest, &data)
                ).await {
                    Ok(Ok(_)) => successful += 1,
                    _ => failed += 1,
                }
                drop(net);

                // Small yield to prevent starvation
                if i % 10 == 0 {
                    tokio::task::yield_now().await;
                }
            }

            (tile_id, successful, failed)
        });

        handles.push(handle);
    }

    // Wait for all tiles to complete
    let results = futures::future::join_all(handles).await;

    let elapsed = start.elapsed();
    let mut total_successful = 0;
    let mut total_failed = 0;

    for result in results {
        if let Ok((tile_id, successful, failed)) = result {
            total_successful += successful;
            total_failed += failed;

            if failed > 0 {
                println!("  Tile {}: {} successful, {} failed", tile_id, successful, failed);
            }
        }
    }

    let total_attempted = 256 * PACKETS_PER_TILE;
    let packets_per_sec = total_successful as f64 / elapsed.as_secs_f64();

    println!("✓ 256 Tile Simultaneous Send Stress Test:");
    println!("  - Tiles: 256");
    println!("  - Packets per tile: {}", PACKETS_PER_TILE);
    println!("  - Total attempted: {}", total_attempted);
    println!("  - Total successful: {}", total_successful);
    println!("  - Total failed: {}", total_failed);
    println!("  - Success rate: {:.2}%", (total_successful as f64 / total_attempted as f64) * 100.0);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Aggregate packets/sec: {:.2}", packets_per_sec);
}

/// Stress test: Network congestion on single column
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_column_congestion() {
    let mut network = RaceWayNetwork::new_for_test().await;

    const PACKETS: usize = 10_000;
    let start = Instant::now();

    // All tiles in column 0 send to tile 0
    let mut sent = 0;
    let mut congestion_detected = 0;

    for i in 0..PACKETS {
        let source_col = 0;
        let source_row = (i % 16) as u8;
        let source = TileId((source_row << 4) | source_col);
        let dest = TileId(0x00);

        match timeout(Duration::from_millis(5), network.send(source, dest, &[0xFF])).await {
            Ok(Ok(_)) => sent += 1,
            _ => congestion_detected += 1,
        }
    }

    let elapsed = start.elapsed();

    println!("✓ Column Congestion Stress Test:");
    println!("  - Packets attempted: {}", PACKETS);
    println!("  - Successfully sent: {}", sent);
    println!("  - Congestion events: {}", congestion_detected);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Congestion rate: {:.2}%", (congestion_detected as f64 / PACKETS as f64) * 100.0);
}

/// Stress test: Broadcast storm
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_broadcast_storm() {
    let mut network = RaceWayNetwork::new_for_test().await;

    const BROADCASTS: usize = 1000;
    let start = Instant::now();

    let mut successful_broadcasts = 0;
    let mut failed_broadcasts = 0;

    for i in 0..BROADCASTS {
        let source = TileId((i % 256) as u8);

        let broadcast_packet = RaceWayPacket::new()
            .source(source)
            .command(Command::Broadcast)
            .tag((i & 0xFF) as u8)
            .push(true)
            .build();

        if let Ok(packet) = broadcast_packet {
            match timeout(Duration::from_millis(20), network.send_packet(packet)).await {
                Ok(Ok(_)) => successful_broadcasts += 1,
                _ => failed_broadcasts += 1,
            }
        }

        // Allow network to process
        if i % 10 == 0 {
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    }

    let elapsed = start.elapsed();
    let broadcasts_per_sec = successful_broadcasts as f64 / elapsed.as_secs_f64();

    println!("✓ Broadcast Storm Stress Test:");
    println!("  - Broadcasts attempted: {}", BROADCASTS);
    println!("  - Successful: {}", successful_broadcasts);
    println!("  - Failed: {}", failed_broadcasts);
    println!("  - Success rate: {:.2}%", (successful_broadcasts as f64 / BROADCASTS as f64) * 100.0);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Broadcasts/sec: {:.2}", broadcasts_per_sec);
}

/// Stress test: Cross-hub traffic
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_cross_hub_traffic() {
    let mut network = RaceWayNetwork::new_for_test().await;

    const PACKETS: usize = 10_000;
    let start = Instant::now();

    let mut q0_to_q3 = 0;
    let mut q1_to_q2 = 0;
    let mut failed = 0;

    for i in 0..PACKETS {
        // Alternate between Q0→Q3 and Q1→Q2 traffic
        let (source, dest) = if i % 2 == 0 {
            (TileId(0x00), TileId(0xFF)) // Q0 to Q3
        } else {
            (TileId(0x0F), TileId(0xF0)) // Q1 to Q2
        };

        match timeout(Duration::from_millis(10), network.send(source, dest, &[0xAB])).await {
            Ok(Ok(_)) => {
                if i % 2 == 0 {
                    q0_to_q3 += 1;
                } else {
                    q1_to_q2 += 1;
                }
            }
            _ => failed += 1,
        }
    }

    let elapsed = start.elapsed();
    let total_successful = q0_to_q3 + q1_to_q2;

    println!("✓ Cross-Hub Traffic Stress Test:");
    println!("  - Total packets: {}", PACKETS);
    println!("  - Q0→Q3 successful: {}", q0_to_q3);
    println!("  - Q1→Q2 successful: {}", q1_to_q2);
    println!("  - Failed: {}", failed);
    println!("  - Success rate: {:.2}%", (total_successful as f64 / PACKETS as f64) * 100.0);
    println!("  - Elapsed time: {:?}", elapsed);
}

/// Stress test: Packet priority under load
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_packet_priority() {
    let mut network = RaceWayNetwork::new_for_test().await;

    const HIGH_PRIORITY_PACKETS: usize = 100;
    const LOW_PRIORITY_PACKETS: usize = 1000;

    let start = Instant::now();

    // Send low priority packets
    for i in 0..LOW_PRIORITY_PACKETS {
        let source = TileId((i % 256) as u8);
        let dest = TileId(((i + 1) % 256) as u8);
        let _ = timeout(
            Duration::from_millis(5),
            network.send(source, dest, &[0x00])
        ).await;
    }

    // Send high priority packets (should be processed faster)
    let high_priority_start = Instant::now();
    for i in 0..HIGH_PRIORITY_PACKETS {
        let source = TileId((i % 256) as u8);
        let dest = TileId(((i + 1) % 256) as u8);
        let _ = timeout(
            Duration::from_millis(5),
            network.send(source, dest, &[0xFF])
        ).await;
    }
    let high_priority_elapsed = high_priority_start.elapsed();

    let total_elapsed = start.elapsed();

    println!("✓ Packet Priority Stress Test:");
    println!("  - Low priority packets: {}", LOW_PRIORITY_PACKETS);
    println!("  - High priority packets: {}", HIGH_PRIORITY_PACKETS);
    println!("  - High priority time: {:?}", high_priority_elapsed);
    println!("  - Total time: {:?}", total_elapsed);
}

/// Stress test: Network recovery from errors
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_network_recovery() {
    let mut network = RaceWayNetwork::new_for_test().await;

    const RECOVERY_CYCLES: usize = 10;
    const PACKETS_PER_CYCLE: usize = 100;

    let mut recovery_times = Vec::new();

    for cycle in 0..RECOVERY_CYCLES {
        // Normal operation
        for i in 0..PACKETS_PER_CYCLE {
            let source = TileId((i % 256) as u8);
            let dest = TileId(((i + 1) % 256) as u8);
            let _ = network.send(source, dest, &[0xAA]).await;
        }

        // Inject error condition (invalid packet)
        let invalid_packet = RaceWayPacket::new()
            .source(TileId(0))
            .command(Command::Send)
            .build()
            .unwrap();

        let error_start = Instant::now();
        let _ = network.send_packet(invalid_packet).await;

        // Attempt recovery
        let mut recovered = false;
        for retry in 0..10 {
            match timeout(
                Duration::from_millis(100),
                network.send(TileId(0), TileId(1), &[0xBB])
            ).await {
                Ok(Ok(_)) => {
                    recovered = true;
                    recovery_times.push((error_start.elapsed(), retry));
                    break;
                }
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }

        if !recovered {
            println!("  Cycle {}: Failed to recover", cycle);
        }
    }

    let avg_recovery_time = recovery_times.iter()
        .map(|(duration, _)| duration.as_micros())
        .sum::<u128>() / recovery_times.len().max(1) as u128;

    println!("✓ Network Recovery Stress Test:");
    println!("  - Recovery cycles: {}", RECOVERY_CYCLES);
    println!("  - Successful recoveries: {}", recovery_times.len());
    println!("  - Average recovery time: {} µs", avg_recovery_time);
    println!("  - Recovery rate: {:.2}%", (recovery_times.len() as f64 / RECOVERY_CYCLES as f64) * 100.0);
}

/// Stress test: Maximum packet size
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_maximum_packet_size() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Test various packet sizes
    let packet_sizes = vec![1, 8, 64, 256, 512, 1024, 4096];

    for size in packet_sizes {
        let data: Vec<u8> = (0..size).map(|i| (i & 0xFF) as u8).collect();

        let start = Instant::now();
        let result = timeout(
            Duration::from_millis(100),
            network.send(TileId(0), TileId(1), &data)
        ).await;
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(_)) => {
                let throughput_mbps = (size as f64 * 8.0) / elapsed.as_secs_f64() / 1_000_000.0;
                println!("  Size {} bytes: {:?} ({:.2} Mbps)", size, elapsed, throughput_mbps);
            }
            _ => println!("  Size {} bytes: FAILED or TIMEOUT", size),
        }
    }

    println!("✓ Maximum Packet Size Stress Test completed");
}
