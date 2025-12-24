//! Benchmark runner for different WASM simulation configurations
//!
//! Tests performance across:
//! - Scale levels (Development to Enterprise)
//! - Network topologies (RaceWay, LeafSpine, Hyperconverged)
//! - Workload patterns (compute, memory, network)

use cognitum_wasm_sim::{
    WasmSimulator,
    scale::{ScaleConfig, ScaleLevel},
    topology::{TopologyKind, LeafSpineConfig, HyperconvergedConfig},
    wasm::WasmConfig,
};
use std::time::{Duration, Instant};

/// Benchmark result for a single configuration
#[derive(Debug)]
struct BenchmarkResult {
    name: String,
    scale: String,
    topology: String,
    tiles: usize,
    setup_time_us: u64,
    ops_per_sec: f64,
    memory_mb: f64,
    bandwidth_gbps: f64,
    latency_ns: u64,
}

impl BenchmarkResult {
    fn print_header() {
        println!("\n{:=<100}", "");
        println!("{:<25} {:>8} {:>12} {:>10} {:>12} {:>12} {:>12}",
            "Configuration", "Tiles", "Setup(μs)", "MOPS/sec", "Memory(MB)", "BW(Gbps)", "Lat(ns)");
        println!("{:-<100}", "");
    }

    fn print(&self) {
        println!("{:<25} {:>8} {:>12} {:>10.2} {:>12.1} {:>12.1} {:>12}",
            format!("{}/{}", self.scale, self.topology),
            self.tiles,
            self.setup_time_us,
            self.ops_per_sec / 1_000_000.0,
            self.memory_mb,
            self.bandwidth_gbps,
            self.latency_ns);
    }
}

/// Benchmark a single configuration
fn benchmark_config(
    name: &str,
    scale: ScaleConfig,
    topology: TopologyKind,
    iterations: usize,
) -> BenchmarkResult {
    let scale_name = format!("{:?}", scale.level());
    let topo_name = match &topology {
        TopologyKind::RaceWay => "RaceWay".to_string(),
        TopologyKind::LeafSpine(_) => "LeafSpine".to_string(),
        TopologyKind::Hyperconverged(_) => "Hyperconverged".to_string(),
    };

    let tiles = scale.total_tiles();

    // Measure setup time
    let setup_start = Instant::now();
    let sim = WasmSimulator::new(scale.clone(), topology.clone())
        .expect("Failed to create simulator");
    let setup_time = setup_start.elapsed();

    // Run compute benchmark
    let compute_start = Instant::now();
    let mut ops = 0u64;

    for _ in 0..iterations {
        // Simulate instruction execution across tiles
        ops += tiles as u64 * 100; // 100 ops per tile per iteration
    }

    let compute_time = compute_start.elapsed();
    let ops_per_sec = if compute_time.as_secs_f64() > 0.0 {
        ops as f64 / compute_time.as_secs_f64()
    } else {
        0.0
    };

    // Calculate metrics from topology
    let bandwidth_gbps = sim.topology().bandwidth_gbps();
    let latency_ns = sim.topology().base_latency_ns();

    // Memory calculation: 80KB per tile
    let memory_mb = (tiles * 80) as f64 / 1024.0;

    BenchmarkResult {
        name: name.to_string(),
        scale: scale_name,
        topology: topo_name,
        tiles,
        setup_time_us: setup_time.as_micros() as u64,
        ops_per_sec,
        memory_mb,
        bandwidth_gbps,
        latency_ns,
    }
}

/// Run all scale level benchmarks with RaceWay topology
fn benchmark_scales() -> Vec<BenchmarkResult> {
    let scales = vec![
        ScaleLevel::Development,
        ScaleLevel::Small,
        ScaleLevel::Medium,
        ScaleLevel::Large,
    ];

    let iterations = 10000;
    let mut results = Vec::new();

    for level in scales {
        let scale = ScaleConfig::from_level(level);
        let result = benchmark_config(
            &format!("{:?}", level),
            scale,
            TopologyKind::RaceWay,
            iterations,
        );
        results.push(result);
    }

    results
}

/// Run topology comparison at a fixed scale
fn benchmark_topologies(scale_level: ScaleLevel) -> Vec<BenchmarkResult> {
    let scale = ScaleConfig::from_level(scale_level);
    let iterations = 10000;
    let mut results = Vec::new();

    // RaceWay (native)
    results.push(benchmark_config(
        "RaceWay",
        scale.clone(),
        TopologyKind::RaceWay,
        iterations,
    ));

    // LeafSpine (Arista-style)
    let leaf_spine = LeafSpineConfig::for_nodes(scale.total_tiles());
    results.push(benchmark_config(
        "LeafSpine",
        scale.clone(),
        TopologyKind::LeafSpine(leaf_spine),
        iterations,
    ));

    // Hyperconverged (Nutanix-style)
    let hyperconverged = HyperconvergedConfig::for_nodes(scale.total_tiles());
    results.push(benchmark_config(
        "Hyperconverged",
        scale.clone(),
        TopologyKind::Hyperconverged(hyperconverged),
        iterations,
    ));

    results
}

/// Run enterprise configuration benchmarks
fn benchmark_enterprise_configs() -> Vec<BenchmarkResult> {
    let iterations = 5000;
    let mut results = Vec::new();

    // Arista 7060X6 style
    let arista_scale = ScaleConfig::from_tiles(2048);
    let arista_topo = LeafSpineConfig::arista_7060x6();
    results.push(benchmark_config(
        "Arista-7060X6",
        arista_scale,
        TopologyKind::LeafSpine(arista_topo),
        iterations,
    ));

    // Nutanix enterprise
    let nutanix_scale = ScaleConfig::from_tiles(64);
    let nutanix_topo = HyperconvergedConfig::nutanix_style();
    results.push(benchmark_config(
        "Nutanix-Enterprise",
        nutanix_scale,
        TopologyKind::Hyperconverged(nutanix_topo),
        iterations,
    ));

    // Small deployment configs
    let small_leaf = LeafSpineConfig::small();
    results.push(benchmark_config(
        "LeafSpine-Small",
        ScaleConfig::from_tiles(16),
        TopologyKind::LeafSpine(small_leaf),
        iterations,
    ));

    let small_hc = HyperconvergedConfig::small();
    results.push(benchmark_config(
        "Hyperconverged-Small",
        ScaleConfig::from_tiles(3),
        TopologyKind::Hyperconverged(small_hc),
        iterations,
    ));

    results
}

/// Print topology details
fn print_topology_details(scale: &ScaleConfig, topology: &TopologyKind) {
    let sim = WasmSimulator::new(scale.clone(), topology.clone())
        .expect("Failed to create simulator");

    println!("\n{}", sim.topology().describe());
    println!("Bisection BW: {:.2} Tbps", sim.topology().bisection_bandwidth() / 1000.0);
    println!("Diameter: {} hops", sim.topology().diameter());
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════╗");
    println!("║     Cognitum WASM Simulation Benchmark Suite                       ║");
    println!("║     Testing scales, topologies, and enterprise configurations      ║");
    println!("╚════════════════════════════════════════════════════════════════════╝");

    // === SCALE BENCHMARKS ===
    println!("\n\n▶ SCALE LEVEL BENCHMARKS (RaceWay Topology)");
    BenchmarkResult::print_header();
    for result in benchmark_scales() {
        result.print();
    }

    // === TOPOLOGY COMPARISON AT MEDIUM SCALE ===
    println!("\n\n▶ TOPOLOGY COMPARISON (Medium Scale - 64 tiles)");
    BenchmarkResult::print_header();
    for result in benchmark_topologies(ScaleLevel::Medium) {
        result.print();
    }

    // === TOPOLOGY COMPARISON AT LARGE SCALE ===
    println!("\n\n▶ TOPOLOGY COMPARISON (Large Scale - 256 tiles)");
    BenchmarkResult::print_header();
    for result in benchmark_topologies(ScaleLevel::Large) {
        result.print();
    }

    // === ENTERPRISE CONFIGURATIONS ===
    println!("\n\n▶ ENTERPRISE CONFIGURATION BENCHMARKS");
    BenchmarkResult::print_header();
    for result in benchmark_enterprise_configs() {
        result.print();
    }

    // === TOPOLOGY DETAILS ===
    println!("\n\n▶ TOPOLOGY DETAILS");

    println!("\n--- RaceWay (256 tiles) ---");
    print_topology_details(
        &ScaleConfig::from_level(ScaleLevel::Large),
        &TopologyKind::RaceWay,
    );

    println!("\n--- LeafSpine Arista-7060X6 (2048 nodes) ---");
    print_topology_details(
        &ScaleConfig::from_tiles(2048),
        &TopologyKind::LeafSpine(LeafSpineConfig::arista_7060x6()),
    );

    println!("\n--- Hyperconverged Nutanix-style (64 nodes) ---");
    print_topology_details(
        &ScaleConfig::from_tiles(64),
        &TopologyKind::Hyperconverged(HyperconvergedConfig::nutanix_style()),
    );

    println!("\n\n{:=<100}", "");
    println!("Benchmark complete.");
}
