//! Broadcast protocol implementation
//!
//! Implements column broadcast, quadrant broadcast, and global barrier sync.

use crate::error::{RaceWayError, Result};
use crate::packet::{Command, RaceWayPacket, TileId};
use std::collections::HashMap;

/// Broadcast domain scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastDomain {
    /// All 8 tiles in same column
    Column,
    /// All 64 tiles in same quadrant (8x8)
    Quadrant,
    /// All 256 tiles (16x16)
    Global,
}

impl BroadcastDomain {
    /// Number of tiles in this broadcast domain
    pub fn tile_count(&self) -> usize {
        match self {
            BroadcastDomain::Column => 8,
            BroadcastDomain::Quadrant => 64,
            BroadcastDomain::Global => 256,
        }
    }

    /// Get all tiles in this broadcast domain
    pub fn tiles(&self, source: TileId) -> Vec<TileId> {
        match self {
            BroadcastDomain::Column => {
                let col = source.column();
                (0..8).map(|row| TileId::new(col, row).unwrap()).collect()
            }
            BroadcastDomain::Quadrant => {
                let quad = source.quadrant();
                let col_base = if quad == 1 || quad == 3 { 8 } else { 0 };
                let row_base = if quad == 2 || quad == 3 { 8 } else { 0 };
                let mut tiles = Vec::new();
                for col in 0..8 {
                    for row in 0..8 {
                        tiles.push(TileId::new(col_base + col, row_base + row).unwrap());
                    }
                }
                tiles
            }
            BroadcastDomain::Global => {
                let mut tiles = Vec::new();
                for col in 0..16 {
                    for row in 0..16 {
                        tiles.push(TileId::new(col, row).unwrap());
                    }
                }
                tiles
            }
        }
    }
}

/// Broadcast state for tracking in-flight broadcasts
#[derive(Debug)]
pub struct BroadcastState {
    /// TAG of the broadcast
    pub tag: u8,
    /// Source tile
    pub source: TileId,
    /// Target domain
    pub domain: BroadcastDomain,
    /// Acknowledgment count
    pub ack_count: usize,
}

/// Broadcast manager for Hub
pub struct BroadcastManager {
    /// Active broadcasts by TAG
    active: HashMap<u8, BroadcastState>,
}

impl BroadcastManager {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    /// Register a new broadcast
    pub fn register(&mut self, packet: &RaceWayPacket) -> Result<()> {
        if !packet.is_broadcast() {
            return Err(RaceWayError::InvalidPacket(
                "Not a broadcast packet".to_string(),
            ));
        }

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

        let state = BroadcastState {
            tag: packet.tag(),
            source: packet.source(),
            domain,
            ack_count: 0,
        };

        self.active.insert(packet.tag(), state);
        Ok(())
    }

    /// Record an acknowledgment
    pub fn acknowledge(&mut self, tag: u8) -> Option<TileId> {
        if let Some(state) = self.active.get_mut(&tag) {
            state.ack_count += 1;
            let expected = state.domain.tile_count() - 1; // Exclude source

            if state.ack_count >= expected {
                // Broadcast complete
                let source = state.source;
                self.active.remove(&tag);
                Some(source)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if a broadcast is complete
    pub fn is_complete(&self, tag: u8) -> bool {
        !self.active.contains_key(&tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_domain_column() {
        let domain = BroadcastDomain::Column;
        assert_eq!(domain.tile_count(), 8);

        let source = TileId::new(5, 3).unwrap();
        let tiles = domain.tiles(source);
        assert_eq!(tiles.len(), 8);

        // All tiles should be in column 5
        for tile in tiles {
            assert_eq!(tile.column(), 5);
        }
    }

    #[test]
    fn test_broadcast_domain_quadrant() {
        let domain = BroadcastDomain::Quadrant;
        assert_eq!(domain.tile_count(), 64);

        let source = TileId::new(9, 2).unwrap(); // Q1
        let tiles = domain.tiles(source);
        assert_eq!(tiles.len(), 64);

        // All tiles should be in Q1 (columns 8-15, rows 0-7)
        for tile in tiles {
            assert!(tile.column() >= 8);
            assert!(tile.row() < 8);
        }
    }

    #[test]
    fn test_broadcast_domain_global() {
        let domain = BroadcastDomain::Global;
        assert_eq!(domain.tile_count(), 256);

        let tiles = domain.tiles(TileId(0));
        assert_eq!(tiles.len(), 256);
    }

    #[test]
    fn test_broadcast_manager() {
        let mut manager = BroadcastManager::new();

        let broadcast = RaceWayPacket::new()
            .source(TileId(0x23))
            .command(Command::Broadcast)
            .tag(0x0F)
            .push(true)
            .build()
            .unwrap();

        manager.register(&broadcast).unwrap();

        // Acknowledge from 7 tiles (8 total - 1 source)
        for _ in 0..6 {
            assert!(manager.acknowledge(0x0F).is_none());
        }

        // Last acknowledgment should complete
        let source = manager.acknowledge(0x0F);
        assert_eq!(source, Some(TileId(0x23)));
        assert!(manager.is_complete(0x0F));
    }
}
