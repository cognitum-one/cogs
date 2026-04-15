//! Scale configuration for simulation
//!
//! Supports configurations from single-tile development to
//! multi-chip enterprise deployments (1000+ tiles)

use serde::{Deserialize, Serialize};

/// Scale levels for quick configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleLevel {
    /// Single tile for development/testing
    Development,

    /// 16 tiles (small cluster)
    Small,

    /// 64 tiles (medium cluster)
    Medium,

    /// 256 tiles (full Cognitum chip)
    Large,

    /// 1024+ tiles (multi-chip enterprise)
    Enterprise,
}

impl Default for ScaleLevel {
    fn default() -> Self {
        ScaleLevel::Large
    }
}

/// Detailed scale configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleConfig {
    /// Number of chips
    pub num_chips: usize,

    /// Tiles per chip
    pub tiles_per_chip: usize,

    /// Memory per tile (KB)
    pub memory_per_tile_kb: usize,

    /// Clock frequency (MHz)
    pub clock_mhz: u32,

    /// Enable multi-chip interconnect
    pub multi_chip_enabled: bool,

    /// Inter-chip bandwidth (Gbps)
    pub inter_chip_bandwidth_gbps: f64,
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self::from_level(ScaleLevel::Large)
    }
}

impl ScaleConfig {
    /// Create configuration from scale level
    pub fn from_level(level: ScaleLevel) -> Self {
        match level {
            ScaleLevel::Development => Self {
                num_chips: 1,
                tiles_per_chip: 1,
                memory_per_tile_kb: 80, // 8KB code + 8KB data + 64KB work
                clock_mhz: 1000,
                multi_chip_enabled: false,
                inter_chip_bandwidth_gbps: 0.0,
            },
            ScaleLevel::Small => Self {
                num_chips: 1,
                tiles_per_chip: 16,
                memory_per_tile_kb: 80,
                clock_mhz: 1000,
                multi_chip_enabled: false,
                inter_chip_bandwidth_gbps: 0.0,
            },
            ScaleLevel::Medium => Self {
                num_chips: 1,
                tiles_per_chip: 64,
                memory_per_tile_kb: 80,
                clock_mhz: 1000,
                multi_chip_enabled: false,
                inter_chip_bandwidth_gbps: 0.0,
            },
            ScaleLevel::Large => Self {
                num_chips: 1,
                tiles_per_chip: 256,
                memory_per_tile_kb: 80,
                clock_mhz: 1000,
                multi_chip_enabled: false,
                inter_chip_bandwidth_gbps: 0.0,
            },
            ScaleLevel::Enterprise => Self {
                num_chips: 4,
                tiles_per_chip: 256,
                memory_per_tile_kb: 80,
                clock_mhz: 1000,
                multi_chip_enabled: true,
                inter_chip_bandwidth_gbps: 400.0, // 400G inter-chip links
            },
        }
    }

    /// Create configuration for specific tile count
    pub fn from_tiles(num_tiles: usize) -> Self {
        let level = if num_tiles <= 1 {
            ScaleLevel::Development
        } else if num_tiles <= 16 {
            ScaleLevel::Small
        } else if num_tiles <= 64 {
            ScaleLevel::Medium
        } else if num_tiles <= 256 {
            ScaleLevel::Large
        } else {
            ScaleLevel::Enterprise
        };

        let mut config = Self::from_level(level);

        // Adjust for exact tile count
        if num_tiles <= 256 {
            config.tiles_per_chip = num_tiles;
        } else {
            config.num_chips = (num_tiles + 255) / 256;
            config.tiles_per_chip = 256;
            config.multi_chip_enabled = true;
        }

        config
    }

    /// Get total tile count
    pub fn total_tiles(&self) -> usize {
        self.num_chips * self.tiles_per_chip
    }

    /// Get total memory in MB
    pub fn total_memory_mb(&self) -> usize {
        self.total_tiles() * self.memory_per_tile_kb / 1024
    }

    /// Get aggregate compute power (estimated GOPS)
    pub fn compute_gops(&self) -> f64 {
        // Each tile can do ~1 GOP/s at 1 GHz
        // With SIMD: 4x for vector ops
        self.total_tiles() as f64 * (self.clock_mhz as f64 / 1000.0)
    }

    /// Get scale level
    pub fn level(&self) -> ScaleLevel {
        let total = self.total_tiles();
        if total <= 1 {
            ScaleLevel::Development
        } else if total <= 16 {
            ScaleLevel::Small
        } else if total <= 64 {
            ScaleLevel::Medium
        } else if total <= 256 {
            ScaleLevel::Large
        } else {
            ScaleLevel::Enterprise
        }
    }

    /// Get description
    pub fn describe(&self) -> String {
        format!(
            "Scale Configuration: {:?}\n\
             - Chips: {}\n\
             - Tiles per chip: {}\n\
             - Total tiles: {}\n\
             - Memory per tile: {} KB\n\
             - Total memory: {} MB\n\
             - Clock: {} MHz\n\
             - Compute: {:.2} GOPS\n\
             - Multi-chip: {}{}",
            self.level(),
            self.num_chips,
            self.tiles_per_chip,
            self.total_tiles(),
            self.memory_per_tile_kb,
            self.total_memory_mb(),
            self.clock_mhz,
            self.compute_gops(),
            if self.multi_chip_enabled { "enabled" } else { "disabled" },
            if self.multi_chip_enabled {
                format!(" ({:.0}G inter-chip)", self.inter_chip_bandwidth_gbps)
            } else {
                String::new()
            }
        )
    }
}

/// Cluster configuration for distributed deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Number of nodes in cluster
    pub num_nodes: usize,

    /// Scale config per node
    pub node_config: ScaleConfig,

    /// Network type (Ethernet, InfiniBand, etc.)
    pub network_type: NetworkType,

    /// Network bandwidth (Gbps)
    pub network_bandwidth_gbps: f64,

    /// Network latency (microseconds)
    pub network_latency_us: f64,
}

/// Network interconnect type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkType {
    /// Standard Ethernet
    Ethernet,

    /// InfiniBand
    InfiniBand,

    /// RDMA over Converged Ethernet
    RoCE,

    /// Custom high-speed interconnect
    Custom,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            num_nodes: 4,
            node_config: ScaleConfig::from_level(ScaleLevel::Large),
            network_type: NetworkType::Ethernet,
            network_bandwidth_gbps: 100.0,
            network_latency_us: 10.0,
        }
    }
}

impl ClusterConfig {
    /// Get total tiles in cluster
    pub fn total_tiles(&self) -> usize {
        self.num_nodes * self.node_config.total_tiles()
    }

    /// Get total memory in GB
    pub fn total_memory_gb(&self) -> usize {
        self.num_nodes * self.node_config.total_memory_mb() / 1024
    }

    /// Get aggregate compute power
    pub fn compute_tops(&self) -> f64 {
        self.num_nodes as f64 * self.node_config.compute_gops() / 1000.0
    }

    /// Describe cluster
    pub fn describe(&self) -> String {
        format!(
            "Cluster Configuration:\n\
             - Nodes: {}\n\
             - Tiles per node: {}\n\
             - Total tiles: {}\n\
             - Total memory: {} GB\n\
             - Aggregate compute: {:.2} TOPS\n\
             - Network: {:?} @ {:.0}G, {:.1}μs latency",
            self.num_nodes,
            self.node_config.total_tiles(),
            self.total_tiles(),
            self.total_memory_gb(),
            self.compute_tops(),
            self.network_type,
            self.network_bandwidth_gbps,
            self.network_latency_us
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_levels() {
        assert_eq!(ScaleConfig::from_level(ScaleLevel::Development).total_tiles(), 1);
        assert_eq!(ScaleConfig::from_level(ScaleLevel::Small).total_tiles(), 16);
        assert_eq!(ScaleConfig::from_level(ScaleLevel::Medium).total_tiles(), 64);
        assert_eq!(ScaleConfig::from_level(ScaleLevel::Large).total_tiles(), 256);
        assert_eq!(ScaleConfig::from_level(ScaleLevel::Enterprise).total_tiles(), 1024);
    }

    #[test]
    fn test_from_tiles() {
        let config = ScaleConfig::from_tiles(128);
        assert_eq!(config.total_tiles(), 128);
        // 128 > 64, so it's Large scale (65-256 range)
        assert_eq!(config.level(), ScaleLevel::Large);
    }

    #[test]
    fn test_multi_chip() {
        let config = ScaleConfig::from_tiles(512);
        assert!(config.multi_chip_enabled);
        assert_eq!(config.num_chips, 2);
    }

    #[test]
    fn test_memory_calculation() {
        let config = ScaleConfig::from_level(ScaleLevel::Large);
        // 256 tiles * 80 KB = 20480 KB = 20 MB
        assert_eq!(config.total_memory_mb(), 20);
    }

    #[test]
    fn test_cluster_config() {
        let cluster = ClusterConfig::default();
        // 4 nodes * 256 tiles = 1024 tiles
        assert_eq!(cluster.total_tiles(), 1024);
    }
}
