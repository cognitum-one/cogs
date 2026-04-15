//! Network statistics tracking

use serde::{Deserialize, Serialize};

/// Network performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Total packets sent
    pub packets_sent: u64,

    /// Total packets received
    pub packets_received: u64,

    /// Dropped packets (congestion)
    pub dropped_packets: u64,

    /// Broadcast packets sent
    pub broadcast_packets: u64,

    /// Total hop count (for average calculation)
    pub total_hops: u64,

    /// Total latency in nanoseconds
    pub total_latency_ns: u64,

    /// Average latency per packet
    pub avg_latency_ns: f64,

    /// Average hops per packet
    pub avg_hops: f64,

    /// Network throughput in Gbps
    pub throughput_gbps: f64,

    /// Peak in-flight packets
    pub peak_in_flight: u64,

    /// Congestion events
    pub congestion_events: u64,
}

impl NetworkStats {
    /// Calculate packet loss rate
    pub fn loss_rate(&self) -> f64 {
        if self.packets_sent == 0 {
            0.0
        } else {
            self.dropped_packets as f64 / self.packets_sent as f64
        }
    }

    /// Calculate delivery rate
    pub fn delivery_rate(&self) -> f64 {
        if self.packets_sent == 0 {
            1.0
        } else {
            self.packets_received as f64 / self.packets_sent as f64
        }
    }

    /// Merge statistics from another instance
    pub fn merge(&mut self, other: &NetworkStats) {
        self.packets_sent += other.packets_sent;
        self.packets_received += other.packets_received;
        self.dropped_packets += other.dropped_packets;
        self.broadcast_packets += other.broadcast_packets;
        self.total_hops += other.total_hops;
        self.total_latency_ns += other.total_latency_ns;
        self.peak_in_flight = self.peak_in_flight.max(other.peak_in_flight);
        self.congestion_events += other.congestion_events;

        // Recalculate averages
        if self.packets_received > 0 {
            self.avg_latency_ns = self.total_latency_ns as f64 / self.packets_sent as f64;
            self.avg_hops = self.total_hops as f64 / self.packets_sent as f64;
        }
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Format as summary string
    pub fn summary(&self) -> String {
        format!(
            "Network Stats:\n\
             - Packets: {} sent, {} received, {} dropped ({:.2}% loss)\n\
             - Latency: {:.2} ns avg, {} total hops ({:.2} avg)\n\
             - Throughput: {:.2} Gbps\n\
             - Congestion events: {}",
            self.packets_sent,
            self.packets_received,
            self.dropped_packets,
            self.loss_rate() * 100.0,
            self.avg_latency_ns,
            self.total_hops,
            self.avg_hops,
            self.throughput_gbps,
            self.congestion_events,
        )
    }
}

/// Per-node statistics
#[derive(Debug, Clone, Default)]
pub struct NodeStats {
    /// Node ID
    pub node_id: u16,

    /// Packets sent from this node
    pub packets_sent: u64,

    /// Packets received by this node
    pub packets_received: u64,

    /// Bytes sent
    pub bytes_sent: u64,

    /// Bytes received
    pub bytes_received: u64,

    /// Buffer utilization (0.0 - 1.0)
    pub buffer_utilization: f64,

    /// Congestion level
    pub congestion_level: u32,
}

impl NodeStats {
    pub fn new(node_id: u16) -> Self {
        Self {
            node_id,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_default() {
        let stats = NetworkStats::default();
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.loss_rate(), 0.0);
        assert_eq!(stats.delivery_rate(), 1.0);
    }

    #[test]
    fn test_loss_rate() {
        let mut stats = NetworkStats::default();
        stats.packets_sent = 100;
        stats.dropped_packets = 10;

        assert_eq!(stats.loss_rate(), 0.1);
    }

    #[test]
    fn test_merge() {
        let mut stats1 = NetworkStats::default();
        stats1.packets_sent = 50;
        stats1.packets_received = 48;

        let mut stats2 = NetworkStats::default();
        stats2.packets_sent = 50;
        stats2.packets_received = 49;

        stats1.merge(&stats2);

        assert_eq!(stats1.packets_sent, 100);
        assert_eq!(stats1.packets_received, 97);
    }
}
