# Cognitum WASM

WebAssembly bindings for the Cognitum ASIC simulator, enabling browser-based hardware simulation.

## Features

- **Zero-copy memory access** - Direct access to WASM linear memory
- **Async execution** - Non-blocking simulation using JavaScript Promises
- **Multiple build targets** - Web, Node.js, and bundler support
- **TypeScript support** - Auto-generated type definitions
- **Small bundle size** - Optimized for web delivery (~50KB gzipped)

## Installation

```bash
npm install @ruv/cognitum-wasm
```

## Usage

### Web (ES Modules)

```javascript
import init, { CognitumWasm, CognitumConfig } from '@ruv/cognitum-wasm';

async function runSimulation() {
  // Initialize WASM module
  await init();

  // Create configuration
  const config = new CognitumConfig(16, 1024 * 1024, 1000);

  // Create simulator
  const sim = new CognitumWasm(config);

  // Load program
  const program = new Uint8Array([0x01, 0x02, 0x03, 0x04]);
  sim.loadProgram(0, program);

  // Run simulation
  await sim.runCycles(1000);

  // Get state
  const state = sim.getState(0);
  console.log('Tile 0 state:', state);

  // Get register value
  const reg1 = sim.getRegister(0, 1);
  console.log('Register 1:', reg1);
}

runSimulation();
```

### Node.js

```javascript
const { CognitumWasm, CognitumConfig } = require('@ruv/cognitum-wasm/pkg-node');

async function main() {
  const config = new CognitumConfig(8, 512 * 1024, 800);
  const sim = new CognitumWasm(config);

  await sim.runCycles(10000);
  const cycles = sim.getTotalCycles();
  console.log(`Completed ${cycles} cycles`);
}

main();
```

## Building

```bash
# Build for web
npm run build

# Build for Node.js
npm run build:nodejs

# Build for bundlers (webpack, etc.)
npm run build:bundler

# Build all targets
npm run build:all
```

## Testing

```bash
# Run tests in headless Chrome
npm test

# Run tests in Firefox
npm run test:firefox
```

## API Reference

### `CognitumConfig`

Configuration for the Cognitum simulator.

```typescript
class CognitumConfig {
  constructor(num_tiles: number, memory_size: number, clock_freq_mhz: number);
  num_tiles: number;
  memory_size: number;
  clock_freq_mhz: number;
  enableDebug(): void;
}
```

### `CognitumWasm`

Main simulator class.

```typescript
class CognitumWasm {
  constructor(config?: CognitumConfig);

  loadProgram(tile_id: number, program: Uint8Array): void;
  runCycles(cycles: number): Promise<void>;

  getState(tile_id: number): TileState;
  getAllStates(): TileState[];

  getRegister(tile_id: number, reg_num: number): number;
  setRegister(tile_id: number, reg_num: number, value: number): void;

  resetTile(tile_id: number): void;
  resetAll(): void;

  getTotalCycles(): bigint;
  isRunning(): boolean;
}
```

## Performance

- **Execution speed**: ~1M cycles/second (browser-dependent)
- **Memory usage**: ~10MB for 16 tiles with 1MB each
- **Bundle size**: ~50KB gzipped

## Browser Compatibility

- Chrome/Edge 91+
- Firefox 89+
- Safari 15+
- Node.js 14+

## License

MIT OR Apache-2.0
