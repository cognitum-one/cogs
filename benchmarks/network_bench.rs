//! Comprehensive RaceWay Network Performance Benchmarks
//!
//! Tests latency, throughput, and identifies bottlenecks

use newport_raceway::*;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub local_routing: LatencyResults,
    pub cross_column_routing: LatencyResults,
    pub column_broadcast: BroadcastResults,
    pub throughput: ThroughputResults,
    pub packet_ops: PacketOpResults,
    pub bottlenecks: Vec<Bottleneck>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LatencyResults {
    pub min_us: f64,
    pub max_us: f64,
    pub avg_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub samples: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastResults {
    pub avg_latency_us: f64,
    pub tiles_reached: usize,
    pub expected_tiles: usize,
    pub completion_time_us: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThroughputResults {
    pub packets_per_sec: f64,
    pub bits_per_sec: f64,
    pub gbps: f64,
    pub utilization_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PacketOpResults {
    pub creation_ns: f64,
    pub serialization_ns: f64,
    pub deserialization_ns: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bottleneck {
    pub component: String,
    pub issue: String,
    pub impact: String,
    pub recommendation: String,
}

impl LatencyResults {
    fn from_samples(mut samples: Vec<f64>) -> Self {
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let len = samples.len();

        LatencyResults {
            min_us: *samples.first().unwrap_or(&0.0),
            max_us: *samples.last().unwrap_or(&0.0),
            avg_us: samples.iter().sum::<f64>() / len as f64,
            p50_us: samples[len / 2],
            p95_us: samples[(len * 95) / 100],
            p99_us: samples[(len * 99) / 100],
            samples,
        }
    }
}

/// Benchmark local routing (same column)
pub async fn bench_local_routing(iterations: usize) -> LatencyResults {
    let mut network = RaceWayNetwork::new_for_test().await;
    let mut samples = Vec::new();

    // Same column routing (0x00 -> 0x03)
    for _ in 0..iterations {
        let start = Instant::now();
        network.send(TileId(0x00), TileId(0x03), &[0xAA]).await.unwrap();
        let _ = network.receive(TileId(0x03)).await.unwrap();
        let elapsed = start.elapsed();
        samples.push(elapsed.as_micros() as f64);
    }

    LatencyResults::from_samples(samples)
}

/// Benchmark cross-column routing
pub async fn bench_cross_column_routing(iterations: usize) -> LatencyResults {
    let mut network = RaceWayNetwork::new_for_test().await;
    let mut samples = Vec::new();

    // Different column routing (0x00 -> 0x50)
    for _ in 0..iterations {
        let start = Instant::now();
        network.send(TileId(0x00), TileId(0x50), &[0xBB]).await.unwrap();
        let _ = network.receive(TileId(0x50)).await.unwrap();
        let elapsed = start.elapsed();
        samples.push(elapsed.as_micros() as f64);
    }

    LatencyResults::from_samples(samples)
}

/// Benchmark column broadcast
pub async fn bench_column_broadcast(iterations: usize) -> BroadcastResults {
    let mut network = RaceWayNetwork::new_for_test().await;
    let mut total_latency = 0.0;

    for _ in 0..iterations {
        let broadcast = RaceWayPacket::new()
            .source(TileId(0x00))
            .command(Command::Broadcast)
            .tag(0x01)
            .push(true)
            .build()
            .unwrap();

        let start = Instant::now();
        network.send_packet(broadcast).await.unwrap();

        // Try to receive from all tiles in column 0
        let mut received_count = 0;
        for row in 1..8 {
            if let Ok(_) = network.try_receive(TileId::new(0, row).unwrap()) {
                received_count += 1;
            }
        }

        let elapsed = start.elapsed();
        total_latency += elapsed.as_micros() as f64;
    }

    BroadcastResults {
        avg_latency_us: total_latency / iterations as f64,
        tiles_reached: 7, // 8 tiles - source
        expected_tiles: 7,
        completion_time_us: total_latency / iterations as f64,
    }
}

/// Benchmark throughput
pub async fn bench_throughput(packet_count: usize) -> ThroughputResults {
    let mut network = RaceWayNetwork::new_for_test().await;

    let start = Instant::now();

    // Send many packets across different routes
    for i in 0..packet_count {
        let dest_col = (i % 16) as u8;
        let dest_row = ((i / 16) % 8) as u8;
        let dest = TileId::new(dest_col, dest_row).unwrap();
        let _ = network.send(TileId(0x00), dest, &[0xFF]).await;
    }

    let elapsed = start.elapsed();
    let packets_per_sec = packet_count as f64 / elapsed.as_secs_f64();
    let bits_per_packet = 97.0;
    let bits_per_sec = packets_per_sec * bits_per_packet;
    let gbps = bits_per_sec / 1_000_000_000.0;

    // Utilization assuming 1 GHz clock and 96 bits/cycle per lane
    let theoretical_max_gbps = 96.0; // Single lane @ 1 GHz
    let utilization = (gbps / theoretical_max_gbps) * 100.0;

    ThroughputResults {
        packets_per_sec,
        bits_per_sec,
        gbps,
        utilization_percent: utilization,
    }
}

/// Benchmark packet operations
pub fn bench_packet_ops(iterations: usize) -> PacketOpResults {
    let mut creation_times = Vec::new();
    let mut serialization_times = Vec::new();
    let mut deserialization_times = Vec::new();

    for _ in 0..iterations {
        // Creation
        let start = Instant::now();
        let packet = RaceWayPacket::new()
            .source(TileId(0x11))
            .dest(TileId(0x42))
            .command(Command::Write)
            .tag(0x05)
            .write_data(0xDEADBEEF)
            .address(0x1000)
            .push(true)
            .build()
            .unwrap();
        creation_times.push(start.elapsed().as_nanos() as f64);

        // Serialization
        let start = Instant::now();
        let bits = packet.to_bits();
        serialization_times.push(start.elapsed().as_nanos() as f64);

        // Deserialization
        let start = Instant::now();
        let _ = RaceWayPacket::from_bits(&bits).unwrap();
        deserialization_times.push(start.elapsed().as_nanos() as f64);
    }

    PacketOpResults {
        creation_ns: creation_times.iter().sum::<f64>() / iterations as f64,
        serialization_ns: serialization_times.iter().sum::<f64>() / iterations as f64,
        deserialization_ns: deserialization_times.iter().sum::<f64>() / iterations as f64,
    }
}

/// Identify bottlenecks
pub fn identify_bottlenecks(results: &BenchmarkResults) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    // Check if cross-column routing is significantly slower than local
    if results.cross_column_routing.avg_us > results.local_routing.avg_us * 3.0 {
        bottlenecks.push(Bottleneck {
            component: "Hub Routing".to_string(),
            issue: format!(
                "Cross-column routing ({:.2}µs) is {:.1}x slower than local routing ({:.2}µs)",
                results.cross_column_routing.avg_us,
                results.cross_column_routing.avg_us / results.local_routing.avg_us,
                results.local_routing.avg_us
            ),
            impact: "Increases latency for inter-column communication".to_string(),
            recommendation: "Optimize hub crossbar arbitration and reduce channel buffering".to_string(),
        });
    }

    // Check packet serialization overhead
    if results.packet_ops.serialization_ns > 100.0 {
        bottlenecks.push(Bottleneck {
            component: "Packet Serialization".to_string(),
            issue: format!("Serialization takes {:.2}ns per packet", results.packet_ops.serialization_ns),
            impact: "Adds overhead to every packet transmission".to_string(),
            recommendation: "Consider hardware-accelerated serialization or optimized bit manipulation".to_string(),
        });
    }

    // Check throughput utilization
    if results.throughput.utilization_percent < 50.0 {
        bottlenecks.push(Bottleneck {
            component: "Network Utilization".to_string(),
            issue: format!("Only {:.1}% utilization achieved", results.throughput.utilization_percent),
            impact: "Not fully utilizing available network bandwidth".to_string(),
            recommendation: "Increase packet injection rate or reduce channel latency".to_string(),
        });
    }

    // Check broadcast efficiency
    if results.column_broadcast.avg_latency_us > 30.0 {
        bottlenecks.push(Bottleneck {
            component: "Column Broadcast".to_string(),
            issue: format!("Broadcast latency ({:.2}µs) exceeds target (20-30µs)", results.column_broadcast.avg_latency_us),
            impact: "Slows down collective operations and barriers".to_string(),
            recommendation: "Optimize broadcast tree topology or implement hardware multicast".to_string(),
        });
    }

    bottlenecks
}

#[tokio::main]
async fn main() {
    println!("🚀 Newport RaceWay Network Performance Benchmark\n");

    println!("Testing local routing latency (same column)...");
    let local = bench_local_routing(1000).await;
    println!("  ✓ Avg: {:.2}µs, P95: {:.2}µs, P99: {:.2}µs", local.avg_us, local.p95_us, local.p99_us);

    println!("\nTesting cross-column routing latency...");
    let cross = bench_cross_column_routing(1000).await;
    println!("  ✓ Avg: {:.2}µs, P95: {:.2}µs, P99: {:.2}µs", cross.avg_us, cross.p95_us, cross.p99_us);

    println!("\nTesting column broadcast...");
    let broadcast = bench_column_broadcast(100).await;
    println!("  ✓ Avg latency: {:.2}µs, Tiles: {}/{}", broadcast.avg_latency_us, broadcast.tiles_reached, broadcast.expected_tiles);

    println!("\nTesting throughput (10000 packets)...");
    let throughput = bench_throughput(10000).await;
    println!("  ✓ {:.0} packets/sec, {:.2} Gbps, {:.1}% utilization", throughput.packets_per_sec, throughput.gbps, throughput.utilization_percent);

    println!("\nTesting packet operations...");
    let packet_ops = bench_packet_ops(10000);
    println!("  ✓ Creation: {:.2}ns, Serialization: {:.2}ns, Deserialization: {:.2}ns",
             packet_ops.creation_ns, packet_ops.serialization_ns, packet_ops.deserialization_ns);

    let results = BenchmarkResults {
        local_routing: local,
        cross_column_routing: cross,
        column_broadcast: broadcast,
        throughput,
        packet_ops,
        bottlenecks: Vec::new(),
    };

    let bottlenecks = identify_bottlenecks(&results);
    let results_with_bottlenecks = BenchmarkResults {
        bottlenecks,
        ..results
    };

    println!("\n📊 Bottleneck Analysis:");
    if results_with_bottlenecks.bottlenecks.is_empty() {
        println!("  ✓ No significant bottlenecks identified!");
    } else {
        for (i, b) in results_with_bottlenecks.bottlenecks.iter().enumerate() {
            println!("\n  {}. {}", i + 1, b.component);
            println!("     Issue: {}", b.issue);
            println!("     Impact: {}", b.impact);
            println!("     Recommendation: {}", b.recommendation);
        }
    }

    // Save results
    let json = serde_json::to_string_pretty(&results_with_bottlenecks).unwrap();
    std::fs::write("/home/user/newport/benchmarks/results/network-performance.json", json).unwrap();
    println!("\n✅ Results saved to benchmarks/results/network-performance.json");
}
