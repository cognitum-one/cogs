# Cognitum NAPI

High-performance Node.js native bindings for the Cognitum ASIC simulator using NAPI-RS.

## Features

- **Native performance** - Direct Rust-to-Node.js integration with zero overhead
- **Async/await support** - Non-blocking simulation using Tokio runtime
- **TypeScript support** - Auto-generated type definitions
- **Cross-platform** - Pre-built binaries for all major platforms
- **Thread-safe** - Safe concurrent access using RwLock

## Installation

```bash
npm install @ruv/cognitum
# or
yarn add @ruv/cognitum
```

## Usage

### Basic Example

```typescript
import { CognitumNode } from '@ruv/cognitum'

async function main() {
  // Create simulator with default config
  const sim = new CognitumNode()

  // Or with custom config
  const config = {
    num_tiles: 8,
    memory_size: 512 * 1024,
    clock_freq_mhz: 800,
    enable_debug: false,
  }
  const customSim = new CognitumNode(config)

  // Load program
  const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
  await sim.loadProgram(0, program)

  // Run simulation (async)
  await sim.runCycles(1000)

  // Get state
  const state = await sim.getState(0)
  console.log('Tile 0:', state)

  // Get total cycles
  const cycles = await sim.getTotalCycles()
  console.log(`Completed ${cycles} cycles`)
}

main()
```

### Synchronous Execution

```typescript
const sim = new CognitumNode()
const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
await sim.loadProgram(0, program)

// Run synchronously (blocking)
sim.runCyclesSync(1000)
```

### Register Access

```typescript
// Set register value
await sim.setRegister(0, 5, 42)

// Get register value
const value = await sim.getRegister(0, 5)
console.log('Register 5:', value)
```

### Performance Metrics

```typescript
const metrics = await sim.getMetrics()
console.log(`Total cycles: ${metrics.total_cycles}`)
console.log(`Active tiles: ${metrics.active_tiles}`)
console.log(`Average IPC: ${metrics.avg_ipc}`)
```

### All Tile States

```typescript
const states = await sim.getAllStates()
states.forEach((state) => {
  console.log(`Tile ${state.tile_id}: PC=${state.pc}, cycles=${state.cycle_count}`)
})
```

## API Reference

### `CognitumConfigNode`

```typescript
interface CognitumConfigNode {
  num_tiles: number
  memory_size: number
  clock_freq_mhz: number
  enable_debug: boolean
}
```

### `TileStateNode`

```typescript
interface TileStateNode {
  tile_id: number
  pc: number
  registers: number[]
  cycle_count: bigint
  status: string
}
```

### `PerformanceMetrics`

```typescript
interface PerformanceMetrics {
  total_cycles: bigint
  active_tiles: number
  total_instructions: bigint
  avg_ipc: number
}
```

### `CognitumNode`

```typescript
class CognitumNode {
  constructor(config?: CognitumConfigNode)

  loadProgram(tile_id: number, program: Buffer): Promise<void>

  runCycles(cycles: number): Promise<void>
  runCyclesSync(cycles: number): void

  getState(tile_id: number): Promise<TileStateNode>
  getAllStates(): Promise<TileStateNode[]>

  getRegister(tile_id: number, reg_num: number): Promise<number>
  setRegister(tile_id: number, reg_num: number, value: number): Promise<void>

  resetTile(tile_id: number): Promise<void>
  resetAll(): Promise<void>

  getTotalCycles(): Promise<bigint>
  isRunning(): Promise<boolean>

  getConfig(): Promise<CognitumConfigNode>
  getMetrics(): Promise<PerformanceMetrics>
}
```

## Building from Source

```bash
# Install dependencies
npm install

# Build debug version
npm run build:debug

# Build release version
npm run build

# Run tests
npm test
```

## Platform Support

Pre-built binaries are available for:

- **Linux**: x64, ARM64, ARM7 (glibc and musl)
- **macOS**: x64, ARM64 (Apple Silicon)
- **Windows**: x64, ARM64

## Performance

- **Execution speed**: 10-20M cycles/second (native)
- **Memory overhead**: ~1MB base + tile memory
- **Async overhead**: <1μs per operation

## Benchmarks

```bash
npm run bench
```

Expected results:
- Load program: ~0.01ms
- Run 1M cycles: ~50-100ms
- Get state: ~0.001ms
- Register access: ~0.0001ms

## Development

```bash
# Format code
npm run format

# Lint
npm run lint

# Build artifacts
npm run artifacts
```

## License

MIT OR Apache-2.0
