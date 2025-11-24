# Cognitum CLI

Command-line interface for the Cognitum ASIC Simulator.

## Installation

```bash
cd cognitum-sim/crates/cognitum-cli
cargo build --release
```

The binary will be available at `target/release/newport`.

## Usage

### Run a Program

```bash
newport run --program fibonacci.bin --tiles 256 --cycles 10000
```

Options:
- `--program FILE`: Program binary to execute
- `--tiles N`: Number of tiles (1-256, default: 256)
- `--cycles N`: Maximum cycles to run
- `--trace`: Enable execution trace
- `--trace-file FILE`: Trace output file
- `--threads N`: Number of worker threads

### Load and Inspect

```bash
newport load --program test.bin --tile 0 --disassemble --memory
```

Options:
- `--program FILE`: Program binary
- `--tile N`: Target tile ID (0-255)
- `--disassemble`: Show disassembly
- `--memory`: Show memory layout

### Debug Mode

```bash
newport debug --program app.bin --breakpoints 0x100 0x200 --tile 0
```

Options:
- `--program FILE`: Program binary
- `--breakpoints ADDR...`: Breakpoint addresses (hex: 0x100)
- `--tile N`: Target tile (default: 0)
- `--pause`: Start paused

### Inspect State

```bash
newport inspect --tiles --memory --raceway --metrics
```

Options:
- `--tiles`: Show all tile states
- `--tile N`: Show specific tile details
- `--memory`: Show memory regions
- `--raceway`: Show RaceWay statistics
- `--metrics`: Show performance metrics

### Run Benchmarks

```bash
newport benchmark --suite basic --iterations 100 --format json
```

Options:
- `--suite NAME`: Benchmark suite (basic, communication, parallel, full)
- `--iterations N`: Number of iterations (default: 10)
- `--format FMT`: Output format (text, json, csv)
- `--output FILE`: Save results to file

## Configuration

Create a `newport.toml` configuration file:

```toml
[simulation]
event_driven = true
cycle_accurate = false
max_cycles = 10_000_000
performance_mode = true

[hardware]
tiles = 256
clock_freq_mhz = 1000

[logging]
level = "info"
trace_packets = false

[performance]
worker_threads = 8
metrics_enabled = true
```

Use with `--config`:

```bash
newport --config newport.toml run --program app.bin
```

## Examples

See the `examples/` directory for sample configuration files:
- `newport.toml`: Full configuration with all options
- `minimal.toml`: Minimal config for testing (4 tiles)

## Development

### Run Tests

```bash
cargo test
```

### Run with Debug Logging

```bash
newport --verbose run --program test.bin
```

Or set log level:

```bash
newport --log-level debug run --program test.bin
```

## Architecture

```
cognitum-cli/
├── src/
│   ├── main.rs           # CLI entry point with clap
│   ├── config.rs         # TOML configuration system
│   └── commands/
│       ├── run.rs        # Run simulation command
│       ├── debug.rs      # Interactive debugger
│       ├── load.rs       # Program loader and inspector
│       ├── inspect.rs    # State inspection
│       └── benchmark.rs  # Performance benchmarks
├── tests/
│   └── cli_tests.rs      # Integration tests
└── examples/
    ├── newport.toml      # Example config
    └── minimal.toml      # Minimal config
```

## License

MIT OR Apache-2.0
