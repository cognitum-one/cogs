/**
 * FIXEL Fabric Simulator
 * Simulates a 2D array of neural processing elements (pixels)
 */

import {
  FabricConfig,
  PixelState,
  PerformanceMetrics,
  DensityTier,
  DENSITY_SPECS,
  Kernel,
  Operation,
  ActivationFunction,
  LearningConfig,
} from './types.js';

export class Fabric {
  private config: FabricConfig;
  private pixels: Float32Array;
  private membranes: Float32Array;
  private thresholds: Float32Array;
  private refractory: Uint8Array;
  private weights: Float32Array;
  private cycleCount: bigint = 0n;
  private startTime: number = 0;

  constructor(config: Partial<FabricConfig> = {}) {
    this.config = {
      width: config.width ?? 64,
      height: config.height ?? 64,
      densityTier: config.densityTier ?? 'medium',
      clockFreqMhz: config.clockFreqMhz ?? DENSITY_SPECS[config.densityTier ?? 'medium'].maxClockMhz,
      memorySize: config.memorySize ?? 1024 * 1024,
      enableDebug: config.enableDebug ?? false,
    };

    const size = this.config.width * this.config.height;
    this.pixels = new Float32Array(size);
    this.membranes = new Float32Array(size);
    this.thresholds = new Float32Array(size).fill(1.0);
    this.refractory = new Uint8Array(size);
    this.weights = new Float32Array(size * 9); // 3x3 neighborhood weights per pixel
  }

  get width(): number {
    return this.config.width;
  }

  get height(): number {
    return this.config.height;
  }

  get size(): number {
    return this.config.width * this.config.height;
  }

  /**
   * Initialize the fabric with random or specified values
   */
  initialize(values?: Float32Array | Uint8Array): void {
    this.startTime = performance.now();
    this.cycleCount = 0n;

    if (values) {
      const normalized = values instanceof Uint8Array
        ? new Float32Array(values).map(v => v / 255.0)
        : values;
      this.pixels.set(normalized.slice(0, this.size));
    } else {
      for (let i = 0; i < this.size; i++) {
        this.pixels[i] = 0;
        this.membranes[i] = 0;
        this.refractory[i] = 0;
      }
    }
  }

  /**
   * Load an image into the fabric
   */
  loadImage(data: Uint8Array | Float32Array, width: number, height: number): void {
    // Resize if necessary
    const scaleX = width / this.config.width;
    const scaleY = height / this.config.height;

    for (let y = 0; y < this.config.height; y++) {
      for (let x = 0; x < this.config.width; x++) {
        const srcX = Math.floor(x * scaleX);
        const srcY = Math.floor(y * scaleY);
        const srcIdx = srcY * width + srcX;
        const dstIdx = y * this.config.width + x;

        let value = data[srcIdx];
        if (data instanceof Uint8Array) {
          value = value / 255.0;
        }
        this.pixels[dstIdx] = value;
      }
    }
    this.cycleCount += BigInt(this.size);
  }

  /**
   * Apply a convolution kernel to all pixels
   */
  convolve(kernel: Kernel): void {
    const { width, height } = this.config;
    const kw = kernel.width;
    const kh = kernel.height;
    const khalf = Math.floor(kh / 2);
    const kwhalf = Math.floor(kw / 2);
    const output = new Float32Array(this.size);

    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        let sum = 0;

        for (let ky = 0; ky < kh; ky++) {
          for (let kx = 0; kx < kw; kx++) {
            const px = x + kx - kwhalf;
            const py = y + ky - khalf;

            if (px >= 0 && px < width && py >= 0 && py < height) {
              const pixelIdx = py * width + px;
              const kernelIdx = ky * kw + kx;
              sum += this.pixels[pixelIdx] * kernel.weights[kernelIdx];
            }
          }
        }

        output[y * width + x] = sum;
      }
    }

    this.pixels = output;
    this.cycleCount += BigInt(this.size * kw * kh);
  }

  /**
   * Apply activation function to all pixels
   */
  activate(fn: ActivationFunction): void {
    for (let i = 0; i < this.size; i++) {
      this.pixels[i] = this.applyActivation(this.pixels[i], fn);
    }
    this.cycleCount += BigInt(this.size);
  }

  private applyActivation(x: number, fn: ActivationFunction): number {
    switch (fn) {
      case 'relu':
        return Math.max(0, x);
      case 'sigmoid':
        return 1 / (1 + Math.exp(-x));
      case 'tanh':
        return Math.tanh(x);
      case 'softmax':
        return Math.exp(x); // Normalization done separately
      case 'swish':
        return x / (1 + Math.exp(-x));
      case 'lif':
        // Leaky integrate-and-fire handled in step()
        return x;
      default:
        return x;
    }
  }

  /**
   * Run one simulation step (for cellular automata / spiking networks)
   */
  step(): void {
    const { width, height } = this.config;
    const output = new Float32Array(this.size);

    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const idx = y * width + x;
        output[idx] = this.computeCell(x, y);
      }
    }

    this.pixels = output;
    this.cycleCount += BigInt(this.size);
  }

  /**
   * Run multiple steps
   */
  run(steps: number): void {
    for (let i = 0; i < steps; i++) {
      this.step();
    }
  }

  /**
   * Compute a single cell update (for Game of Life, spiking, etc.)
   */
  private computeCell(x: number, y: number): number {
    const { width, height } = this.config;
    const idx = y * width + x;
    let neighbors = 0;
    let neighborSum = 0;

    // Count neighbors in 3x3 neighborhood
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;

        const nx = x + dx;
        const ny = y + dy;

        if (nx >= 0 && nx < width && ny >= 0 && ny < height) {
          const nidx = ny * width + nx;
          const val = this.pixels[nidx];
          if (val > 0.5) neighbors++;
          neighborSum += val;
        }
      }
    }

    return this.pixels[idx] + neighborSum * 0.01; // Default: weighted sum
  }

  /**
   * Get pixel value at coordinates
   */
  getPixel(x: number, y: number): number {
    if (x < 0 || x >= this.config.width || y < 0 || y >= this.config.height) {
      return 0;
    }
    return this.pixels[y * this.config.width + x];
  }

  /**
   * Set pixel value at coordinates
   */
  setPixel(x: number, y: number, value: number): void {
    if (x >= 0 && x < this.config.width && y >= 0 && y < this.config.height) {
      this.pixels[y * this.config.width + x] = value;
    }
  }

  /**
   * Get all pixel values as a flat array
   */
  getPixels(): Float32Array {
    return new Float32Array(this.pixels);
  }

  /**
   * Get pixels as 2D array
   */
  getPixels2D(): Float32Array[] {
    const result: Float32Array[] = [];
    for (let y = 0; y < this.config.height; y++) {
      const row = new Float32Array(this.config.width);
      for (let x = 0; x < this.config.width; x++) {
        row[x] = this.pixels[y * this.config.width + x];
      }
      result.push(row);
    }
    return result;
  }

  /**
   * Get membrane potentials (for spiking neurons)
   */
  getMembranes(): Float32Array {
    return new Float32Array(this.membranes);
  }

  /**
   * Inject current at edge pixels (for reservoir computing)
   */
  injectEdge(signal: Float32Array): void {
    const { width, height } = this.config;
    const signalLen = signal.length;

    // Inject along left edge
    for (let y = 0; y < height && y < signalLen; y++) {
      this.pixels[y * width] += signal[y];
    }

    this.cycleCount += BigInt(Math.min(height, signalLen));
  }

  /**
   * Read output from edge pixels
   */
  readEdge(): Float32Array {
    const { width, height } = this.config;
    const output = new Float32Array(height);

    // Read from right edge
    for (let y = 0; y < height; y++) {
      output[y] = this.pixels[y * width + (width - 1)];
    }

    return output;
  }

  /**
   * Get performance metrics
   */
  getMetrics(): PerformanceMetrics {
    const elapsedMs = performance.now() - this.startTime;
    const densitySpec = DENSITY_SPECS[this.config.densityTier];

    const powerMw = this.size * densitySpec.powerPerPixelMw;
    const opsPerSec = Number(this.cycleCount) / (elapsedMs / 1000);

    return {
      totalCycles: this.cycleCount,
      activeTiles: this.size,
      powerMw,
      throughputOpsPerSec: opsPerSec,
      memoryBandwidthGbps: (this.size * 8 * opsPerSec) / 1e9,
      utilizationPercent: 100, // Simplified
    };
  }

  /**
   * Get current cycle count
   */
  getCycles(): bigint {
    return this.cycleCount;
  }

  /**
   * Get configuration
   */
  getConfig(): FabricConfig {
    return { ...this.config };
  }

  /**
   * Reset the fabric
   */
  reset(): void {
    this.pixels.fill(0);
    this.membranes.fill(0);
    this.refractory.fill(0);
    this.cycleCount = 0n;
    this.startTime = performance.now();
  }

  /**
   * Calculate mean squared error vs reference
   */
  mse(reference: Float32Array): number {
    let sum = 0;
    const len = Math.min(this.size, reference.length);
    for (let i = 0; i < len; i++) {
      const diff = this.pixels[i] - reference[i];
      sum += diff * diff;
    }
    return sum / len;
  }

  /**
   * Normalize pixel values to [0, 1]
   */
  normalize(): void {
    let min = Infinity;
    let max = -Infinity;

    for (let i = 0; i < this.size; i++) {
      if (this.pixels[i] < min) min = this.pixels[i];
      if (this.pixels[i] > max) max = this.pixels[i];
    }

    const range = max - min;
    if (range > 0) {
      for (let i = 0; i < this.size; i++) {
        this.pixels[i] = (this.pixels[i] - min) / range;
      }
    }
  }

  /**
   * Apply absolute value to all pixels
   */
  abs(): void {
    for (let i = 0; i < this.size; i++) {
      this.pixels[i] = Math.abs(this.pixels[i]);
    }
  }

  /**
   * Apply threshold to convert to binary
   */
  threshold(value: number): void {
    for (let i = 0; i < this.size; i++) {
      this.pixels[i] = this.pixels[i] > value ? 1.0 : 0.0;
    }
  }
}
