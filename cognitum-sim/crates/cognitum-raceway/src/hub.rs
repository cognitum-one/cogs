//! Hub router implementation
//!
//! Central hub connecting quadrants in the Cognitum RaceWay.
//! Based on Hub.v - routes between left/right halves and columns.

use crate::broadcast::BroadcastManager;
use crate::error::{RaceWayError, Result};
use crate::packet::{Command, RaceWayPacket};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Hub router connecting quadrants
///
/// The Hub connects:
/// - West Half (columns 0-7)
/// - East Half (columns 8-15)
/// - Center Column interfaces
///
/// Based on Hub.v implementation with:
/// - 12x12 crossbar
/// - Broadcast state machine
/// - Priority arbitration
pub struct Hub {
    /// Channels from west quadrant
    west_rx: mpsc::UnboundedReceiver<RaceWayPacket>,
    west_tx: mpsc::UnboundedSender<RaceWayPacket>,

    /// Channels from east quadrant
    east_rx: mpsc::UnboundedReceiver<RaceWayPacket>,
    east_tx: mpsc::UnboundedSender<RaceWayPacket>,

    /// Channels to/from center columns
    center_channels: HashMap<
        u8,
        (
            mpsc::UnboundedSender<RaceWayPacket>,
            mpsc::UnboundedReceiver<RaceWayPacket>,
        ),
    >,

    /// Broadcast manager
    broadcast_mgr: BroadcastManager,

    /// Timeout counter for broadcasts (9-bit counter from Hub.v)
    timeout_counter: u16,
}

impl Hub {
    pub fn new(
        west_rx: mpsc::UnboundedReceiver<RaceWayPacket>,
        west_tx: mpsc::UnboundedSender<RaceWayPacket>,
        east_rx: mpsc::UnboundedReceiver<RaceWayPacket>,
        east_tx: mpsc::UnboundedSender<RaceWayPacket>,
    ) -> Self {
        Hub {
            west_rx,
            west_tx,
            east_rx,
            east_tx,
            center_channels: HashMap::new(),
            broadcast_mgr: BroadcastManager::new(),
            timeout_counter: 0,
        }
    }

    /// Route packet through the hub
    pub fn route(&mut self, packet: RaceWayPacket) -> Result<()> {
        let dest = packet.dest();
        let source = packet.source();

        // Determine destination quadrant
        let dest_quadrant = dest.quadrant();
        let source_quadrant = source.quadrant();

        if packet.is_broadcast() {
            self.handle_broadcast(packet)
        } else {
            // Point-to-point routing
            if dest_quadrant < 2 && source_quadrant >= 2 {
                // South to North
                self.west_tx
                    .send(packet)
                    .map_err(|_| RaceWayError::ChannelFull)?;
            } else if dest_quadrant >= 2 && source_quadrant < 2 {
                // North to South
                self.east_tx
                    .send(packet)
                    .map_err(|_| RaceWayError::ChannelFull)?;
            } else if dest.column() < 8 {
                // West half
                self.west_tx
                    .send(packet)
                    .map_err(|_| RaceWayError::ChannelFull)?;
            } else {
                // East half
                self.east_tx
                    .send(packet)
                    .map_err(|_| RaceWayError::ChannelFull)?;
            }
            Ok(())
        }
    }

    /// Handle broadcast packet
    ///
    /// Implements broadcast flow from Hub.v:
    /// 1. Register broadcast with TAG
    /// 2. Distribute to other channels
    /// 3. Wait for loop completion
    /// 4. Send acknowledgment to source
    fn handle_broadcast(&mut self, packet: RaceWayPacket) -> Result<()> {
        // Register broadcast
        self.broadcast_mgr.register(&packet)?;

        // Distribute to all channels except source
        let source = packet.source();

        // Send to west if source not in west
        if source.column() >= 8 {
            let _ = self.west_tx.send(packet.clone());
        }

        // Send to east if source not in east
        if source.column() < 8 {
            let _ = self.east_tx.send(packet.clone());
        }

        // Send to center columns
        for (col_id, (tx, _)) in &self.center_channels {
            if *col_id != source.column() {
                let _ = tx.send(packet.clone());
            }
        }

        Ok(())
    }

    /// Handle broadcast completion (loop response)
    fn complete_broadcast(&mut self, packet: RaceWayPacket) -> Result<()> {
        let tag = packet.tag();

        if let Some(source) = self.broadcast_mgr.acknowledge(tag) {
            // Broadcast complete - send acknowledgment to source
            let ack = RaceWayPacket::new()
                .source(packet.dest())
                .dest(source)
                .command(Command::BroadcastAck)
                .tag(tag)
                .push(true)
                .build()?;

            self.route(ack)?;
        }

        Ok(())
    }

    /// Run the hub routing loop
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // Packets from west
                Some(packet) = self.west_rx.recv() => {
                    if packet.is_broadcast() {
                        let _ = self.complete_broadcast(packet);
                    } else {
                        let _ = self.route(packet);
                    }
                }

                // Packets from east
                Some(packet) = self.east_rx.recv() => {
                    if packet.is_broadcast() {
                        let _ = self.complete_broadcast(packet);
                    } else {
                        let _ = self.route(packet);
                    }
                }

                else => break,
            }

            // Increment timeout counter (matches Hub.v 9-bit counter)
            self.timeout_counter = (self.timeout_counter + 1) & 0x1FF;
        }
    }

    /// Add a center column channel
    pub fn add_center_column(
        &mut self,
        column_id: u8,
        tx: mpsc::UnboundedSender<RaceWayPacket>,
        rx: mpsc::UnboundedReceiver<RaceWayPacket>,
    ) {
        self.center_channels.insert(column_id, (tx, rx));
    }
}

/// Crossbar for routing (12x12 as in Hub.v)
pub struct Crossbar<const INPUTS: usize, const OUTPUTS: usize> {
    /// Input ports
    inputs: [Option<RaceWayPacket>; INPUTS],
    /// Output selection matrix
    select: [[bool; INPUTS]; OUTPUTS],
}

impl<const I: usize, const O: usize> Crossbar<I, O> {
    pub fn new() -> Self {
        Crossbar {
            inputs: [const { None }; I],
            select: [[false; I]; O],
        }
    }

    /// Set input port
    pub fn set_input(&mut self, port: usize, packet: Option<RaceWayPacket>) {
        if port < I {
            self.inputs[port] = packet;
        }
    }

    /// Configure output selection
    pub fn select_input(&mut self, output: usize, input: usize) {
        if output < O && input < I {
            // Clear previous selection
            for i in 0..I {
                self.select[output][i] = false;
            }
            self.select[output][input] = true;
        }
    }

    /// Get output
    pub fn get_output(&self, port: usize) -> Option<&RaceWayPacket> {
        if port >= O {
            return None;
        }

        for (i, &selected) in self.select[port].iter().enumerate() {
            if selected {
                return self.inputs[i].as_ref();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TileId;

    #[tokio::test]
    async fn test_hub_routing() {
        let (west_tx, _west_rx) = mpsc::unbounded_channel();
        let (_hub_west_tx, hub_west_rx) = mpsc::unbounded_channel();
        let (east_tx, mut east_rx) = mpsc::unbounded_channel();
        let (_hub_east_tx, hub_east_rx) = mpsc::unbounded_channel();

        let mut hub = Hub::new(hub_west_rx, west_tx, hub_east_rx, east_tx);

        // Route from west to east
        let packet = RaceWayPacket::new()
            .source(TileId::new(2, 3).unwrap()) // Q0
            .dest(TileId::new(10, 5).unwrap()) // Q1
            .push(true)
            .build()
            .unwrap();

        hub.route(packet.clone()).unwrap();

        // Should be routed to east
        let received = east_rx.try_recv();
        assert!(received.is_ok());
    }

    #[test]
    fn test_crossbar() {
        let mut crossbar = Crossbar::<4, 4>::new();

        let packet = RaceWayPacket::new()
            .source(TileId(0x11))
            .dest(TileId(0x22))
            .push(true)
            .build()
            .unwrap();

        crossbar.set_input(0, Some(packet.clone()));
        crossbar.select_input(2, 0);

        let output = crossbar.get_output(2);
        assert!(output.is_some());
        assert_eq!(output.unwrap().source(), TileId(0x11));
    }

    #[test]
    fn test_broadcast_manager() {
        let mut mgr = BroadcastManager::new();

        let broadcast = RaceWayPacket::new()
            .source(TileId(0x00))
            .command(Command::Broadcast)
            .tag(0x42)
            .push(true)
            .build()
            .unwrap();

        mgr.register(&broadcast).unwrap();

        // 7 acknowledgments needed (8 tiles - 1 source)
        for _ in 0..6 {
            assert!(mgr.acknowledge(0x42).is_none());
        }

        let source = mgr.acknowledge(0x42);
        assert_eq!(source, Some(TileId(0x00)));
    }
}
