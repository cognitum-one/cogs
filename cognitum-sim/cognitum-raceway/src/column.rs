//! Column interconnect implementation
//!
//! Routes packets within a column of 8 tiles.

use crate::packet::{RaceWayPacket, TileId};
use crate::error::{RaceWayError, Result};
use tokio::sync::mpsc;
use std::collections::HashMap;

/// Column interconnect for 8 tiles
pub struct ColumnInterconnect {
    column_id: u8,
    tile_senders: HashMap<TileId, mpsc::UnboundedSender<RaceWayPacket>>,
    from_tiles: mpsc::UnboundedReceiver<RaceWayPacket>,
    to_hub: mpsc::UnboundedSender<RaceWayPacket>,
    from_hub: mpsc::UnboundedReceiver<RaceWayPacket>,
}

impl ColumnInterconnect {
    pub fn new(
        column_id: u8,
        to_hub: mpsc::UnboundedSender<RaceWayPacket>,
        from_hub: mpsc::UnboundedReceiver<RaceWayPacket>,
    ) -> (Self, mpsc::UnboundedSender<RaceWayPacket>) {
        let (from_tiles_tx, from_tiles_rx) = mpsc::unbounded_channel();

        let column = ColumnInterconnect {
            column_id,
            tile_senders: HashMap::new(),
            from_tiles: from_tiles_rx,
            to_hub,
            from_hub,
        };

        (column, from_tiles_tx)
    }

    /// Register a tile in this column
    pub fn register_tile(&mut self, tile_id: TileId, tx: mpsc::UnboundedSender<RaceWayPacket>) {
        self.tile_senders.insert(tile_id, tx);
    }

    /// Route a packet within the column or to hub
    pub async fn route(&mut self, packet: RaceWayPacket) -> Result<()> {
        let dest = packet.dest();

        // Check if destination is in this column
        if dest.column() == self.column_id {
            // Local routing within column
            if let Some(tx) = self.tile_senders.get(&dest) {
                tx.send(packet).map_err(|_| RaceWayError::ChannelFull)?;
            } else {
                return Err(RaceWayError::RoutingError(
                    format!("Tile {:?} not found in column {}", dest, self.column_id)
                ));
            }
        } else {
            // Route to hub for cross-column delivery
            self.to_hub.send(packet).map_err(|_| RaceWayError::ChannelFull)?;
        }

        Ok(())
    }

    /// Handle broadcast within column
    pub async fn broadcast(&mut self, packet: RaceWayPacket) -> Result<()> {
        // Send to all tiles in this column except source
        let source = packet.source();

        for (tile_id, tx) in &self.tile_senders {
            if *tile_id != source {
                tx.send(packet.clone()).map_err(|_| RaceWayError::ChannelFull)?;
            }
        }

        Ok(())
    }

    /// Run the column routing loop
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // From tiles to column/hub
                Some(packet) = self.from_tiles.recv() => {
                    if packet.is_broadcast() {
                        let _ = self.broadcast(packet.clone()).await;
                        // Also send to hub for cross-column broadcast
                        let _ = self.to_hub.send(packet);
                    } else {
                        let _ = self.route(packet).await;
                    }
                }

                // From hub to tiles
                Some(packet) = self.from_hub.recv() => {
                    let dest = packet.dest();
                    if let Some(tx) = self.tile_senders.get(&dest) {
                        let _ = tx.send(packet);
                    }
                }

                else => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_column_routing() {
        let (to_hub_tx, mut to_hub_rx) = mpsc::unbounded_channel();
        let (from_hub_tx, from_hub_rx) = mpsc::unbounded_channel();

        let (mut column, from_tiles_tx) = ColumnInterconnect::new(5, to_hub_tx, from_hub_rx);

        // Register tiles in column 5
        let (tile0_tx, mut tile0_rx) = mpsc::unbounded_channel();
        let (tile1_tx, mut tile1_rx) = mpsc::unbounded_channel();

        column.register_tile(TileId::new(5, 0).unwrap(), tile0_tx);
        column.register_tile(TileId::new(5, 1).unwrap(), tile1_tx);

        // Route packet within column
        let packet = RaceWayPacket::new()
            .source(TileId::new(5, 0).unwrap())
            .dest(TileId::new(5, 1).unwrap())
            .push(true)
            .build()
            .unwrap();

        column.route(packet.clone()).await.unwrap();

        // Tile 1 should receive
        let received = tile1_rx.recv().await.unwrap();
        assert_eq!(received.dest(), TileId::new(5, 1).unwrap());
    }

    #[tokio::test]
    async fn test_column_to_hub_routing() {
        let (to_hub_tx, mut to_hub_rx) = mpsc::unbounded_channel();
        let (from_hub_tx, from_hub_rx) = mpsc::unbounded_channel();

        let (mut column, from_tiles_tx) = ColumnInterconnect::new(5, to_hub_tx, from_hub_rx);

        // Route packet to different column (should go to hub)
        let packet = RaceWayPacket::new()
            .source(TileId::new(5, 0).unwrap())
            .dest(TileId::new(8, 3).unwrap()) // Different column
            .push(true)
            .build()
            .unwrap();

        column.route(packet.clone()).await.unwrap();

        // Hub should receive
        let received = to_hub_rx.recv().await.unwrap();
        assert_eq!(received.dest().column(), 8);
    }
}
