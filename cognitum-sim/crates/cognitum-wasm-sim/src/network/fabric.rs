//! Network fabric simulation
//!
//! Provides routing, buffering, and congestion simulation

use super::packet::{Packet, Priority, QoS};
use super::router::PacketRouter;
use super::stats::NetworkStats;
use crate::error::{Result, WasmSimError};

use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::cmp::Ordering;
use tokio::sync::mpsc;

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Number of nodes in network
    pub num_nodes: usize,

    /// Base latency in nanoseconds
    pub latency_ns: u64,

    /// Bandwidth in Gbps
    pub bandwidth_gbps: f64,

    /// Buffer depth per port
    pub buffer_depth: usize,

    /// Topology name
    pub topology_name: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            num_nodes: 256,
            latency_ns: 5,
            bandwidth_gbps: 100.0,
            buffer_depth: 8,
            topology_name: "default".into(),
        }
    }
}

/// Scheduled packet with delivery time
#[derive(Clone)]
struct ScheduledPacket {
    packet: Packet,
    delivery_cycle: u64,
}

impl Ord for ScheduledPacket {
    fn cmp(&self, other: &Self) -> Ordering {
        // Earlier delivery times have higher priority (min-heap)
        other.delivery_cycle.cmp(&self.delivery_cycle)
            .then_with(|| (other.packet.priority as u8).cmp(&(self.packet.priority as u8)))
    }
}

impl PartialOrd for ScheduledPacket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScheduledPacket {
    fn eq(&self, other: &Self) -> bool {
        self.delivery_cycle == other.delivery_cycle
    }
}

impl Eq for ScheduledPacket {}

/// Network fabric simulator
pub struct NetworkFabric {
    /// Configuration
    config: NetworkConfig,

    /// Current simulation cycle
    cycle: u64,

    /// In-flight packets (priority queue by delivery time)
    in_flight: BinaryHeap<ScheduledPacket>,

    /// Per-node receive buffers
    rx_buffers: HashMap<u16, VecDeque<Packet>>,

    /// Router for path computation
    router: PacketRouter,

    /// Statistics
    stats: NetworkStats,

    /// Congestion tracking (node -> backpressure level)
    congestion: HashMap<u16, u32>,

    /// Credit-based flow control (node -> available credits)
    credits: HashMap<u16, u32>,
}

impl NetworkFabric {
    /// Create new network fabric
    pub fn new(config: NetworkConfig) -> Result<Self> {
        let router = PacketRouter::new(config.num_nodes);

        let mut rx_buffers = HashMap::new();
        let mut credits = HashMap::new();

        for i in 0..config.num_nodes {
            rx_buffers.insert(i as u16, VecDeque::with_capacity(config.buffer_depth));
            credits.insert(i as u16, config.buffer_depth as u32);
        }

        Ok(Self {
            config,
            cycle: 0,
            in_flight: BinaryHeap::new(),
            rx_buffers,
            router,
            stats: NetworkStats::default(),
            congestion: HashMap::new(),
            credits,
        })
    }

    /// Route a packet through the network
    pub async fn route_packet(&mut self, packet: Packet) -> Result<()> {
        // Check destination validity
        if packet.destination as usize >= self.config.num_nodes && !packet.is_broadcast() {
            return Err(WasmSimError::NetworkError(format!(
                "Invalid destination: {}",
                packet.destination
            )));
        }

        // Check credit availability
        let credits = self.credits.get(&packet.destination).copied().unwrap_or(0);
        if credits == 0 && !packet.is_broadcast() {
            self.stats.dropped_packets += 1;
            return Ok(()); // Drop packet due to congestion
        }

        // Compute route and latency
        let (hops, base_latency) = if packet.is_broadcast() {
            (1, self.config.latency_ns * 2) // Broadcast has higher latency
        } else {
            let path = self.router.compute_path(packet.source, packet.destination);
            let latency = path.len() as u64 * self.config.latency_ns;
            (path.len(), latency)
        };

        // Apply QoS-based latency adjustment
        let qos_factor = match packet.qos {
            QoS::RealTime => 0.5,
            QoS::LowLatency => 0.75,
            QoS::BestEffort => 1.0,
            QoS::HighThroughput => 1.25,
            QoS::Reliable => 1.5,
        };

        let adjusted_latency = (base_latency as f64 * qos_factor) as u64;
        let delivery_cycle = self.cycle + adjusted_latency.max(1);

        // Schedule packet delivery
        let mut scheduled_packet = packet.clone();
        scheduled_packet.hops = hops as u8;

        self.in_flight.push(ScheduledPacket {
            packet: scheduled_packet,
            delivery_cycle,
        });

        // Update statistics
        self.stats.packets_sent += 1;
        self.stats.total_hops += hops as u64;
        self.stats.total_latency_ns += adjusted_latency;

        // Consume credit
        if !packet.is_broadcast() {
            if let Some(c) = self.credits.get_mut(&packet.destination) {
                *c = c.saturating_sub(1);
            }
        }

        Ok(())
    }

    /// Process one simulation tick
    pub async fn tick(&mut self) -> Result<()> {
        self.cycle += 1;

        // Deliver packets whose delivery time has arrived
        while let Some(scheduled) = self.in_flight.peek() {
            if scheduled.delivery_cycle > self.cycle {
                break;
            }

            let scheduled = self.in_flight.pop().unwrap();
            let packet = scheduled.packet;

            if packet.is_broadcast() {
                // Deliver to all nodes
                for i in 0..self.config.num_nodes {
                    self.deliver_to_node(i as u16, packet.clone())?;
                }
                self.stats.broadcast_packets += 1;
            } else {
                self.deliver_to_node(packet.destination, packet)?;
            }
        }

        // Refresh credits periodically
        if self.cycle % 10 == 0 {
            for (_, credits) in self.credits.iter_mut() {
                *credits = (*credits + 1).min(self.config.buffer_depth as u32);
            }
        }

        Ok(())
    }

    /// Deliver packet to a specific node
    fn deliver_to_node(&mut self, node: u16, packet: Packet) -> Result<()> {
        let buffer = self.rx_buffers.entry(node).or_insert_with(|| {
            VecDeque::with_capacity(self.config.buffer_depth)
        });

        if buffer.len() >= self.config.buffer_depth {
            // Buffer full - apply backpressure
            *self.congestion.entry(node).or_insert(0) += 1;
            self.stats.dropped_packets += 1;
        } else {
            buffer.push_back(packet);
            self.stats.packets_received += 1;

            // Return credit
            if let Some(c) = self.credits.get_mut(&node) {
                *c = (*c + 1).min(self.config.buffer_depth as u32);
            }
        }

        Ok(())
    }

    /// Receive packet for a node (non-blocking)
    pub fn receive(&mut self, node: u16) -> Option<Packet> {
        self.rx_buffers.get_mut(&node).and_then(|buf| buf.pop_front())
    }

    /// Check if packets are pending for a node
    pub fn has_pending(&self, node: u16) -> bool {
        self.rx_buffers.get(&node).map(|buf| !buf.is_empty()).unwrap_or(false)
    }

    /// Get current cycle
    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    /// Get statistics
    pub async fn stats(&self) -> NetworkStats {
        let mut stats = self.stats.clone();

        // Calculate averages
        if stats.packets_received > 0 {
            stats.avg_latency_ns = stats.total_latency_ns as f64 / stats.packets_sent as f64;
            stats.avg_hops = stats.total_hops as f64 / stats.packets_sent as f64;
        }

        // Calculate throughput (packets per cycle * cycle rate * packet size)
        let packet_size_bits = 128.0 * 8.0; // 128 bytes * 8 bits
        let packets_per_ns = stats.packets_received as f64 / (self.cycle as f64 + 1.0);
        stats.throughput_gbps = packets_per_ns * packet_size_bits / 1e9;

        stats
    }

    /// Get congestion level for a node
    pub fn congestion_level(&self, node: u16) -> u32 {
        self.congestion.get(&node).copied().unwrap_or(0)
    }

    /// Get in-flight packet count
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = NetworkStats::default();
        self.congestion.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fabric_creation() {
        let fabric = NetworkFabric::new(NetworkConfig::default()).unwrap();
        assert_eq!(fabric.cycle(), 0);
    }

    #[tokio::test]
    async fn test_packet_routing() {
        let mut fabric = NetworkFabric::new(NetworkConfig {
            num_nodes: 16,
            latency_ns: 5,
            bandwidth_gbps: 100.0,
            buffer_depth: 8,
            topology_name: "test".into(),
        }).unwrap();

        let packet = Packet::write(0, 1, 0x1000, 0xDEADBEEF);
        fabric.route_packet(packet).await.unwrap();

        // Tick enough times for delivery
        for _ in 0..10 {
            fabric.tick().await.unwrap();
        }

        let received = fabric.receive(1);
        assert!(received.is_some());
    }

    #[tokio::test]
    async fn test_broadcast() {
        let mut fabric = NetworkFabric::new(NetworkConfig {
            num_nodes: 4,
            latency_ns: 1,
            bandwidth_gbps: 100.0,
            buffer_depth: 8,
            topology_name: "test".into(),
        }).unwrap();

        let packet = Packet::broadcast(0, &[1, 2, 3, 4]);
        fabric.route_packet(packet).await.unwrap();

        for _ in 0..10 {
            fabric.tick().await.unwrap();
        }

        // All nodes should receive
        for i in 0..4 {
            assert!(fabric.has_pending(i));
        }
    }
}
