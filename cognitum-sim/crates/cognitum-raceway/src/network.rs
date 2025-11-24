//! RaceWay network integration
//!
//! Connects tiles, columns, and hubs into a complete network.

use crate::broadcast::{BroadcastDomain, BroadcastManager};
use crate::error::{RaceWayError, Result};
use crate::packet::{Command, RaceWayPacket, TileId};
use crossbeam::queue::SegQueue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

/// Packet buffer pool for reducing allocations
pub struct PacketPool {
    buffers: Arc<SegQueue<Vec<u8>>>,
    buffer_size: usize,
}

impl PacketPool {
    /// Create a new packet pool
    pub fn new(buffer_size: usize, initial_capacity: usize) -> Self {
        let pool = Self {
            buffers: Arc::new(SegQueue::new()),
            buffer_size,
        };

        // Pre-allocate buffers
        for _ in 0..initial_capacity {
            pool.buffers.push(vec![0u8; buffer_size]);
        }

        pool
    }

    /// Get a buffer from the pool
    pub fn get_buffer(&self) -> Vec<u8> {
        self.buffers
            .pop()
            .unwrap_or_else(|| vec![0u8; self.buffer_size])
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&self, mut buffer: Vec<u8>) {
        if buffer.len() == self.buffer_size {
            buffer.clear();
            buffer.resize(self.buffer_size, 0);
            self.buffers.push(buffer);
        }
    }

    /// Get pool size
    pub fn size(&self) -> usize {
        self.buffers.len()
    }
}

/// Packet batch for efficient sending
pub struct PacketBatch {
    packets: Vec<RaceWayPacket>,
    max_size: usize,
}

impl PacketBatch {
    /// Create a new packet batch
    pub fn new(max_size: usize) -> Self {
        Self {
            packets: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Add a packet to the batch
    /// Returns Some(packets) if batch is full, None otherwise
    pub fn add(&mut self, packet: RaceWayPacket) -> Option<Vec<RaceWayPacket>> {
        self.packets.push(packet);
        if self.packets.len() >= self.max_size {
            Some(std::mem::replace(
                &mut self.packets,
                Vec::with_capacity(self.max_size),
            ))
        } else {
            None
        }
    }

    /// Flush all packets in the batch
    pub fn flush(&mut self) -> Vec<RaceWayPacket> {
        std::mem::replace(&mut self.packets, Vec::with_capacity(self.max_size))
    }

    /// Get current batch size
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
}

/// Complete RaceWay network for testing
pub struct RaceWayNetwork {
    /// Tile receivers for testing
    tile_receivers: HashMap<TileId, mpsc::UnboundedReceiver<RaceWayPacket>>,
    /// Tile senders for injecting packets
    tile_senders: HashMap<TileId, mpsc::UnboundedSender<RaceWayPacket>>,
    /// Broadcast manager for tracking in-flight broadcasts
    broadcast_mgr: BroadcastManager,
    /// Default timeout for broadcast operations (configurable)
    broadcast_timeout: Duration,
    /// Packet buffer pool
    packet_pool: Arc<PacketPool>,
}

impl RaceWayNetwork {
    /// Create a new network for testing
    pub async fn new_for_test() -> Self {
        let mut network = RaceWayNetwork {
            tile_receivers: HashMap::new(),
            tile_senders: HashMap::new(),
            broadcast_mgr: BroadcastManager::new(),
            broadcast_timeout: Duration::from_secs(5),
            packet_pool: Arc::new(PacketPool::new(128, 1000)),
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

        debug!(
            "RaceWayNetwork initialized with {} tiles",
            network.tile_senders.len()
        );
        network
    }

    /// Set broadcast timeout (useful for testing)
    pub fn set_broadcast_timeout(&mut self, timeout_duration: Duration) {
        self.broadcast_timeout = timeout_duration;
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
        if packet.is_broadcast() {
            debug!(
                "Broadcasting packet from {:?} with tag 0x{:02X}, command {:?}",
                packet.source(),
                packet.tag(),
                packet.command()
            );
            self.handle_broadcast(packet).await
        } else {
            let dest = packet.dest();
            if let Some(tx) = self.tile_senders.get(&dest) {
                debug!("Sending packet from {:?} to {:?}", packet.source(), dest);
                tx.send(packet).map_err(|_| RaceWayError::ChannelFull)?;
                Ok(())
            } else {
                Err(RaceWayError::RoutingError(format!(
                    "Tile {:?} not found",
                    dest
                )))
            }
        }
    }

    /// Handle broadcast packet
    async fn handle_broadcast(&mut self, packet: RaceWayPacket) -> Result<()> {
        // Register the broadcast
        self.broadcast_mgr.register(&packet)?;

        let source = packet.source();
        let tag = packet.tag();
        let domain = match packet.command() {
            Command::Broadcast => BroadcastDomain::Column,
            Command::BarrierSync => BroadcastDomain::Global,
            Command::Multicast => BroadcastDomain::Quadrant,
            _ => {
                return Err(RaceWayError::InvalidPacket(
                    "Invalid broadcast command".to_string(),
                ))
            }
        };

        debug!(
            "Broadcast registered: source={:?}, tag=0x{:02X}, domain={:?}",
            source, tag, domain
        );

        // Get all tiles in the broadcast domain
        let target_tiles = domain.tiles(source);
        debug!("Broadcasting to {} tiles", target_tiles.len());

        // Send to all tiles in domain except source
        let mut sent_count = 0;
        for tile_id in &target_tiles {
            if *tile_id != source {
                if let Some(tx) = self.tile_senders.get(tile_id) {
                    tx.send(packet.clone())
                        .map_err(|_| RaceWayError::ChannelFull)?;
                    sent_count += 1;
                    debug!("Broadcast sent to tile {:?}", tile_id);
                }
            }
        }

        debug!("Broadcast sent to {} tiles (excluding source)", sent_count);

        // Simulate acknowledgments from all tiles
        // In a real system, tiles would send acknowledgments back
        let expected_acks = domain.tile_count() - 1; // Exclude source
        for _ in 0..expected_acks {
            self.broadcast_mgr.acknowledge(tag);
        }

        // Check if broadcast is complete
        if self.broadcast_mgr.is_complete(tag) {
            debug!(
                "Broadcast complete, sending BroadcastAck to source {:?}",
                source
            );

            // Send BroadcastAck back to source
            let ack = RaceWayPacket::new()
                .source(source) // Acknowledgment comes from the source tile itself
                .dest(source)
                .command(Command::BroadcastAck)
                .tag(tag)
                .push(true)
                .build()?;

            if let Some(tx) = self.tile_senders.get(&source) {
                tx.send(ack).map_err(|_| RaceWayError::ChannelFull)?;
                debug!("BroadcastAck sent to source {:?}", source);
            }
        }

        Ok(())
    }

    /// Receive a packet at a tile with timeout
    pub async fn receive(&mut self, tile_id: TileId) -> Result<RaceWayPacket> {
        if let Some(rx) = self.tile_receivers.get_mut(&tile_id) {
            match timeout(self.broadcast_timeout, rx.recv()).await {
                Ok(Some(packet)) => {
                    debug!("Tile {:?} received packet: {:?}", tile_id, packet.command());
                    Ok(packet)
                }
                Ok(None) => {
                    warn!("Tile {:?} channel closed", tile_id);
                    Err(RaceWayError::Timeout)
                }
                Err(_) => {
                    warn!(
                        "Tile {:?} receive timeout after {:?}",
                        tile_id, self.broadcast_timeout
                    );
                    Err(RaceWayError::Timeout)
                }
            }
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

    /// Get the packet pool
    pub fn packet_pool(&self) -> Arc<PacketPool> {
        Arc::clone(&self.packet_pool)
    }

    /// Send a batch of packets
    pub async fn send_batch(&mut self, packets: Vec<RaceWayPacket>) -> Result<()> {
        for packet in packets {
            self.send_packet(packet).await?;
        }
        Ok(())
    }

    /// Send packets concurrently from multiple sources
    pub async fn send_concurrent(
        &mut self,
        packets: Vec<(TileId, TileId, Vec<u8>)>,
    ) -> Result<Vec<Result<()>>> {
        let mut tasks = Vec::new();

        for (source, dest, data) in packets {
            let packet = RaceWayPacket::new()
                .source(source)
                .dest(dest)
                .command(Command::Write)
                .data(&data)
                .push(true)
                .build()?;

            let sender = self.tile_senders.get(&dest).cloned();

            let task = tokio::spawn(async move {
                if let Some(tx) = sender {
                    tx.send(packet).map_err(|_| RaceWayError::ChannelFull)?;
                    Ok(())
                } else {
                    Err(RaceWayError::RoutingError(format!(
                        "Tile {:?} not found",
                        dest
                    )))
                }
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(RaceWayError::RoutingError(format!(
                    "Task join error: {}",
                    e
                )))),
            }
        }

        Ok(results)
    }

    /// Receive multiple packets from different tiles concurrently
    pub async fn receive_concurrent(
        &mut self,
        tile_ids: Vec<TileId>,
    ) -> Result<Vec<Result<RaceWayPacket>>> {
        let mut tasks = Vec::new();

        for tile_id in tile_ids {
            if let Some(mut rx) = self.tile_receivers.remove(&tile_id) {
                let task = tokio::spawn(async move {
                    let result = rx.recv().await.ok_or(RaceWayError::Timeout);
                    (tile_id, rx, result)
                });
                tasks.push(task);
            }
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok((tile_id, rx, result)) => {
                    self.tile_receivers.insert(tile_id, rx);
                    results.push(result);
                }
                Err(e) => results.push(Err(RaceWayError::RoutingError(format!(
                    "Task join error: {}",
                    e
                )))),
            }
        }

        Ok(results)
    }

    /// Create a packet batch
    pub fn create_batch(&self, max_size: usize) -> PacketBatch {
        PacketBatch::new(max_size)
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

    #[tokio::test]
    async fn test_broadcast_timeout() {
        let mut network = RaceWayNetwork::new_for_test().await;
        network.set_broadcast_timeout(Duration::from_millis(100));

        // Try to receive from a tile that has no packets (should timeout)
        let tile_id = TileId::new(0, 0).unwrap();
        let result = network.receive(tile_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_broadcast_completion() {
        let mut network = RaceWayNetwork::new_for_test().await;

        let broadcast = RaceWayPacket::new()
            .source(TileId::new(3, 2).unwrap())
            .command(Command::Broadcast)
            .tag(0x42)
            .write_data(0xDEADBEEF)
            .push(true)
            .build()
            .unwrap();

        network.send_packet(broadcast).await.unwrap();

        // Source tile should receive BroadcastAck
        let ack = network.receive(TileId::new(3, 2).unwrap()).await.unwrap();
        assert_eq!(ack.command(), Command::BroadcastAck);
        assert_eq!(ack.tag(), 0x42);
    }

    #[tokio::test]
    async fn test_column_broadcast_delivery() {
        let mut network = RaceWayNetwork::new_for_test().await;

        let source = TileId::new(5, 3).unwrap();
        let broadcast = RaceWayPacket::new()
            .source(source)
            .command(Command::Broadcast)
            .tag(0x07)
            .write_data(0x12345678)
            .push(true)
            .build()
            .unwrap();

        network.send_packet(broadcast).await.unwrap();

        // All tiles in column 5 (except source) should receive the broadcast
        for row in 0..8 {
            let tile_id = TileId::new(5, row).unwrap();
            if tile_id != source {
                let received = network.receive(tile_id).await.unwrap();
                assert_eq!(received.command(), Command::Broadcast);
                assert_eq!(received.data0(), 0x12345678);
            }
        }
    }
}
