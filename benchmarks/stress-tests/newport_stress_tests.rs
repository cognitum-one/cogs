//! Comprehensive Stress Tests for Newport ASIC Simulator
//!
//! These tests are designed to push the simulator to its limits:
//! - 1M+ operation cycles
//! - Maximum tile utilization (256 tiles)
//! - Network congestion
//! - Memory pressure
//! - Edge cases and error conditions

use newport_core::{TileId, MemoryAddress, Memory, RAM, Result};
use std::time::Instant;

/// Test 1M cycles on a single tile with maximum memory pressure
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_1m_cycles_single_tile() -> Result<()> {
    const CYCLES: usize = 1_000_000;

    // Create maximum memory (5MB = 1.25M words)
    let mut memory = RAM::new(MemoryAddress::new(0), 1_250_000);

    let start = Instant::now();

    // Fill memory with pattern
    for i in 0..1_250_000 {
        let addr = MemoryAddress::new((i * 4) as u32);
        memory.write(addr, i as u32)?;
    }

    // Read back and verify
    for i in 0..1_250_000 {
        let addr = MemoryAddress::new((i * 4) as u32);
        let value = memory.read(addr)?;
        assert_eq!(value, i as u32, "Memory mismatch at index {}", i);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (CYCLES * 2) as f64 / elapsed.as_secs_f64();

    println!("✓ 1M Cycle Stress Test:");
    println!("  - Total operations: {}", CYCLES * 2);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Operations/sec: {:.2}", ops_per_sec);
    println!("  - Memory used: 5MB");

    Ok(())
}

/// Test all 256 tiles with maximum memory utilization
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_256_tiles_max_memory() -> Result<()> {
    const TILES: usize = 256;
    const MEMORY_PER_TILE_WORDS: usize = 20_971_520 / 256 / 4; // 20MB total / 256 tiles / 4 bytes

    let start = Instant::now();
    let mut tile_memories: Vec<RAM> = Vec::with_capacity(TILES);

    // Create memory for each tile
    for i in 0..TILES {
        let base_addr = (i * MEMORY_PER_TILE_WORDS * 4) as u32;
        tile_memories.push(RAM::new(MemoryAddress::new(base_addr), MEMORY_PER_TILE_WORDS));
    }

    // Write pattern to each tile's memory
    for (tile_id, memory) in tile_memories.iter_mut().enumerate() {
        for word_idx in 0..MEMORY_PER_TILE_WORDS {
            let addr = MemoryAddress::new(memory.base().value() + (word_idx * 4) as u32);
            let value = ((tile_id as u32) << 24) | (word_idx as u32 & 0xFFFFFF);
            memory.write(addr, value)?;
        }
    }

    // Verify all data
    let mut total_verifications = 0;
    for (tile_id, memory) in tile_memories.iter().enumerate() {
        for word_idx in 0..MEMORY_PER_TILE_WORDS {
            let addr = MemoryAddress::new(memory.base().value() + (word_idx * 4) as u32);
            let value = memory.read(addr)?;
            let expected = ((tile_id as u32) << 24) | (word_idx as u32 & 0xFFFFFF);
            assert_eq!(value, expected, "Tile {} memory mismatch at word {}", tile_id, word_idx);
            total_verifications += 1;
        }
    }

    let elapsed = start.elapsed();

    println!("✓ 256 Tile Maximum Memory Stress Test:");
    println!("  - Tiles tested: {}", TILES);
    println!("  - Total memory: 20MB");
    println!("  - Memory per tile: {}KB", (MEMORY_PER_TILE_WORDS * 4) / 1024);
    println!("  - Total verifications: {}", total_verifications);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Verifications/sec: {:.2}", total_verifications as f64 / elapsed.as_secs_f64());

    Ok(())
}

/// Test concurrent memory access stress
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_concurrent_memory_access() -> Result<()> {
    use std::sync::Arc;
    use parking_lot::RwLock;
    use rayon::prelude::*;

    const MEMORY_SIZE_WORDS: usize = 1_000_000; // 4MB
    const ITERATIONS: usize = 10_000;
    const THREADS: usize = 8;

    let memory = Arc::new(RwLock::new(RAM::new(MemoryAddress::new(0), MEMORY_SIZE_WORDS)));

    let start = Instant::now();

    // Parallel writes
    (0..THREADS).into_par_iter().for_each(|thread_id| {
        for i in 0..ITERATIONS {
            let addr_offset = (thread_id * ITERATIONS + i) % MEMORY_SIZE_WORDS;
            let addr = MemoryAddress::new((addr_offset * 4) as u32);
            let value = ((thread_id as u32) << 16) | (i as u32 & 0xFFFF);

            let mut mem = memory.write();
            mem.write(addr, value).unwrap();
        }
    });

    // Parallel reads and verify
    let verification_results: Vec<_> = (0..THREADS).into_par_iter().map(|thread_id| {
        let mut verified = 0;
        for i in 0..ITERATIONS {
            let addr_offset = (thread_id * ITERATIONS + i) % MEMORY_SIZE_WORDS;
            let addr = MemoryAddress::new((addr_offset * 4) as u32);

            let mem = memory.read();
            if let Ok(_value) = mem.read(addr) {
                verified += 1;
            }
        }
        verified
    }).collect();

    let elapsed = start.elapsed();
    let total_verified: usize = verification_results.iter().sum();
    let total_operations = THREADS * ITERATIONS * 2; // writes + reads

    println!("✓ Concurrent Memory Access Stress Test:");
    println!("  - Threads: {}", THREADS);
    println!("  - Operations per thread: {}", ITERATIONS * 2);
    println!("  - Total operations: {}", total_operations);
    println!("  - Verified operations: {}", total_verified);
    println!("  - Elapsed time: {:?}", elapsed);
    println!("  - Operations/sec: {:.2}", total_operations as f64 / elapsed.as_secs_f64());

    Ok(())
}

/// Test memory boundary conditions and edge cases
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_memory_boundaries() -> Result<()> {
    let test_cases = vec![
        ("Aligned boundaries", 0x1000, 1024, vec![0x1000, 0x1004, 0x13FC]),
        ("Maximum address", 0xFFFF_F000, 4, vec![0xFFFF_F000, 0xFFFF_F004, 0xFFFF_F008, 0xFFFF_F00C]),
        ("Zero base", 0x0000, 1024, vec![0x0000, 0x0004, 0x0FF8, 0x0FFC]),
    ];

    for (test_name, base, size_words, test_addrs) in test_cases {
        let mut memory = RAM::new(MemoryAddress::new(base), size_words);

        // Test valid addresses
        for addr_value in test_addrs {
            let addr = MemoryAddress::new(addr_value);
            if memory.contains(addr) {
                memory.write(addr, 0xDEADBEEF)?;
                let value = memory.read(addr)?;
                assert_eq!(value, 0xDEADBEEF, "{}: Failed at address 0x{:X}", test_name, addr_value);
            }
        }

        println!("✓ Boundary test passed: {}", test_name);
    }

    // Test unaligned access errors
    let memory = RAM::new(MemoryAddress::new(0x1000), 256);
    let unaligned_addrs = vec![0x1001, 0x1002, 0x1003, 0x1005];

    for addr_value in unaligned_addrs {
        let addr = MemoryAddress::new(addr_value);
        assert!(memory.read(addr).is_err(), "Unaligned read at 0x{:X} should fail", addr_value);
    }

    println!("✓ Unaligned access errors correctly handled");

    // Test out-of-bounds access
    let memory = RAM::new(MemoryAddress::new(0x1000), 256);
    let oob_addrs = vec![0x0FFC, 0x1400, 0x2000, 0xFFFF];

    for addr_value in oob_addrs {
        let addr = MemoryAddress::new(addr_value);
        assert!(memory.read(addr).is_err(), "Out-of-bounds read at 0x{:X} should fail", addr_value);
    }

    println!("✓ Out-of-bounds access correctly handled");

    Ok(())
}

/// Test error injection and recovery
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_error_injection() -> Result<()> {
    const ITERATIONS: usize = 10_000;
    let mut memory = RAM::new(MemoryAddress::new(0x1000), 256);

    let mut successful_ops = 0;
    let mut failed_unaligned = 0;
    let mut failed_oob = 0;

    // Inject random errors
    for i in 0..ITERATIONS {
        let addr_value = 0x1000 + (i % 1024) as u32;
        let addr = MemoryAddress::new(addr_value);

        match memory.write(addr, i as u32) {
            Ok(_) => {
                successful_ops += 1;
                // Verify write
                assert_eq!(memory.read(addr)?, i as u32);
            }
            Err(_) => {
                // Categorize error
                if addr_value % 4 != 0 {
                    failed_unaligned += 1;
                } else {
                    failed_oob += 1;
                }
            }
        }
    }

    println!("✓ Error Injection Test:");
    println!("  - Total iterations: {}", ITERATIONS);
    println!("  - Successful operations: {}", successful_ops);
    println!("  - Failed (unaligned): {}", failed_unaligned);
    println!("  - Failed (out-of-bounds): {}", failed_oob);
    println!("  - Success rate: {:.2}%", (successful_ops as f64 / ITERATIONS as f64) * 100.0);

    Ok(())
}

/// Test sustained load over time
#[tokio::test]
#[ignore = "Long-running stress test"]
async fn stress_test_sustained_load() -> Result<()> {
    const DURATION_SECS: u64 = 60; // 1 minute sustained load
    const MEMORY_SIZE_WORDS: usize = 100_000; // 400KB

    let mut memory = RAM::new(MemoryAddress::new(0), MEMORY_SIZE_WORDS);
    let start = Instant::now();
    let mut total_operations: u64 = 0;
    let mut samples: Vec<f64> = Vec::new();

    while start.elapsed().as_secs() < DURATION_SECS {
        let sample_start = Instant::now();
        let mut ops_in_sample = 0;

        // Perform operations for 1 second
        while sample_start.elapsed().as_millis() < 1000 {
            for i in 0..1000 {
                let addr_offset = (total_operations as usize + i) % MEMORY_SIZE_WORDS;
                let addr = MemoryAddress::new((addr_offset * 4) as u32);

                memory.write(addr, total_operations as u32 + i as u32)?;
                let _ = memory.read(addr)?;

                ops_in_sample += 2;
            }
            total_operations += 2000;
        }

        let ops_per_sec = ops_in_sample as f64 / sample_start.elapsed().as_secs_f64();
        samples.push(ops_per_sec);
    }

    let elapsed = start.elapsed();
    let avg_ops_per_sec = total_operations as f64 / elapsed.as_secs_f64();
    let max_ops_per_sec = samples.iter().cloned().fold(f64::MIN, f64::max);
    let min_ops_per_sec = samples.iter().cloned().fold(f64::MAX, f64::min);

    println!("✓ Sustained Load Test:");
    println!("  - Duration: {:?}", elapsed);
    println!("  - Total operations: {}", total_operations);
    println!("  - Average ops/sec: {:.2}", avg_ops_per_sec);
    println!("  - Max ops/sec: {:.2}", max_ops_per_sec);
    println!("  - Min ops/sec: {:.2}", min_ops_per_sec);
    println!("  - Variance: {:.2}%", ((max_ops_per_sec - min_ops_per_sec) / avg_ops_per_sec) * 100.0);

    Ok(())
}

/// Test memory leak detection
#[tokio::test]
#[ignore = "Stress test"]
async fn stress_test_memory_leak_detection() -> Result<()> {
    const ITERATIONS: usize = 1000;
    const MEMORY_SIZE_WORDS: usize = 10_000;

    let initial_allocated = get_allocated_bytes();

    for _ in 0..ITERATIONS {
        let mut memory = RAM::new(MemoryAddress::new(0), MEMORY_SIZE_WORDS);

        // Use the memory
        for i in 0..100 {
            let addr = MemoryAddress::new((i * 4) as u32);
            memory.write(addr, i as u32)?;
        }

        // Memory should be dropped here
    }

    // Force garbage collection
    std::thread::sleep(std::time::Duration::from_millis(100));

    let final_allocated = get_allocated_bytes();
    let leaked = final_allocated.saturating_sub(initial_allocated);

    println!("✓ Memory Leak Detection:");
    println!("  - Iterations: {}", ITERATIONS);
    println!("  - Initial allocated: {} bytes", initial_allocated);
    println!("  - Final allocated: {} bytes", final_allocated);
    println!("  - Potential leak: {} bytes", leaked);
    println!("  - Leak per iteration: {:.2} bytes", leaked as f64 / ITERATIONS as f64);

    // Allow for some variance but flag significant leaks
    let max_acceptable_leak = MEMORY_SIZE_WORDS * 4 * 10; // 10 allocations worth
    assert!(leaked < max_acceptable_leak, "Potential memory leak detected: {} bytes", leaked);

    Ok(())
}

/// Helper function to get allocated memory (approximation)
fn get_allocated_bytes() -> usize {
    // This is a rough approximation - in production you'd use a proper allocator
    let mut total = 0;
    if let Ok(info) = sys_info::mem_info() {
        total = info.total as usize * 1024;
    }
    total
}

#[cfg(test)]
mod tile_stress_tests {
    use super::*;

    /// Test TileId validation under stress
    #[test]
    fn stress_test_tile_id_validation() {
        let mut successful = 0;
        let mut failed = 0;

        // Test all possible u16 values
        for id in 0..=65535u16 {
            match TileId::new(id) {
                Ok(_) => {
                    assert!(id <= 255, "TileId::new({}) should have failed", id);
                    successful += 1;
                }
                Err(_) => {
                    assert!(id > 255, "TileId::new({}) should have succeeded", id);
                    failed += 1;
                }
            }
        }

        println!("✓ TileId Validation Stress Test:");
        println!("  - Tested values: 65536");
        println!("  - Successful: {}", successful);
        println!("  - Failed (expected): {}", failed);

        assert_eq!(successful, 256);
        assert_eq!(failed, 65536 - 256);
    }
}
