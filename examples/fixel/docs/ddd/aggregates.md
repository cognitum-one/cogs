# FIXEL Aggregates and Entities

## Aggregate Design Principles

FIXEL follows these aggregate design principles:
1. **Single aggregate root per cluster** - Fabric is the primary root
2. **Invariant protection** - All rules enforced within aggregate
3. **Transactional consistency** - Operations complete atomically
4. **Eventual consistency** - Cross-aggregate references by ID

---

## Aggregate 1: Fabric (Root)

The primary aggregate containing the entire computational surface.

### Aggregate Boundary

```
┌─────────────────────────────────────────────────────────────┐
│                      FABRIC AGGREGATE                        │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  Fabric (Root)                       │   │
│  │  - id: FabricId                                     │   │
│  │  - tier: DensityTier                                │   │
│  │  - dimensions: { width, height }                    │   │
│  │  - state: FabricState                               │   │
│  │  - metrics: FabricMetrics                           │   │
│  └─────────────────────────────────────────────────────┘   │
│           │                                                  │
│           │ contains                                         │
│           ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                 Tiles (Entity)                       │   │
│  │  - tileId: TileId                                   │   │
│  │  - position: { tileX, tileY }                       │   │
│  │  - sharedMemory: TileMemory                         │   │
│  │  - controller: TileController                       │   │
│  └─────────────────────────────────────────────────────┘   │
│           │                                                  │
│           │ contains                                         │
│           ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │               Cognitums (Entity)                     │   │
│  │  - position: { x, y }                               │   │
│  │  - state: CognitumState                             │   │
│  │  - memory: Uint8Array                               │   │
│  │  - neighbors: NeighborLinks                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                              │
│  Invariants:                                                 │
│  - All cognitums within bounds                              │
│  - Tile count = ceil(width/16) × ceil(height/16)           │
│  - Total power ≤ tier.maxPower                              │
│  - Memory allocation ≤ tier.sramPerPixel × pixelCount      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Implementation

```typescript
// Fabric Aggregate Root
class Fabric {
  private readonly id: FabricId;
  private readonly tier: DensityTier;
  private readonly tiles: Map<string, Tile>;
  private state: FabricState;

  // Factory method - only way to create
  static create(tier: DensityTier): Fabric {
    const fabric = new Fabric(FabricId.generate(), tier);
    fabric.initializeTiles();
    fabric.validateInvariants();
    return fabric;
  }

  // Aggregate operations
  loadImage(pixels: number[][]): void {
    this.assertState('READY');
    this.forEachCognitum((cog, x, y) => {
      cog.setValue(pixels[y][x]);
    });
    this.validateInvariants();
  }

  convolve(kernel: Kernel): void {
    this.assertState('READY');
    this.transitionTo('PROCESSING');

    for (const tile of this.tiles.values()) {
      tile.broadcastKernel(kernel);
      tile.executeConvolution();
    }

    this.transitionTo('READY');
  }

  // Invariant protection
  private validateInvariants(): void {
    const totalPower = this.calculateTotalPower();
    if (totalPower > this.tier.maxPower) {
      throw new InvariantViolation('Power budget exceeded');
    }
  }
}
```

---

## Aggregate 2: NeuralModel

A separate aggregate for neural network configuration.

### Aggregate Boundary

```
┌─────────────────────────────────────────────────────────────┐
│                   NEURAL MODEL AGGREGATE                     │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                NeuralModel (Root)                    │   │
│  │  - id: ModelId                                      │   │
│  │  - name: string                                     │   │
│  │  - architecture: NetworkArchitecture                │   │
│  │  - quantization: QuantizationConfig                 │   │
│  │  - totalParams: number                              │   │
│  └─────────────────────────────────────────────────────┘   │
│           │                                                  │
│           │ contains                                         │
│           ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                 Layers (Entity)                      │   │
│  │  - layerId: LayerId                                 │   │
│  │  - type: 'conv' | 'dense' | 'pool' | 'activation'  │   │
│  │  - inputShape: Shape                                │   │
│  │  - outputShape: Shape                               │   │
│  │  - weights: WeightTensor (nullable)                 │   │
│  └─────────────────────────────────────────────────────┘   │
│           │                                                  │
│           │ contains (optional)                              │
│           ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │               WeightTensor (Value Object)            │   │
│  │  - shape: number[]                                  │   │
│  │  - data: Int8Array                                  │   │
│  │  - scale: number                                    │   │
│  │  - zeroPoint: number                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                              │
│  Invariants:                                                 │
│  - Layer shapes must be compatible (output → input)         │
│  - Weights quantized to INT8                                │
│  - Total size ≤ target tier memory                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Entity Definitions

### Tile (Entity within Fabric)

```typescript
class Tile {
  readonly tileId: TileId;
  private cognitums: Cognitum[][];
  private sharedMemory: TileMemory;
  private controller: TileController;

  // Identity based on position
  equals(other: Tile): boolean {
    return this.tileId.equals(other.tileId);
  }

  broadcastKernel(kernel: Kernel): void {
    this.controller.broadcast(kernel.toBytes());
  }

  reduce(op: ReductionOp): number {
    return this.controller.reduce(this.cognitums, op);
  }
}
```

### Cognitum (Entity within Tile)

```typescript
class Cognitum {
  readonly x: number;
  readonly y: number;
  private state: CognitumState;
  private memory: Uint8Array;

  get id(): string {
    return `${this.x},${this.y}`;
  }

  integrate(input: number): void {
    this.state.membrane += input;
    this.state.membrane = Math.min(this.state.membrane, 255);
  }

  tick(): SpikeEvent | null {
    // Leak
    this.state.membrane = Math.floor(this.state.membrane * 0.95);

    // Fire?
    if (this.state.membrane >= this.state.threshold) {
      this.state.membrane = 0;
      return new SpikeEvent(this.x, this.y, Date.now());
    }
    return null;
  }
}
```

---

## Value Objects

### Position

```typescript
class Position {
  constructor(readonly x: number, readonly y: number) {
    if (x < 0 || y < 0) throw new Error('Position must be non-negative');
  }

  equals(other: Position): boolean {
    return this.x === other.x && this.y === other.y;
  }

  neighbors(): Position[] {
    return [
      new Position(this.x, this.y - 1), // North
      new Position(this.x + 1, this.y), // East
      new Position(this.x, this.y + 1), // South
      new Position(this.x - 1, this.y), // West
    ];
  }
}
```

### Spike

```typescript
class Spike {
  constructor(
    readonly source: Position,
    readonly timestamp: number,
    readonly amplitude: number
  ) {
    if (amplitude < 0 || amplitude > 255) {
      throw new Error('Amplitude must be 0-255');
    }
  }

  attenuate(distance: number): Spike {
    const newAmplitude = Math.floor(this.amplitude / (1 + distance));
    return new Spike(this.source, this.timestamp, newAmplitude);
  }
}
```

### DensityTier

```typescript
class DensityTier {
  private constructor(
    readonly name: TierName,
    readonly resolution: Resolution,
    readonly transistorsPerPixel: number,
    readonly sramPerPixel: number,
    readonly powerPerPixel: number,
    readonly targetCost: number
  ) {}

  static readonly NANO = new DensityTier('NANO', { width: 64, height: 64 }, 100, 16, 0.01, 0.50);
  static readonly MICRO = new DensityTier('MICRO', { width: 256, height: 256 }, 10000, 64, 0.1, 5);
  static readonly STANDARD = new DensityTier('STANDARD', { width: 1920, height: 1080 }, 500000, 256, 0.3, 25);
  static readonly PRO = new DensityTier('PRO', { width: 3840, height: 2160 }, 2300000, 512, 0.6, 80);
  static readonly ULTRA = new DensityTier('ULTRA', { width: 7680, height: 4320 }, 2300000, 1024, 1.2, 200);

  get pixelCount(): number {
    return this.resolution.width * this.resolution.height;
  }

  get totalMemory(): number {
    return this.pixelCount * this.sramPerPixel;
  }
}
```

---

## Cross-Aggregate References

Aggregates reference each other by ID only:

```typescript
class InferenceSession {
  constructor(
    private readonly fabricId: FabricId,  // Reference, not object
    private readonly modelId: ModelId      // Reference, not object
  ) {}

  async execute(
    fabricRepo: FabricRepository,
    modelRepo: ModelRepository
  ): Promise<InferenceResult> {
    const fabric = await fabricRepo.findById(this.fabricId);
    const model = await modelRepo.findById(this.modelId);
    return this.runInference(fabric, model);
  }
}
```
