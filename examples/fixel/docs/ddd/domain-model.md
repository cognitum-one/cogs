# FIXEL Domain Model

## Overview

The FIXEL domain model represents a cognitive display architecture where computation is distributed across pixels organized into a hierarchical fabric structure.

## Core Domain Concepts

### 1. Cognitum (Value Object → Entity)

The fundamental computational unit embedded in each pixel.

```
Cognitum
├── Identity: Position (x, y)
├── State
│   ├── membrane: number (0-255)
│   ├── threshold: number (0-255)
│   ├── accumulator: number (16-bit)
│   └── refractory: number (cycles remaining)
├── Memory
│   ├── sram: Uint8Array (tier-dependent size)
│   └── registers: Uint8Array[16]
└── Behavior
    ├── integrate(input: number): void
    ├── fire(): SpikeEvent
    ├── execute(instruction: Instruction): void
    └── tick(time: number): SpikeEvent[]
```

**Invariants:**
- Membrane potential cannot exceed 255
- Accumulator saturates at INT16 bounds
- Refractory period prevents immediate re-firing
- Memory access is bounded by tier allocation

### 2. Fabric (Aggregate Root)

The 2D grid of Cognitums forming the computational surface.

```
Fabric
├── Identity: UUID
├── Dimensions: width × height
├── Tier: DensityTier
├── Cognitums: Map<Position, Cognitum>
├── TileControllers: Map<TileId, TileController>
└── Behavior
    ├── loadImage(pixels: number[][]): void
    ├── convolve(kernel: Kernel): void
    ├── propagateSpikes(weights: WeightMatrix): void
    ├── activate(fn: ActivationFunction): void
    ├── tileReduce(op: ReductionOp): number[][]
    └── getMetrics(): FabricMetrics
```

**Invariants:**
- All Cognitums must be within fabric bounds
- Tile boundaries must align with fabric dimensions
- Total power must not exceed tier budget

### 3. Tile (Entity)

A 16×16 group of Cognitums sharing resources.

```
Tile
├── Identity: TileId (tileX, tileY)
├── SharedMemory: Uint8Array (64KB)
│   ├── weightBuffer: 32KB
│   ├── featureBuffer: 16KB
│   └── controlBuffer: 16KB
├── Controller: TileController
└── Behavior
    ├── broadcast(data: Uint8Array, offset: number): void
    ├── reduce(op: ReductionOp): number
    ├── loadWeights(weights: Uint8Array): void
    └── synchronize(): void
```

### 4. Spike (Value Object)

An event representing neural activation.

```
Spike
├── source: Position
├── timestamp: number
├── amplitude: number (0-255)
└── propagationRadius: number
```

### 5. DensityTier (Value Object)

Configuration defining hardware capabilities.

```
DensityTier
├── name: 'NANO' | 'MICRO' | 'STANDARD' | 'PRO' | 'ULTRA'
├── resolution: { width: number, height: number }
├── transistorsPerPixel: number
├── sramPerPixel: number (bytes)
├── powerPerPixel: number (µW)
├── tileSharedMemory: number (bytes)
└── targetCost: number (USD)
```

## Domain Relationships

```
┌─────────────────────────────────────────────────────────────┐
│                         Fabric                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Tile (16×16)                      │   │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                   │   │
│  │  │Cogni│←→│Cogni│←→│Cogni│←→│Cogni│  ←── Mesh     │   │
│  │  └──↑──┘ └──↑──┘ └──↑──┘ └──↑──┘      Links       │   │
│  │     ↓       ↓       ↓       ↓                       │   │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                   │   │
│  │  │Cogni│←→│Cogni│←→│Cogni│←→│Cogni│               │   │
│  │  └─────┘ └─────┘ └─────┘ └─────┘                   │   │
│  │              ↑                                       │   │
│  │              │ Shared Memory Access                  │   │
│  │  ┌───────────┴────────────────────────┐             │   │
│  │  │      Tile Controller               │             │   │
│  │  │  ┌──────────────────────────────┐ │             │   │
│  │  │  │ Weight Buffer (32KB)         │ │             │   │
│  │  │  │ Feature Buffer (16KB)        │ │             │   │
│  │  │  │ Control Buffer (16KB)        │ │             │   │
│  │  │  └──────────────────────────────┘ │             │   │
│  │  └────────────────────────────────────┘             │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## State Transitions

### Cognitum Lifecycle

```
       ┌──────────────────────────────────────┐
       │                                      │
       ▼                                      │
   ┌───────┐    integrate()    ┌───────────┐ │
   │ IDLE  │ ─────────────────→│ INTEGRATING│ │
   └───────┘                   └───────────┘ │
       ▲                            │        │
       │                            │ threshold
       │                            │ reached
       │                            ▼        │
   ┌───────────┐   refractory  ┌───────┐    │
   │REFRACTORY │←──────────────│ FIRING │────┘
   └───────────┘   period ends └───────┘
                                    │
                                    │ emit spike
                                    ▼
                              [SpikeEvent]
```

### Fabric Operation Modes

```
   ┌──────────────┐
   │ INITIALIZING │
   └──────────────┘
          │
          │ loadImage() / configure()
          ▼
   ┌──────────────┐
   │   READY      │←──────────────────┐
   └──────────────┘                   │
          │                           │
          │ execute()                 │
          ▼                           │
   ┌──────────────┐    complete       │
   │  PROCESSING  │───────────────────┘
   └──────────────┘
          │
          │ error / halt
          ▼
   ┌──────────────┐
   │   HALTED     │
   └──────────────┘
```

## Bounded Context Integration

The FIXEL domain integrates with:

1. **Display Subsystem** (downstream)
   - Receives pixel values from Cognitum outputs
   - Manages refresh timing

2. **Host Interface** (upstream)
   - Loads neural network weights
   - Provides input images/streams
   - Retrieves classification results

3. **Power Management** (supporting)
   - Monitors per-tile power consumption
   - Enforces thermal throttling

See [Context Map](./context-map.md) for integration patterns.
