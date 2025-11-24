# Cognitum Simulator

High-performance ASIC simulator with dual bindings for browser (WASM) and Node.js (NAPI).

## 📦 Packages

### `cognitum-wasm` - WebAssembly Bindings
Browser-compatible simulator using WebAssembly for zero-install hardware simulation.

- **Target**: Web browsers, Deno, Cloudflare Workers
- **Size**: ~50KB gzipped
- **Performance**: ~1M cycles/second
- **Package**: `@ruv/cognitum-wasm`

[Documentation](./cognitum-wasm/README.md)

### `cognitum-napi` - Node.js Native Bindings
Native Node.js module using NAPI-RS for maximum performance.

- **Target**: Node.js 14+
- **Performance**: 10-20M cycles/second
- **Thread-safe**: RwLock-based concurrency
- **Package**: `@ruv/cognitum`

[Documentation](./cognitum-napi/README.md)

## 🚀 Quick Start

### WASM (Browser)

```bash
npm install @ruv/cognitum-wasm
```

```javascript
import init, { CognitumWasm, CognitumConfig } from '@ruv/cognitum-wasm';

await init();
const config = new CognitumConfig(16, 1024 * 1024, 1000);
const sim = new CognitumWasm(config);

const program = new Uint8Array([0x01, 0x02, 0x03, 0x04]);
sim.loadProgram(0, program);
await sim.runCycles(1000);

const state = sim.getState(0);
console.log('Tile 0:', state);
```

### NAPI (Node.js)

```bash
npm install @ruv/cognitum
```

```javascript
const { CognitumNode } = require('@ruv/cognitum');

const config = {
  num_tiles: 16,
  memory_size: 1024 * 1024,
  clock_freq_mhz: 1000,
  enable_debug: false,
};

const sim = new CognitumNode(config);
const program = Buffer.from([0x01, 0x02, 0x03, 0x04]);
await sim.loadProgram(0, program);
await sim.runCycles(1000);

const state = await sim.getState(0);
console.log('Tile 0:', state);
```

## 🛠️ Development

### Prerequisites

- **Rust**: 1.70+ with `wasm32-unknown-unknown` target
- **Node.js**: 14+
- **wasm-pack**: For WASM builds
- **Yarn**: For NAPI builds

### Build All

```bash
# Build both WASM and NAPI bindings
./scripts/build-all.sh

# Build release versions
./scripts/build-all.sh --release
```

### Test All

```bash
# Run all tests (WASM + NAPI)
./scripts/test-all.sh
```

### Individual Builds

#### WASM

```bash
cd cognitum-wasm

# Web target
npm run build

# Node.js target
npm run build:nodejs

# Bundler target
npm run build:bundler

# All targets
npm run build:all

# Run tests
npm test
```

#### NAPI

```bash
cd cognitum-napi

# Debug build
npm run build:debug

# Release build
npm run build

# Run tests
npm test

# Format code
npm run format

# Lint
npm run lint
```

## 📊 Performance Comparison

| Feature | WASM | NAPI |
|---------|------|------|
| **Speed** | ~1M cycles/sec | 10-20M cycles/sec |
| **Startup** | Instant | <10ms |
| **Memory** | Linear memory | Native heap |
| **Threading** | Web Workers | Tokio async |
| **Bundle Size** | ~50KB | Platform binary |
| **Platform** | Universal | Per-platform |

## 🏗️ Architecture

```
cognitum-sim/
├── cognitum-wasm/          # WebAssembly bindings
│   ├── src/
│   │   └── lib.rs        # wasm-bindgen implementation
│   ├── Cargo.toml
│   └── package.json
│
├── cognitum-napi/          # Node.js native bindings
│   ├── src/
│   │   └── lib.rs        # napi-rs implementation
│   ├── build.rs
│   ├── Cargo.toml
│   └── package.json
│
├── .github/workflows/
│   ├── wasm.yml          # WASM CI/CD
│   └── napi.yml          # NAPI CI/CD
│
└── scripts/
    ├── build-all.sh      # Build both targets
    ├── test-all.sh       # Test both targets
    └── publish.sh        # Publish to npm
```

## 🔄 CI/CD

### GitHub Actions

Both packages have automated CI/CD pipelines:

**WASM Pipeline** (`.github/workflows/wasm.yml`):
- Build for web, Node.js, and bundler targets
- Test in Chrome and Firefox
- Bundle size checking
- Linting and formatting

**NAPI Pipeline** (`.github/workflows/napi.yml`):
- Multi-platform builds (Linux, macOS, Windows)
- Cross-compilation for ARM
- Universal macOS binaries
- Automated npm publishing on tags

### Publishing

```bash
# Dry run (no actual publish)
./scripts/publish.sh --dry-run

# Publish to npm
./scripts/publish.sh
```

Or use GitHub releases:

```bash
git tag v0.1.0
git push origin v0.1.0
# GitHub Actions will automatically publish to npm
```

## 📝 API Reference

Both packages expose similar APIs with platform-specific optimizations.

### Configuration

```typescript
interface CognitumConfig {
  num_tiles: number;        // 1-255
  memory_size: number;      // Bytes per tile
  clock_freq_mhz: number;   // MHz
  enable_debug: boolean;
}
```

### Tile State

```typescript
interface TileState {
  tile_id: number;
  pc: number;
  registers: number[];
  cycle_count: bigint;
  status: string;
}
```

### Methods

- `loadProgram(tile_id, program)` - Load program into tile
- `runCycles(cycles)` - Run simulation
- `getState(tile_id)` - Get tile state
- `getAllStates()` - Get all tile states
- `getRegister(tile_id, reg_num)` - Read register
- `setRegister(tile_id, reg_num, value)` - Write register
- `resetTile(tile_id)` - Reset single tile
- `resetAll()` - Reset entire simulator
- `getTotalCycles()` - Get total cycle count
- `isRunning()` - Check if running

## 🧪 Testing

### WASM Tests

```bash
cd cognitum-wasm

# Chrome
wasm-pack test --headless --chrome

# Firefox
wasm-pack test --headless --firefox
```

### NAPI Tests

```bash
cd cognitum-napi

# Node.js test runner
npm test
```

## 📄 License

MIT OR Apache-2.0

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🔗 Links

- [WASM Package Documentation](./cognitum-wasm/README.md)
- [NAPI Package Documentation](./cognitum-napi/README.md)
- [GitHub Repository](https://github.com/ruv/cognitum)
- [npm - WASM Package](https://www.npmjs.com/package/@ruv/cognitum-wasm)
- [npm - NAPI Package](https://www.npmjs.com/package/@ruv/cognitum)
