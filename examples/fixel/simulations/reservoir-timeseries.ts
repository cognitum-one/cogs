/**
 * FIXEL Reservoir Time Series Prediction
 * Echo State Network / Liquid State Machine for temporal processing
 *
 * Demonstrates:
 * - Injecting signals at edge pixels
 * - Reservoir dynamics evolution
 * - Training linear readout layer
 * - Predicting future values
 */

import { ReservoirComputer } from './neural.js';
import { Fabric } from './fabric.js';
import {
  DensityTier,
  SimulationResult,
  TimeSeriesPoint,
  ReservoirState,
  DENSITY_SPECS,
} from './types.js';

export interface ReservoirConfig {
  reservoirSize: number;
  inputSize: number;
  outputSize: number;
  spectralRadius: number;
  leakRate: number;
  sparsity: number;
  densityTier: DensityTier;
}

export interface ReservoirResult extends SimulationResult {
  predictions: Float32Array[];
  groundTruth: Float32Array[];
  mse: number;
  rmse: number;
  mae: number;
  reservoirState: ReservoirState;
  trainingMse: number;
}

// Time series generators
export const TIME_SERIES = {
  /**
   * Mackey-Glass chaotic time series
   */
  mackeyGlass(length: number, tau: number = 17): Float32Array {
    const data = new Float32Array(length);
    const history = new Float32Array(tau + 1).fill(0.9);
    let x = 0.9;

    for (let t = 0; t < length; t++) {
      const xTau = history[t % (tau + 1)];
      const dx = (0.2 * xTau) / (1 + Math.pow(xTau, 10)) - 0.1 * x;
      x = x + dx;
      history[(t + 1) % (tau + 1)] = x;
      data[t] = x;
    }

    // Normalize to [0, 1]
    let min = Infinity, max = -Infinity;
    for (let i = 0; i < length; i++) {
      if (data[i] < min) min = data[i];
      if (data[i] > max) max = data[i];
    }
    for (let i = 0; i < length; i++) {
      data[i] = (data[i] - min) / (max - min);
    }

    return data;
  },

  /**
   * Lorenz attractor (x component)
   */
  lorenz(length: number, dt: number = 0.01): Float32Array {
    const data = new Float32Array(length);
    let x = 1, y = 1, z = 1;
    const sigma = 10, rho = 28, beta = 8 / 3;

    for (let t = 0; t < length; t++) {
      const dx = sigma * (y - x);
      const dy = x * (rho - z) - y;
      const dz = x * y - beta * z;

      x += dx * dt;
      y += dy * dt;
      z += dz * dt;

      data[t] = x;
    }

    // Normalize
    let min = Infinity, max = -Infinity;
    for (let i = 0; i < length; i++) {
      if (data[i] < min) min = data[i];
      if (data[i] > max) max = data[i];
    }
    for (let i = 0; i < length; i++) {
      data[i] = (data[i] - min) / (max - min);
    }

    return data;
  },

  /**
   * Sinusoidal with multiple frequencies
   */
  multiSine(length: number): Float32Array {
    const data = new Float32Array(length);

    for (let t = 0; t < length; t++) {
      data[t] = 0.5 + 0.3 * Math.sin(2 * Math.PI * t / 50) +
                      0.15 * Math.sin(2 * Math.PI * t / 20) +
                      0.05 * Math.sin(2 * Math.PI * t / 7);
    }

    return data;
  },

  /**
   * NARMA (Nonlinear AutoRegressive Moving Average)
   */
  narma(length: number, order: number = 10): Float32Array {
    const data = new Float32Array(length);
    const input = new Float32Array(length);

    // Random input
    for (let t = 0; t < length; t++) {
      input[t] = Math.random() * 0.5;
    }

    // NARMA equation
    for (let t = order; t < length; t++) {
      let sum = 0;
      for (let i = 0; i < order; i++) {
        sum += data[t - 1 - i];
      }

      data[t] = 0.3 * data[t - 1] +
                0.05 * data[t - 1] * sum +
                1.5 * input[t - order] * input[t - 1] +
                0.1;

      // Clip to prevent explosion
      data[t] = Math.max(0, Math.min(1, data[t]));
    }

    return data;
  },

  /**
   * Square wave with noise
   */
  squareWave(length: number, period: number = 50): Float32Array {
    const data = new Float32Array(length);

    for (let t = 0; t < length; t++) {
      const base = (Math.floor(t / period) % 2 === 0) ? 0.8 : 0.2;
      data[t] = base + (Math.random() - 0.5) * 0.1;
    }

    return data;
  },
};

/**
 * Create time-delayed embedding from 1D series
 */
function createEmbedding(
  data: Float32Array,
  embeddingDim: number,
  delay: number = 1
): Float32Array[] {
  const result: Float32Array[] = [];

  for (let t = (embeddingDim - 1) * delay; t < data.length; t++) {
    const embedding = new Float32Array(embeddingDim);
    for (let d = 0; d < embeddingDim; d++) {
      embedding[d] = data[t - d * delay];
    }
    result.push(embedding);
  }

  return result;
}

/**
 * Reservoir computing on neural fabric
 */
export class FabricReservoir {
  private fabric: Fabric;
  private outputWeights: Float32Array;
  private spectralRadius: number;
  private leakRate: number;
  private cycleCount: bigint = 0n;
  private startTime: number;

  constructor(
    width: number,
    height: number,
    outputSize: number,
    options: {
      spectralRadius?: number;
      leakRate?: number;
      densityTier?: DensityTier;
    } = {}
  ) {
    this.fabric = new Fabric({
      width,
      height,
      densityTier: options.densityTier ?? 'medium',
    });

    this.spectralRadius = options.spectralRadius ?? 0.9;
    this.leakRate = options.leakRate ?? 0.3;
    this.outputWeights = new Float32Array(outputSize * width * height);
    this.startTime = performance.now();

    this.fabric.initialize();
  }

  /**
   * Inject signal at left edge and let dynamics evolve
   */
  update(input: Float32Array): void {
    // Inject at left edge
    this.fabric.injectEdge(input);

    // Let reservoir dynamics evolve
    this.fabric.step();

    this.cycleCount += BigInt(this.fabric.size);
  }

  /**
   * Read from right edge as output
   */
  readout(): Float32Array {
    const state = this.fabric.readEdge();
    const outputSize = this.outputWeights.length / this.fabric.size;
    const output = new Float32Array(outputSize);

    // Linear readout
    for (let i = 0; i < outputSize; i++) {
      let sum = 0;
      for (let j = 0; j < state.length; j++) {
        sum += state[j] * this.outputWeights[i * state.length + j];
      }
      output[i] = sum;
    }

    return output;
  }

  /**
   * Get full reservoir state
   */
  getState(): Float32Array {
    return this.fabric.getPixels();
  }

  /**
   * Train output weights using collected states and targets
   */
  train(
    states: Float32Array[],
    targets: Float32Array[],
    regularization: number = 1e-6
  ): number {
    const stateSize = states[0].length;
    const outputSize = targets[0].length;
    const n = states.length;

    // Simple gradient descent training
    const learningRate = 0.01;
    const epochs = 100;
    let finalMse = 0;

    for (let epoch = 0; epoch < epochs; epoch++) {
      let mse = 0;

      for (let t = 0; t < n; t++) {
        // Forward pass
        const output = new Float32Array(outputSize);
        for (let i = 0; i < outputSize; i++) {
          for (let j = 0; j < stateSize; j++) {
            output[i] += states[t][j] * this.outputWeights[i * stateSize + j];
          }
        }

        // Compute error and update weights
        for (let i = 0; i < outputSize; i++) {
          const error = output[i] - targets[t][i];
          mse += error * error;

          for (let j = 0; j < stateSize; j++) {
            this.outputWeights[i * stateSize + j] -=
              learningRate * error * states[t][j];
          }
        }
      }

      finalMse = mse / (n * outputSize);
    }

    return finalMse;
  }

  /**
   * Reset reservoir state
   */
  reset(): void {
    this.fabric.reset();
  }

  get cycles(): bigint {
    return this.cycleCount;
  }
}

/**
 * Run reservoir time series prediction
 */
export async function runReservoirPrediction(
  config: Partial<ReservoirConfig> = {},
  seriesType: keyof typeof TIME_SERIES = 'mackeyGlass',
  seriesLength: number = 2000,
  predictionHorizon: number = 50
): Promise<ReservoirResult> {
  const fullConfig: ReservoirConfig = {
    reservoirSize: config.reservoirSize ?? 500,
    inputSize: config.inputSize ?? 1,
    outputSize: config.outputSize ?? 1,
    spectralRadius: config.spectralRadius ?? 0.9,
    leakRate: config.leakRate ?? 0.3,
    sparsity: config.sparsity ?? 0.1,
    densityTier: config.densityTier ?? 'medium',
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  // Generate time series
  const series = TIME_SERIES[seriesType](seriesLength);

  // Create reservoir
  const reservoir = new ReservoirComputer(
    fullConfig.reservoirSize,
    fullConfig.inputSize,
    fullConfig.outputSize,
    {
      spectralRadius: fullConfig.spectralRadius,
      leakRate: fullConfig.leakRate,
      sparsity: fullConfig.sparsity,
      densityTier: fullConfig.densityTier,
    }
  );

  // Split into train and test
  const trainSize = Math.floor(seriesLength * 0.7);
  const testSize = seriesLength - trainSize;
  const washout = 100;

  // Prepare training data
  const trainInputs: Float32Array[] = [];
  const trainTargets: Float32Array[] = [];

  for (let t = 0; t < trainSize - 1; t++) {
    trainInputs.push(new Float32Array([series[t]]));
    trainTargets.push(new Float32Array([series[t + 1]]));
  }

  // Train reservoir
  const trainingMse = reservoir.train(trainInputs, trainTargets);

  // Test: predict future values
  const predictions: Float32Array[] = [];
  const groundTruth: Float32Array[] = [];

  // Reset and warm up
  reservoir.reset();
  for (let t = 0; t < trainSize; t++) {
    reservoir.update(new Float32Array([series[t]]));
  }

  // Multi-step prediction
  for (let t = 0; t < predictionHorizon; t++) {
    const pred = reservoir.readout();
    predictions.push(new Float32Array(pred));

    const actual = series[trainSize + t];
    groundTruth.push(new Float32Array([actual]));

    // Feed prediction back (autoregressive)
    reservoir.update(pred);
  }

  // Calculate metrics
  let mse = 0, mae = 0;
  for (let t = 0; t < predictions.length; t++) {
    const error = predictions[t][0] - groundTruth[t][0];
    mse += error * error;
    mae += Math.abs(error);
  }
  mse /= predictions.length;
  mae /= predictions.length;
  const rmse = Math.sqrt(mse);

  const wallTimeMs = performance.now() - startTime;
  const metrics = reservoir.getMetrics();

  return {
    success: true,
    cycles: metrics.totalCycles,
    wallTimeMs,
    powerMw: metrics.powerMw,
    accuracy: 1 - rmse, // Approximate accuracy
    outputData: new Float32Array(predictions.flatMap(p => Array.from(p))),
    predictions,
    groundTruth,
    mse,
    rmse,
    mae,
    reservoirState: reservoir.getState(),
    trainingMse,
    metrics,
  };
}

/**
 * Run with fabric-based reservoir
 */
export async function runFabricReservoir(
  width: number = 32,
  height: number = 32,
  seriesType: keyof typeof TIME_SERIES = 'multiSine',
  seriesLength: number = 1000,
  densityTier: DensityTier = 'medium'
): Promise<ReservoirResult> {
  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[densityTier];

  // Generate time series
  const series = TIME_SERIES[seriesType](seriesLength);

  // Create fabric reservoir
  const reservoir = new FabricReservoir(width, height, 1, {
    spectralRadius: 0.9,
    leakRate: 0.3,
    densityTier,
  });

  // Training phase
  const trainSize = Math.floor(seriesLength * 0.7);
  const washout = 50;
  const states: Float32Array[] = [];
  const targets: Float32Array[] = [];

  for (let t = 0; t < trainSize; t++) {
    // Create input (inject at edge)
    const input = new Float32Array(height);
    for (let y = 0; y < height; y++) {
      input[y] = series[t];
    }

    reservoir.update(input);

    if (t >= washout && t < trainSize - 1) {
      states.push(reservoir.getState());
      targets.push(new Float32Array([series[t + 1]]));
    }
  }

  // Train readout
  const trainingMse = reservoir.train(states, targets);

  // Test phase
  const predictions: Float32Array[] = [];
  const groundTruth: Float32Array[] = [];
  const predictionHorizon = 50;

  for (let t = 0; t < predictionHorizon; t++) {
    const pred = reservoir.readout();
    predictions.push(new Float32Array(pred));
    groundTruth.push(new Float32Array([series[trainSize + t]]));

    // Feed back
    const input = new Float32Array(height).fill(pred[0]);
    reservoir.update(input);
  }

  // Calculate metrics
  let mse = 0, mae = 0;
  for (let t = 0; t < predictions.length; t++) {
    const error = predictions[t][0] - groundTruth[t][0];
    mse += error * error;
    mae += Math.abs(error);
  }
  mse /= predictions.length;
  mae /= predictions.length;
  const rmse = Math.sqrt(mse);

  const wallTimeMs = performance.now() - startTime;
  const pixelCount = width * height;

  return {
    success: true,
    cycles: reservoir.cycles,
    wallTimeMs,
    powerMw: pixelCount * densitySpec.powerPerPixelMw,
    accuracy: 1 - rmse,
    outputData: new Float32Array(predictions.flatMap(p => Array.from(p))),
    predictions,
    groundTruth,
    mse,
    rmse,
    mae,
    reservoirState: {
      nodes: reservoir.getState(),
      connections: new Float32Array(0),
      spectralRadius: 0.9,
      leakRate: 0.3,
    },
    trainingMse,
    metrics: {
      totalCycles: reservoir.cycles,
      activeTiles: pixelCount,
      powerMw: pixelCount * densitySpec.powerPerPixelMw,
      throughputOpsPerSec: Number(reservoir.cycles) / (wallTimeMs / 1000),
      memoryBandwidthGbps: 0,
      utilizationPercent: 100,
    },
  };
}

/**
 * Print time series comparison
 */
function printComparison(
  predictions: Float32Array[],
  groundTruth: Float32Array[],
  maxPoints: number = 20
): void {
  const n = Math.min(predictions.length, maxPoints);
  console.log('  t  | Predicted | Actual  | Error');
  console.log('-----|-----------|---------|-------');
  for (let t = 0; t < n; t++) {
    const pred = predictions[t][0];
    const actual = groundTruth[t][0];
    const error = Math.abs(pred - actual);
    console.log(
      `${t.toString().padStart(4)} |  ${pred.toFixed(4)}  | ${actual.toFixed(4)} | ${error.toFixed(4)}`
    );
  }
}

// Main demonstration
async function main(): Promise<void> {
  console.log('=== FIXEL Reservoir Time Series Prediction ===\n');

  // Test different time series types
  console.log('--- Time Series Type Comparison ---');
  const seriesTypes: (keyof typeof TIME_SERIES)[] = [
    'mackeyGlass', 'lorenz', 'multiSine', 'narma', 'squareWave'
  ];

  for (const seriesType of seriesTypes) {
    const result = await runReservoirPrediction({
      reservoirSize: 500,
      spectralRadius: 0.95,
      leakRate: 0.3,
      densityTier: 'medium',
    }, seriesType, 2000, 50);

    console.log(`\n${seriesType}:`);
    console.log(`  Training MSE: ${result.trainingMse.toFixed(6)}`);
    console.log(`  Test RMSE: ${result.rmse.toFixed(6)}`);
    console.log(`  Test MAE: ${result.mae.toFixed(6)}`);
    console.log(`  Power: ${result.powerMw.toFixed(3)} mW`);
  }

  // Test with fabric reservoir
  console.log('\n\n--- Fabric Reservoir (32x32) ---');
  const fabricResult = await runFabricReservoir(32, 32, 'multiSine', 1000, 'medium');
  console.log(`RMSE: ${fabricResult.rmse.toFixed(6)}`);
  console.log(`Power: ${fabricResult.powerMw.toFixed(3)} mW`);
  console.log(`Cycles: ${fabricResult.cycles}`);

  // Compare density tiers
  console.log('\n\n--- Density Tier Comparison (Mackey-Glass) ---');
  const tiers: DensityTier[] = ['low', 'medium', 'high', 'ultra'];

  for (const tier of tiers) {
    const result = await runReservoirPrediction({
      reservoirSize: 500,
      densityTier: tier,
    }, 'mackeyGlass', 2000, 50);

    console.log(`\n${tier.toUpperCase()}:`);
    console.log(`  RMSE: ${result.rmse.toFixed(6)}`);
    console.log(`  Power: ${result.powerMw.toFixed(3)} mW`);
    console.log(`  Throughput: ${(result.metrics.throughputOpsPerSec / 1e6).toFixed(2)} M ops/sec`);
  }

  // Show predictions vs ground truth
  console.log('\n\n--- Sample Predictions (Multi-Sine) ---');
  const detailedResult = await runReservoirPrediction({
    reservoirSize: 500,
    densityTier: 'medium',
  }, 'multiSine', 2000, 20);

  printComparison(detailedResult.predictions, detailedResult.groundTruth);
}

export { main };

if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}
