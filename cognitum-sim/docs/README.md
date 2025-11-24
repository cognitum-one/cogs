# Cognitum ASIC Simulator

A high-performance cycle-accurate simulator for the Cognitum 256-processor ASIC architecture written in Rust.

## Architecture Overview

Cognitum is a 256-processor ASIC organized in a 16x16 tile grid with the following key components:

### Processors
- **A2S (Adaptive Speculative Superscalar)** CPU cores
- RISC-V ISA with custom extensions
- Out-of-order execution with speculative execution
- 8-stage pipeline per core

### Memory Hierarchy
- **L1 Cache**: 32KB I-cache + 32KB D-cache per tile
- **L2 Cache**: 256KB shared per quadrant
- **L3 Cache**: 4MB shared across all tiles
- **ML-Enhanced Prefetching**: Predictive cache management

### Interconnect
- **RaceWay**: High-speed mesh interconnect
- Adaptive routing with congestion control
- 512-bit links between tiles
- Hardware-accelerated packet forwarding

### Coprocessors
- **Crypto Units**: AES, SHA, RSA acceleration
- **AI Accelerators**: Matrix multiplication, tensor ops
- **DSP Units**: Signal processing primitives

### I/O Interfaces
- PCIe Gen 5 (x16)
- 100GbE Ethernet
- USB 3.2 Gen 2

## Project Structure

```
cognitum-sim/
├── crates/
│   ├── cognitum-core/       # Core types and memory primitives
│   ├── cognitum-processor/  # A2S CPU implementation
│   ├── cognitum-memory/     # Memory subsystem
│   ├── cognitum-raceway/    # Interconnect fabric
│   ├── cognitum-coprocessor/# Crypto/AI accelerators
│   ├── cognitum-io/         # I/O interfaces
│   ├── cognitum-sim/        # Simulation engine
│   ├── cognitum-debug/      # Debugger tools
│   ├── cognitum-cli/        # CLI application
│   └── newport/            # Top-level library
├── docs/                   # Documentation
├── examples/               # Example programs
└── tests/                  # Integration tests
```

## Building

### Prerequisites
- Rust 1.75 or later
- Cargo

### Build all crates
```bash
cargo build --workspace --release
```

### Run tests
```bash
cargo test --workspace
```

### Run benchmarks
```bash
cargo bench --workspace
```

## Running Simulations

### Basic usage
```bash
cargo run --bin newport -- run --program examples/hello.bin --tiles 256
```

### Debug mode
```bash
cargo run --bin newport -- debug --program examples/test.bin --breakpoints 0x1000
```

### Trace execution
```bash
cargo run --bin newport -- run --program examples/app.bin --trace --trace-file output.trace
```

## WASM Support

Build for WebAssembly:
```bash
wasm-pack build crates/newport --target web
```

## NAPI Support

Build Node.js bindings:
```bash
napi build --platform --release crates/newport
```

## Documentation

Generate and open documentation:
```bash
cargo doc --workspace --no-deps --open
```

## Performance

The simulator is optimized for:
- Parallel execution using Tokio async runtime
- Lock-free data structures where possible
- SIMD operations for vector processing
- Memory-mapped I/O for large datasets

Expected performance:
- **Simulation Speed**: ~10M instructions/second per core
- **Memory Bandwidth**: ~100 GB/s simulated
- **Interconnect Latency**: <10ns simulated

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

Dual-licensed under MIT and Apache 2.0. See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).

## Authors

Created by rUv.io and TekStart.

## References

- [Architecture Documentation](architecture/)
- [API Reference](https://docs.rs/newport)
- [Design Decisions](design/)
