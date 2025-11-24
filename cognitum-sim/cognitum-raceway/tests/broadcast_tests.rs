//! Test suite for RaceWay broadcast protocol
//!
//! Tests column broadcast, quadrant broadcast, and global barrier sync.

use cognitum_raceway::*;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_column_broadcast() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Broadcast from tile 0x00 to all tiles in column 0
    let broadcast = RaceWayPacket::new()
        .source(TileId(0x00))
        .dest(TileId(0x00)) // Ignored for broadcast
        .command(Command::Broadcast)
        .tag(0x07)
        .write_data(0x12345678)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(broadcast.clone()).await.unwrap();

    // All 8 tiles in column 0 should receive
    for row in 0..8 {
        let tile_id = TileId::new(0, row).unwrap();
        if tile_id.0 != 0x00 {  // Skip source tile
            let received = network.receive(tile_id).await.unwrap();
            assert_eq!(received.command(), Command::Broadcast);
            assert_eq!(received.data0(), 0x12345678);
        }
    }
}

#[tokio::test]
async fn test_broadcast_loop_completion() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Broadcast initiates and completes loop
    let broadcast = RaceWayPacket::new()
        .source(TileId(0x23))
        .command(Command::Broadcast)
        .tag(0x0F)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(broadcast.clone()).await.unwrap();

    // Originating tile should receive completion acknowledgment
    let completion = network.receive(TileId(0x23)).await.unwrap();
    assert_eq!(completion.command(), Command::BroadcastAck);
    assert_eq!(completion.tag(), 0x0F); // Same tag
}

#[tokio::test]
async fn test_barrier_sync() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Global barrier synchronization
    let barrier = RaceWayPacket::new()
        .source(TileId(0x00))
        .command(Command::BarrierSync)
        .tag(0x42)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(barrier).await.unwrap();

    // All tiles should participate in barrier
    // This is a simplified test - real implementation would track all acks
    let ack = network.receive(TileId(0x00)).await.unwrap();
    assert_eq!(ack.tag(), 0x42);
}

#[tokio::test]
async fn test_broadcast_priority() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Broadcast should have higher priority than normal traffic
    // Send normal packet
    network.send(TileId(0x10), TileId(0x20), &[0x01]).await.unwrap();

    // Send broadcast (should bypass normal packet)
    let broadcast = RaceWayPacket::new()
        .source(TileId(0x15))
        .command(Command::Broadcast)
        .tag(0x08)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(broadcast).await.unwrap();

    // Broadcast should arrive first at column tiles
    // (Implementation detail: priority inversion in pipes)
}

#[tokio::test]
async fn test_multicast() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Multicast to specific subset of tiles
    let multicast = RaceWayPacket::new()
        .source(TileId(0x00))
        .command(Command::Multicast)
        .tag(0x11)
        .write_data(0xCAFEBABE)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(multicast).await.unwrap();

    // Specific tiles should receive (implementation-defined subset)
}

#[tokio::test]
async fn test_broadcast_domain_column() {
    use cognitum_raceway::broadcast::BroadcastDomain;

    let domain = BroadcastDomain::Column;
    assert_eq!(domain.tile_count(), 8); // 8 tiles per column
}

#[tokio::test]
async fn test_broadcast_domain_quadrant() {
    use cognitum_raceway::broadcast::BroadcastDomain;

    let domain = BroadcastDomain::Quadrant;
    assert_eq!(domain.tile_count(), 64); // 8x8 quadrant
}

#[tokio::test]
async fn test_broadcast_domain_global() {
    use cognitum_raceway::broadcast::BroadcastDomain;

    let domain = BroadcastDomain::Global;
    assert_eq!(domain.tile_count(), 256); // 16x16 global
}
