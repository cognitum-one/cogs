//! Test suite for RaceWay routing
//!
//! Tests point-to-point routing, broadcast, and crossbar switching.

use cognitum_raceway::*;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_point_to_point_routing() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Send packet from tile 0 to tile 5
    network.send(TileId(0), TileId(5), &[0xFF]).await.unwrap();

    // Receive at tile 5
    let received = network.receive(TileId(5)).await.unwrap();
    assert_eq!(received.source(), TileId(0));
    assert_eq!(received.dest(), TileId(5));
    assert_eq!(received.data0(), 0xFF);
}

#[tokio::test]
async fn test_same_column_routing() {
    // Tiles in same column should route locally
    let mut network = RaceWayNetwork::new_for_test().await;

    // Column 0: tiles 0x00, 0x01, 0x02, ... 0x07
    network
        .send(TileId(0x00), TileId(0x03), &[0xAB, 0xCD])
        .await
        .unwrap();

    let received = network.receive(TileId(0x03)).await.unwrap();
    assert_eq!(received.source(), TileId(0x00));
}

#[tokio::test]
async fn test_cross_column_routing() {
    // Different columns should route through hub
    let mut network = RaceWayNetwork::new_for_test().await;

    // From column 0 (0x00) to column 5 (0x50)
    network
        .send(TileId(0x00), TileId(0x50), &[0x12, 0x34, 0x56, 0x78])
        .await
        .unwrap();

    let received = network.receive(TileId(0x50)).await.unwrap();
    assert_eq!(received.source(), TileId(0x00));
    assert_eq!(received.dest(), TileId(0x50));
}

#[tokio::test]
async fn test_response_routing() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Send request
    let request = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Read)
        .tag(0x07)
        .address(0x1000)
        .push(true)
        .build()
        .unwrap();

    network.send_packet(request.clone()).await.unwrap();

    // Create and send response (source/dest swapped)
    let response = request.to_response(Command::ReadData.to_u8());

    // Response should route back to original sender
    assert_eq!(response.dest(), TileId(0x11));
    assert_eq!(response.source(), TileId(0x42));
}

#[tokio::test]
async fn test_tag_matching() {
    let mut network = RaceWayNetwork::new_for_test().await;

    // Send multiple requests with different tags
    for tag in 0..4 {
        let packet = RaceWayPacket::new()
            .source(TileId(0x10))
            .dest(TileId(0x20))
            .command(Command::Write)
            .tag(tag)
            .write_data(tag as u32)
            .push(true)
            .build()
            .unwrap();

        network.send_packet(packet).await.unwrap();
    }

    // Responses should match request tags
    for _ in 0..4 {
        let received = network.receive(TileId(0x20)).await.unwrap();
        assert_eq!(received.data0(), received.tag() as u32);
    }
}

#[tokio::test]
async fn test_dimension_order_routing() {
    // Dimension-order: route in column first, then row
    let mut network = RaceWayNetwork::new_for_test().await;

    // From tile (0,0) to (5,3): should go column 0->5 first, then row 0->3
    network
        .send(
            TileId::new(0, 0).unwrap(),
            TileId::new(5, 3).unwrap(),
            &[0xAA],
        )
        .await
        .unwrap();

    let received = network.receive(TileId::new(5, 3).unwrap()).await.unwrap();
    assert_eq!(received.data0(), 0xAA);
}
