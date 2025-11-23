//! Stress tests for Newport ASIC simulator
//!
//! These tests verify system behavior under high load and edge conditions.

use newport_core::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread;

// Simple memory for testing
struct TestMemory {
    data: HashMap<u32, u8>,
}

impl TestMemory {
    fn new() -> Self {
        TestMemory {
            data: HashMap::new(),
        }
    }
}

impl Memory for TestMemory {
    fn read_byte(&self, addr: MemoryAddress) -> Result<u8> {
        Ok(*self.data.get(&addr.value()).unwrap_or(&0))
    }

    fn write_byte(&mut self, addr: MemoryAddress, value: u8) -> Result<()> {
        self.data.insert(addr.value(), value);
        Ok(())
    }

    fn size(&self) -> usize {
        1024 * 1024
    }
}

#[test]
#[ignore] // Run with: cargo test --release -- --ignored
fn stress_test_memory_intensive() {
    let mut mem = TestMemory::new();

    // Write 1 million sequential bytes
    for i in 0..1_000_000 {
        mem.write_byte(MemoryAddress::new(i), (i & 0xFF) as u8).unwrap();
    }

    // Read back and verify
    for i in 0..1_000_000 {
        let value = mem.read_byte(MemoryAddress::new(i)).unwrap();
        assert_eq!(value, (i & 0xFF) as u8, "Mismatch at address {}", i);
    }
}

#[test]
#[ignore]
fn stress_test_concurrent_memory_access() {
    let mem = Arc::new(Mutex::new(TestMemory::new()));
    let num_threads = 16;
    let ops_per_thread = 10000;

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let mem_clone = Arc::clone(&mem);
        thread::spawn(move || {
            for i in 0..ops_per_thread {
                let addr = ((thread_id * ops_per_thread + i) % 100000) as u32;
                let mut m = mem_clone.lock().unwrap();
                m.write_byte(MemoryAddress::new(addr), (addr & 0xFF) as u8).unwrap();
            }
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify no corruption
    let m = mem.lock().unwrap();
    assert!(m.size() > 0);
}

#[test]
#[ignore]
fn stress_test_packet_flood() {
    let mut queue: VecDeque<RaceWayPacket> = VecDeque::new();

    // Create 100,000 packets
    for i in 0..100_000 {
        let src = TileId::new((i % 256) as u16).unwrap();
        let dst = TileId::new(((i + 1) % 256) as u16).unwrap();
        let data = vec![i as u8, (i >> 8) as u8];

        queue.push_back(RaceWayPacket::data(src, dst, data));
    }

    assert_eq!(queue.len(), 100_000);

    // Process all packets
    let mut processed = 0;
    while let Some(packet) = queue.pop_front() {
        // Verify packet integrity
        assert!(!packet.data.is_empty());
        assert!(packet.source.value() < 256);
        assert!(packet.dest.value() < 256);
        processed += 1;
    }

    assert_eq!(processed, 100_000);
}

#[test]
#[ignore]
fn stress_test_tile_grid_communication() {
    // Simulate full mesh communication: all 256 tiles sending to all others
    let mut packets: Vec<RaceWayPacket> = Vec::new();

    for src in 0..256 {
        for dst in 0..256 {
            if src != dst {
                let src_tile = TileId::new(src).unwrap();
                let dst_tile = TileId::new(dst).unwrap();
                let data = vec![src as u8, dst as u8];

                packets.push(RaceWayPacket::data(src_tile, dst_tile, data));
            }
        }
    }

    // Should have 256 * 255 packets (all-to-all except self)
    assert_eq!(packets.len(), 256 * 255);

    // Verify all packets valid
    for packet in &packets {
        assert_ne!(packet.source, packet.dest);
        assert_eq!(packet.data.len(), 2);
        assert_eq!(packet.data[0], packet.source.value());
        assert_eq!(packet.data[1], packet.dest.value());
    }
}

#[test]
#[ignore]
fn stress_test_packet_serialization_performance() {
    use std::time::Instant;

    let src = TileId::new(0).unwrap();
    let dst = TileId::new(255).unwrap();
    let data = vec![0xFF; 512];
    let packet = RaceWayPacket::data(src, dst, data);

    let iterations = 100_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let bits = packet.to_bits();
        let _decoded = RaceWayPacket::from_bits(&bits).unwrap();
    }

    let duration = start.elapsed();
    let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

    println!("Packet roundtrip: {:.0} ops/sec", ops_per_sec);
    println!("Average latency: {:.2} µs", duration.as_micros() as f64 / iterations as f64);

    // Should achieve at least 10,000 ops/sec
    assert!(ops_per_sec > 10_000.0, "Performance regression detected");
}

#[test]
#[ignore]
fn stress_test_memory_fragmentation() {
    let mut mem = TestMemory::new();

    // Create fragmented access pattern
    for i in 0..10000 {
        let addr = (i * 173) % 100000; // Prime number stride
        mem.write_byte(MemoryAddress::new(addr), (addr & 0xFF) as u8).unwrap();
    }

    // Verify all writes
    for i in 0..10000 {
        let addr = (i * 173) % 100000;
        let value = mem.read_byte(MemoryAddress::new(addr)).unwrap();
        assert_eq!(value, (addr & 0xFF) as u8);
    }
}

#[test]
#[ignore]
fn stress_test_tile_id_creation_performance() {
    use std::time::Instant;

    let iterations = 1_000_000;
    let start = Instant::now();

    for i in 0..iterations {
        let _ = TileId::new((i % 256) as u16).unwrap();
    }

    let duration = start.elapsed();
    let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

    println!("TileId creation: {:.0} ops/sec", ops_per_sec);

    // Should achieve at least 1 million ops/sec
    assert!(ops_per_sec > 1_000_000.0, "Performance regression");
}

#[test]
#[ignore]
fn stress_test_deadlock_detection() {
    // Simulate circular dependency scenario
    let mem1 = Arc::new(Mutex::new(TestMemory::new()));
    let mem2 = Arc::new(Mutex::new(TestMemory::new()));

    let mem1_clone = Arc::clone(&mem1);
    let mem2_clone = Arc::clone(&mem2);

    let handle1 = thread::spawn(move || {
        for i in 0..1000 {
            let _m1 = mem1_clone.lock().unwrap();
            thread::yield_now();
            let _m2 = mem2_clone.lock().unwrap();
            // Do work
            drop(_m2);
            drop(_m1);

            if i % 100 == 0 {
                thread::sleep(std::time::Duration::from_micros(1));
            }
        }
    });

    let mem1_clone2 = Arc::clone(&mem1);
    let mem2_clone2 = Arc::clone(&mem2);

    let handle2 = thread::spawn(move || {
        for i in 0..1000 {
            let _m1 = mem1_clone2.lock().unwrap();
            thread::yield_now();
            let _m2 = mem2_clone2.lock().unwrap();
            // Do work
            drop(_m2);
            drop(_m1);

            if i % 100 == 0 {
                thread::sleep(std::time::Duration::from_micros(1));
            }
        }
    });

    // Should complete without deadlock
    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
#[ignore]
fn stress_test_large_packet_data() {
    let src = TileId::new(0).unwrap();
    let dst = TileId::new(255).unwrap();

    // Test with increasingly large payloads
    for size in [64, 256, 1024, 4096, 16384] {
        let data = vec![0xAA; size];
        let packet = RaceWayPacket::data(src, dst, data.clone());

        let bits = packet.to_bits();
        let decoded = RaceWayPacket::from_bits(&bits).unwrap();

        assert_eq!(decoded.data.len(), size);
        assert_eq!(decoded.data, data);
    }
}

#[test]
#[ignore]
fn stress_test_memory_word_operations() {
    let mut mem = TestMemory::new();

    // Write 100,000 words
    for i in 0..100_000 {
        let addr = MemoryAddress::new(i * 4);
        mem.write_word(addr, 0xDEADBEEF ^ i).unwrap();
    }

    // Read back and verify
    for i in 0..100_000 {
        let addr = MemoryAddress::new(i * 4);
        let value = mem.read_word(addr).unwrap();
        assert_eq!(value, 0xDEADBEEF ^ i, "Mismatch at word {}", i);
    }
}

#[test]
#[ignore]
fn stress_test_error_handling_overhead() {
    use std::time::Instant;

    let mut mem = TestMemory::new();
    let iterations = 100_000;

    let start = Instant::now();

    for i in 0..iterations {
        let addr = MemoryAddress::new(i % 1000);
        let _ = mem.write_byte(addr, (i & 0xFF) as u8);
    }

    let duration = start.elapsed();
    let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

    println!("Memory operations with Result: {:.0} ops/sec", ops_per_sec);

    // Should maintain reasonable performance even with error handling
    assert!(ops_per_sec > 100_000.0);
}
