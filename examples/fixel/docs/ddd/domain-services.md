# FIXEL Domain Services

## Overview

Domain services encapsulate operations that don't naturally belong to a single entity or value object. In FIXEL, these are primarily coordination and transformation operations.

---

## Service 1: InferenceOrchestrator

Coordinates neural network inference across fabric and model aggregates.

```typescript
interface InferenceOrchestrator {
  execute(
    session: InferenceSession,
    fabric: Fabric,
    model: NeuralModel,
    input: ImageBuffer
  ): Promise<InferenceResult>;

  streamExecute(
    session: InferenceSession,
    fabric: Fabric,
    model: NeuralModel,
    frames: AsyncIterable<ImageBuffer>
  ): AsyncIterable<InferenceResult>;
}

class InferenceOrchestratorImpl implements InferenceOrchestrator {
  constructor(
    private readonly powerService: PowerManagementService,
    private readonly metricsService: MetricsCollector
  ) {}

  async execute(
    session: InferenceSession,
    fabric: Fabric,
    model: NeuralModel,
    input: ImageBuffer
  ): Promise<InferenceResult> {
    await this.powerService.checkBudget(fabric.tier, model.estimatedPower);
    fabric.loadImage(input);
    session.markStarted();

    for (const layer of model.layers) {
      session.startLayer(layer.index);

      switch (layer.type) {
        case 'conv':
          fabric.convolve(layer.kernel);
          break;
        case 'activation':
          fabric.activate(layer.activationType);
          break;
        case 'pool':
          fabric.pool(layer.poolSize, layer.poolType);
          break;
        case 'dense':
          await this.executeDenseLayer(fabric, layer);
          break;
      }

      session.completeLayer(layer.index, fabric.getMetrics().cyclesUsed);

      if (this.powerService.isOverBudget(fabric)) {
        await this.powerService.throttle(fabric);
      }
    }

    const output = fabric.extractOutput();
    session.markCompleted();

    return new InferenceResult(output, session.totalCycles, session.energyUsed);
  }
}
```

---

## Service 2: WeightDeploymentService

Handles quantization and deployment of neural network weights to fabric.

```typescript
interface WeightDeploymentService {
  deploy(
    model: NeuralModel,
    fabric: Fabric,
    strategy: DeploymentStrategy
  ): Promise<DeploymentResult>;

  updateWeights(
    fabric: Fabric,
    layerIndex: number,
    deltaWeights: WeightTensor
  ): Promise<void>;

  estimateMemory(
    model: NeuralModel,
    tier: DensityTier
  ): MemoryEstimate;
}

class WeightDeploymentServiceImpl implements WeightDeploymentService {
  async deploy(
    model: NeuralModel,
    fabric: Fabric,
    strategy: DeploymentStrategy
  ): Promise<DeploymentResult> {
    const plan = this.createDeploymentPlan(model, fabric, strategy);

    for (const assignment of plan.tileAssignments) {
      const tile = fabric.getTile(assignment.tileId);
      const weights = model.getLayerWeights(assignment.layerIndex);
      const quantized = this.ensureQuantized(weights);
      const tileLayout = this.transformLayout(quantized, assignment.accessPattern);
      await tile.loadWeights(tileLayout, assignment.offset);
    }

    return new DeploymentResult(plan.tileAssignments.length, plan.totalBytes);
  }

  estimateMemory(model: NeuralModel, tier: DensityTier): MemoryEstimate {
    const modelSize = model.totalSize();
    const tileMemory = tier.tileSharedMemory;
    const tilesNeeded = Math.ceil(modelSize / tileMemory);
    const tilesAvailable = Math.ceil(tier.pixelCount / 256);

    return {
      modelSize,
      tilesNeeded,
      tilesAvailable,
      fits: tilesNeeded <= tilesAvailable,
      utilizationPercent: (tilesNeeded / tilesAvailable) * 100
    };
  }
}
```

---

## Service 3: SpikeRoutingService

Manages spike propagation across the fabric mesh.

```typescript
interface SpikeRoutingService {
  propagate(
    fabric: Fabric,
    spikes: Spike[],
    connections: SynapticConnections
  ): PropagationResult;

  planRouting(
    connectivity: ConnectivityPattern,
    fabricDimensions: Dimensions
  ): RoutingPlan;

  applySTDP(
    connections: SynapticConnections,
    preSpikes: Spike[],
    postSpikes: Spike[],
    learningRate: number
  ): SynapticConnections;
}

class SpikeRoutingServiceImpl implements SpikeRoutingService {
  propagate(
    fabric: Fabric,
    spikes: Spike[],
    connections: SynapticConnections
  ): PropagationResult {
    const deliveredSpikes: DeliveredSpike[] = [];
    let totalHops = 0;

    for (const spike of spikes) {
      const targets = connections.getTargets(spike.source);

      for (const target of targets) {
        const route = this.calculateRoute(spike.source, target.position);
        const weight = target.weight;
        const amplitude = Math.floor(spike.amplitude * weight);
        const attenuated = Math.floor(amplitude / (1 + route.length * 0.1));

        if (attenuated > 0) {
          fabric.getCognitum(target.position).integrate(attenuated);
          deliveredSpikes.push({ spike, target: target.position, amplitude: attenuated, hops: route.length });
          totalHops += route.length;
        }
      }
    }

    return {
      delivered: deliveredSpikes.length,
      dropped: spikes.length * connections.avgFanout - deliveredSpikes.length,
      totalHops,
      avgLatency: totalHops / Math.max(deliveredSpikes.length, 1)
    };
  }

  // XY routing (deadlock-free)
  private calculateRoute(src: Position, dst: Position): Direction[] {
    const route: Direction[] = [];
    let current = src;

    while (current.x !== dst.x) {
      route.push(current.x < dst.x ? 'EAST' : 'WEST');
      current = new Position(current.x + (current.x < dst.x ? 1 : -1), current.y);
    }

    while (current.y !== dst.y) {
      route.push(current.y < dst.y ? 'SOUTH' : 'NORTH');
      current = new Position(current.x, current.y + (current.y < dst.y ? 1 : -1));
    }

    return route;
  }
}
```

---

## Service 4: PowerManagementService

Monitors and controls power consumption.

```typescript
interface PowerManagementService {
  checkBudget(tier: DensityTier, estimatedPower: number): Promise<boolean>;
  getCurrentPower(fabric: Fabric): PowerReport;
  throttle(fabric: Fabric): Promise<void>;
  setPowerMode(fabric: Fabric, mode: PowerMode): void;
}

class PowerManagementServiceImpl implements PowerManagementService {
  private readonly THERMAL_WARNING = 85;  // degrees C
  private readonly THERMAL_CRITICAL = 95;

  async checkBudget(tier: DensityTier, estimatedPower: number): Promise<boolean> {
    return estimatedPower <= tier.maxPower * 0.9; // 90% headroom
  }

  getCurrentPower(fabric: Fabric): PowerReport {
    const tileReports = new Map<TileId, TilePowerReport>();
    let totalPower = 0;

    for (const tile of fabric.tiles) {
      const activeCognitums = tile.countActive();
      const activePower = activeCognitums * fabric.tier.powerPerPixel;
      const leakagePower = 256 * fabric.tier.powerPerPixel * 0.1;

      tileReports.set(tile.id, {
        activePower,
        leakagePower,
        temperature: this.estimateTemperature(activePower + leakagePower),
        utilization: activeCognitums / 256
      });

      totalPower += activePower + leakagePower;
    }

    return { timestamp: Date.now(), tileReports, totalPower };
  }

  async throttle(fabric: Fabric): Promise<void> {
    const report = this.getCurrentPower(fabric);

    const hotTiles = Array.from(report.tileReports.entries())
      .filter(([_, r]) => r.temperature > this.THERMAL_WARNING)
      .sort((a, b) => b[1].temperature - a[1].temperature);

    for (const [tileId, tileReport] of hotTiles) {
      if (tileReport.temperature > this.THERMAL_CRITICAL) {
        fabric.getTile(tileId).halt();
      } else {
        fabric.getTile(tileId).reduceClock(0.5);
      }
    }
  }

  setPowerMode(fabric: Fabric, mode: PowerMode): void {
    const configs: Record<PowerMode, { divider: number; gateIdle: boolean }> = {
      performance: { divider: 1, gateIdle: false },
      balanced: { divider: 2, gateIdle: false },
      efficiency: { divider: 4, gateIdle: true },
      standby: { divider: 8, gateIdle: true }
    };

    const config = configs[mode];
    fabric.setClockDivider(config.divider);
    if (config.gateIdle) fabric.gateIdleTiles();
  }
}
```

---

## Service 5: MetricsCollectionService

Collects and aggregates performance metrics.

```typescript
interface MetricsCollectionService {
  startCollection(sessionId: SessionId): void;
  recordOperation(sessionId: SessionId, operation: OperationType, cycles: number, energy: number): void;
  getMetrics(sessionId: SessionId): SessionMetrics;
  export(sessionId: SessionId, format: ExportFormat): string;
}

class MetricsCollectionServiceImpl implements MetricsCollectionService {
  private sessions = new Map<string, MetricsAccumulator>();

  startCollection(sessionId: SessionId): void {
    this.sessions.set(sessionId.value, new MetricsAccumulator());
  }

  recordOperation(sessionId: SessionId, operation: OperationType, cycles: number, energy: number): void {
    const acc = this.sessions.get(sessionId.value);
    if (!acc) throw new Error(`Session ${sessionId.value} not found`);
    acc.add({ timestamp: Date.now(), operation, cycles, energy_uJ: energy });
  }

  getMetrics(sessionId: SessionId): SessionMetrics {
    const acc = this.sessions.get(sessionId.value);
    if (!acc) throw new Error(`Session ${sessionId.value} not found`);

    return {
      totalCycles: acc.totalCycles,
      totalEnergy_uJ: acc.totalEnergy,
      operationBreakdown: acc.operationCounts,
      throughput_fps: acc.calculateThroughput(),
      efficiency_TOpsPerW: acc.calculateEfficiency()
    };
  }
}
```

---

## Service Registry

```typescript
class DomainServiceRegistry {
  private static instance: DomainServiceRegistry;
  private readonly services = new Map<string, any>();

  static getInstance(): DomainServiceRegistry {
    if (!this.instance) {
      this.instance = new DomainServiceRegistry();
      this.instance.registerDefaults();
    }
    return this.instance;
  }

  private registerDefaults(): void {
    const metrics = new MetricsCollectionServiceImpl();
    const power = new PowerManagementServiceImpl();

    this.register('inference', new InferenceOrchestratorImpl(power, metrics));
    this.register('weights', new WeightDeploymentServiceImpl());
    this.register('spikes', new SpikeRoutingServiceImpl());
    this.register('power', power);
    this.register('metrics', metrics);
  }

  register<T>(name: string, service: T): void {
    this.services.set(name, service);
  }

  get<T>(name: string): T {
    const service = this.services.get(name);
    if (!service) throw new Error(`Service not found: ${name}`);
    return service as T;
  }
}

// Usage
const inference = DomainServiceRegistry.getInstance().get<InferenceOrchestrator>('inference');
await inference.execute(session, fabric, model, input);
```
