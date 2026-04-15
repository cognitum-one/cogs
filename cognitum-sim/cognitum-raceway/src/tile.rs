//! Tile addressing for the 16x16 tile array

use crate::error::{RaceWayError, Result};

/// Tile identifier in the 16x16 array
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId(pub u8);

impl TileId {
    /// Create a new TileId from column and row
    pub fn new(column: u8, row: u8) -> Result<Self> {
        if column >= 16 || row >= 16 {
            return Err(RaceWayError::InvalidTileId((column << 4) | row));
        }
        Ok(TileId((column << 4) | row))
    }

    /// Get the column (bits 7:4)
    pub fn column(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }

    /// Get the row (bits 3:0)
    pub fn row(&self) -> u8 {
        self.0 & 0x0F
    }

    /// Get the quadrant (0-3)
    pub fn quadrant(&self) -> u8 {
        let col = self.column();
        let row = self.row();
        match (col >= 8, row >= 8) {
            (false, false) => 0, // Q0 (NW)
            (true, false) => 1,  // Q1 (NE)
            (false, true) => 2,  // Q2 (SW)
            (true, true) => 3,   // Q3 (SE)
        }
    }

    /// Check if this is the same column as another tile
    pub fn same_column(&self, other: &TileId) -> bool {
        self.column() == other.column()
    }

    /// Check if this is the same quadrant as another tile
    pub fn same_quadrant(&self, other: &TileId) -> bool {
        self.quadrant() == other.quadrant()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_id_creation() {
        let tile = TileId::new(5, 3).unwrap();
        assert_eq!(tile.0, 0x53);
        assert_eq!(tile.column(), 5);
        assert_eq!(tile.row(), 3);
    }

    #[test]
    fn test_tile_id_invalid() {
        assert!(TileId::new(16, 0).is_err());
        assert!(TileId::new(0, 16).is_err());
    }

    #[test]
    fn test_quadrant_mapping() {
        assert_eq!(TileId::new(0, 0).unwrap().quadrant(), 0); // Q0 (NW)
        assert_eq!(TileId::new(7, 7).unwrap().quadrant(), 0);
        assert_eq!(TileId::new(8, 0).unwrap().quadrant(), 1); // Q1 (NE)
        assert_eq!(TileId::new(15, 7).unwrap().quadrant(), 1);
        assert_eq!(TileId::new(0, 8).unwrap().quadrant(), 2); // Q2 (SW)
        assert_eq!(TileId::new(7, 15).unwrap().quadrant(), 2);
        assert_eq!(TileId::new(8, 8).unwrap().quadrant(), 3); // Q3 (SE)
        assert_eq!(TileId::new(15, 15).unwrap().quadrant(), 3);
    }

    #[test]
    fn test_same_column() {
        let tile1 = TileId::new(5, 3).unwrap();
        let tile2 = TileId::new(5, 7).unwrap();
        let tile3 = TileId::new(6, 3).unwrap();

        assert!(tile1.same_column(&tile2));
        assert!(!tile1.same_column(&tile3));
    }
}
