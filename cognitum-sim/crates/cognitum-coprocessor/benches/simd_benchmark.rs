//! SIMD Coprocessor Performance Benchmarks
//!
//! Target: 524 GOPS aggregate performance across 256 tiles
//! Per-tile target: 524 / 256 = 2.047 GOPS

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use cognitum_coprocessor::simd::{SimdCoprocessor, SimdVector, Matrix4x4};

// ==================== Vector Arithmetic Benchmarks ====================

fn bench_vadd16(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(100);
    let b = SimdVector::splat(50);

    c.bench_function("vadd16", |bencher| {
        bencher.iter(|| {
            black_box(cop.vadd16(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_vadd8(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(0x0505);
    let b = SimdVector::splat(0x0303);

    c.bench_function("vadd8", |bencher| {
        bencher.iter(|| {
            black_box(cop.vadd8(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_vadd32(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[100, 0, 200, 0, 300, 0, 400, 0]);
    let b = SimdVector::from_slice(&[50, 0, 60, 0, 70, 0, 80, 0]);

    c.bench_function("vadd32", |bencher| {
        bencher.iter(|| {
            black_box(cop.vadd32(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_vmul16(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(100);
    let b = SimdVector::splat(50);

    c.bench_function("vmul16", |bencher| {
        bencher.iter(|| {
            black_box(cop.vmul16(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_vdot16(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(100);
    let b = SimdVector::splat(50);

    c.bench_function("vdot16", |bencher| {
        bencher.iter(|| {
            black_box(cop.vdot16(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_vmadd(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(10);
    let b = SimdVector::splat(5);
    let c_vec = SimdVector::splat(100);

    c.bench_function("vmadd", |bencher| {
        bencher.iter(|| {
            black_box(cop.vmadd(black_box(&a), black_box(&b), black_box(&c_vec)));
        });
    });
}

// ==================== Matrix Operation Benchmarks ====================

fn bench_mmul(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::from_array(&[
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    ]);
    let b = Matrix4x4::from_array(&[
        [16, 15, 14, 13],
        [12, 11, 10, 9],
        [8, 7, 6, 5],
        [4, 3, 2, 1],
    ]);

    c.bench_function("mmul_4x4", |bencher| {
        bencher.iter(|| {
            black_box(cop.mmul(black_box(&a), black_box(&b)));
        });
    });
}

fn bench_mmadd(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::identity();
    let b = Matrix4x4::from_array(&[
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    ]);
    let c_mat = Matrix4x4::from_array(&[
        [100, 100, 100, 100],
        [100, 100, 100, 100],
        [100, 100, 100, 100],
        [100, 100, 100, 100],
    ]);

    c.bench_function("mmadd_4x4", |bencher| {
        bencher.iter(|| {
            black_box(cop.mmadd(black_box(&a), black_box(&b), black_box(&c_mat)));
        });
    });
}

// ==================== Neural Network Primitive Benchmarks ====================

fn bench_relu(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[-5, -1, 0, 1, 5, -10, 10, -100, 100, 50, -50, 25, -25, 75, -75, 0]);

    c.bench_function("relu", |bencher| {
        bencher.iter(|| {
            black_box(cop.relu(black_box(&v)));
        });
    });
}

fn bench_sigmoid(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[-5000, -2048, -1024, -512, 0, 512, 1024, 2048, 5000]);

    c.bench_function("sigmoid", |bencher| {
        bencher.iter(|| {
            black_box(cop.sigmoid(black_box(&v)));
        });
    });
}

fn bench_softmax(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[100, 200, 150, 300, 250, 180, 220, 190, 280, 160, 240, 170, 260, 210, 230, 270]);

    c.bench_function("softmax", |bencher| {
        bencher.iter(|| {
            black_box(cop.softmax(black_box(&v)));
        });
    });
}

fn bench_pool_max_2x2(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();
    let input = SimdVector::from_slice(&[
        1, 2, 3, 4,
        5, 10, 7, 8,
        9, 11, 15, 12,
        13, 14, 20, 16,
    ]);

    c.bench_function("pool_max_2x2", |bencher| {
        bencher.iter(|| {
            black_box(cop.pool_max_2x2(black_box(&input)));
        });
    });
}

fn bench_conv2d_3x3(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();

    let input = [
        1, 2, 3, 4, 5,
        6, 7, 8, 9, 10,
        11, 12, 13, 14, 15,
        16, 17, 18, 19, 20,
        21, 22, 23, 24, 25,
    ];

    let kernel = [
        -1, 0, 1,
        -2, 0, 2,
        -1, 0, 1,
    ];

    c.bench_function("conv2d_3x3", |bencher| {
        bencher.iter(|| {
            black_box(cop.conv2d_3x3(black_box(&input), black_box(&kernel)));
        });
    });
}

fn bench_conv2d_5x5(c: &mut Criterion) {
    let mut cop = SimdCoprocessor::new();

    let mut input = [0i16; 81];
    for i in 0..81 {
        input[i] = (i as i16 + 1) * 10;
    }

    let kernel = [
        1, 2, 3, 2, 1,
        2, 4, 6, 4, 2,
        3, 6, 9, 6, 3,
        2, 4, 6, 4, 2,
        1, 2, 3, 2, 1,
    ];

    c.bench_function("conv2d_5x5", |bencher| {
        bencher.iter(|| {
            black_box(cop.conv2d_5x5(black_box(&input), black_box(&kernel)));
        });
    });
}

// ==================== Throughput Benchmarks ====================

fn bench_operation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    for batch_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(BenchmarkId::new("vadd16", batch_size), batch_size, |bencher, &size| {
            let mut cop = SimdCoprocessor::new();
            let a = SimdVector::splat(100);
            let b = SimdVector::splat(50);

            bencher.iter(|| {
                for _ in 0..size {
                    black_box(cop.vadd16(black_box(&a), black_box(&b)));
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("vmul16", batch_size), batch_size, |bencher, &size| {
            let mut cop = SimdCoprocessor::new();
            let a = SimdVector::splat(100);
            let b = SimdVector::splat(50);

            bencher.iter(|| {
                for _ in 0..size {
                    black_box(cop.vmul16(black_box(&a), black_box(&b)));
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("vdot16", batch_size), batch_size, |bencher, &size| {
            let mut cop = SimdCoprocessor::new();
            let a = SimdVector::splat(100);
            let b = SimdVector::splat(50);

            bencher.iter(|| {
                for _ in 0..size {
                    black_box(cop.vdot16(black_box(&a), black_box(&b)));
                }
            });
        });
    }

    group.finish();
}

// ==================== GOPS Performance Test ====================

fn bench_gops_target(c: &mut Criterion) {
    let mut group = c.benchmark_group("gops_performance");

    // Target: 2.047 GOPS per tile (524 GOPS / 256 tiles)
    // At 1 GHz, this is 2.047 operations per cycle

    group.bench_function("mixed_operations_1000", |bencher| {
        let mut cop = SimdCoprocessor::new();
        let a = SimdVector::splat(100);
        let b = SimdVector::splat(50);
        let c = SimdVector::splat(25);

        bencher.iter(|| {
            cop.reset_counters();

            for _ in 0..1000 {
                // Mix of operations to simulate real workload
                black_box(cop.vadd16(&a, &b));
                black_box(cop.vmul16(&a, &b));
                black_box(cop.vmadd(&a, &b, &c));
                black_box(cop.relu(&a));
            }

            black_box(cop.get_gops());
        });
    });

    group.bench_function("neural_network_layer", |bencher| {
        let mut cop = SimdCoprocessor::new();

        // Simulate a small neural network layer
        let weights = SimdVector::from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let input = SimdVector::from_slice(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160]);
        let bias = SimdVector::splat(100);

        bencher.iter(|| {
            cop.reset_counters();

            for _ in 0..1000 {
                // Forward pass: weights × input + bias → relu
                let mul = cop.vmul16(&weights, &input);
                let add = cop.vadd16(&mul, &bias);
                let activated = cop.relu(&add);
                black_box(activated);
            }

            black_box(cop.get_gops());
        });
    });

    group.finish();
}

// ==================== Aggregate Performance Test (256 Tiles) ====================

fn bench_aggregate_524_gops(c: &mut Criterion) {
    c.bench_function("aggregate_524_gops_simulation", |bencher| {
        // Simulate 256 tiles working in parallel
        // Each tile needs to achieve ~2.047 GOPS
        // Total target: 524 GOPS

        bencher.iter(|| {
            let mut tiles: Vec<SimdCoprocessor> = (0..256).map(|_| SimdCoprocessor::new()).collect();

            // Each tile performs operations
            for tile in &mut tiles {
                tile.reset_counters();

                let a = SimdVector::splat(100);
                let b = SimdVector::splat(50);

                // Perform enough operations to hit target
                // Target: 2.047 ops/cycle
                // Run 1000 cycles worth of work
                for _ in 0..1000 {
                    tile.vadd16(&a, &b); // 16 ops, 1 cycle
                    tile.vmul16(&a, &b); // 16 ops, 2 cycles
                }
            }

            // Calculate aggregate GOPS
            let total_ops: u64 = tiles.iter().map(|t| t.get_op_count()).sum();
            let max_cycles: u64 = tiles.iter().map(|t| t.get_cycle_count()).max().unwrap();
            let aggregate_gops = (total_ops as f64) / (max_cycles as f64);

            black_box(aggregate_gops);
        });
    });
}

criterion_group!(
    vector_arithmetic,
    bench_vadd16,
    bench_vadd8,
    bench_vadd32,
    bench_vmul16,
    bench_vdot16,
    bench_vmadd
);

criterion_group!(
    matrix_operations,
    bench_mmul,
    bench_mmadd
);

criterion_group!(
    neural_primitives,
    bench_relu,
    bench_sigmoid,
    bench_softmax,
    bench_pool_max_2x2,
    bench_conv2d_3x3,
    bench_conv2d_5x5
);

criterion_group!(
    throughput,
    bench_operation_throughput
);

criterion_group!(
    gops_performance,
    bench_gops_target,
    bench_aggregate_524_gops
);

criterion_main!(
    vector_arithmetic,
    matrix_operations,
    neural_primitives,
    throughput,
    gops_performance
);
