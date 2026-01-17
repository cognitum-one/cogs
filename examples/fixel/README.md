# FIXEL Simulator

A TypeScript simulation of the FIXEL (Finn Pixel) cognitive display architecture where each pixel contains its own cognitum chip.

## Overview

FIXEL is a revolutionary display architecture that embeds compute capability in every pixel:

| Tier | Resolution | Transistors/px | SRAM/px | Power/px | Cost |
|------|------------|---------------|---------|----------|------|
| **Nano** | 64×64 | 100 | 16B | 0.01 µW | $0.50 |
| **Micro** | 256×256 | 10K | 64B | 0.1 µW | $5 |
| **Standard** | 1920×1080 | 500K | 256B | 0.3 µW | $25 |
| **Pro** | 3840×2160 | 2.3M | 512B | 0.6 µW | $80 |
| **Ultra** | 7680×4320 | 2.3M | 1KB | 1.2 µW | $200 |

## Installation

```bash
cd examples/fixel
npm install
npm run build
```

## Quick Start

```typescript
import { Fabric, createStandardFabric, getTierSummary, STANDARD_TIER } from '@fixel/simulator';

// Create a Full HD fabric
const fabric = createStandardFabric();

// Load an image
const image = generateTestPattern(1920, 1080);
fabric.loadImage(image);

// Apply edge detection (Sobel)
fabric.convolve3x3([
  [-1, 0, 1],
  [-2, 0, 2],
  [-1, 0, 1]
]);
fabric.activate('relu');

// Get metrics
console.log(fabric.getMetrics());
console.log(getTierSummary(STANDARD_TIER));
```

## Running Simulations

```bash
# Edge detection demo
npm run demo:edge

# Conway's Game of Life
npm run demo:life

# Spiking neural network MNIST
npm run demo:snn

# Lattice Boltzmann fluid simulation
npm run demo:fluid
```

## Architecture

```
examples/fixel/
├── src/
│   ├── cognitum.ts      # Per-pixel compute unit
│   ├── fabric.ts        # 2D grid of cognitums
│   └── index.ts         # Main exports
├── specs/
│   ├── density-tiers.ts # Tier specifications
│   ├── tier-capabilities.md
│   └── cost-analysis.md
├── simulations/
│   ├── types.ts         # Shared types
│   ├── fabric.ts        # Fabric simulation
│   ├── neural.ts        # Neural networks (SNN, CNN, Reservoir)
│   └── *.ts             # Demo simulations
├── tests/
│   ├── test-runner.ts   # Test framework
│   └── cognitum.test.ts # Unit tests
└── docs/
    ├── adr/             # Architecture Decision Records
    └── ddd/             # Domain-Driven Design docs
```

## Key Features

### Cognitum (Per-Pixel Compute)
- Spiking neural network support (LIF neurons)
- 8-bit MAC operations
- Local SRAM storage
- 4-neighbor mesh interconnect

### Fabric (Pixel Grid)
- Convolution kernels (3×3, 5×5, NxN)
- Activation functions (ReLU, sigmoid, tanh)
- Tile-based reduction operations
- Spike propagation with weights
- Power and utilization metrics

### Neural Networks
- `SpikingLayer`: Leaky Integrate-and-Fire neurons
- `SpikingNetwork`: Multi-layer SNNs
- `ReservoirComputer`: Echo state networks
- `ConvLayer`: Convolution on fabric

## Testing

```bash
npm test
```

## Documentation

- [FIXEL Architecture](../../plans/fixel/FIXEL_ARCHITECTURE.md)
- [Technical Specification](../../plans/fixel/FIXEL_TECHNICAL_SPEC.md)
- [Intelligence Analysis](../../plans/fixel/FIXEL_INTELLIGENCE_ANALYSIS.md)
- [Cost Analysis](specs/cost-analysis.md)
- [Tier Capabilities](specs/tier-capabilities.md)

## License

MIT
