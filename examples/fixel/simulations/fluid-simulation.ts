/**
 * FIXEL Fluid Simulation
 * Lattice Boltzmann Method (LBM) for fluid dynamics
 *
 * Demonstrates:
 * - D2Q9 velocity distribution per pixel
 * - Collision and streaming steps
 * - Boundary conditions
 * - Flow visualization
 * - Comparison to reference solutions
 */

import { Fabric } from './fabric.js';
import {
  DensityTier,
  SimulationResult,
  FluidCell,
  DENSITY_SPECS,
} from './types.js';

// D2Q9 lattice velocities
// Direction: 0=rest, 1-4=cardinal, 5-8=diagonal
const D2Q9_VELOCITIES = [
  [0, 0],   // 0: rest
  [1, 0],   // 1: east
  [0, 1],   // 2: north
  [-1, 0],  // 3: west
  [0, -1],  // 4: south
  [1, 1],   // 5: northeast
  [-1, 1],  // 6: northwest
  [-1, -1], // 7: southwest
  [1, -1],  // 8: southeast
];

// D2Q9 weights
const D2Q9_WEIGHTS = [
  4 / 9,   // 0: rest
  1 / 9,   // 1-4: cardinal
  1 / 9,
  1 / 9,
  1 / 9,
  1 / 36,  // 5-8: diagonal
  1 / 36,
  1 / 36,
  1 / 36,
];

// Opposite directions for bounce-back
const D2Q9_OPPOSITE = [0, 3, 4, 1, 2, 7, 8, 5, 6];

export interface FluidConfig {
  width: number;
  height: number;
  densityTier: DensityTier;
  viscosity: number;      // Kinematic viscosity
  inletVelocity: number;  // Inlet flow velocity
  reynoldsNumber?: number;
  obstacleType: 'none' | 'cylinder' | 'square' | 'airfoil';
}

export interface FluidResult extends SimulationResult {
  velocityField: { x: Float32Array; y: Float32Array };
  densityField: Float32Array;
  vorticityField: Float32Array;
  streamlines: number[][];
  timesteps: number;
  maxVelocity: number;
  dragCoefficient?: number;
  liftCoefficient?: number;
}

/**
 * Lattice Boltzmann fluid simulation on FIXEL fabric
 */
class LBMFluid {
  private width: number;
  private height: number;
  private viscosity: number;
  private omega: number;  // Relaxation parameter
  private f: Float32Array[];  // Distribution functions
  private fTemp: Float32Array[];  // Temporary for streaming
  private obstacle: Uint8Array;  // Obstacle mask
  private cycleCount: bigint = 0n;
  private startTime: number;
  private densityTier: DensityTier;

  constructor(config: FluidConfig) {
    this.width = config.width;
    this.height = config.height;
    this.viscosity = config.viscosity;
    this.densityTier = config.densityTier;

    // Relaxation parameter from viscosity
    this.omega = 1 / (3 * this.viscosity + 0.5);

    // Initialize distribution functions (9 directions per cell)
    const size = this.width * this.height;
    this.f = [];
    this.fTemp = [];
    for (let q = 0; q < 9; q++) {
      this.f.push(new Float32Array(size));
      this.fTemp.push(new Float32Array(size));
    }

    this.obstacle = new Uint8Array(size);

    this.initialize(config.inletVelocity);
    this.createObstacle(config.obstacleType);
    this.startTime = performance.now();
  }

  /**
   * Initialize with uniform flow
   */
  private initialize(inletVelocity: number): void {
    const rho0 = 1.0;  // Reference density
    const ux0 = inletVelocity;
    const uy0 = 0;

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const idx = y * this.width + x;

        // Compute equilibrium distribution
        for (let q = 0; q < 9; q++) {
          const cx = D2Q9_VELOCITIES[q][0];
          const cy = D2Q9_VELOCITIES[q][1];
          const cu = cx * ux0 + cy * uy0;
          const u2 = ux0 * ux0 + uy0 * uy0;

          this.f[q][idx] = D2Q9_WEIGHTS[q] * rho0 * (
            1 + 3 * cu + 4.5 * cu * cu - 1.5 * u2
          );
        }
      }
    }
  }

  /**
   * Create obstacle geometry
   */
  private createObstacle(type: string): void {
    const cx = Math.floor(this.width * 0.25);
    const cy = Math.floor(this.height / 2);
    const r = Math.floor(this.height / 8);

    switch (type) {
      case 'cylinder':
        for (let y = 0; y < this.height; y++) {
          for (let x = 0; x < this.width; x++) {
            const dist = Math.sqrt((x - cx) ** 2 + (y - cy) ** 2);
            if (dist <= r) {
              this.obstacle[y * this.width + x] = 1;
            }
          }
        }
        break;

      case 'square':
        for (let y = cy - r; y <= cy + r; y++) {
          for (let x = cx - r; x <= cx + r; x++) {
            if (y >= 0 && y < this.height && x >= 0 && x < this.width) {
              this.obstacle[y * this.width + x] = 1;
            }
          }
        }
        break;

      case 'airfoil':
        // NACA 0012 approximation
        for (let x = cx - r * 2; x <= cx + r * 2; x++) {
          const t = (x - (cx - r * 2)) / (r * 4);  // 0 to 1
          const thickness = 0.12 * r * 5 * (
            0.2969 * Math.sqrt(t) -
            0.1260 * t -
            0.3516 * t * t +
            0.2843 * t * t * t -
            0.1036 * t * t * t * t
          );

          const yTop = Math.floor(cy + thickness);
          const yBottom = Math.floor(cy - thickness);

          for (let y = yBottom; y <= yTop; y++) {
            if (y >= 0 && y < this.height && x >= 0 && x < this.width) {
              this.obstacle[y * this.width + x] = 1;
            }
          }
        }
        break;
    }
  }

  /**
   * Collision step (BGK approximation)
   */
  private collide(): void {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const idx = y * this.width + x;

        // Skip obstacles
        if (this.obstacle[idx]) continue;

        // Compute macroscopic quantities
        let rho = 0, ux = 0, uy = 0;
        for (let q = 0; q < 9; q++) {
          rho += this.f[q][idx];
          ux += D2Q9_VELOCITIES[q][0] * this.f[q][idx];
          uy += D2Q9_VELOCITIES[q][1] * this.f[q][idx];
        }
        ux /= rho;
        uy /= rho;

        // Compute equilibrium and relax
        const u2 = ux * ux + uy * uy;
        for (let q = 0; q < 9; q++) {
          const cx = D2Q9_VELOCITIES[q][0];
          const cy = D2Q9_VELOCITIES[q][1];
          const cu = cx * ux + cy * uy;

          const feq = D2Q9_WEIGHTS[q] * rho * (
            1 + 3 * cu + 4.5 * cu * cu - 1.5 * u2
          );

          this.f[q][idx] = this.f[q][idx] - this.omega * (this.f[q][idx] - feq);
        }
      }
    }

    this.cycleCount += BigInt(this.width * this.height * 9);
  }

  /**
   * Streaming step
   */
  private stream(): void {
    // Copy current distributions
    for (let q = 0; q < 9; q++) {
      this.fTemp[q].set(this.f[q]);
    }

    // Stream to neighbors
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const idx = y * this.width + x;

        for (let q = 0; q < 9; q++) {
          const cx = D2Q9_VELOCITIES[q][0];
          const cy = D2Q9_VELOCITIES[q][1];

          // Source position
          let sx = x - cx;
          let sy = y - cy;

          // Periodic boundaries in y
          if (sy < 0) sy = this.height - 1;
          if (sy >= this.height) sy = 0;

          // Inlet/outlet in x
          if (sx < 0) sx = 0;  // Use inlet value
          if (sx >= this.width) sx = this.width - 1;

          const sidx = sy * this.width + sx;

          // Bounce-back for obstacles
          if (this.obstacle[sidx]) {
            this.f[q][idx] = this.fTemp[D2Q9_OPPOSITE[q]][idx];
          } else {
            this.f[q][idx] = this.fTemp[q][sidx];
          }
        }
      }
    }

    this.cycleCount += BigInt(this.width * this.height * 9);
  }

  /**
   * Apply boundary conditions
   */
  private applyBoundaries(inletVelocity: number): void {
    // Inlet (left boundary): Zou-He velocity BC
    for (let y = 1; y < this.height - 1; y++) {
      const idx = y * this.width;
      const ux = inletVelocity;
      const uy = 0;

      const rho = (this.f[0][idx] + this.f[2][idx] + this.f[4][idx] +
                   2 * (this.f[3][idx] + this.f[6][idx] + this.f[7][idx])) /
                  (1 - ux);

      this.f[1][idx] = this.f[3][idx] + (2 / 3) * rho * ux;
      this.f[5][idx] = this.f[7][idx] + (1 / 6) * rho * ux -
                        0.5 * (this.f[2][idx] - this.f[4][idx]);
      this.f[8][idx] = this.f[6][idx] + (1 / 6) * rho * ux +
                        0.5 * (this.f[2][idx] - this.f[4][idx]);
    }

    // Outlet (right boundary): zero gradient
    for (let y = 0; y < this.height; y++) {
      const idx = y * this.width + (this.width - 1);
      const idxPrev = y * this.width + (this.width - 2);
      for (let q = 0; q < 9; q++) {
        this.f[q][idx] = this.f[q][idxPrev];
      }
    }

    // Top and bottom: no-slip walls
    for (let x = 0; x < this.width; x++) {
      // Bottom
      const idxBot = x;
      this.f[2][idxBot] = this.f[4][idxBot];
      this.f[5][idxBot] = this.f[7][idxBot];
      this.f[6][idxBot] = this.f[8][idxBot];

      // Top
      const idxTop = (this.height - 1) * this.width + x;
      this.f[4][idxTop] = this.f[2][idxTop];
      this.f[7][idxTop] = this.f[5][idxTop];
      this.f[8][idxTop] = this.f[6][idxTop];
    }
  }

  /**
   * Run one timestep
   */
  step(inletVelocity: number): void {
    this.collide();
    this.stream();
    this.applyBoundaries(inletVelocity);
  }

  /**
   * Get velocity field
   */
  getVelocity(): { x: Float32Array; y: Float32Array; magnitude: Float32Array } {
    const size = this.width * this.height;
    const ux = new Float32Array(size);
    const uy = new Float32Array(size);
    const mag = new Float32Array(size);

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const idx = y * this.width + x;

        if (this.obstacle[idx]) {
          ux[idx] = 0;
          uy[idx] = 0;
          continue;
        }

        let rho = 0, vx = 0, vy = 0;
        for (let q = 0; q < 9; q++) {
          rho += this.f[q][idx];
          vx += D2Q9_VELOCITIES[q][0] * this.f[q][idx];
          vy += D2Q9_VELOCITIES[q][1] * this.f[q][idx];
        }

        ux[idx] = vx / rho;
        uy[idx] = vy / rho;
        mag[idx] = Math.sqrt(ux[idx] ** 2 + uy[idx] ** 2);
      }
    }

    return { x: ux, y: uy, magnitude: mag };
  }

  /**
   * Get density field
   */
  getDensity(): Float32Array {
    const size = this.width * this.height;
    const rho = new Float32Array(size);

    for (let idx = 0; idx < size; idx++) {
      for (let q = 0; q < 9; q++) {
        rho[idx] += this.f[q][idx];
      }
    }

    return rho;
  }

  /**
   * Compute vorticity field
   */
  getVorticity(): Float32Array {
    const velocity = this.getVelocity();
    const vorticity = new Float32Array(this.width * this.height);

    for (let y = 1; y < this.height - 1; y++) {
      for (let x = 1; x < this.width - 1; x++) {
        const idx = y * this.width + x;

        // duy/dx - dux/dy
        const duyDx = (velocity.y[idx + 1] - velocity.y[idx - 1]) / 2;
        const duxDy = (velocity.x[idx + this.width] - velocity.x[idx - this.width]) / 2;

        vorticity[idx] = duyDx - duxDy;
      }
    }

    return vorticity;
  }

  /**
   * Compute drag and lift coefficients (for obstacle)
   */
  getForceCoefficients(inletVelocity: number): { drag: number; lift: number } {
    let fx = 0, fy = 0;

    // Sum momentum exchange at obstacle boundaries
    for (let y = 1; y < this.height - 1; y++) {
      for (let x = 1; x < this.width - 1; x++) {
        const idx = y * this.width + x;
        if (!this.obstacle[idx]) continue;

        // Check neighbors
        for (let q = 1; q < 9; q++) {
          const cx = D2Q9_VELOCITIES[q][0];
          const cy = D2Q9_VELOCITIES[q][1];
          const nx = x + cx;
          const ny = y + cy;

          if (nx >= 0 && nx < this.width && ny >= 0 && ny < this.height) {
            const nidx = ny * this.width + nx;
            if (!this.obstacle[nidx]) {
              // Momentum transfer
              fx += cx * (this.f[q][nidx] + this.f[D2Q9_OPPOSITE[q]][nidx]);
              fy += cy * (this.f[q][nidx] + this.f[D2Q9_OPPOSITE[q]][nidx]);
            }
          }
        }
      }
    }

    // Normalize by dynamic pressure
    const obstacleHeight = this.height / 4;  // Approximate
    const dynamicPressure = 0.5 * inletVelocity * inletVelocity * obstacleHeight;

    return {
      drag: fx / (dynamicPressure || 1),
      lift: fy / (dynamicPressure || 1),
    };
  }

  get cycles(): bigint {
    return this.cycleCount;
  }

  get size(): number {
    return this.width * this.height;
  }
}

/**
 * Run fluid simulation
 */
export async function runFluidSimulation(
  config: Partial<FluidConfig> = {},
  maxTimesteps: number = 1000
): Promise<FluidResult> {
  const fullConfig: FluidConfig = {
    width: config.width ?? 200,
    height: config.height ?? 80,
    densityTier: config.densityTier ?? 'medium',
    viscosity: config.viscosity ?? 0.02,
    inletVelocity: config.inletVelocity ?? 0.1,
    obstacleType: config.obstacleType ?? 'cylinder',
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  const fluid = new LBMFluid(fullConfig);

  // Run simulation
  for (let t = 0; t < maxTimesteps; t++) {
    fluid.step(fullConfig.inletVelocity);
  }

  // Collect results
  const velocity = fluid.getVelocity();
  const density = fluid.getDensity();
  const vorticity = fluid.getVorticity();
  const forces = fluid.getForceCoefficients(fullConfig.inletVelocity);

  // Find max velocity
  let maxVelocity = 0;
  for (let i = 0; i < velocity.magnitude.length; i++) {
    if (velocity.magnitude[i] > maxVelocity) {
      maxVelocity = velocity.magnitude[i];
    }
  }

  const wallTimeMs = performance.now() - startTime;
  const pixelCount = fullConfig.width * fullConfig.height;
  const powerMw = pixelCount * densitySpec.powerPerPixelMw;

  return {
    success: true,
    cycles: fluid.cycles,
    wallTimeMs,
    powerMw,
    outputData: velocity.magnitude,
    velocityField: { x: velocity.x, y: velocity.y },
    densityField: density,
    vorticityField: vorticity,
    streamlines: [],  // Would compute if needed
    timesteps: maxTimesteps,
    maxVelocity,
    dragCoefficient: forces.drag,
    liftCoefficient: forces.lift,
    metrics: {
      totalCycles: fluid.cycles,
      activeTiles: pixelCount,
      powerMw,
      throughputOpsPerSec: Number(fluid.cycles) / (wallTimeMs / 1000),
      memoryBandwidthGbps: (pixelCount * 9 * 4 * Number(fluid.cycles)) / (wallTimeMs * 1e6),
      utilizationPercent: 100,
    },
  };
}

/**
 * Compute reference solution (Poiseuille flow for validation)
 */
export function computePoiseuilleReference(
  height: number,
  viscosity: number,
  pressureGradient: number
): Float32Array {
  const uMax = (pressureGradient * height * height) / (8 * viscosity);
  const profile = new Float32Array(height);

  for (let y = 0; y < height; y++) {
    const yNorm = (y + 0.5) / height;  // 0 to 1
    profile[y] = 4 * uMax * yNorm * (1 - yNorm);
  }

  return profile;
}

/**
 * Compare simulation to reference (RMSE)
 */
export function compareToReference(
  simulated: Float32Array,
  reference: Float32Array
): { rmse: number; maxError: number; correlation: number } {
  const n = Math.min(simulated.length, reference.length);
  let mse = 0, maxError = 0;
  let sumSim = 0, sumRef = 0, sumSimSq = 0, sumRefSq = 0, sumProd = 0;

  for (let i = 0; i < n; i++) {
    const error = Math.abs(simulated[i] - reference[i]);
    mse += error * error;
    if (error > maxError) maxError = error;

    sumSim += simulated[i];
    sumRef += reference[i];
    sumSimSq += simulated[i] * simulated[i];
    sumRefSq += reference[i] * reference[i];
    sumProd += simulated[i] * reference[i];
  }

  mse /= n;
  const rmse = Math.sqrt(mse);

  // Pearson correlation
  const numr = n * sumProd - sumSim * sumRef;
  const denr = Math.sqrt((n * sumSimSq - sumSim * sumSim) *
                          (n * sumRefSq - sumRef * sumRef));
  const correlation = denr > 0 ? numr / denr : 0;

  return { rmse, maxError, correlation };
}

/**
 * Print flow field as ASCII
 */
function printFlowField(
  velocity: { x: Float32Array; y: Float32Array },
  width: number,
  height: number,
  downsample: number = 4
): void {
  const arrows = ['>', '^', '<', 'v', '/', '\\', '/', '\\', 'o'];

  for (let y = 0; y < height; y += downsample) {
    let row = '';
    for (let x = 0; x < width; x += downsample) {
      const idx = y * width + x;
      const vx = velocity.x[idx];
      const vy = velocity.y[idx];
      const mag = Math.sqrt(vx * vx + vy * vy);

      if (mag < 0.001) {
        row += '.';
      } else {
        // Determine direction
        const angle = Math.atan2(vy, vx);
        const octant = Math.round((angle + Math.PI) / (Math.PI / 4)) % 8;
        row += arrows[octant];
      }
    }
    console.log(row);
  }
}

/**
 * Print vorticity field as ASCII
 */
function printVorticityField(
  vorticity: Float32Array,
  width: number,
  height: number,
  downsample: number = 4
): void {
  // Normalize vorticity
  let maxVort = 0;
  for (let i = 0; i < vorticity.length; i++) {
    if (Math.abs(vorticity[i]) > maxVort) {
      maxVort = Math.abs(vorticity[i]);
    }
  }

  const chars = '-~=+*#@';

  for (let y = 0; y < height; y += downsample) {
    let row = '';
    for (let x = 0; x < width; x += downsample) {
      const idx = y * width + x;
      const normalized = maxVort > 0 ? vorticity[idx] / maxVort : 0;
      const level = Math.floor((normalized + 1) * 0.5 * (chars.length - 1));
      row += chars[Math.max(0, Math.min(chars.length - 1, level))];
    }
    console.log(row);
  }
}

// Main demonstration
async function main(): Promise<void> {
  console.log('=== FIXEL Fluid Simulation (Lattice Boltzmann) ===\n');

  // Test different obstacle types
  console.log('--- Obstacle Type Comparison ---');
  const obstacles: ('none' | 'cylinder' | 'square' | 'airfoil')[] = [
    'none', 'cylinder', 'square', 'airfoil'
  ];

  for (const obstacle of obstacles) {
    const result = await runFluidSimulation({
      width: 100,
      height: 40,
      densityTier: 'medium',
      viscosity: 0.02,
      inletVelocity: 0.1,
      obstacleType: obstacle,
    }, 500);

    console.log(`\n${obstacle.toUpperCase()}:`);
    console.log(`  Max velocity: ${result.maxVelocity.toFixed(4)}`);
    console.log(`  Cycles: ${result.cycles}`);
    console.log(`  Wall time: ${result.wallTimeMs.toFixed(2)} ms`);
    if (result.dragCoefficient !== undefined) {
      console.log(`  Drag coefficient: ${result.dragCoefficient.toFixed(4)}`);
      console.log(`  Lift coefficient: ${result.liftCoefficient!.toFixed(4)}`);
    }
  }

  // Density tier comparison
  console.log('\n\n--- Density Tier Performance ---');
  const tiers: DensityTier[] = ['low', 'medium', 'high', 'ultra'];

  for (const tier of tiers) {
    const result = await runFluidSimulation({
      width: 100,
      height: 40,
      densityTier: tier,
      obstacleType: 'cylinder',
    }, 500);

    console.log(`\n${tier.toUpperCase()}:`);
    console.log(`  Power: ${result.powerMw.toFixed(3)} mW`);
    console.log(`  Throughput: ${(result.metrics.throughputOpsPerSec / 1e6).toFixed(2)} M ops/sec`);
    console.log(`  Time: ${result.wallTimeMs.toFixed(2)} ms`);
  }

  // Visualize flow
  console.log('\n\n--- Flow Visualization (Cylinder wake) ---\n');
  const visResult = await runFluidSimulation({
    width: 80,
    height: 32,
    densityTier: 'medium',
    obstacleType: 'cylinder',
  }, 1000);

  console.log('Velocity field (arrows show direction):');
  printFlowField(visResult.velocityField, 80, 32, 2);

  console.log('\nVorticity field (- to @ shows magnitude):');
  printVorticityField(visResult.vorticityField, 80, 32, 2);

  // Validation against Poiseuille flow
  console.log('\n\n--- Validation: Poiseuille Flow ---');
  const validationResult = await runFluidSimulation({
    width: 100,
    height: 40,
    densityTier: 'medium',
    obstacleType: 'none',
    viscosity: 0.02,
    inletVelocity: 0.05,
  }, 2000);

  // Extract centerline profile
  const centerX = 75;  // Measure at 3/4 width
  const simProfile = new Float32Array(40);
  for (let y = 0; y < 40; y++) {
    simProfile[y] = validationResult.velocityField.x[y * 100 + centerX];
  }

  const refProfile = computePoiseuilleReference(40, 0.02, 0.001);
  const comparison = compareToReference(simProfile, refProfile);

  console.log(`RMSE vs Poiseuille: ${comparison.rmse.toFixed(6)}`);
  console.log(`Max error: ${comparison.maxError.toFixed(6)}`);
  console.log(`Correlation: ${comparison.correlation.toFixed(4)}`);
}

export { main };

if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}
