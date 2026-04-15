# Cognitum SDK

High-level Rust SDK for the Cognitum ASIC Simulator.

## Overview

The Cognitum SDK provides a simple, ergonomic API for running simulations of the Cognitum 256-tile stack processor ASIC. Built on Tokio for efficient async execution with a builder-based configuration system.

## Features

- **Simple API**: High-level abstractions over complex simulator internals
- **Async/Await**: Built on Tokio for efficient async execution
- **Type Safety**: Strongly typed tile IDs, addresses, and registers
- **Builder Pattern**: Fluent configuration API
- **Comprehensive Errors**: Rich error types with context
- **Performance Metrics**: Built-in performance tracking and statistics

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
newport = "0.1"
tokio = { version = "1.35", features = ["full"] }
```

Basic usage:

```rust
use newport::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create simulator
    let mut cognitum = CognitumSDK::new()?;

    // Load program
    let program = vec![0x30, 0x31, 0x28, 0x34]; // ZERO, ONE, ADD, HALT
    newport.load_program(TileId(0), &program)?;

    // Run simulation
    let results = newport.run().await?;

    println!("Completed in {} cycles", results.cycles);
    println!("IPC: {:.2}", results.ipc());

    Ok(())
}
```

## Configuration

Use the builder pattern for custom configuration:

```rust
let config = CognitumConfig::builder()
    .tiles(128)                  // Use 128 tiles
    .trace(true)                 // Enable tracing
    .max_cycles(1_000_000)       // Limit cycles
    .worker_threads(16)          // 16 worker threads
    .build()?;

let mut cognitum = CognitumSDK::with_config(config)?;
```

## API Overview

### Core Types

- `CognitumSDK`: Main simulator interface
- `CognitumConfig`: Configuration builder
- `SimulationResults`: Results and statistics
- `TileId`: Tile identifier (0-255)
- `MemoryAddress`: 32-bit memory address
- `ProgramCounter`: Instruction pointer

### Main Methods

```rust
// Creation
CognitumSDK::new() -> Result<Self>
CognitumSDK::with_config(config) -> Result<Self>

// Program loading
load_program(tile: TileId, binary: &[u8]) -> Result<()>

// Execution
run() -> Result<SimulationResults>
run_cycles(cycles: u64) -> Result<SimulationResults>
step() -> Result<()>
reset()

// State
is_complete() -> bool
config() -> &CognitumConfig
```

### Configuration Options

```rust
CognitumConfigBuilder::
    tiles(n: usize)                         // Number of tiles (1-256)
    trace(enabled: bool)                    // Enable tracing
    trace_file(path: PathBuf)               // Trace output file
    max_cycles(cycles: u64)                 // Maximum cycles
    worker_threads(n: usize)                // Worker thread count
    packet_timeout(duration: Duration)      // RaceWay timeout
    parallel_execution(enabled: bool)       // Parallel execution
    random_seed(seed: u64)                  // Deterministic mode
    build() -> Result<CognitumConfig>
```

### Simulation Results

```rust
struct SimulationResults {
    cycles: u64,                // Total cycles
    instructions: u64,          // Instructions executed
    execution_time: Duration,   // Wall-clock time
    packets_sent: u64,          // RaceWay packets
    packets_received: u64,      // RaceWay packets
    active_tiles: usize,        // Active tile count
    halted_tiles: usize,        // Halted tiles
    error_tiles: usize,         // Tiles with errors
    max_stack_depth: usize,     // Max stack depth
    memory_operations: u64,     // Memory ops
}

// Methods
ipc() -> f64                    // Instructions per cycle
cycles_per_second() -> f64      // Performance
packet_delivery_ratio() -> f64  // Network efficiency
is_success() -> bool            // No errors
```

## Examples

See the `examples/` directory:

- `hello_newport.rs`: Basic simulation
- `config_builder.rs`: Custom configuration
- `multi_tile.rs`: Parallel tile execution

Run examples:

```bash
cargo run --example hello_newport
cargo run --example config_builder
cargo run --example multi_tile
```

## Error Handling

The SDK uses the `Result<T>` type with `CognitumError`:

```rust
pub enum CognitumError {
    Simulation(String),      // Simulation errors
    LoadError(String),       // Program loading
    ConfigError(String),     // Configuration
    TileError(u8, String),   // Tile-specific
    MemoryError(u32, String),// Memory access
    RaceWayError(String),    // Communication
    Timeout(u64),            // Execution timeout
    IoError(io::Error),      // I/O errors
    General(anyhow::Error),  // Other errors
}
```

## Features

### Optional Features

- `coprocessor`: Enable cryptographic coprocessors
- `io`: Enable external I/O interfaces
- `debug`: Enable debugging tools
- `serde`: Serialization support
- `full`: All optional features

Enable in `Cargo.toml`:

```toml
[dependencies]
newport = { version = "0.1", features = ["full"] }
```

## Architecture

```
newport/
├── src/
│   ├── lib.rs       # Main library
│   ├── prelude.rs   # Convenient imports
│   ├── sdk.rs       # CognitumSDK implementation
│   ├── config.rs    # Configuration builder
│   ├── results.rs   # Simulation results
│   └── error.rs     # Error types
├── examples/
│   ├── hello_newport.rs
│   ├── config_builder.rs
│   └── multi_tile.rs
└── tests/
    └── integration_tests.rs
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_sdk_creation
```

## Documentation

Generate documentation:

```bash
cargo doc --open
```

## License

MIT OR Apache-2.0

## Related Crates

- `cognitum-cli`: Command-line interface
- `cognitum-core`: Core simulator types
- `cognitum-processor`: Processor implementation
- `cognitum-memory`: Memory system
- `cognitum-raceway`: Interconnect
