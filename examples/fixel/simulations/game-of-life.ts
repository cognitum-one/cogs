/**
 * FIXEL Game of Life Simulation
 * Conway's Game of Life on neural fabric
 *
 * Demonstrates:
 * - Cellular automata on fabric
 * - Neighbor counting via local computation
 * - Rule application per pixel
 * - Measuring generations per second
 */

import { Fabric } from './fabric.js';
import {
  DensityTier,
  SimulationResult,
  DENSITY_SPECS,
} from './types.js';

export interface GameOfLifeConfig {
  width: number;
  height: number;
  densityTier: DensityTier;
  initialDensity: number;  // Probability of cell being alive initially
  wrapAround: boolean;     // Toroidal boundary conditions
}

export interface GameOfLifeResult extends SimulationResult {
  generations: number;
  generationsPerSecond: number;
  finalPopulation: number;
  populationHistory: number[];
  stableGeneration?: number;  // Generation where pattern stabilized
}

/**
 * Custom Game of Life fabric with proper rules
 */
class LifeFabric extends Fabric {
  private wrapAround: boolean;
  private cells: Uint8Array;

  constructor(width: number, height: number, densityTier: DensityTier, wrapAround: boolean) {
    super({ width, height, densityTier });
    this.wrapAround = wrapAround;
    this.cells = new Uint8Array(width * height);
  }

  /**
   * Initialize with random pattern
   */
  randomize(density: number): void {
    for (let i = 0; i < this.cells.length; i++) {
      this.cells[i] = Math.random() < density ? 1 : 0;
    }
    this.syncToPixels();
  }

  /**
   * Initialize with specific pattern
   */
  setPattern(pattern: number[][], offsetX: number = 0, offsetY: number = 0): void {
    this.cells.fill(0);

    for (let py = 0; py < pattern.length; py++) {
      for (let px = 0; px < pattern[py].length; px++) {
        const x = (offsetX + px) % this.width;
        const y = (offsetY + py) % this.height;
        if (pattern[py][px]) {
          this.cells[y * this.width + x] = 1;
        }
      }
    }

    this.syncToPixels();
  }

  /**
   * Sync cells to pixel values
   */
  private syncToPixels(): void {
    for (let i = 0; i < this.cells.length; i++) {
      this.setPixel(i % this.width, Math.floor(i / this.width), this.cells[i]);
    }
  }

  /**
   * Count live neighbors for a cell
   */
  private countNeighbors(x: number, y: number): number {
    let count = 0;

    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;

        let nx = x + dx;
        let ny = y + dy;

        if (this.wrapAround) {
          nx = (nx + this.width) % this.width;
          ny = (ny + this.height) % this.height;
        } else {
          if (nx < 0 || nx >= this.width || ny < 0 || ny >= this.height) {
            continue;
          }
        }

        count += this.cells[ny * this.width + nx];
      }
    }

    return count;
  }

  /**
   * Execute one generation of Game of Life
   */
  stepLife(): void {
    const newCells = new Uint8Array(this.cells.length);

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const idx = y * this.width + x;
        const neighbors = this.countNeighbors(x, y);
        const alive = this.cells[idx];

        // Conway's rules:
        // 1. Any live cell with 2 or 3 neighbors survives
        // 2. Any dead cell with exactly 3 neighbors becomes alive
        // 3. All other cells die or stay dead
        if (alive) {
          newCells[idx] = (neighbors === 2 || neighbors === 3) ? 1 : 0;
        } else {
          newCells[idx] = (neighbors === 3) ? 1 : 0;
        }
      }
    }

    this.cells = newCells;
    this.syncToPixels();
  }

  /**
   * Get current population (number of live cells)
   */
  getPopulation(): number {
    let count = 0;
    for (let i = 0; i < this.cells.length; i++) {
      count += this.cells[i];
    }
    return count;
  }

  /**
   * Get cells as array
   */
  getCells(): Uint8Array {
    return new Uint8Array(this.cells);
  }

  /**
   * Check if two cell states are equal
   */
  stateEquals(other: Uint8Array): boolean {
    if (this.cells.length !== other.length) return false;
    for (let i = 0; i < this.cells.length; i++) {
      if (this.cells[i] !== other[i]) return false;
    }
    return true;
  }
}

// Classic patterns
export const PATTERNS = {
  // Still lifes
  block: [
    [1, 1],
    [1, 1],
  ],

  beehive: [
    [0, 1, 1, 0],
    [1, 0, 0, 1],
    [0, 1, 1, 0],
  ],

  // Oscillators
  blinker: [
    [1, 1, 1],
  ],

  toad: [
    [0, 1, 1, 1],
    [1, 1, 1, 0],
  ],

  beacon: [
    [1, 1, 0, 0],
    [1, 1, 0, 0],
    [0, 0, 1, 1],
    [0, 0, 1, 1],
  ],

  pulsar: [
    [0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0],
  ],

  // Spaceships
  glider: [
    [0, 1, 0],
    [0, 0, 1],
    [1, 1, 1],
  ],

  lwss: [  // Lightweight spaceship
    [0, 1, 0, 0, 1],
    [1, 0, 0, 0, 0],
    [1, 0, 0, 0, 1],
    [1, 1, 1, 1, 0],
  ],

  // Methuselahs
  rPentomino: [
    [0, 1, 1],
    [1, 1, 0],
    [0, 1, 0],
  ],

  acorn: [
    [0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [1, 1, 0, 0, 1, 1, 1],
  ],

  // Guns
  gosperGliderGun: [
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,1,0,0,0,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,1,1],
    [0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,1,0,0,0,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,1,1],
    [1,1,0,0,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    [1,1,0,0,0,0,0,0,0,0,1,0,0,0,1,0,1,1,0,0,0,0,1,0,1,0,0,0,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
  ],
};

/**
 * Run Game of Life simulation
 */
export async function runGameOfLife(
  config: Partial<GameOfLifeConfig> = {},
  maxGenerations: number = 1000
): Promise<GameOfLifeResult> {
  const fullConfig: GameOfLifeConfig = {
    width: config.width ?? 64,
    height: config.height ?? 64,
    densityTier: config.densityTier ?? 'medium',
    initialDensity: config.initialDensity ?? 0.25,
    wrapAround: config.wrapAround ?? true,
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  const fabric = new LifeFabric(
    fullConfig.width,
    fullConfig.height,
    fullConfig.densityTier,
    fullConfig.wrapAround
  );

  // Initialize with random pattern
  fabric.randomize(fullConfig.initialDensity);

  const populationHistory: number[] = [];
  let generations = 0;
  let stableGeneration: number | undefined;
  let previousState = fabric.getCells();
  let previousPreviousState = new Uint8Array(previousState.length);

  // Run generations
  for (let gen = 0; gen < maxGenerations; gen++) {
    const population = fabric.getPopulation();
    populationHistory.push(population);

    // Check for extinction
    if (population === 0) {
      stableGeneration = gen;
      break;
    }

    // Step first, THEN check for stability
    fabric.stepLife();
    generations++;

    // Check for stability (period-1 or period-2 oscillator)
    // Compare NEW state after stepping with PREVIOUS state before stepping
    const currentState = fabric.getCells();
    if (fabric.stateEquals(previousState) ||
        (gen > 0 && fabric.stateEquals(previousPreviousState))) {
      stableGeneration = gen + 1;  // We already stepped, so report the generation we stabilized at
      break;
    }

    previousPreviousState = previousState;
    previousState = currentState;
  }

  const wallTimeMs = performance.now() - startTime;
  const generationsPerSecond = generations / (wallTimeMs / 1000);
  const finalPopulation = fabric.getPopulation();
  const pixelCount = fullConfig.width * fullConfig.height;
  const powerMw = pixelCount * densitySpec.powerPerPixelMw;

  return {
    success: true,
    cycles: BigInt(generations * pixelCount * 9), // 9 neighbors per cell
    wallTimeMs,
    powerMw,
    outputData: new Float32Array(fabric.getCells()),
    generations,
    generationsPerSecond,
    finalPopulation,
    populationHistory,
    stableGeneration,
    metrics: {
      totalCycles: BigInt(generations * pixelCount * 9),
      activeTiles: pixelCount,
      powerMw,
      throughputOpsPerSec: generationsPerSecond * pixelCount * 9,
      memoryBandwidthGbps: (pixelCount * 9 * generationsPerSecond) / 1e9,
      utilizationPercent: 100,
    },
  };
}

/**
 * Run with a specific pattern
 */
export async function runWithPattern(
  patternName: keyof typeof PATTERNS,
  config: Partial<GameOfLifeConfig> = {},
  maxGenerations: number = 1000
): Promise<GameOfLifeResult> {
  const fullConfig: GameOfLifeConfig = {
    width: config.width ?? 64,
    height: config.height ?? 64,
    densityTier: config.densityTier ?? 'medium',
    initialDensity: 0, // Not used for pattern
    wrapAround: config.wrapAround ?? true,
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  const fabric = new LifeFabric(
    fullConfig.width,
    fullConfig.height,
    fullConfig.densityTier,
    fullConfig.wrapAround
  );

  // Get pattern and center it
  const pattern = PATTERNS[patternName];
  const offsetX = Math.floor((fullConfig.width - pattern[0].length) / 2);
  const offsetY = Math.floor((fullConfig.height - pattern.length) / 2);
  fabric.setPattern(pattern, offsetX, offsetY);

  const populationHistory: number[] = [];
  let generations = 0;
  let stableGeneration: number | undefined;
  let previousState = fabric.getCells();
  let previousPreviousState = new Uint8Array(previousState.length);

  for (let gen = 0; gen < maxGenerations; gen++) {
    populationHistory.push(fabric.getPopulation());

    // Step first, THEN check for stability
    fabric.stepLife();
    generations++;

    // Check for stability (period-1 or period-2 oscillator)
    // Compare NEW state after stepping with PREVIOUS state before stepping
    const currentState = fabric.getCells();
    if (fabric.stateEquals(previousState) ||
        (gen > 0 && fabric.stateEquals(previousPreviousState))) {
      stableGeneration = gen + 1;  // We already stepped, so report the generation we stabilized at
      break;
    }

    previousPreviousState = previousState;
    previousState = currentState;
  }

  const wallTimeMs = performance.now() - startTime;
  const generationsPerSecond = generations / (wallTimeMs / 1000);
  const pixelCount = fullConfig.width * fullConfig.height;

  return {
    success: true,
    cycles: BigInt(generations * pixelCount * 9),
    wallTimeMs,
    powerMw: pixelCount * densitySpec.powerPerPixelMw,
    outputData: new Float32Array(fabric.getCells()),
    generations,
    generationsPerSecond,
    finalPopulation: fabric.getPopulation(),
    populationHistory,
    stableGeneration,
    metrics: {
      totalCycles: BigInt(generations * pixelCount * 9),
      activeTiles: pixelCount,
      powerMw: pixelCount * densitySpec.powerPerPixelMw,
      throughputOpsPerSec: generationsPerSecond * pixelCount * 9,
      memoryBandwidthGbps: 0,
      utilizationPercent: 100,
    },
  };
}

/**
 * Print grid as ASCII
 */
export function printGrid(cells: Uint8Array | Float32Array, width: number, height: number): void {
  for (let y = 0; y < height; y++) {
    let row = '';
    for (let x = 0; x < width; x++) {
      row += cells[y * width + x] > 0.5 ? '#' : '.';
    }
    console.log(row);
  }
}

// Main demonstration
async function main(): Promise<void> {
  console.log('=== FIXEL Game of Life Simulation ===\n');

  // Test at maximum speed
  console.log('--- Maximum Speed Benchmark ---');
  const tiers: DensityTier[] = ['low', 'medium', 'high', 'ultra'];

  for (const tier of tiers) {
    const result = await runGameOfLife({
      width: 128,
      height: 128,
      densityTier: tier,
      initialDensity: 0.25,
      wrapAround: true,
    }, 500);

    console.log(`\n${tier.toUpperCase()} tier:`);
    console.log(`  Generations: ${result.generations}`);
    console.log(`  Speed: ${result.generationsPerSecond.toFixed(1)} gens/sec`);
    console.log(`  Wall time: ${result.wallTimeMs.toFixed(2)} ms`);
    console.log(`  Power: ${result.powerMw.toFixed(3)} mW`);
    console.log(`  Final population: ${result.finalPopulation}`);
    if (result.stableGeneration !== undefined) {
      console.log(`  Stabilized at gen: ${result.stableGeneration}`);
    }
  }

  // Test classic patterns
  console.log('\n\n--- Classic Patterns ---');
  const patterns: (keyof typeof PATTERNS)[] = ['glider', 'blinker', 'rPentomino'];

  for (const patternName of patterns) {
    const result = await runWithPattern(patternName, {
      width: 40,
      height: 40,
      densityTier: 'medium',
    }, 200);

    console.log(`\n${patternName}:`);
    console.log(`  Generations run: ${result.generations}`);
    console.log(`  Speed: ${result.generationsPerSecond.toFixed(1)} gens/sec`);
    console.log(`  Final population: ${result.finalPopulation}`);
    if (result.stableGeneration !== undefined) {
      console.log(`  Stabilized at gen: ${result.stableGeneration}`);
    }
  }

  // Visual output of glider
  console.log('\n--- Glider Pattern (initial state) ---');
  const gliderResult = await runWithPattern('glider', { width: 20, height: 10 }, 0);
  printGrid(gliderResult.outputData, 20, 10);

  console.log('\n--- Glider Pattern (after 20 gens) ---');
  const gliderAfter = await runWithPattern('glider', { width: 20, height: 10 }, 20);
  printGrid(gliderAfter.outputData, 20, 10);
}

export { main };

if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}
