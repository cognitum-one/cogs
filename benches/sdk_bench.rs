//! SDK Core Performance Benchmarks
//!
//! Target performance:
//! - Cycle execution: < 100ns per cycle
//! - State capture: < 1μs per tile
//! - Memory efficiency for 256 processors

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

// Mock SDK types for benchmarking
#[derive(Clone)]
struct ProcessorState {
    registers: [u32; 16],
    pc: u32,
    sp: u16,
    flags: u8,
}

impl ProcessorState {
    fn new() -> Self {
        Self {
            registers: [0; 16],
            pc: 0,
            sp: 0,
            flags: 0,
        }
    }

    fn random() -> Self {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        let seed = hasher.finish();

        let mut registers = [0u32; 16];
        for i in 0..16 {
            registers[i] = ((seed >> (i * 4)) & 0xFFFFFFFF) as u32;
        }

        Self {
            registers,
            pc: (seed & 0xFFFFFFFF) as u32,
            sp: ((seed >> 32) & 0xFFFF) as u16,
            flags: ((seed >> 48) & 0xFF) as u8,
        }
    }

    // Simulate single cycle execution
    #[inline]
    fn execute_cycle(&mut self) {
        self.pc = self.pc.wrapping_add(1);
        self.registers[0] = self.registers[0].wrapping_add(self.registers[1]);
    }
}

#[derive(Clone)]
struct TileState {
    processor: ProcessorState,
    local_memory: Vec<u8>,
    message_queue_len: usize,
}

impl TileState {
    fn new() -> Self {
        Self {
            processor: ProcessorState::new(),
            local_memory: vec![0; 4096],
            message_queue_len: 0,
        }
    }

    fn random() -> Self {
        Self {
            processor: ProcessorState::random(),
            local_memory: vec![0; 4096],
            message_queue_len: 0,
        }
    }

    // Capture complete tile state (< 1μs target)
    fn capture(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(128);

        // Serialize processor state
        for &reg in &self.processor.registers {
            state.extend_from_slice(&reg.to_le_bytes());
        }
        state.extend_from_slice(&self.processor.pc.to_le_bytes());
        state.extend_from_slice(&self.processor.sp.to_le_bytes());
        state.push(self.processor.flags);

        state
    }
}

fn bench_cycle_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("cycle_execution");
    group.throughput(Throughput::Elements(1));

    // Single processor single cycle
    group.bench_function("single_cycle", |b| {
        let mut state = ProcessorState::new();
        b.iter(|| {
            state.execute_cycle();
            black_box(&state);
        });
    });

    // Batch cycle execution
    for batch_size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("batch_cycles", batch_size),
            &batch_size,
            |b, &size| {
                let mut state = ProcessorState::new();
                b.iter(|| {
                    for _ in 0..size {
                        state.execute_cycle();
                    }
                    black_box(&state);
                });
            },
        );
    }

    group.finish();
}

fn bench_state_capture(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_capture");

    // Single tile state capture (target: < 1μs)
    group.bench_function("single_tile", |b| {
        let tile = TileState::random();
        b.iter(|| {
            let captured = tile.capture();
            black_box(captured);
        });
    });

    // Batch state capture (256 tiles)
    for num_tiles in [4, 16, 64, 256] {
        group.throughput(Throughput::Elements(num_tiles));
        group.bench_with_input(
            BenchmarkId::new("batch_tiles", num_tiles),
            &num_tiles,
            |b, &count| {
                let tiles: Vec<TileState> = (0..count).map(|_| TileState::random()).collect();
                b.iter(|| {
                    let captured: Vec<Vec<u8>> = tiles.iter().map(|t| t.capture()).collect();
                    black_box(captured);
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    // Memory allocation patterns for 256 processors
    group.bench_function("allocate_256_processors", |b| {
        b.iter(|| {
            let processors: Vec<ProcessorState> = (0..256)
                .map(|_| ProcessorState::new())
                .collect();
            black_box(processors);
        });
    });

    // Memory allocation with pre-allocation
    group.bench_function("allocate_256_processors_preallocated", |b| {
        b.iter(|| {
            let mut processors = Vec::with_capacity(256);
            for _ in 0..256 {
                processors.push(ProcessorState::new());
            }
            black_box(processors);
        });
    });

    // Memory pool simulation
    group.bench_function("memory_pool_reuse", |b| {
        let mut pool: Vec<ProcessorState> = (0..256)
            .map(|_| ProcessorState::new())
            .collect();

        b.iter(|| {
            // Simulate reusing pooled memory
            for state in pool.iter_mut() {
                *state = ProcessorState::new();
            }
            black_box(&pool);
        });
    });

    group.finish();
}

fn bench_parallel_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_operations");

    // Sequential processing
    group.bench_function("sequential_256_tiles", |b| {
        let tiles: Vec<TileState> = (0..256).map(|_| TileState::random()).collect();
        b.iter(|| {
            let results: Vec<Vec<u8>> = tiles.iter().map(|t| t.capture()).collect();
            black_box(results);
        });
    });

    // Simulated parallel processing (using chunks)
    group.bench_function("chunked_256_tiles", |b| {
        let tiles: Vec<TileState> = (0..256).map(|_| TileState::random()).collect();
        b.iter(|| {
            let results: Vec<Vec<Vec<u8>>> = tiles
                .chunks(64)
                .map(|chunk| chunk.iter().map(|t| t.capture()).collect())
                .collect();
            black_box(results);
        });
    });

    group.finish();
}

fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    // Linear memory access (cache-friendly)
    group.bench_function("linear_access", |b| {
        let data: Vec<u32> = (0..1024).collect();
        b.iter(|| {
            let mut sum = 0u64;
            for &val in &data {
                sum = sum.wrapping_add(val as u64);
            }
            black_box(sum);
        });
    });

    // Random memory access (cache-unfriendly)
    group.bench_function("random_access", |b| {
        let data: Vec<u32> = (0..1024).collect();
        let indices: Vec<usize> = (0..1024).map(|i| (i * 7919) % 1024).collect();
        b.iter(|| {
            let mut sum = 0u64;
            for &idx in &indices {
                sum = sum.wrapping_add(data[idx] as u64);
            }
            black_box(sum);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cycle_execution,
    bench_state_capture,
    bench_memory_efficiency,
    bench_parallel_operations,
    bench_cache_efficiency,
);
criterion_main!(benches);
