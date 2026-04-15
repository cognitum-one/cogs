//! Performance benchmarks for Newport ASIC simulator

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use newport_core::*;
use std::collections::HashMap;

// Simple memory implementation for benchmarking
struct BenchMemory {
    data: HashMap<u32, u8>,
}

impl BenchMemory {
    fn new() -> Self {
        BenchMemory {
            data: HashMap::with_capacity(1024 * 1024),
        }
    }
}

impl Memory for BenchMemory {
    fn read_byte(&self, addr: MemoryAddress) -> Result<u8> {
        Ok(*self.data.get(&addr.as_u32()).unwrap_or(&0))
    }

    fn write_byte(&mut self, addr: MemoryAddress, value: u8) -> Result<()> {
        self.data.insert(addr.as_u32(), value);
        Ok(())
    }

    fn size(&self) -> usize {
        1024 * 1024
    }
}

fn bench_memory_sequential_read(c: &mut Criterion) {
    let mut mem = BenchMemory::new();

    // Pre-fill memory
    for i in 0..1000 {
        mem.write_byte(MemoryAddress::new(i), (i & 0xFF) as u8).unwrap();
    }

    c.bench_function("memory_sequential_read_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let _ = mem.read_byte(black_box(MemoryAddress::new(i)));
            }
        });
    });
}

fn bench_memory_sequential_write(c: &mut Criterion) {
    c.bench_function("memory_sequential_write_1000", |b| {
        let mut mem = BenchMemory::new();
        b.iter(|| {
            for i in 0..1000 {
                let _ = mem.write_byte(
                    black_box(MemoryAddress::new(i)),
                    black_box((i & 0xFF) as u8),
                );
            }
        });
    });
}

fn bench_memory_word_operations(c: &mut Criterion) {
    let mut mem = BenchMemory::new();

    c.bench_function("memory_word_write_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let _ = mem.write_word(
                    black_box(MemoryAddress::new(i * 4)),
                    black_box(0xDEADBEEF),
                );
            }
        });
    });
}

fn bench_tile_id_creation(c: &mut Criterion) {
    c.bench_function("tile_id_new", |b| {
        b.iter(|| {
            for i in 0..256 {
                let _ = TileId::new(black_box(i));
            }
        });
    });

    c.bench_function("tile_id_from_coords", |b| {
        b.iter(|| {
            for row in 0..16 {
                for col in 0..16 {
                    let _ = TileId::from_coords(black_box(row), black_box(col));
                }
            }
        });
    });
}

fn bench_packet_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_serialization");

    for size in [8, 64, 512].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let src = TileId::new(0).unwrap();
            let dst = TileId::new(255).unwrap();
            let data = vec![0xFF; size];
            let packet = RaceWayPacket::data(src, dst, data);

            b.iter(|| {
                let bits = packet.to_bits();
                black_box(bits);
            });
        });
    }

    group.finish();
}

fn bench_packet_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_deserialization");

    for size in [8, 64, 512].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let src = TileId::new(0).unwrap();
            let dst = TileId::new(255).unwrap();
            let data = vec![0xFF; size];
            let packet = RaceWayPacket::data(src, dst, data);
            let bits = packet.to_bits();

            b.iter(|| {
                let decoded = RaceWayPacket::from_bits(black_box(&bits)).unwrap();
                black_box(decoded);
            });
        });
    }

    group.finish();
}

fn bench_packet_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_roundtrip");

    for size in [8, 64, 512].iter() {
        group.throughput(Throughput::Bytes(*size as u64 * 2)); // serialize + deserialize

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let src = TileId::new(0).unwrap();
            let dst = TileId::new(255).unwrap();
            let data = vec![0xFF; size];

            b.iter(|| {
                let packet = RaceWayPacket::data(src, dst, data.clone());
                let bits = packet.to_bits();
                let decoded = RaceWayPacket::from_bits(&bits).unwrap();
                black_box(decoded);
            });
        });
    }

    group.finish();
}

fn bench_tile_grid_operations(c: &mut Criterion) {
    c.bench_function("tile_grid_full_scan", |b| {
        b.iter(|| {
            for row in 0..16 {
                for col in 0..16 {
                    let tile = TileId::from_coords(row, col).unwrap();
                    black_box(tile);
                }
            }
        });
    });
}

fn bench_memory_random_access(c: &mut Criterion) {
    let mut mem = BenchMemory::new();

    // Generate pseudo-random addresses
    let addresses: Vec<u32> = (0..1000).map(|i| (i * 7919) % 10000).collect();

    c.bench_function("memory_random_read_1000", |b| {
        b.iter(|| {
            for &addr in &addresses {
                let _ = mem.read_byte(black_box(MemoryAddress::new(addr)));
            }
        });
    });
}

criterion_group!(
    memory_benches,
    bench_memory_sequential_read,
    bench_memory_sequential_write,
    bench_memory_word_operations,
    bench_memory_random_access
);

criterion_group!(
    tile_benches,
    bench_tile_id_creation,
    bench_tile_grid_operations
);

criterion_group!(
    packet_benches,
    bench_packet_serialization,
    bench_packet_deserialization,
    bench_packet_roundtrip
);

criterion_main!(memory_benches, tile_benches, packet_benches);
