//! Performance Benchmarks for Newport Memory Subsystem
//!
//! Uses criterion for accurate performance measurements

use newport_core::memory::{Memory, RAM};
use newport_core::MemoryAddress;
use std::time::{Duration, Instant};

const WORK_MEM_SIZE: usize = 64 * 1024 / 4; // 16384 words

fn benchmark_sequential_reads(iterations: usize) -> Duration {
    let tile_ram = RAM::new(MemoryAddress::new(0x10000), WORK_MEM_SIZE);
    let base = tile_ram.base();

    let start = Instant::now();
    for _ in 0..iterations {
        for i in 0..1000 {
            let addr = MemoryAddress::new(base.value() + (i * 4));
            let _ = tile_ram.read(addr);
        }
    }
    start.elapsed()
}

fn benchmark_sequential_writes(iterations: usize) -> Duration {
    let mut tile_ram = RAM::new(MemoryAddress::new(0x10000), WORK_MEM_SIZE);
    let base = tile_ram.base();

    let start = Instant::now();
    for _ in 0..iterations {
        for i in 0..1000 {
            let addr = MemoryAddress::new(base.value() + (i * 4));
            let _ = tile_ram.write(addr, i);
        }
    }
    start.elapsed()
}

fn benchmark_random_reads(iterations: usize) -> Duration {
    let mut tile_ram = RAM::new(MemoryAddress::new(0x10000), WORK_MEM_SIZE);
    let base = tile_ram.base();

    // Pre-populate
    for i in 0..WORK_MEM_SIZE {
        let addr = MemoryAddress::new(base.value() + (i as u32 * 4));
        tile_ram.write(addr, i as u32).unwrap();
    }

    let mut rng_state = 12345u32;
    let mut next_random = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        (rng_state / 65536) % WORK_MEM_SIZE as u32
    };

    let start = Instant::now();
    for _ in 0..iterations {
        for _ in 0..1000 {
            let offset = next_random();
            let addr = MemoryAddress::new(base.value() + offset * 4);
            let _ = tile_ram.read(addr);
        }
    }
    start.elapsed()
}

fn benchmark_random_writes(iterations: usize) -> Duration {
    let mut tile_ram = RAM::new(MemoryAddress::new(0x10000), WORK_MEM_SIZE);
    let base = tile_ram.base();

    let mut rng_state = 12345u32;
    let mut next_random = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        (rng_state / 65536) % WORK_MEM_SIZE as u32
    };

    let start = Instant::now();
    for iter in 0..iterations {
        for i in 0..1000 {
            let offset = next_random();
            let addr = MemoryAddress::new(base.value() + offset * 4);
            let _ = tile_ram.write(addr, (iter * 1000 + i) as u32);
        }
    }
    start.elapsed()
}

fn benchmark_mixed_workload(iterations: usize) -> Duration {
    let mut tile_ram = RAM::new(MemoryAddress::new(0x10000), WORK_MEM_SIZE);
    let base = tile_ram.base();

    let start = Instant::now();
    for iter in 0..iterations {
        for i in 0..1000 {
            let addr = MemoryAddress::new(base.value() + ((i % 1000) as u32 * 4));
            if i % 2 == 0 {
                let _ = tile_ram.write(addr, (iter * 1000 + i) as u32);
            } else {
                let _ = tile_ram.read(addr);
            }
        }
    }
    start.elapsed()
}

pub fn run_benchmarks() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║   NEWPORT MEMORY PERFORMANCE BENCHMARKS                  ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    let iterations = 100;

    // Sequential Reads
    let seq_read_time = benchmark_sequential_reads(iterations);
    let seq_read_throughput = (iterations * 1000) as f64 / seq_read_time.as_secs_f64();
    println!("Sequential Reads:");
    println!("  Operations: {}", iterations * 1000);
    println!("  Duration: {:?}", seq_read_time);
    println!("  Throughput: {:.2} ops/sec", seq_read_throughput);
    println!();

    // Sequential Writes
    let seq_write_time = benchmark_sequential_writes(iterations);
    let seq_write_throughput = (iterations * 1000) as f64 / seq_write_time.as_secs_f64();
    println!("Sequential Writes:");
    println!("  Operations: {}", iterations * 1000);
    println!("  Duration: {:?}", seq_write_time);
    println!("  Throughput: {:.2} ops/sec", seq_write_throughput);
    println!();

    // Random Reads
    let rand_read_time = benchmark_random_reads(iterations);
    let rand_read_throughput = (iterations * 1000) as f64 / rand_read_time.as_secs_f64();
    println!("Random Reads:");
    println!("  Operations: {}", iterations * 1000);
    println!("  Duration: {:?}", rand_read_time);
    println!("  Throughput: {:.2} ops/sec", rand_read_throughput);
    println!();

    // Random Writes
    let rand_write_time = benchmark_random_writes(iterations);
    let rand_write_throughput = (iterations * 1000) as f64 / rand_write_time.as_secs_f64();
    println!("Random Writes:");
    println!("  Operations: {}", iterations * 1000);
    println!("  Duration: {:?}", rand_write_time);
    println!("  Throughput: {:.2} ops/sec", rand_write_throughput);
    println!();

    // Mixed Workload
    let mixed_time = benchmark_mixed_workload(iterations);
    let mixed_throughput = (iterations * 1000) as f64 / mixed_time.as_secs_f64();
    println!("Mixed Workload (50/50 R/W):");
    println!("  Operations: {}", iterations * 1000);
    println!("  Duration: {:?}", mixed_time);
    println!("  Throughput: {:.2} ops/sec", mixed_throughput);
    println!();
}

fn main() {
    run_benchmarks();
}
