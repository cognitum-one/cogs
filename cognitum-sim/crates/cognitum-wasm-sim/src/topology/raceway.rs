//! RaceWay hierarchical mesh topology
//!
//! Native Cognitum NoC topology with:
//! - 256 tiles in 16x16 array
//! - 32 columns (8 tiles per column)
//! - 4 quadrants (64 tiles each)
//! - 2 hubs (North/South for cross-quadrant routing)

use super::Topology;
use crate::error::{Result, WasmSimError};
use crate::scale::ScaleConfig;

/// RaceWay topology implementation
pub struct RaceWayTopology {
    /// Number of tiles
    num_tiles: usize,

    /// Grid dimensions
    grid_width: usize,
    grid_height: usize,

    /// Tiles per column
    tiles_per_column: usize,

    /// Number of columns
    num_columns: usize,

    /// Number of quadrants
    num_quadrants: usize,

    /// Number of hubs
    num_hubs: usize,

    /// Lane bandwidth (bits)
    lane_width: usize,

    /// Lanes per column
    lanes_per_column: usize,
}

impl RaceWayTopology {
    /// Create RaceWay topology from scale configuration
    pub fn new(scale: &ScaleConfig) -> Result<Self> {
        let num_tiles = scale.total_tiles();

        // Calculate grid dimensions (prefer square-ish)
        let grid_width = (num_tiles as f64).sqrt().ceil() as usize;
        let grid_height = (num_tiles + grid_width - 1) / grid_width;

        // RaceWay standard: 8 tiles per column
        let tiles_per_column = if num_tiles >= 256 { 8 } else { 4.min(num_tiles) };
        let num_columns = (num_tiles + tiles_per_column - 1) / tiles_per_column;

        // 4 quadrants for 256 tiles, fewer for smaller scales
        let num_quadrants = if num_tiles >= 256 { 4 } else if num_tiles >= 64 { 2 } else { 1 };

        // 2 hubs for full scale, 1 for smaller
        let num_hubs = if num_tiles >= 256 { 2 } else { 1 };

        Ok(Self {
            num_tiles,
            grid_width,
            grid_height,
            tiles_per_column,
            num_columns,
            num_quadrants,
            num_hubs,
            lane_width: 96,        // 96 bits per lane
            lanes_per_column: 4,   // 4 lanes per column
        })
    }

    /// Get quadrant for a tile ID
    pub fn quadrant(&self, tile_id: u16) -> u8 {
        if self.num_quadrants == 1 {
            return 0;
        }

        let tile = tile_id as usize;
        let x = tile % self.grid_width;
        let y = tile / self.grid_width;

        let half_width = self.grid_width / 2;
        let half_height = self.grid_height / 2;

        match (x >= half_width, y >= half_height) {
            (false, false) => 0, // NW
            (true, false) => 1,  // NE
            (false, true) => 2,  // SW
            (true, true) => 3,   // SE
        }
    }

    /// Get column for a tile ID
    pub fn column(&self, tile_id: u16) -> u8 {
        ((tile_id as usize) % self.num_columns) as u8
    }

    /// Get hub for cross-quadrant routing
    pub fn hub_for_route(&self, src_quad: u8, dst_quad: u8) -> u8 {
        // North hub handles Q0/Q1, South hub handles Q2/Q3
        if src_quad < 2 && dst_quad < 2 {
            0 // North hub
        } else if src_quad >= 2 && dst_quad >= 2 {
            1 // South hub
        } else {
            // Cross-hub routing (both hubs involved)
            if src_quad < 2 { 0 } else { 1 }
        }
    }

    /// Calculate intra-column latency (cycles)
    pub fn intra_column_latency(&self) -> u64 {
        2 // 2 cycles within column
    }

    /// Calculate cross-column latency (cycles)
    pub fn cross_column_latency(&self) -> u64 {
        3 // 3 cycles between columns
    }

    /// Calculate cross-quadrant latency (cycles)
    pub fn cross_quadrant_latency(&self) -> u64 {
        5 // 5 cycles via hub
    }

    /// Calculate total latency between tiles
    pub fn latency(&self, src: u16, dst: u16) -> u64 {
        if src == dst {
            return 0;
        }

        let src_col = self.column(src);
        let dst_col = self.column(dst);
        let src_quad = self.quadrant(src);
        let dst_quad = self.quadrant(dst);

        if src_col == dst_col {
            // Same column
            self.intra_column_latency()
        } else if src_quad == dst_quad {
            // Same quadrant, different column
            self.cross_column_latency()
        } else {
            // Different quadrants - go through hub
            self.cross_quadrant_latency()
        }
    }
}

impl Topology for RaceWayTopology {
    fn name(&self) -> &str {
        "RaceWay"
    }

    fn node_count(&self) -> usize {
        self.num_tiles
    }

    fn base_latency_ns(&self) -> u64 {
        // At 1 GHz, 1 cycle = 1 ns
        // Average latency ~3 cycles
        3
    }

    fn bandwidth_gbps(&self) -> f64 {
        // Per-lane: 96 bits @ 1 GHz = 96 Gbps
        // 4 lanes per column = 384 Gbps per column
        // Total: 384 * 32 columns = 12.288 Tbps theoretical
        // Practical: ~6 Tbps with contention
        (self.lane_width * self.lanes_per_column * self.num_columns) as f64 / 1000.0
    }

    fn buffer_depth(&self) -> usize {
        4 // 4-deep FIFOs (matches hardware)
    }

    fn path_length(&self, src: u16, dst: u16) -> usize {
        if src == dst {
            return 0;
        }

        let src_quad = self.quadrant(src);
        let dst_quad = self.quadrant(dst);

        if src_quad == dst_quad {
            // Intra-quadrant: 1-2 hops
            2
        } else {
            // Cross-quadrant: 3-4 hops via hub
            4
        }
    }

    fn describe(&self) -> String {
        format!(
            "RaceWay Hierarchical Mesh\n\
             - Tiles: {} ({}x{})\n\
             - Columns: {} ({} tiles each)\n\
             - Quadrants: {}\n\
             - Hubs: {}\n\
             - Lane width: {} bits x {} lanes\n\
             - Bandwidth: {:.2} Tbps",
            self.num_tiles,
            self.grid_width,
            self.grid_height,
            self.num_columns,
            self.tiles_per_column,
            self.num_quadrants,
            self.num_hubs,
            self.lane_width,
            self.lanes_per_column,
            self.bandwidth_gbps() / 1000.0
        )
    }

    fn directly_connected(&self, src: u16, dst: u16) -> bool {
        // Check if in same column
        if self.column(src) == self.column(dst) {
            let src_pos = src as usize / self.num_columns;
            let dst_pos = dst as usize / self.num_columns;
            return (src_pos as i32 - dst_pos as i32).abs() <= 1;
        }

        // Check if adjacent columns in same row
        let src_col = self.column(src) as i32;
        let dst_col = self.column(dst) as i32;
        if (src_col - dst_col).abs() == 1 {
            let src_row = src as usize / self.grid_width;
            let dst_row = dst as usize / self.grid_width;
            return src_row == dst_row;
        }

        false
    }

    fn neighbors(&self, node: u16) -> Vec<u16> {
        let mut neighbors = Vec::new();
        let n = node as usize;
        let x = n % self.grid_width;
        let y = n / self.grid_width;

        // Right
        if x + 1 < self.grid_width && n + 1 < self.num_tiles {
            neighbors.push((n + 1) as u16);
        }

        // Left
        if x > 0 {
            neighbors.push((n - 1) as u16);
        }

        // Down
        if y + 1 < self.grid_height && n + self.grid_width < self.num_tiles {
            neighbors.push((n + self.grid_width) as u16);
        }

        // Up
        if y > 0 {
            neighbors.push((n - self.grid_width) as u16);
        }

        neighbors
    }

    fn bisection_bandwidth(&self) -> f64 {
        // Half of lanes cross the bisection
        (self.lane_width * self.lanes_per_column * self.num_columns / 2) as f64 / 1000.0
    }

    fn diameter(&self) -> usize {
        // Max path length in RaceWay
        if self.num_quadrants > 1 {
            6 // Worst case: corner to corner via hub
        } else {
            4 // Small scale
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raceway_creation() {
        let scale = ScaleConfig::from_tiles(256);
        let topo = RaceWayTopology::new(&scale).unwrap();

        assert_eq!(topo.node_count(), 256);
        assert_eq!(topo.num_quadrants, 4);
        assert_eq!(topo.num_hubs, 2);
    }

    #[test]
    fn test_quadrant_assignment() {
        let scale = ScaleConfig::from_tiles(256);
        let topo = RaceWayTopology::new(&scale).unwrap();

        // In 16x16 grid:
        // - Tile 0 at (0,0) -> NW quadrant (0)
        // - Tile 15 at (15,0) -> NE quadrant (1) since x=15 >= 8
        // - Tile 255 at (15,15) -> SE quadrant (3)
        assert_eq!(topo.quadrant(0), 0);   // NW
        assert_eq!(topo.quadrant(15), 1);  // NE (x=15 >= half_width=8)
        assert_eq!(topo.quadrant(255), 3); // SE
    }

    #[test]
    fn test_path_length() {
        let scale = ScaleConfig::from_tiles(256);
        let topo = RaceWayTopology::new(&scale).unwrap();

        // Same tile
        assert_eq!(topo.path_length(0, 0), 0);

        // Same quadrant
        assert_eq!(topo.path_length(0, 1), 2);
    }
}
