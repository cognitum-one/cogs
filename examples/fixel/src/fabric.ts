/**
 * FIXEL Fabric Simulator
 *
 * The Fabric is a 2D grid of Cognitum chips that forms the cognitive display.
 * It provides mesh interconnect routing, hierarchical tile structure,
 * global clock distribution, and power management.
 */

import { Cognitum, CognitumConfig, SpikeEvent, CognitumMetrics } from './cognitum';

/**
 * Density tier specifications for different use cases
 */
export type DensityTier = 'nano' | 'micro' | 'standard' | 'pro' | 'ultra';

export interface DensityTierSpec {
  width: number;
  height: number;
  totalPixels: number;
  transistorsPerPixel: number;
  sramBytesPerPixel: number;
  useCase: string;
}

/**
 * Density tier specifications
 */
export const DENSITY_TIERS: Record<DensityTier, DensityTierSpec> = {
  nano: {
    width: 64,
    height: 64,
    totalPixels: 4096,
    transistorsPerPixel: 100,
    sramBytesPerPixel: 16,
    useCase: 'IoT devices'
  },
  micro: {
    width: 256,
    height: 256,
    totalPixels: 65536,
    transistorsPerPixel: 10000,
    sramBytesPerPixel: 64,
    useCase: 'Wearables'
  },
  standard: {
    width: 1920,
    height: 1080,
    totalPixels: 2073600,
    transistorsPerPixel: 500000,
    sramBytesPerPixel: 256,
    useCase: 'Tablets'
  },
  pro: {
    width: 3840,
    height: 2160,
    totalPixels: 8294400,
    transistorsPerPixel: 2300000,
    sramBytesPerPixel: 512,
    useCase: 'Monitors'
  },
  ultra: {
    width: 7680,
    height: 4320,
    totalPixels: 33177600,
    transistorsPerPixel: 2300000,
    sramBytesPerPixel: 1024,
    useCase: 'Workstations'
  }
};

/**
 * Fabric configuration
 */
export interface FabricConfig {
  width: number;
  height: number;
  densityTier: DensityTier;
  clockSpeed: number;  // MHz
  powerBudget: number; // Watts
}

/**
 * Tile structure for hierarchical organization
 */
export interface Tile {
  id: number;
  startX: number;
  startY: number;
  endX: number;
  endY: number;
  cognitums: Cognitum[];
  powerState: 'active' | 'idle' | 'sleep';
  powerBudget: number;
  currentPower: number;
}

/**
 * Tile metrics for monitoring
 */
export interface TileMetrics {
  tileId: number;
  powerConsumptionWatts: number;
  utilizationPercent: number;
  averageSpikeRate: number;
  activeCognitums: number;
}

/**
 * Fabric-wide metrics
 */
export interface FabricMetrics {
  totalPowerWatts: number;
  averageUtilization: number;
  totalSpikeRate: number;
  tileMetrics: TileMetrics[];
  activeTiles: number;
  cycleCount: number;
}

/**
 * Reduction operations for tile-level computation
 */
export type ReduceOp = 'sum' | 'max' | 'min';

/**
 * Wave propagation data
 */
export interface WaveData {
  sourceX: number;
  sourceY: number;
  timestamp: number;
  payload: Uint8Array;
  radius: number;
}

/**
 * FIXEL Fabric - 2D Grid of Cognitum Chips
 */
export class Fabric {
  readonly width: number;
  readonly height: number;
  readonly densityTier: DensityTier;
  readonly tierSpec: DensityTierSpec;
  readonly clockSpeed: number;
  readonly powerBudget: number;

  private grid: Cognitum[][];
  private tiles: Tile[];
  private readonly tileSize: number = 16;
  private tilesX: number;
  private tilesY: number;

  private cycleCount: number = 0;
  private globalClock: number = 0;
  private isRunning: boolean = false;

  // Pending waves for global wave propagation
  private pendingWaves: WaveData[] = [];

  /**
   * Create a new Fabric instance
   */
  constructor(config: FabricConfig) {
    const tierSpec = DENSITY_TIERS[config.densityTier];

    // Use provided dimensions or default to tier dimensions
    this.width = config.width || tierSpec.width;
    this.height = config.height || tierSpec.height;
    this.densityTier = config.densityTier;
    this.tierSpec = tierSpec;
    this.clockSpeed = config.clockSpeed;
    this.powerBudget = config.powerBudget;

    // Calculate tile dimensions
    this.tilesX = Math.ceil(this.width / this.tileSize);
    this.tilesY = Math.ceil(this.height / this.tileSize);

    // Initialize grid and tiles
    this.grid = this.initializeGrid();
    this.tiles = this.initializeTiles();
  }

  /**
   * Initialize the 2D grid of Cognitums
   */
  private initializeGrid(): Cognitum[][] {
    const grid: Cognitum[][] = [];

    for (let y = 0; y < this.height; y++) {
      const row: Cognitum[] = [];
      for (let x = 0; x < this.width; x++) {
        const config: CognitumConfig = {
          transistorCount: this.tierSpec.transistorsPerPixel,
          sramBytes: this.tierSpec.sramBytesPerPixel,
          clockSpeedMHz: this.clockSpeed,
          x,
          y
        };
        row.push(new Cognitum(config));
      }
      grid.push(row);
    }

    return grid;
  }

  /**
   * Initialize hierarchical tile structure (16x16 tiles)
   */
  private initializeTiles(): Tile[] {
    const tiles: Tile[] = [];
    let tileId = 0;
    const powerPerTile = this.powerBudget / (this.tilesX * this.tilesY);

    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        const startX = tx * this.tileSize;
        const startY = ty * this.tileSize;
        const endX = Math.min(startX + this.tileSize, this.width);
        const endY = Math.min(startY + this.tileSize, this.height);

        // Collect cognitums for this tile
        const cognitums: Cognitum[] = [];
        for (let y = startY; y < endY; y++) {
          for (let x = startX; x < endX; x++) {
            cognitums.push(this.grid[y][x]);
          }
        }

        tiles.push({
          id: tileId++,
          startX,
          startY,
          endX,
          endY,
          cognitums,
          powerState: 'active',
          powerBudget: powerPerTile,
          currentPower: 0
        });
      }
    }

    return tiles;
  }

  /**
   * Get a Cognitum at specific coordinates
   */
  getCognitum(x: number, y: number): Cognitum | null {
    if (x < 0 || x >= this.width || y < 0 || y >= this.height) {
      return null;
    }
    return this.grid[y][x];
  }

  /**
   * Get the tile containing a specific coordinate
   */
  getTileAt(x: number, y: number): Tile | null {
    const tx = Math.floor(x / this.tileSize);
    const ty = Math.floor(y / this.tileSize);
    const tileIndex = ty * this.tilesX + tx;

    if (tileIndex < 0 || tileIndex >= this.tiles.length) {
      return null;
    }
    return this.tiles[tileIndex];
  }

  /**
   * Get all tiles
   */
  getTiles(): Tile[] {
    return [...this.tiles];
  }

  /**
   * Broadcast a spike to all neighbors of a Cognitum (4-connected mesh)
   */
  neighborBroadcast(sourceX: number, sourceY: number, weight: number = 1.0): void {
    const spike: SpikeEvent = {
      sourceX,
      sourceY,
      timestamp: this.globalClock,
      weight
    };

    // 4-connected neighbors: up, down, left, right
    const neighbors = [
      { x: sourceX, y: sourceY - 1 },     // up
      { x: sourceX, y: sourceY + 1 },     // down
      { x: sourceX - 1, y: sourceY },     // left
      { x: sourceX + 1, y: sourceY }      // right
    ];

    for (const neighbor of neighbors) {
      const cognitum = this.getCognitum(neighbor.x, neighbor.y);
      if (cognitum) {
        cognitum.receiveSpike(spike);
      }
    }
  }

  /**
   * Broadcast a spike to 8-connected neighbors (includes diagonals)
   */
  neighborBroadcast8(sourceX: number, sourceY: number, weight: number = 1.0): void {
    const spike: SpikeEvent = {
      sourceX,
      sourceY,
      timestamp: this.globalClock,
      weight
    };

    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;
        const cognitum = this.getCognitum(sourceX + dx, sourceY + dy);
        if (cognitum) {
          // Diagonal connections have reduced weight
          const diagWeight = (dx !== 0 && dy !== 0) ? weight * 0.707 : weight;
          cognitum.receiveSpike({ ...spike, weight: diagWeight });
        }
      }
    }
  }

  /**
   * Perform a reduction operation across a tile
   */
  tileReduce(tileId: number, op: ReduceOp): number {
    const tile = this.tiles[tileId];
    if (!tile) {
      throw new Error(`Tile ${tileId} not found`);
    }

    const values = tile.cognitums.map(c => c.getState().membrane);

    switch (op) {
      case 'sum':
        return values.reduce((a, b) => a + b, 0);
      case 'max':
        return Math.max(...values);
      case 'min':
        return Math.min(...values);
      default:
        throw new Error(`Unknown reduce operation: ${op}`);
    }
  }

  /**
   * Perform a reduction across all tiles
   */
  globalReduce(op: ReduceOp): number {
    const tileResults = this.tiles.map((_, i) => this.tileReduce(i, op));

    switch (op) {
      case 'sum':
        return tileResults.reduce((a, b) => a + b, 0);
      case 'max':
        return Math.max(...tileResults);
      case 'min':
        return Math.min(...tileResults);
      default:
        throw new Error(`Unknown reduce operation: ${op}`);
    }
  }

  /**
   * Initiate a global wave from a source point
   * The wave propagates outward at one pixel per clock cycle
   */
  globalWave(sourceX: number, sourceY: number, payload: Uint8Array): void {
    this.pendingWaves.push({
      sourceX,
      sourceY,
      timestamp: this.globalClock,
      payload,
      radius: 0
    });
  }

  /**
   * Process pending wave propagation
   */
  private processWaves(): void {
    const newWaves: WaveData[] = [];

    for (const wave of this.pendingWaves) {
      wave.radius++;
      const maxRadius = Math.max(this.width, this.height);

      if (wave.radius <= maxRadius) {
        // Send spikes to cognitums at the current wavefront
        this.propagateWavefront(wave);
        newWaves.push(wave);
      }
    }

    this.pendingWaves = newWaves;
  }

  /**
   * Propagate a wavefront at the current radius
   */
  private propagateWavefront(wave: WaveData): void {
    const { sourceX, sourceY, radius, payload } = wave;

    // Propagate along the wavefront (circle approximation using Manhattan distance)
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const distance = Math.abs(x - sourceX) + Math.abs(y - sourceY);
        if (distance === radius) {
          const cognitum = this.getCognitum(x, y);
          if (cognitum) {
            // Attenuate weight with distance
            const weight = 1.0 / Math.sqrt(radius);
            cognitum.receiveSpike({
              sourceX,
              sourceY,
              timestamp: this.globalClock,
              weight,
              payload
            });
          }
        }
      }
    }
  }

  /**
   * Execute one clock cycle across the entire fabric
   */
  tick(): void {
    this.globalClock++;
    this.cycleCount++;

    // Process waves
    this.processWaves();

    // Execute all cognitums in parallel (simulated)
    const allSpikes: { x: number; y: number; spikes: SpikeEvent[] }[] = [];

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const cognitum = this.grid[y][x];
        const spikes = cognitum.tick(this.globalClock);
        if (spikes.length > 0) {
          allSpikes.push({ x, y, spikes });
        }
      }
    }

    // Route spikes through mesh interconnect
    for (const { x, y, spikes } of allSpikes) {
      for (const spike of spikes) {
        this.neighborBroadcast(x, y, spike.weight);
      }
    }

    // Update tile power consumption
    this.updateTilePower();
  }

  /**
   * Execute multiple clock cycles
   */
  run(cycles: number): void {
    this.isRunning = true;
    for (let i = 0; i < cycles && this.isRunning; i++) {
      this.tick();
    }
    this.isRunning = false;
  }

  /**
   * Stop execution
   */
  stop(): void {
    this.isRunning = false;
  }

  /**
   * Update power consumption for each tile
   */
  private updateTilePower(): void {
    for (const tile of this.tiles) {
      if (tile.powerState === 'sleep') {
        tile.currentPower = 0;
        continue;
      }

      let totalPowerUw = 0;
      for (const cognitum of tile.cognitums) {
        const metrics = cognitum.getMetrics(this.cycleCount);
        totalPowerUw += metrics.powerMicroWatts;
      }

      // Convert microWatts to Watts
      tile.currentPower = totalPowerUw / 1e6;

      // Throttle if over budget
      if (tile.currentPower > tile.powerBudget && tile.powerState === 'active') {
        tile.powerState = 'idle';
      }
    }
  }

  /**
   * Set tile power state
   */
  setTilePowerState(tileId: number, state: 'active' | 'idle' | 'sleep'): void {
    const tile = this.tiles[tileId];
    if (tile) {
      tile.powerState = state;
    }
  }

  /**
   * Get metrics for a specific tile
   */
  getTileMetrics(tileId: number): TileMetrics {
    const tile = this.tiles[tileId];
    if (!tile) {
      throw new Error(`Tile ${tileId} not found`);
    }

    let totalPowerUw = 0;
    let totalUtilization = 0;
    let totalSpikeRate = 0;
    let activeCognitums = 0;

    for (const cognitum of tile.cognitums) {
      const metrics = cognitum.getMetrics(this.cycleCount);
      totalPowerUw += metrics.powerMicroWatts;
      totalUtilization += metrics.utilization;
      totalSpikeRate += metrics.spikeRate;
      if (metrics.utilization > 0) {
        activeCognitums++;
      }
    }

    const count = tile.cognitums.length;

    return {
      tileId,
      powerConsumptionWatts: totalPowerUw / 1e6,
      utilizationPercent: (totalUtilization / count) * 100,
      averageSpikeRate: totalSpikeRate / count,
      activeCognitums
    };
  }

  /**
   * Get fabric-wide metrics
   */
  getMetrics(): FabricMetrics {
    const tileMetrics = this.tiles.map((_, i) => this.getTileMetrics(i));

    const totalPowerWatts = tileMetrics.reduce((sum, tm) => sum + tm.powerConsumptionWatts, 0);
    const avgUtilization = tileMetrics.reduce((sum, tm) => sum + tm.utilizationPercent, 0) / tileMetrics.length;
    const totalSpikeRate = tileMetrics.reduce((sum, tm) => sum + tm.averageSpikeRate * (this.tileSize * this.tileSize), 0);
    const activeTiles = tileMetrics.filter(tm => tm.activeCognitums > 0).length;

    return {
      totalPowerWatts,
      averageUtilization: avgUtilization,
      totalSpikeRate,
      tileMetrics,
      activeTiles,
      cycleCount: this.cycleCount
    };
  }

  /**
   * Get power consumption per tile as a 2D heatmap
   */
  getPowerHeatmap(): number[][] {
    const heatmap: number[][] = [];

    for (let ty = 0; ty < this.tilesY; ty++) {
      const row: number[] = [];
      for (let tx = 0; tx < this.tilesX; tx++) {
        const tileId = ty * this.tilesX + tx;
        const metrics = this.getTileMetrics(tileId);
        row.push(metrics.powerConsumptionWatts);
      }
      heatmap.push(row);
    }

    return heatmap;
  }

  /**
   * Get utilization statistics
   */
  getUtilizationStats(): { min: number; max: number; avg: number; stdDev: number } {
    const utilizations: number[] = [];

    for (const tile of this.tiles) {
      for (const cognitum of tile.cognitums) {
        const metrics = cognitum.getMetrics(this.cycleCount);
        utilizations.push(metrics.utilization);
      }
    }

    const sum = utilizations.reduce((a, b) => a + b, 0);
    const avg = sum / utilizations.length;
    const min = Math.min(...utilizations);
    const max = Math.max(...utilizations);

    const squaredDiffs = utilizations.map(u => Math.pow(u - avg, 2));
    const avgSquaredDiff = squaredDiffs.reduce((a, b) => a + b, 0) / squaredDiffs.length;
    const stdDev = Math.sqrt(avgSquaredDiff);

    return { min, max, avg, stdDev };
  }

  /**
   * Get spike rates across the fabric
   */
  getSpikeRates(): { total: number; perTile: number[]; perCognitum: number[][] } {
    let total = 0;
    const perTile: number[] = [];
    const perCognitum: number[][] = [];

    for (const tile of this.tiles) {
      let tileTotal = 0;
      for (const cognitum of tile.cognitums) {
        const metrics = cognitum.getMetrics(this.cycleCount);
        tileTotal += metrics.spikeRate;
        total += metrics.spikeRate;
      }
      perTile.push(tileTotal);
    }

    // Build 2D array of spike rates
    for (let y = 0; y < this.height; y++) {
      const row: number[] = [];
      for (let x = 0; x < this.width; x++) {
        const metrics = this.grid[y][x].getMetrics(this.cycleCount);
        row.push(metrics.spikeRate);
      }
      perCognitum.push(row);
    }

    return { total, perTile, perCognitum };
  }

  /**
   * Reset the entire fabric
   */
  reset(): void {
    this.cycleCount = 0;
    this.globalClock = 0;
    this.isRunning = false;
    this.pendingWaves = [];

    for (const row of this.grid) {
      for (const cognitum of row) {
        cognitum.reset();
      }
    }

    for (const tile of this.tiles) {
      tile.powerState = 'active';
      tile.currentPower = 0;
    }
  }

  /**
   * Get fabric configuration summary
   */
  getConfigSummary(): string {
    return [
      `FIXEL Fabric Configuration:`,
      `  Dimensions: ${this.width}x${this.height} (${this.width * this.height} pixels)`,
      `  Density Tier: ${this.densityTier} (${this.tierSpec.useCase})`,
      `  Transistors/Pixel: ${this.tierSpec.transistorsPerPixel.toLocaleString()}`,
      `  SRAM/Pixel: ${this.tierSpec.sramBytesPerPixel} bytes`,
      `  Clock Speed: ${this.clockSpeed} MHz`,
      `  Power Budget: ${this.powerBudget} W`,
      `  Tiles: ${this.tilesX}x${this.tilesY} (${this.tiles.length} total, ${this.tileSize}x${this.tileSize} each)`,
      `  Total SRAM: ${((this.width * this.height * this.tierSpec.sramBytesPerPixel) / (1024 * 1024)).toFixed(2)} MB`,
      `  Total Transistors: ${((this.width * this.height * this.tierSpec.transistorsPerPixel) / 1e9).toFixed(2)} billion`
    ].join('\n');
  }

  /**
   * Create a fabric from a density tier with default settings
   */
  static fromTier(tier: DensityTier, clockSpeed: number = 100, powerBudget: number = 10): Fabric {
    const spec = DENSITY_TIERS[tier];
    return new Fabric({
      width: spec.width,
      height: spec.height,
      densityTier: tier,
      clockSpeed,
      powerBudget
    });
  }
}
