/**
 * FIXEL Spiking MNIST Simulation
 * MNIST digit recognition using Spiking Neural Networks
 *
 * Demonstrates:
 * - Encoding digits as spike trains (rate/temporal coding)
 * - Spiking neuron layers with LIF dynamics
 * - Spike-based classification
 * - Power-efficient neuromorphic computing
 */

import { SpikingNetwork, SpikingLayer, DenseLayer } from './neural.js';
import {
  DensityTier,
  SimulationResult,
  MnistSample,
  SpikeEvent,
  DENSITY_SPECS,
} from './types.js';

export interface SpikingMnistConfig {
  densityTier: DensityTier;
  hiddenLayers: number[];
  timesteps: number;
  threshold: number;
  tau: number;
  encodingType: 'rate' | 'temporal' | 'latency';
}

export interface SpikingMnistResult extends SimulationResult {
  predictions: number[];
  correctCount: number;
  totalSamples: number;
  spikeCount: number;
  spikesPerNeuron: number;
  confusionMatrix: number[][];
  energyPerInference: number; // pJ per inference
}

/**
 * Generate synthetic MNIST-like samples
 * (In production, load actual MNIST dataset)
 */
function generateSyntheticMnist(count: number): MnistSample[] {
  const samples: MnistSample[] = [];

  for (let i = 0; i < count; i++) {
    const label = i % 10;
    const image = new Float32Array(28 * 28);

    // Create digit-like patterns
    // Each digit has a characteristic pattern
    switch (label) {
      case 0: // Circle
        for (let y = 0; y < 28; y++) {
          for (let x = 0; x < 28; x++) {
            const dist = Math.sqrt((x - 14) ** 2 + (y - 14) ** 2);
            if (dist > 6 && dist < 10) {
              image[y * 28 + x] = Math.random() * 0.3 + 0.7;
            }
          }
        }
        break;

      case 1: // Vertical line
        for (let y = 4; y < 24; y++) {
          for (let x = 12; x < 16; x++) {
            image[y * 28 + x] = Math.random() * 0.3 + 0.7;
          }
        }
        break;

      case 2: // Top arc + diagonal + bottom line
        for (let x = 8; x < 20; x++) {
          const y = 6 + Math.floor(2 * Math.sin((x - 8) * Math.PI / 12));
          if (y >= 0 && y < 28) image[y * 28 + x] = 0.9;
        }
        for (let i = 0; i < 14; i++) {
          const x = 18 - i;
          const y = 10 + i;
          if (x >= 0 && x < 28 && y >= 0 && y < 28) {
            image[y * 28 + x] = 0.8;
          }
        }
        for (let x = 8; x < 20; x++) image[22 * 28 + x] = 0.9;
        break;

      case 3: // Two arcs
        for (let x = 8; x < 20; x++) {
          image[6 * 28 + x] = 0.9;
          image[13 * 28 + x] = 0.8;
          image[22 * 28 + x] = 0.9;
        }
        image[6 * 28 + 19] = 0.9;
        image[22 * 28 + 19] = 0.9;
        break;

      case 4: // L-shape + vertical
        for (let y = 4; y < 16; y++) image[y * 28 + 8] = 0.9;
        for (let x = 8; x < 20; x++) image[14 * 28 + x] = 0.9;
        for (let y = 4; y < 24; y++) image[y * 28 + 18] = 0.9;
        break;

      case 5: // S-shape
        for (let x = 8; x < 20; x++) {
          image[6 * 28 + x] = 0.9;
          image[13 * 28 + x] = 0.8;
          image[22 * 28 + x] = 0.9;
        }
        for (let y = 6; y < 13; y++) image[y * 28 + 8] = 0.9;
        for (let y = 13; y < 22; y++) image[y * 28 + 19] = 0.9;
        break;

      case 6: // Loop with tail
        for (let y = 4; y < 14; y++) image[y * 28 + 10] = 0.9;
        for (let y = 12; y < 24; y++) {
          for (let x = 8; x < 20; x++) {
            const dist = Math.sqrt((x - 14) ** 2 + (y - 18) ** 2);
            if (dist > 3 && dist < 6) {
              image[y * 28 + x] = 0.9;
            }
          }
        }
        break;

      case 7: // Top line + diagonal
        for (let x = 8; x < 20; x++) image[6 * 28 + x] = 0.9;
        for (let i = 0; i < 16; i++) {
          const x = 18 - Math.floor(i * 0.4);
          const y = 6 + i;
          if (x >= 0 && x < 28) image[y * 28 + x] = 0.9;
        }
        break;

      case 8: // Two circles
        for (let y = 0; y < 28; y++) {
          for (let x = 0; x < 28; x++) {
            const dist1 = Math.sqrt((x - 14) ** 2 + (y - 9) ** 2);
            const dist2 = Math.sqrt((x - 14) ** 2 + (y - 19) ** 2);
            if ((dist1 > 3 && dist1 < 6) || (dist2 > 3 && dist2 < 6)) {
              image[y * 28 + x] = 0.9;
            }
          }
        }
        break;

      case 9: // Circle with tail
        for (let y = 4; y < 16; y++) {
          for (let x = 8; x < 20; x++) {
            const dist = Math.sqrt((x - 14) ** 2 + (y - 10) ** 2);
            if (dist > 3 && dist < 6) {
              image[y * 28 + x] = 0.9;
            }
          }
        }
        for (let y = 14; y < 24; y++) image[y * 28 + 18] = 0.9;
        break;
    }

    // Add noise
    for (let j = 0; j < image.length; j++) {
      image[j] += (Math.random() - 0.5) * 0.1;
      image[j] = Math.max(0, Math.min(1, image[j]));
    }

    samples.push({ image, label });
  }

  return samples;
}

/**
 * Rate encoding: pixel intensity -> spike rate
 */
function rateEncode(
  image: Float32Array,
  timesteps: number
): Float32Array[] {
  const spikeTrain: Float32Array[] = [];

  for (let t = 0; t < timesteps; t++) {
    const frame = new Float32Array(image.length);
    for (let i = 0; i < image.length; i++) {
      // Higher intensity = higher probability of spike
      if (Math.random() < image[i] * 0.3) {
        frame[i] = 1.0;
      }
    }
    spikeTrain.push(frame);
  }

  return spikeTrain;
}

/**
 * Temporal encoding: first spike time encodes intensity
 */
function temporalEncode(
  image: Float32Array,
  timesteps: number
): Float32Array[] {
  const spikeTrain: Float32Array[] = [];
  const spikeTime = new Float32Array(image.length);

  // Calculate spike times (inverse of intensity)
  for (let i = 0; i < image.length; i++) {
    if (image[i] > 0.1) {
      spikeTime[i] = Math.floor((1 - image[i]) * timesteps * 0.8);
    } else {
      spikeTime[i] = -1; // No spike
    }
  }

  for (let t = 0; t < timesteps; t++) {
    const frame = new Float32Array(image.length);
    for (let i = 0; i < image.length; i++) {
      if (spikeTime[i] === t) {
        frame[i] = 1.0;
      }
    }
    spikeTrain.push(frame);
  }

  return spikeTrain;
}

/**
 * Latency encoding: spike time proportional to intensity
 */
function latencyEncode(
  image: Float32Array,
  timesteps: number
): Float32Array[] {
  const spikeTrain: Float32Array[] = [];

  // Brighter pixels spike earlier
  for (let t = 0; t < timesteps; t++) {
    const frame = new Float32Array(image.length);
    const threshold = 1 - (t / timesteps);

    for (let i = 0; i < image.length; i++) {
      if (image[i] >= threshold && image[i] > 0.1) {
        // Only spike once
        let alreadySpiked = false;
        for (let pt = 0; pt < t; pt++) {
          if (spikeTrain[pt]?.[i] > 0) {
            alreadySpiked = true;
            break;
          }
        }
        if (!alreadySpiked) {
          frame[i] = 1.0;
        }
      }
    }
    spikeTrain.push(frame);
  }

  return spikeTrain;
}

/**
 * Create and train a spiking network for MNIST
 */
export async function runSpikingMnist(
  config: Partial<SpikingMnistConfig> = {},
  sampleCount: number = 100
): Promise<SpikingMnistResult> {
  const fullConfig: SpikingMnistConfig = {
    densityTier: config.densityTier ?? 'medium',
    hiddenLayers: config.hiddenLayers ?? [256, 128],
    timesteps: config.timesteps ?? 50,
    threshold: config.threshold ?? 1.0,
    tau: config.tau ?? 20,
    encodingType: config.encodingType ?? 'rate',
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  // Create network architecture: 784 -> hidden -> 10
  const layerSizes = [784, ...fullConfig.hiddenLayers, 10];
  const network = new SpikingNetwork(layerSizes, fullConfig.densityTier);

  // Generate synthetic data
  const samples = generateSyntheticMnist(sampleCount);

  // Process each sample
  const predictions: number[] = [];
  let totalSpikes = 0;
  const confusionMatrix: number[][] = Array(10).fill(null)
    .map(() => Array(10).fill(0));

  for (const sample of samples) {
    // Encode image to spike train
    let spikeTrain: Float32Array[];
    switch (fullConfig.encodingType) {
      case 'temporal':
        spikeTrain = temporalEncode(sample.image, fullConfig.timesteps);
        break;
      case 'latency':
        spikeTrain = latencyEncode(sample.image, fullConfig.timesteps);
        break;
      default:
        spikeTrain = rateEncode(sample.image, fullConfig.timesteps);
    }

    // Process through network
    const spikeCounts = network.process(spikeTrain);

    // Count total spikes
    const allSpikes = network.getAllSpikes();
    totalSpikes += allSpikes.length;

    // Find prediction (class with most output spikes)
    let maxCount = spikeCounts[0];
    let prediction = 0;
    for (let i = 1; i < spikeCounts.length; i++) {
      if (spikeCounts[i] > maxCount) {
        maxCount = spikeCounts[i];
        prediction = i;
      }
    }

    predictions.push(prediction);
    confusionMatrix[sample.label][prediction]++;
  }

  // Calculate accuracy
  let correctCount = 0;
  for (let i = 0; i < samples.length; i++) {
    if (predictions[i] === samples[i].label) {
      correctCount++;
    }
  }

  const accuracy = correctCount / samples.length;
  const wallTimeMs = performance.now() - startTime;

  // Calculate power and energy
  const totalNeurons = layerSizes.reduce((a, b) => a + b, 0);
  const powerMw = totalNeurons * densitySpec.powerPerPixelMw;
  const energyPerInference = (powerMw * 1e-3 * (wallTimeMs / samples.length) * 1e-3) * 1e12; // pJ

  const metrics = network.getMetrics();

  return {
    success: true,
    cycles: metrics.totalCycles,
    wallTimeMs,
    powerMw,
    accuracy,
    outputData: new Float32Array(predictions),
    predictions,
    correctCount,
    totalSamples: samples.length,
    spikeCount: totalSpikes,
    spikesPerNeuron: totalSpikes / (totalNeurons * samples.length),
    confusionMatrix,
    energyPerInference,
    metrics: {
      ...metrics,
      powerMw,
    },
  };
}

/**
 * Analyze spike patterns for a single sample
 */
export function analyzeSpikes(
  image: Float32Array,
  config: Partial<SpikingMnistConfig> = {}
): {
  spikes: SpikeEvent[];
  firingRates: Float32Array;
  layerActivity: number[];
} {
  const fullConfig: SpikingMnistConfig = {
    densityTier: config.densityTier ?? 'medium',
    hiddenLayers: config.hiddenLayers ?? [256, 128],
    timesteps: config.timesteps ?? 50,
    threshold: config.threshold ?? 1.0,
    tau: config.tau ?? 20,
    encodingType: config.encodingType ?? 'rate',
  };

  const layerSizes = [784, ...fullConfig.hiddenLayers, 10];
  const network = new SpikingNetwork(layerSizes, fullConfig.densityTier);

  // Encode and process
  const spikeTrain = rateEncode(image, fullConfig.timesteps);
  network.process(spikeTrain);

  const spikes = network.getAllSpikes();

  // Calculate firing rates per layer
  const layerActivity = layerSizes.map(() => 0);
  for (const spike of spikes) {
    layerActivity[spike.layer]++;
  }

  // Normalize by layer size and timesteps
  for (let i = 0; i < layerSizes.length; i++) {
    layerActivity[i] /= layerSizes[i] * fullConfig.timesteps;
  }

  return {
    spikes,
    firingRates: new Float32Array(layerActivity),
    layerActivity,
  };
}

/**
 * Print confusion matrix
 */
function printConfusionMatrix(matrix: number[][]): void {
  console.log('   | 0   1   2   3   4   5   6   7   8   9');
  console.log('---+' + '-'.repeat(40));
  for (let i = 0; i < 10; i++) {
    let row = ` ${i} |`;
    for (let j = 0; j < 10; j++) {
      row += matrix[i][j].toString().padStart(4);
    }
    console.log(row);
  }
}

// Main demonstration
async function main(): Promise<void> {
  console.log('=== FIXEL Spiking MNIST Simulation ===\n');

  // Test with different density tiers
  console.log('--- Accuracy vs Density Tier ---');
  const tiers: DensityTier[] = ['low', 'medium', 'high', 'ultra'];

  for (const tier of tiers) {
    const result = await runSpikingMnist({
      densityTier: tier,
      hiddenLayers: [128, 64],
      timesteps: 50,
      encodingType: 'rate',
    }, 100);

    console.log(`\n${tier.toUpperCase()} tier:`);
    console.log(`  Accuracy: ${(result.accuracy! * 100).toFixed(1)}%`);
    console.log(`  Correct: ${result.correctCount}/${result.totalSamples}`);
    console.log(`  Spikes: ${result.spikeCount} total`);
    console.log(`  Spikes/neuron: ${result.spikesPerNeuron.toFixed(3)}`);
    console.log(`  Power: ${result.powerMw.toFixed(3)} mW`);
    console.log(`  Energy/inference: ${result.energyPerInference.toFixed(1)} pJ`);
    console.log(`  Wall time: ${result.wallTimeMs.toFixed(2)} ms`);
  }

  // Test different encoding types
  console.log('\n\n--- Encoding Type Comparison ---');
  const encodings: ('rate' | 'temporal' | 'latency')[] = ['rate', 'temporal', 'latency'];

  for (const encoding of encodings) {
    const result = await runSpikingMnist({
      densityTier: 'medium',
      hiddenLayers: [128, 64],
      timesteps: 50,
      encodingType: encoding,
    }, 100);

    console.log(`\n${encoding.toUpperCase()} encoding:`);
    console.log(`  Accuracy: ${(result.accuracy! * 100).toFixed(1)}%`);
    console.log(`  Spikes: ${result.spikeCount}`);
    console.log(`  Energy: ${result.energyPerInference.toFixed(1)} pJ/inference`);
  }

  // Show confusion matrix
  console.log('\n\n--- Confusion Matrix (Medium tier, Rate encoding) ---');
  const detailedResult = await runSpikingMnist({
    densityTier: 'medium',
    hiddenLayers: [256, 128],
    timesteps: 100,
    encodingType: 'rate',
  }, 200);

  printConfusionMatrix(detailedResult.confusionMatrix);
  console.log(`\nOverall accuracy: ${(detailedResult.accuracy! * 100).toFixed(1)}%`);
}

export { main };

if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}
