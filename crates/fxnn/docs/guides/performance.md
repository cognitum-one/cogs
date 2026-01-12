# Performance Tuning Guide

This guide covers optimizing FXNN simulations for maximum performance.

## Quick Performance Checklist

- [ ] Compile with `--release`
- [ ] Use appropriate cutoffs (2.5σ - 3.0σ)
- [ ] Enable `parallel` feature for >1000 atoms
- [ ] Use cell list neighbor searching
- [ ] Batch simulation steps

## Compilation

### Release Mode

Always use release builds for production:

```bash
cargo run --release --example my_simulation
```

Release mode enables:
- Full optimization (`-O3`)
- LTO (Link-Time Optimization)
- SIMD auto-vectorization

### Feature Flags

```toml
[dependencies]
fxnn = { version = "0.1", features = ["simd", "parallel"] }
```

| Feature | Speedup | When to Use |
|---------|---------|-------------|
| `simd` | 2-4x | Always (default) |
| `parallel` | N cores | >1000 atoms |

## Algorithmic Optimization

### Cutoff Selection

| Cutoff | Accuracy | Speed |
|--------|----------|-------|
| 2.5σ | Good | Fast |
| 3.0σ | Better | Moderate |
| 4.0σ | Excellent | Slow |

```rust
// 2.5σ cutoff is usually sufficient
let lj = LennardJones::new(1.0, 1.0, 2.5);
```

### Neighbor List Updates

```rust
// Neighbor list rebuilds every 20 steps by default
// Adjust based on system dynamics:

sim.set_neighbor_update_frequency(10);  // Fast-moving systems
sim.set_neighbor_update_frequency(50);  // Slow equilibrium
```

### Timestep

Larger timesteps = fewer calculations:

```rust
// NVE (energy conservation matters)
sim.with_timestep(0.001);  // Conservative

// NVT (thermostat corrects drift)
sim.with_timestep(0.002);  // Can be larger
```

## Memory Optimization

### Data Layout

FXNN uses cache-friendly data layouts:

```rust
// Atoms are stored contiguously
// Position, velocity, force data is interleaved
// for good cache locality
```

### Pre-allocation

```rust
// Reserve capacity for known system size
let mut atoms = Vec::with_capacity(10_000);
```

## Parallel Execution

### Enable Parallelism

```toml
[dependencies]
fxnn = { version = "0.1", features = ["parallel"] }
```

### Scaling

| Atoms | 1 Core | 4 Cores | 8 Cores |
|-------|--------|---------|---------|
| 1,000 | 50k/s | 50k/s | 50k/s |
| 10,000 | 5k/s | 18k/s | 30k/s |
| 100,000 | 400/s | 1.5k/s | 2.5k/s |

Parallelism helps most with >1000 atoms.

## WASM Optimization

### Build for Speed

```bash
# Use opt-level 3
RUSTFLAGS="-C opt-level=3" wasm-pack build --release
```

### Minimize Data Transfer

```javascript
// Bad: Transfer data every frame
for (let i = 0; i < 1000; i++) {
    sim.run(1);
    const pos = sim.get_positions();  // Expensive!
}

// Good: Batch steps, transfer once
sim.run(1000);
const pos = sim.get_positions();
```

### Use SharedArrayBuffer

```javascript
// Enable cross-origin isolation for SharedArrayBuffer
// This allows zero-copy data sharing with workers
```

## Benchmarking

### Run Benchmarks

```bash
cd crates/fxnn
cargo bench
```

### Key Metrics

| Benchmark | Target | Notes |
|-----------|--------|-------|
| LJ 1000 atoms | <500μs | Force calculation only |
| Full step 1000 | <2ms | Includes integration |
| SIMD distance | <100μs | 10k atom pairs |

### Profile

```bash
# Using flamegraph
cargo flamegraph --bench benchmarks

# Using perf
perf record --call-graph dwarf cargo bench
perf report
```

## Common Bottlenecks

### 1. Neighbor List Build

**Symptom**: Slow with many atoms
**Solution**: Increase skin distance, reduce update frequency

```rust
sim.set_neighbor_skin(0.5);  // Larger skin = fewer rebuilds
```

### 2. Force Calculation

**Symptom**: CPU-bound on large systems
**Solution**: Enable parallel feature, reduce cutoff

### 3. Memory Bandwidth

**Symptom**: Parallel scaling plateaus
**Solution**: Use Structure-of-Arrays layout

### 4. Branch Misprediction

**Symptom**: SIMD not helping
**Solution**: Ensure uniform workload (similar neighbor counts)

## Advanced Tuning

### SIMD Width

FXNN auto-detects SIMD:

```rust
// SSE: 4-wide (128-bit)
// AVX2: 8-wide (256-bit)
// AVX-512: 16-wide (512-bit)
```

### Cache Blocking

For very large systems (>100k atoms):

```rust
// Process atoms in cache-sized blocks
// This is handled automatically by the cell list
```

### Memory Alignment

```rust
// Atom data is 64-byte aligned for cache line efficiency
#[repr(C, align(64))]
pub struct Atom { ... }
```

## Monitoring

### Runtime Metrics

```rust
println!("Steps/second: {}", sim.steps_per_second());
println!("Time per step: {:?}", sim.avg_step_time());
```

### Energy Conservation

```rust
// Monitor energy drift in NVE
let e0 = sim.total_energy();
sim.run(10_000);
let e1 = sim.total_energy();
let drift = (e1 - e0).abs() / e0.abs() * 100.0;
println!("Energy drift: {:.4}%", drift);
```

## Summary

1. **Always use `--release`**
2. **Enable `parallel` for large systems**
3. **Use 2.5σ cutoff unless you need high accuracy**
4. **Batch steps, minimize data transfer in WASM**
5. **Profile before optimizing**
