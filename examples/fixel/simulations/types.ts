/**
 * FIXEL Simulation Types
 * Type definitions for the Neural Fabric and simulation components
 */

// Density tier configuration
export type DensityTier = 'low' | 'medium' | 'high' | 'ultra';

// Configuration for the Fabric
export interface FabricConfig {
  width: number;
  height: number;
  densityTier: DensityTier;
  clockFreqMhz: number;
  memorySize: number;
  enableDebug: boolean;
}

// Pixel/cell state in the fabric
export interface PixelState {
  x: number;
  y: number;
  value: number;
  membrane: number;  // For spiking neurons
  threshold: number;
  refractory: number;
}

// Neural operation types
export type ActivationFunction = 'relu' | 'sigmoid' | 'tanh' | 'softmax' | 'swish' | 'lif';

export type Operation =
  | 'forward'
  | 'inference'
  | 'backward'
  | 'weight_update'
  | 'load_weights'
  | 'store_weights';

// Learning configuration
export interface LearningConfig {
  learningRate: number;
  momentum: number;
  targetVector?: Float32Array;
}

// Weight memory interface
export interface WeightMemory {
  address: number;
  data: Float32Array;
  size: number;
}

// Performance metrics
export interface PerformanceMetrics {
  totalCycles: bigint;
  activeTiles: number;
  powerMw: number;
  throughputOpsPerSec: number;
  memoryBandwidthGbps: number;
  utilizationPercent: number;
}

// Simulation result
export interface SimulationResult {
  success: boolean;
  cycles: bigint;
  wallTimeMs: number;
  powerMw: number;
  accuracy?: number;
  outputData: Float32Array | Uint8Array;
  metrics: PerformanceMetrics;
}

// Kernel definition for convolution operations
export interface Kernel {
  width: number;
  height: number;
  weights: Float32Array;
}

// Spike event for spiking neural networks
export interface SpikeEvent {
  neuronId: number;
  time: number;
  layer: number;
}

// Reservoir state for reservoir computing
export interface ReservoirState {
  nodes: Float32Array;
  connections: Float32Array;
  spectralRadius: number;
  leakRate: number;
}

// Fluid cell state for Lattice Boltzmann
export interface FluidCell {
  density: number;
  velocityX: number;
  velocityY: number;
  distributions: Float32Array; // D2Q9: 9 distribution functions
}

// Simulation configuration options
export interface SimulationOptions {
  maxSteps: number;
  outputInterval: number;
  enableVisualization: boolean;
  densityTier: DensityTier;
  randomSeed?: number;
}

// Image data structure
export interface ImageData {
  width: number;
  height: number;
  channels: number;
  data: Uint8Array | Float32Array;
}

// MNIST dataset sample
export interface MnistSample {
  image: Float32Array; // 28x28 = 784 values
  label: number;       // 0-9
}

// Time series data point
export interface TimeSeriesPoint {
  timestamp: number;
  value: number;
}

// Density tier specifications
export const DENSITY_SPECS: Record<DensityTier, {
  pixelsPerMm2: number;
  powerPerPixelMw: number;
  maxClockMhz: number;
  memoryPerPixelBytes: number;
}> = {
  low: {
    pixelsPerMm2: 100,
    powerPerPixelMw: 0.1,
    maxClockMhz: 500,
    memoryPerPixelBytes: 16,
  },
  medium: {
    pixelsPerMm2: 400,
    powerPerPixelMw: 0.05,
    maxClockMhz: 750,
    memoryPerPixelBytes: 32,
  },
  high: {
    pixelsPerMm2: 1600,
    powerPerPixelMw: 0.025,
    maxClockMhz: 1000,
    memoryPerPixelBytes: 64,
  },
  ultra: {
    pixelsPerMm2: 6400,
    powerPerPixelMw: 0.0125,
    maxClockMhz: 1500,
    memoryPerPixelBytes: 128,
  },
};
