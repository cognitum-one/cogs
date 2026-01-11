/**
 * FIXEL Edge Detection Simulation
 * Classic Sobel edge detection on neural fabric
 *
 * Demonstrates:
 * - Loading images into the fabric
 * - Applying convolution kernels (Sobel X and Y)
 * - Computing gradient magnitude
 * - Measuring cycles, power, and accuracy
 */

import { Fabric } from './fabric.js';
import {
  DensityTier,
  SimulationOptions,
  SimulationResult,
  Kernel,
  DENSITY_SPECS,
} from './types.js';

// Sobel kernels for edge detection
const SOBEL_X: Kernel = {
  width: 3,
  height: 3,
  weights: new Float32Array([
    -1, 0, 1,
    -2, 0, 2,
    -1, 0, 1,
  ]),
};

const SOBEL_Y: Kernel = {
  width: 3,
  height: 3,
  weights: new Float32Array([
    -1, -2, -1,
     0,  0,  0,
     1,  2,  1,
  ]),
};

// Alternative kernels
const PREWITT_X: Kernel = {
  width: 3,
  height: 3,
  weights: new Float32Array([
    -1, 0, 1,
    -1, 0, 1,
    -1, 0, 1,
  ]),
};

const PREWITT_Y: Kernel = {
  width: 3,
  height: 3,
  weights: new Float32Array([
    -1, -1, -1,
     0,  0,  0,
     1,  1,  1,
  ]),
};

export interface EdgeDetectionConfig {
  width: number;
  height: number;
  densityTier: DensityTier;
  edgeThreshold: number;
  kernelType: 'sobel' | 'prewitt';
}

export interface EdgeDetectionResult extends SimulationResult {
  edgeMap: Float32Array;
  gradientX: Float32Array;
  gradientY: Float32Array;
  edgeCount: number;
  edgePercentage: number;
}

/**
 * Generate a test image with known edges
 */
function generateTestImage(width: number, height: number): Float32Array {
  const image = new Float32Array(width * height);

  // Create a pattern with sharp edges
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = y * width + x;

      // Vertical stripe
      if (x >= width / 4 && x < width * 3 / 4) {
        image[idx] = 1.0;
      }

      // Horizontal stripe
      if (y >= height / 4 && y < height * 3 / 4) {
        image[idx] = image[idx] === 1.0 ? 0.5 : 0.75;
      }

      // Circular region
      const cx = width / 2;
      const cy = height / 2;
      const r = Math.min(width, height) / 6;
      const dist = Math.sqrt((x - cx) ** 2 + (y - cy) ** 2);
      if (dist < r) {
        image[idx] = 0.25;
      }
    }
  }

  return image;
}

/**
 * Generate ground truth edge map for accuracy calculation
 */
function generateGroundTruthEdges(
  image: Float32Array,
  width: number,
  height: number,
  threshold: number
): Float32Array {
  const edges = new Float32Array(width * height);

  for (let y = 1; y < height - 1; y++) {
    for (let x = 1; x < width - 1; x++) {
      const idx = y * width + x;
      const center = image[idx];

      // Check all 8 neighbors
      let maxDiff = 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (dx === 0 && dy === 0) continue;
          const nidx = (y + dy) * width + (x + dx);
          const diff = Math.abs(center - image[nidx]);
          if (diff > maxDiff) maxDiff = diff;
        }
      }

      edges[idx] = maxDiff > threshold / 2 ? 1.0 : 0.0;
    }
  }

  return edges;
}

/**
 * Calculate accuracy against ground truth
 */
function calculateAccuracy(
  detected: Float32Array,
  groundTruth: Float32Array,
  threshold: number
): number {
  let correct = 0;
  const total = detected.length;

  for (let i = 0; i < total; i++) {
    const detectedEdge = detected[i] > threshold;
    const truthEdge = groundTruth[i] > 0.5;
    if (detectedEdge === truthEdge) {
      correct++;
    }
  }

  return correct / total;
}

/**
 * Run edge detection simulation
 */
export async function runEdgeDetection(
  config: Partial<EdgeDetectionConfig> = {}
): Promise<EdgeDetectionResult> {
  const fullConfig: EdgeDetectionConfig = {
    width: config.width ?? 64,
    height: config.height ?? 64,
    densityTier: config.densityTier ?? 'medium',
    edgeThreshold: config.edgeThreshold ?? 0.1,
    kernelType: config.kernelType ?? 'sobel',
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[fullConfig.densityTier];

  // Create fabric instances for X and Y gradient computation
  const fabricX = new Fabric({
    width: fullConfig.width,
    height: fullConfig.height,
    densityTier: fullConfig.densityTier,
  });

  const fabricY = new Fabric({
    width: fullConfig.width,
    height: fullConfig.height,
    densityTier: fullConfig.densityTier,
  });

  // Generate or load test image
  const testImage = generateTestImage(fullConfig.width, fullConfig.height);

  // Load image into both fabrics
  fabricX.initialize(testImage);
  fabricY.initialize(testImage);

  // Select kernels based on type
  const kernelX = fullConfig.kernelType === 'sobel' ? SOBEL_X : PREWITT_X;
  const kernelY = fullConfig.kernelType === 'sobel' ? SOBEL_Y : PREWITT_Y;

  // Apply Sobel/Prewitt kernels
  fabricX.convolve(kernelX);
  fabricY.convolve(kernelY);

  // Get gradient components
  const gradientX = fabricX.getPixels();
  const gradientY = fabricY.getPixels();

  // Compute gradient magnitude: sqrt(Gx^2 + Gy^2)
  const edgeMap = new Float32Array(fullConfig.width * fullConfig.height);
  for (let i = 0; i < edgeMap.length; i++) {
    edgeMap[i] = Math.sqrt(gradientX[i] ** 2 + gradientY[i] ** 2);
  }

  // Normalize edge map
  let maxEdge = 0;
  for (let i = 0; i < edgeMap.length; i++) {
    if (edgeMap[i] > maxEdge) maxEdge = edgeMap[i];
  }
  if (maxEdge > 0) {
    for (let i = 0; i < edgeMap.length; i++) {
      edgeMap[i] /= maxEdge;
    }
  }

  // Count edges above threshold
  let edgeCount = 0;
  for (let i = 0; i < edgeMap.length; i++) {
    if (edgeMap[i] > fullConfig.edgeThreshold) {
      edgeCount++;
    }
  }

  // Calculate accuracy against ground truth
  const groundTruth = generateGroundTruthEdges(
    testImage,
    fullConfig.width,
    fullConfig.height,
    fullConfig.edgeThreshold
  );
  const accuracy = calculateAccuracy(edgeMap, groundTruth, fullConfig.edgeThreshold);

  // Aggregate metrics
  const totalCycles = fabricX.getCycles() + fabricY.getCycles();
  const wallTimeMs = performance.now() - startTime;
  const pixelCount = fullConfig.width * fullConfig.height;
  const powerMw = pixelCount * densitySpec.powerPerPixelMw * 2; // Two fabrics

  return {
    success: true,
    cycles: totalCycles,
    wallTimeMs,
    powerMw,
    accuracy,
    outputData: edgeMap,
    edgeMap,
    gradientX,
    gradientY,
    edgeCount,
    edgePercentage: (edgeCount / pixelCount) * 100,
    metrics: {
      totalCycles,
      activeTiles: pixelCount * 2,
      powerMw,
      throughputOpsPerSec: Number(totalCycles) / (wallTimeMs / 1000),
      memoryBandwidthGbps: (pixelCount * 4 * Number(totalCycles)) / (wallTimeMs * 1e6),
      utilizationPercent: 100,
    },
  };
}

/**
 * Run edge detection with custom image data
 */
export async function detectEdges(
  imageData: Uint8Array | Float32Array,
  width: number,
  height: number,
  options: Partial<EdgeDetectionConfig> = {}
): Promise<EdgeDetectionResult> {
  const config: EdgeDetectionConfig = {
    width,
    height,
    densityTier: options.densityTier ?? 'medium',
    edgeThreshold: options.edgeThreshold ?? 0.1,
    kernelType: options.kernelType ?? 'sobel',
  };

  const startTime = performance.now();
  const densitySpec = DENSITY_SPECS[config.densityTier];

  const fabricX = new Fabric({
    width,
    height,
    densityTier: config.densityTier,
  });

  const fabricY = new Fabric({
    width,
    height,
    densityTier: config.densityTier,
  });

  // Normalize input if needed
  let normalizedImage: Float32Array;
  if (imageData instanceof Uint8Array) {
    normalizedImage = new Float32Array(imageData.length);
    for (let i = 0; i < imageData.length; i++) {
      normalizedImage[i] = imageData[i] / 255.0;
    }
  } else {
    normalizedImage = imageData;
  }

  fabricX.initialize(normalizedImage);
  fabricY.initialize(normalizedImage);

  const kernelX = config.kernelType === 'sobel' ? SOBEL_X : PREWITT_X;
  const kernelY = config.kernelType === 'sobel' ? SOBEL_Y : PREWITT_Y;

  fabricX.convolve(kernelX);
  fabricY.convolve(kernelY);

  const gradientX = fabricX.getPixels();
  const gradientY = fabricY.getPixels();

  const edgeMap = new Float32Array(width * height);
  for (let i = 0; i < edgeMap.length; i++) {
    edgeMap[i] = Math.sqrt(gradientX[i] ** 2 + gradientY[i] ** 2);
  }

  // Normalize
  let maxEdge = 0;
  for (let i = 0; i < edgeMap.length; i++) {
    if (edgeMap[i] > maxEdge) maxEdge = edgeMap[i];
  }
  if (maxEdge > 0) {
    for (let i = 0; i < edgeMap.length; i++) {
      edgeMap[i] /= maxEdge;
    }
  }

  let edgeCount = 0;
  for (let i = 0; i < edgeMap.length; i++) {
    if (edgeMap[i] > config.edgeThreshold) edgeCount++;
  }

  const totalCycles = fabricX.getCycles() + fabricY.getCycles();
  const wallTimeMs = performance.now() - startTime;
  const pixelCount = width * height;
  const powerMw = pixelCount * densitySpec.powerPerPixelMw * 2;

  return {
    success: true,
    cycles: totalCycles,
    wallTimeMs,
    powerMw,
    accuracy: undefined,
    outputData: edgeMap,
    edgeMap,
    gradientX,
    gradientY,
    edgeCount,
    edgePercentage: (edgeCount / pixelCount) * 100,
    metrics: {
      totalCycles,
      activeTiles: pixelCount * 2,
      powerMw,
      throughputOpsPerSec: Number(totalCycles) / (wallTimeMs / 1000),
      memoryBandwidthGbps: (pixelCount * 4 * Number(totalCycles)) / (wallTimeMs * 1e6),
      utilizationPercent: 100,
    },
  };
}

/**
 * Print edge map as ASCII art (for debugging)
 */
export function printEdgeMap(
  edgeMap: Float32Array,
  width: number,
  height: number,
  threshold: number = 0.1
): void {
  const chars = ' .:-=+*#%@';
  for (let y = 0; y < height; y++) {
    let row = '';
    for (let x = 0; x < width; x++) {
      const val = edgeMap[y * width + x];
      const idx = Math.min(Math.floor(val * chars.length), chars.length - 1);
      row += chars[idx];
    }
    console.log(row);
  }
}

// Main demonstration
async function main(): Promise<void> {
  console.log('=== FIXEL Edge Detection Simulation ===\n');

  // Test with different density tiers
  const tiers: DensityTier[] = ['low', 'medium', 'high', 'ultra'];

  for (const tier of tiers) {
    console.log(`\n--- Density Tier: ${tier.toUpperCase()} ---`);

    const result = await runEdgeDetection({
      width: 64,
      height: 64,
      densityTier: tier,
      edgeThreshold: 0.15,
      kernelType: 'sobel',
    });

    console.log(`Cycles: ${result.cycles}`);
    console.log(`Wall time: ${result.wallTimeMs.toFixed(2)} ms`);
    console.log(`Power: ${result.powerMw.toFixed(3)} mW`);
    console.log(`Accuracy: ${((result.accuracy ?? 0) * 100).toFixed(1)}%`);
    console.log(`Edges detected: ${result.edgeCount} (${result.edgePercentage.toFixed(1)}%)`);
    console.log(`Throughput: ${(result.metrics.throughputOpsPerSec / 1e6).toFixed(2)} M ops/sec`);
  }

  // Show a small visual output
  console.log('\n--- Edge Map Visualization (32x32) ---\n');
  const smallResult = await runEdgeDetection({
    width: 32,
    height: 32,
    densityTier: 'medium',
  });
  printEdgeMap(smallResult.edgeMap, 32, 32, 0.1);
}

// Export main for CLI usage
export { main };

// Run if executed directly
if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}
