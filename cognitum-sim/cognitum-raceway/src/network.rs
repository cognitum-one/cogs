//! RaceWay network integration
//!
//! Connects tiles, columns, and hubs into a complete network.

use crate::packet::{Command, RaceWayPacket, TileId};
use crate::column::ColumnInterconnect;
use crate::hub::Hub;
use crate::error::{RaceWayError, Result};
use tokio::sync::mpsc;
use std::collections::HashMap;

/// Complete RaceWay network for testing
pub struct RaceWayNetwork {
    /// Tile receivers for testing
    tile_receivers: HashMap<TileId, mpsc::UnboundedReceiver<RaceWayPacket>>,
    /// Tile senders for injecting packets
    tile_senders: HashMap<TileId, mpsc::UnboundedSender<RaceWayPacket>>,
}

impl RaceWayNetwork {
    /// Create a new network for testing
    pub async fn new_for_test() -> Self {
        let mut network = RaceWayNetwork {
            tile_receivers: HashMap::new(),
            tile_senders: HashMap::new(),
        };

        // Create tiles and columns
        for col in 0..16 {
            for row in 0..8 {
                let tile_id = TileId::new(col, row).unwrap();
                let (tx, rx) = mpsc::unbounded_channel();
                network.tile_receivers.insert(tile_id, rx);
                network.tile_senders.insert(tile_id, tx);
            }
        }

        network
    }

    /// Send a packet from source to destination
    pub async fn send(&mut self, source: TileId, dest: TileId, data: &[u8]) -> Result<()> {
        let packet = RaceWayPacket::new()
            .source(source)
            .dest(dest)
            .command(Command::Write)
            .data(data)
            .push(true)
            .build()?;

        self.send_packet(packet).await
    }

    /// Send a pre-built packet
    pub async fn send_packet(&mut self, packet: RaceWayPacket) -> Result<()> {
        let dest = packet.dest();

        if let Some(tx) = self.tile_senders.get(&dest) {
            tx.send(packet).map_err(|_| RaceWayError::ChannelFull)?;
            Ok(())
        } else {
            Err(RaceWayError::RoutingError(format!(
                "Tile {:?} not found",
                dest
            )))
        }
    }

    /// Receive a packet at a tile
    pub async fn receive(&mut self, tile_id: TileId) -> Result<RaceWayPacket> {
        if let Some(rx) = self.tile_receivers.get_mut(&tile_id) {
            rx.recv()
                .await
                .ok_or(RaceWayError::Timeout)
        } else {
            Err(RaceWayError::InvalidTileId(tile_id.0))
        }
    }

    /// Try to receive without blocking
    pub fn try_receive(&mut self, tile_id: TileId) -> Result<RaceWayPacket> {
        if let Some(rx) = self.tile_receivers.get_mut(&tile_id) {
            rx.try_recv().map_err(|_| RaceWayError::Timeout)
        } else {
            Err(RaceWayError::InvalidTileId(tile_id.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_creation() {
        let network = RaceWayNetwork::new_for_test().await;

        // Should have 16 columns × 8 rows = 128 tiles
        assert_eq!(network.tile_receivers.len(), 128);
        assert_eq!(network.tile_senders.len(), 128);
    }

    #[tokio::test]
    async fn test_network_send_receive() {
        let mut network = RaceWayNetwork::new_for_test().await;

        let source = TileId::new(0, 0).unwrap();
        let dest = TileId::new(5, 3).unwrap();

        network.send(source, dest, &[0xAB, 0xCD]).await.unwrap();

        let received = network.receive(dest).await.unwrap();
        assert_eq!(received.source(), source);
        assert_eq!(received.dest(), dest);
    }
}
