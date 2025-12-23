//! Cognitum WASM Simulation Engine
//!
//! Provides realistic simulation of the 8KB WASM execution system with:
//! - Multiple scale configurations (1 to 1024+ tiles)
//! - Enterprise network topologies (Arista leaf-spine, Nutanix hyperconverged)
//! - Full WASM MVP instruction set simulation
//! - SIMD v128 operations
//! - Custom neural extensions
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    WASM Simulation Engine                    │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Scale Configurations                                        │
//! │  ├── Development (1 tile)                                   │
//! │  ├── Small (16 tiles)                                       │
//! │  ├── Medium (64 tiles)                                      │
//! │  ├── Large (256 tiles)                                      │
//! │  └── Enterprise (1024+ tiles, multi-chip)                   │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Network Topologies                                          │
//! │  ├── RaceWay (hierarchical mesh NoC)                        │
//! │  ├── LeafSpine (Arista-style datacenter CLOS)               │
//! │  └── Hyperconverged (Nutanix-style distributed)             │
//! ├─────────────────────────────────────────────────────────────┤
//! │  WASM Engine Components                                      │
//! │  ├── Decoder (bytecode → internal ops)                      │
//! │  ├── Executor (instruction execution)                       │
//! │  ├── Stack (register-mapped with spill/fill)                │
//! │  ├── Memory (8KB code + 8KB data + bounds checking)         │
//! │  └── SIMD (v128 vector operations)                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod error;
pub mod wasm;
pub mod network;
pub mod topology;
pub mod scale;

// Re-exports
pub use error::{WasmSimError, Result};
pub use wasm::{WasmEngine, WasmConfig, WasmTile};
pub use network::{NetworkFabric, NetworkConfig, PacketRouter};
pub use topology::{Topology, TopologyKind, LeafSpineConfig, HyperconvergedConfig};
pub use scale::{ScaleConfig, ScaleLevel, ClusterConfig};

/// Version identifier for v1 architecture compatibility
pub const WASM_SIM_VERSION: &str = "1.0.0";

/// Memory configuration matching hardware spec
pub mod memory_spec {
    /// Code memory size (8KB = 4K × 16-bit instructions)
    pub const CODE_MEMORY_SIZE: usize = 8 * 1024;

    /// Data memory size (8KB = 2K × 32-bit words)
    pub const DATA_MEMORY_SIZE: usize = 8 * 1024;

    /// Work memory size (64KB = 16K × 32-bit words)
    pub const WORK_MEMORY_SIZE: usize = 64 * 1024;

    /// Total memory per tile
    pub const TOTAL_TILE_MEMORY: usize = CODE_MEMORY_SIZE + DATA_MEMORY_SIZE + WORK_MEMORY_SIZE;

    /// WASM page size (64KB standard)
    pub const WASM_PAGE_SIZE: usize = 64 * 1024;
}

/// Main simulation coordinator
pub struct WasmSimulator {
    /// Scale configuration
    scale: ScaleConfig,

    /// Network topology
    topology: Box<dyn Topology + Send + Sync>,

    /// WASM tiles
    tiles: Vec<WasmTile>,

    /// Network fabric
    network: NetworkFabric,

    /// Simulation statistics
    stats: SimulationStats,

    /// Running state
    running: std::sync::atomic::AtomicBool,
}

/// Simulation statistics
#[derive(Debug, Clone, Default)]
pub struct SimulationStats {
    pub total_instructions: u64,
    pub total_cycles: u64,
    pub total_packets: u64,
    pub network_latency_avg_ns: f64,
    pub network_throughput_gbps: f64,
    pub memory_operations: u64,
    pub simd_operations: u64,
    pub wasm_traps: u64,
}

impl WasmSimulator {
    /// Create a new WASM simulator with specified configuration
    pub fn new(scale: ScaleConfig, topology_kind: TopologyKind) -> Result<Self> {
        let topology: Box<dyn Topology + Send + Sync> = match topology_kind {
            TopologyKind::RaceWay => Box::new(topology::RaceWayTopology::new(&scale)?),
            TopologyKind::LeafSpine(config) => Box::new(topology::LeafSpineTopology::new(&scale, config)?),
            TopologyKind::Hyperconverged(config) => Box::new(topology::HyperconvergedTopology::new(&scale, config)?),
        };

        let network_config = NetworkConfig::from_topology(&*topology);
        let network = NetworkFabric::new(network_config)?;

        let mut tiles = Vec::with_capacity(scale.total_tiles());
        for tile_id in 0..scale.total_tiles() {
            tiles.push(WasmTile::new(tile_id as u16, WasmConfig::default())?);
        }

        Ok(Self {
            scale,
            topology,
            tiles,
            network,
            stats: SimulationStats::default(),
            running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Create with preset scale level
    pub fn with_scale(level: ScaleLevel) -> Result<Self> {
        let scale = ScaleConfig::from_level(level);
        let topology = match level {
            ScaleLevel::Development | ScaleLevel::Small => TopologyKind::RaceWay,
            ScaleLevel::Medium | ScaleLevel::Large => TopologyKind::LeafSpine(LeafSpineConfig::default()),
            ScaleLevel::Enterprise => TopologyKind::Hyperconverged(HyperconvergedConfig::default()),
        };
        Self::new(scale, topology)
    }

    /// Get number of tiles
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Load WASM bytecode into a specific tile
    pub fn load_wasm(&mut self, tile_id: u16, bytecode: &[u8]) -> Result<()> {
        let tile = self.tiles.get_mut(tile_id as usize)
            .ok_or(WasmSimError::InvalidTileId(tile_id))?;
        tile.load_bytecode(bytecode)
    }

    /// Run simulation for specified cycles
    pub async fn run_cycles(&mut self, cycles: u64) -> Result<SimulationStats> {
        use std::sync::atomic::Ordering;

        self.running.store(true, Ordering::SeqCst);

        for _cycle in 0..cycles {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            // Execute one cycle on all tiles
            for tile in &mut self.tiles {
                if let Some(packet) = tile.step()? {
                    self.network.route_packet(packet).await?;
                    self.stats.total_packets += 1;
                }
                self.stats.total_instructions += tile.instructions_this_cycle() as u64;
            }

            // Process network
            self.network.tick().await?;

            self.stats.total_cycles += 1;
        }

        self.running.store(false, Ordering::SeqCst);
        self.update_network_stats().await;

        Ok(self.stats.clone())
    }

    /// Get current statistics
    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    /// Get topology description
    pub fn topology_info(&self) -> String {
        self.topology.describe()
    }

    /// Get scale info
    pub fn scale_info(&self) -> &ScaleConfig {
        &self.scale
    }

    async fn update_network_stats(&mut self) {
        let net_stats = self.network.stats().await;
        self.stats.network_latency_avg_ns = net_stats.avg_latency_ns;
        self.stats.network_throughput_gbps = net_stats.throughput_gbps;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_creation() {
        let sim = WasmSimulator::with_scale(ScaleLevel::Development).unwrap();
        assert_eq!(sim.tile_count(), 1);
    }

    #[test]
    fn test_small_scale() {
        let sim = WasmSimulator::with_scale(ScaleLevel::Small).unwrap();
        assert_eq!(sim.tile_count(), 16);
    }

    #[test]
    fn test_large_scale() {
        let sim = WasmSimulator::with_scale(ScaleLevel::Large).unwrap();
        assert_eq!(sim.tile_count(), 256);
    }
}
