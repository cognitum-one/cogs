/**
 * FIXEL Neural Network Simulation Capabilities
 *
 * Neural network architectures that run natively on the FIXEL pixel processing fabric.
 * Each network leverages the fabric's Cognitum mesh connectivity and per-pixel computation.
 */

import { Fabric, FabricMetrics, ReduceOp } from './fabric.js';
import { Cognitum, SpikeEvent } from './cognitum.js';

// High-resolution timer for performance measurement
// Using Date.now() for cross-environment compatibility
const getTimestamp = (): number => Date.now();

// ============================================================================
// Common Types and Interfaces
// ============================================================================

export interface NeuralMetrics {
  /** Spikes per second (for spiking networks) */
  spikesPerSecond: number;
  /** Estimated power consumption in mW */
  powerMW: number;
  /** Accuracy (if applicable) */
  accuracy: number;
  /** Total cycles executed */
  cycles: number;
  /** Operations per second */
  opsPerSecond: number;
  /** Active neurons/pixels */
  activeNeurons: number;
}

export interface TrainResult {
  /** Training loss over epochs */
  lossHistory: number[];
  /** Final metrics */
  metrics: NeuralMetrics;
  /** Training duration in ms */
  durationMs: number;
}

export interface InferResult {
  /** Output data */
  output: number[][] | number[];
  /** Inference metrics */
  metrics: NeuralMetrics;
  /** Inference duration in ms */
  durationMs: number;
}

/**
 * Helper to convert fabric metrics to neural metrics
 */
function fabricToNeuralMetrics(fabricMetrics: FabricMetrics, clockSpeedMHz: number): NeuralMetrics {
  return {
    spikesPerSecond: fabricMetrics.totalSpikeRate,
    powerMW: fabricMetrics.totalPowerWatts * 1000,
    accuracy: 0,
    cycles: fabricMetrics.cycleCount,
    opsPerSecond: fabricMetrics.cycleCount * clockSpeedMHz * 1e6,
    activeNeurons: fabricMetrics.activeTiles * 256, // Approximate
  };
}

// ============================================================================
// Cellular Neural Network (CNN - Chua's Model)
// ============================================================================

/**
 * Classic Chua Cellular Neural Network
 *
 * Each pixel (Cognitum) is a cell with state x that evolves according to:
 *   dx/dt = -x + A*y + B*u + I
 *
 * Where:
 *   - A is the 3x3 feedback template (neighbor outputs)
 *   - B is the 3x3 input template (neighbor inputs)
 *   - y = f(x) is the output nonlinearity (piecewise linear)
 *   - u is the input
 *   - I is the bias current
 *
 * Applications: edge detection, noise reduction, image segmentation
 */
export class CellularNeuralNetwork {
  private fabric: Fabric;
  private templateA: number[][];  // Feedback template
  private templateB: number[][];  // Input template
  private bias: number;
  private tau: number;  // Time constant
  private dt: number;   // Integration step

  // Memory layout in Cognitum SRAM (bytes)
  private static readonly STATE_OFFSET = 0;     // 4 bytes - cell state (float32)
  private static readonly INPUT_OFFSET = 4;     // 4 bytes - input value (float32)
  private static readonly OUTPUT_OFFSET = 8;    // 4 bytes - output value (float32)

  constructor(fabric: Fabric, options: {
    templateA?: number[][];
    templateB?: number[][];
    bias?: number;
    tau?: number;
    dt?: number;
  } = {}) {
    this.fabric = fabric;

    // Default: Edge detection templates
    this.templateA = options.templateA ?? [
      [0, 0, 0],
      [0, 2, 0],
      [0, 0, 0],
    ];

    this.templateB = options.templateB ?? [
      [-1, -1, -1],
      [-1, 8, -1],
      [-1, -1, -1],
    ];

    this.bias = options.bias ?? -0.5;
    this.tau = options.tau ?? 1.0;
    this.dt = options.dt ?? 0.1;
  }

  /**
   * Set feedback template (A matrix)
   */
  setTemplateA(template: number[][]): void {
    if (template.length !== 3 || template[0].length !== 3) {
      throw new Error('Template A must be 3x3');
    }
    this.templateA = template;
  }

  /**
   * Set input template (B matrix)
   */
  setTemplateB(template: number[][]): void {
    if (template.length !== 3 || template[0].length !== 3) {
      throw new Error('Template B must be 3x3');
    }
    this.templateB = template;
  }

  /**
   * Piecewise linear output function
   */
  private outputFunction(x: number): number {
    return 0.5 * (Math.abs(x + 1) - Math.abs(x - 1));
  }

  /**
   * Write a float32 to Cognitum memory
   */
  private writeFloat(cognitum: Cognitum, offset: number, value: number): void {
    const buffer = new ArrayBuffer(4);
    new Float32Array(buffer)[0] = value;
    cognitum.writeMemory(offset, new Uint8Array(buffer));
  }

  /**
   * Read a float32 from Cognitum memory
   */
  private readFloat(cognitum: Cognitum, offset: number): number {
    const data = cognitum.readMemory(offset, 4);
    return new Float32Array(data.buffer)[0];
  }

  /**
   * Load input image into the network
   */
  loadInput(image: number[][]): void {
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          const value = image[y]?.[x] ?? 0;
          // Normalize to [-1, 1] range
          const normalized = (value / 127.5) - 1;

          this.writeFloat(cognitum, CellularNeuralNetwork.INPUT_OFFSET, normalized);
          this.writeFloat(cognitum, CellularNeuralNetwork.STATE_OFFSET, 0);
          this.writeFloat(cognitum, CellularNeuralNetwork.OUTPUT_OFFSET, 0);
          cognitum.setMembrane(normalized);
        }
      }
    }
  }

  /**
   * Perform one integration step
   */
  private step(): void {
    const width = this.fabric.width;
    const height = this.fabric.height;
    const newStates: number[][] = [];

    // Calculate new states
    for (let y = 0; y < height; y++) {
      newStates[y] = [];
      for (let x = 0; x < width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (!cognitum) {
          newStates[y][x] = 0;
          continue;
        }

        const currentX = this.readFloat(cognitum, CellularNeuralNetwork.STATE_OFFSET);
        const u = this.readFloat(cognitum, CellularNeuralNetwork.INPUT_OFFSET);

        // Calculate template convolutions
        let feedbackSum = 0;
        let inputSum = 0;

        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const neighbor = this.fabric.getCognitum(x + dx, y + dy);
            if (neighbor) {
              const neighborX = this.readFloat(neighbor, CellularNeuralNetwork.STATE_OFFSET);
              const neighborU = this.readFloat(neighbor, CellularNeuralNetwork.INPUT_OFFSET);
              const neighborY = this.outputFunction(neighborX);

              feedbackSum += this.templateA[dy + 1][dx + 1] * neighborY;
              inputSum += this.templateB[dy + 1][dx + 1] * neighborU;
            }
          }
        }

        // State evolution: dx/dt = -x + A*y + B*u + I
        const dxdt = (-currentX + feedbackSum + inputSum + this.bias) / this.tau;
        newStates[y][x] = currentX + dxdt * this.dt;
      }
    }

    // Update states
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          const newState = newStates[y][x];
          this.writeFloat(cognitum, CellularNeuralNetwork.STATE_OFFSET, newState);
          const output = this.outputFunction(newState);
          this.writeFloat(cognitum, CellularNeuralNetwork.OUTPUT_OFFSET, output);
          cognitum.setMembrane(output);
        }
      }
    }

    this.fabric.tick();
  }

  /**
   * Train (converge) the network
   */
  train(options: { maxIterations?: number; convergenceThreshold?: number } = {}): TrainResult {
    const startTime = getTimestamp();
    const maxIterations = options.maxIterations ?? 100;
    const convergenceThreshold = options.convergenceThreshold ?? 0.001;
    const lossHistory: number[] = [];

    let prevEnergy = Infinity;

    for (let i = 0; i < maxIterations; i++) {
      this.step();

      // Calculate energy (convergence measure)
      let energy = 0;
      for (let y = 0; y < this.fabric.height; y++) {
        for (let x = 0; x < this.fabric.width; x++) {
          const cognitum = this.fabric.getCognitum(x, y);
          if (cognitum) {
            const state = this.readFloat(cognitum, CellularNeuralNetwork.STATE_OFFSET);
            energy += state * state;
          }
        }
      }
      energy /= (this.fabric.width * this.fabric.height);
      lossHistory.push(energy);

      if (Math.abs(energy - prevEnergy) < convergenceThreshold) {
        break;
      }
      prevEnergy = energy;
    }

    return {
      lossHistory,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Run inference (single pass after training)
   */
  infer(input: number[][]): InferResult {
    const startTime = getTimestamp();
    this.loadInput(input);
    this.train({ maxIterations: 50 });

    const output = this.extractOutput();

    return {
      output,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Extract output image
   */
  extractOutput(): number[][] {
    const output: number[][] = [];
    for (let y = 0; y < this.fabric.height; y++) {
      output[y] = [];
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          // Map from [-1, 1] to [0, 255]
          const value = this.readFloat(cognitum, CellularNeuralNetwork.OUTPUT_OFFSET);
          output[y][x] = Math.round((value + 1) * 127.5);
        } else {
          output[y][x] = 0;
        }
      }
    }
    return output;
  }

  /**
   * Get current metrics
   */
  getMetrics(): NeuralMetrics {
    const fabricMetrics = this.fabric.getMetrics();
    return {
      spikesPerSecond: 0, // Not applicable for CNN
      powerMW: fabricMetrics.totalPowerWatts * 1000,
      accuracy: 1.0, // Template-based, deterministic
      cycles: fabricMetrics.cycleCount,
      opsPerSecond: fabricMetrics.cycleCount * this.fabric.clockSpeed * 1e6,
      activeNeurons: this.fabric.width * this.fabric.height,
    };
  }

  /**
   * Predefined templates for common operations
   */
  static readonly Templates = {
    edgeDetection: {
      A: [[0, 0, 0], [0, 2, 0], [0, 0, 0]],
      B: [[-1, -1, -1], [-1, 8, -1], [-1, -1, -1]],
    },
    noiseReduction: {
      A: [[0.5, 0.5, 0.5], [0.5, 2, 0.5], [0.5, 0.5, 0.5]],
      B: [[0.125, 0.125, 0.125], [0.125, 0, 0.125], [0.125, 0.125, 0.125]],
    },
    cornerDetection: {
      A: [[0, 0, 0], [0, 3, 0], [0, 0, 0]],
      B: [[-1, 0, -1], [0, 4, 0], [-1, 0, -1]],
    },
    holesFilling: {
      A: [[0.5, 0.5, 0.5], [0.5, 1.5, 0.5], [0.5, 0.5, 0.5]],
      B: [[0, 0, 0], [0, 1, 0], [0, 0, 0]],
    },
  };
}

// ============================================================================
// Spiking Neural Network (LIF Neurons)
// ============================================================================

/**
 * Spiking Neural Network with Leaky Integrate-and-Fire neurons
 *
 * Uses the native Cognitum spike mechanism:
 * - Each Cognitum has membrane potential that integrates inputs
 * - Threshold-based spike generation
 * - Leak/decay between timesteps
 * - Refractory period after spiking
 *
 * Supports optional STDP (Spike-Timing-Dependent Plasticity) learning
 */
export class SpikingNeuralNetwork {
  private fabric: Fabric;
  private threshold: number;
  private enableSTDP: boolean;
  private stdpLearningRate: number;
  private stdpTimeWindow: number;
  private currentTime: number = 0;
  private spikeCount: number = 0;

  // Memory layout in Cognitum SRAM
  // Weights for 8 neighbors: 8 * 4 = 32 bytes
  private static readonly WEIGHTS_OFFSET = 0;
  private static readonly LAST_SPIKE_OFFSET = 32;  // 4 bytes
  private static readonly SPIKE_COUNT_OFFSET = 36; // 4 bytes

  constructor(fabric: Fabric, options: {
    threshold?: number;
    enableSTDP?: boolean;
    stdpLearningRate?: number;
    stdpTimeWindow?: number;
  } = {}) {
    this.fabric = fabric;
    this.threshold = options.threshold ?? 1.0;
    this.enableSTDP = options.enableSTDP ?? false;
    this.stdpLearningRate = options.stdpLearningRate ?? 0.01;
    this.stdpTimeWindow = options.stdpTimeWindow ?? 20;

    this.initializeWeights();
  }

  /**
   * Write a float32 to Cognitum memory
   */
  private writeFloat(cognitum: Cognitum, offset: number, value: number): void {
    const buffer = new ArrayBuffer(4);
    new Float32Array(buffer)[0] = value;
    cognitum.writeMemory(offset, new Uint8Array(buffer));
  }

  /**
   * Read a float32 from Cognitum memory
   */
  private readFloat(cognitum: Cognitum, offset: number): number {
    const data = cognitum.readMemory(offset, 4);
    return new Float32Array(data.buffer)[0];
  }

  /**
   * Initialize synaptic weights (stored in Cognitum SRAM)
   */
  private initializeWeights(): void {
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          // Initialize weights for 8 neighbors
          for (let i = 0; i < 8; i++) {
            const weight = Math.random() * 0.5;
            this.writeFloat(cognitum, SpikingNeuralNetwork.WEIGHTS_OFFSET + i * 4, weight);
          }
          // Initialize spike tracking
          this.writeFloat(cognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET, -1000);
          this.writeFloat(cognitum, SpikingNeuralNetwork.SPIKE_COUNT_OFFSET, 0);
          // Set threshold
          cognitum.setThreshold(this.threshold);
        }
      }
    }
  }

  /**
   * Get neighbor index (0-7 for 8-connected)
   */
  private getNeighborIndex(dx: number, dy: number): number {
    // Map dx,dy to index: (-1,-1)=0, (0,-1)=1, (1,-1)=2, (-1,0)=3, (1,0)=4, (-1,1)=5, (0,1)=6, (1,1)=7
    const lookup: Record<string, number> = {
      '-1,-1': 0, '0,-1': 1, '1,-1': 2,
      '-1,0': 3, '1,0': 4,
      '-1,1': 5, '0,1': 6, '1,1': 7,
    };
    return lookup[`${dx},${dy}`] ?? 0;
  }

  /**
   * Set weight between neurons
   */
  setWeight(x: number, y: number, dx: number, dy: number, weight: number): void {
    const cognitum = this.fabric.getCognitum(x, y);
    if (cognitum) {
      const idx = this.getNeighborIndex(dx, dy);
      this.writeFloat(cognitum, SpikingNeuralNetwork.WEIGHTS_OFFSET + idx * 4, weight);
    }
  }

  /**
   * Get weight between neurons
   */
  getWeight(x: number, y: number, dx: number, dy: number): number {
    const cognitum = this.fabric.getCognitum(x, y);
    if (cognitum) {
      const idx = this.getNeighborIndex(dx, dy);
      return this.readFloat(cognitum, SpikingNeuralNetwork.WEIGHTS_OFFSET + idx * 4);
    }
    return 0;
  }

  /**
   * Inject input spikes at specified locations
   */
  injectSpikes(spikes: Array<{ x: number; y: number; strength?: number }>): void {
    for (const spike of spikes) {
      const cognitum = this.fabric.getCognitum(spike.x, spike.y);
      if (cognitum) {
        const potential = cognitum.getMembranePotential();
        const strength = (spike.strength ?? 1.0) * 0x4000; // Scale to 16-bit
        cognitum.accumulateMembrane(Math.round(strength / 256)); // Convert to 8-bit for accumulate
      }
    }
  }

  /**
   * Inject input from image (rate coding)
   */
  injectFromImage(image: number[][], scale: number = 1.0): void {
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          const intensity = (image[y]?.[x] ?? 0) / 255 * scale;
          const input = Math.round(intensity * 127); // Scale to signed 8-bit
          cognitum.accumulateMembrane(input);
        }
      }
    }
  }

  /**
   * Perform one simulation timestep
   */
  step(): number {
    // Run one tick of the fabric (handles spike propagation)
    this.fabric.tick();
    this.currentTime++;

    // Count spikes and apply STDP if enabled
    let stepSpikes = 0;

    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          const state = cognitum.getState();

          // Check if this cognitum spiked by checking the spike flag
          if (state.flags.spike) {
            stepSpikes++;
            const prevCount = this.readFloat(cognitum, SpikingNeuralNetwork.SPIKE_COUNT_OFFSET);
            this.writeFloat(cognitum, SpikingNeuralNetwork.SPIKE_COUNT_OFFSET, prevCount + 1);
            this.writeFloat(cognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET, this.currentTime);

            // STDP: Update weights based on timing
            if (this.enableSTDP) {
              this.applySTDP(x, y);
            }
          }
        }
      }
    }

    this.spikeCount += stepSpikes;
    return stepSpikes;
  }

  /**
   * Apply STDP learning rule
   */
  private applySTDP(x: number, y: number): void {
    const postCognitum = this.fabric.getCognitum(x, y);
    if (!postCognitum) return;

    const postSpikeTime = this.readFloat(postCognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET);

    // Check all neighbors
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;

        const preCognitum = this.fabric.getCognitum(x + dx, y + dy);
        if (preCognitum) {
          const preSpikeTime = this.readFloat(preCognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET);
          const dt = postSpikeTime - preSpikeTime;

          if (Math.abs(dt) < this.stdpTimeWindow) {
            const weight = this.getWeight(x + dx, y + dy, -dx, -dy);
            let dw: number;

            if (dt > 0) {
              // Pre before post: LTP (strengthen)
              dw = this.stdpLearningRate * Math.exp(-dt / this.stdpTimeWindow);
            } else {
              // Post before pre: LTD (weaken)
              dw = -this.stdpLearningRate * Math.exp(dt / this.stdpTimeWindow) * 0.5;
            }

            const newWeight = Math.max(0, Math.min(1, weight + dw));
            this.setWeight(x + dx, y + dy, -dx, -dy, newWeight);
          }
        }
      }
    }
  }

  /**
   * Train the network with STDP
   */
  train(options: {
    epochs?: number;
    inputSequence?: number[][][];
    targetPattern?: number[][];
  } = {}): TrainResult {
    const startTime = getTimestamp();
    const epochs = options.epochs ?? 100;
    const lossHistory: number[] = [];

    // Note: If STDP is disabled, training will not update weights
    // Users should enable STDP for learning

    for (let epoch = 0; epoch < epochs; epoch++) {
      // Inject input sequence if provided
      if (options.inputSequence) {
        for (const frame of options.inputSequence) {
          this.injectFromImage(frame, 0.5);
          this.step();
        }
      } else {
        // Random input for unsupervised learning
        this.step();
      }

      // Track activity as proxy for loss
      const spikeRates = this.fabric.getSpikeRates();
      const avgRate = spikeRates.total / (this.fabric.width * this.fabric.height);
      lossHistory.push(1 - Math.min(1, avgRate));
    }

    return {
      lossHistory,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Run inference
   */
  infer(input: number[][], timesteps: number = 10): InferResult {
    const startTime = getTimestamp();
    this.spikeCount = 0;

    for (let t = 0; t < timesteps; t++) {
      this.injectFromImage(input, 0.3);
      this.step();
    }

    // Output is membrane potential per neuron
    const output: number[][] = [];
    for (let y = 0; y < this.fabric.height; y++) {
      output[y] = [];
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          // Get membrane potential and normalize to 0-255 range
          output[y][x] = Math.round((cognitum.getMembranePotential() / 0x8000 + 1) * 127.5);
        } else {
          output[y][x] = 0;
        }
      }
    }

    return {
      output,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Get spike raster (for visualization)
   */
  getSpikeRaster(): Array<{ x: number; y: number; time: number }> {
    const raster: Array<{ x: number; y: number; time: number }> = [];

    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          // Read last spike time from our SRAM storage
          const lastSpikeTime = this.readFloat(cognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET);
          if (lastSpikeTime >= 0 && lastSpikeTime <= this.currentTime) {
            raster.push({ x, y, time: lastSpikeTime });
          }
        }
      }
    }

    return raster;
  }

  /**
   * Get current metrics
   */
  getMetrics(): NeuralMetrics {
    const fabricMetrics = this.fabric.getMetrics();
    const durationSec = this.currentTime / 1000; // Assuming 1ms timestep

    return {
      spikesPerSecond: durationSec > 0 ? this.spikeCount / durationSec : fabricMetrics.totalSpikeRate,
      powerMW: fabricMetrics.totalPowerWatts * 1000,
      accuracy: 0, // Needs external evaluation
      cycles: this.currentTime,
      opsPerSecond: fabricMetrics.cycleCount * this.fabric.clockSpeed * 1e6,
      activeNeurons: fabricMetrics.activeTiles * 256,
    };
  }

  /**
   * Reset network state (but keep weights)
   */
  reset(): void {
    this.fabric.reset();
    this.currentTime = 0;
    this.spikeCount = 0;

    // Re-initialize spike tracking (keep weights)
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          this.writeFloat(cognitum, SpikingNeuralNetwork.LAST_SPIKE_OFFSET, -1000);
          this.writeFloat(cognitum, SpikingNeuralNetwork.SPIKE_COUNT_OFFSET, 0);
        }
      }
    }
  }
}

// ============================================================================
// Reservoir Computer (Echo State Network)
// ============================================================================

/**
 * Reservoir Computer using the fabric as a dynamical reservoir
 *
 * The fabric provides:
 * - Natural dynamics through mesh connectivity
 * - High-dimensional nonlinear transformation
 * - Edge pixels serve as input injection points
 * - Internal state can be read out for training
 *
 * Only the readout layer is trained (linear regression)
 */
export class ReservoirComputer {
  private fabric: Fabric;
  private inputScale: number;
  private spectralRadius: number;
  private leakRate: number;
  private readoutWeights: number[] | null = null;
  private inputDim: number;
  private outputDim: number;
  private reservoirStates: number[][] = [];

  // Memory layout
  private static readonly STATE_OFFSET = 0;      // 4 bytes
  private static readonly INTERNAL_OFFSET = 4;   // 4 bytes

  constructor(fabric: Fabric, options: {
    inputScale?: number;
    spectralRadius?: number;
    leakRate?: number;
    inputDim?: number;
    outputDim?: number;
  } = {}) {
    this.fabric = fabric;
    this.inputScale = options.inputScale ?? 0.5;
    this.spectralRadius = options.spectralRadius ?? 0.9;
    this.leakRate = options.leakRate ?? 0.3;
    this.inputDim = options.inputDim ?? fabric.width;  // Edge pixels
    this.outputDim = options.outputDim ?? 1;

    this.initializeReservoir();
  }

  /**
   * Write a float32 to Cognitum memory
   */
  private writeFloat(cognitum: Cognitum, offset: number, value: number): void {
    const buffer = new ArrayBuffer(4);
    new Float32Array(buffer)[0] = value;
    cognitum.writeMemory(offset, new Uint8Array(buffer));
  }

  /**
   * Read a float32 from Cognitum memory
   */
  private readFloat(cognitum: Cognitum, offset: number): number {
    const data = cognitum.readMemory(offset, 4);
    return new Float32Array(data.buffer)[0];
  }

  /**
   * Initialize reservoir states
   */
  private initializeReservoir(): void {
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          // Random initial state
          const state = (Math.random() - 0.5) * 0.1;
          this.writeFloat(cognitum, ReservoirComputer.STATE_OFFSET, state);
          cognitum.setMembrane(state);
        }
      }
    }
  }

  /**
   * Inject input at edge pixels (left edge)
   */
  injectInput(input: number[]): void {
    const height = Math.min(input.length, this.fabric.height);
    for (let y = 0; y < height; y++) {
      const cognitum = this.fabric.getCognitum(0, y);
      if (cognitum) {
        const current = this.readFloat(cognitum, ReservoirComputer.STATE_OFFSET);
        const injection = input[y] * this.inputScale;
        this.writeFloat(cognitum, ReservoirComputer.STATE_OFFSET, current + injection);
        cognitum.setMembrane(current + injection);
      }
    }
  }

  /**
   * Perform one reservoir update step
   */
  step(): void {
    const width = this.fabric.width;
    const height = this.fabric.height;
    const newStates: number[][] = [];

    for (let y = 0; y < height; y++) {
      newStates[y] = [];
      for (let x = 0; x < width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (!cognitum) {
          newStates[y][x] = 0;
          continue;
        }

        const currentState = this.readFloat(cognitum, ReservoirComputer.STATE_OFFSET);

        // Sum weighted inputs from neighbors (using fabric mesh)
        let neighborSum = 0;
        let neighborCount = 0;

        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;

            const neighbor = this.fabric.getCognitum(x + dx, y + dy);
            if (neighbor) {
              const neighborState = this.readFloat(neighbor, ReservoirComputer.STATE_OFFSET);
              // Weight based on distance
              const weight = (1 / Math.sqrt(dx * dx + dy * dy)) * this.spectralRadius;
              neighborSum += neighborState * weight;
              neighborCount++;
            }
          }
        }

        // Normalize and apply nonlinearity
        if (neighborCount > 0) {
          neighborSum /= neighborCount;
        }

        // Leaky integration with tanh nonlinearity
        const newState = (1 - this.leakRate) * currentState +
                         this.leakRate * Math.tanh(neighborSum);
        newStates[y][x] = newState;
      }
    }

    // Update fabric
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          this.writeFloat(cognitum, ReservoirComputer.STATE_OFFSET, newStates[y][x]);
          cognitum.setMembrane(newStates[y][x]);
        }
      }
    }

    this.fabric.tick();
  }

  /**
   * Collect reservoir state for readout
   */
  collectState(): number[] {
    const state: number[] = [];
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          state.push(this.readFloat(cognitum, ReservoirComputer.STATE_OFFSET));
        }
      }
    }
    return state;
  }

  /**
   * Verify echo state property (spectral radius < 1)
   */
  verifyEchoState(testLength: number = 100): { valid: boolean; lyapunovExponent: number } {
    // Run without input and check for convergence
    const initialEnergy = this.collectState().reduce((a, b) => a + b * b, 0);

    for (let i = 0; i < testLength; i++) {
      this.step();
    }

    const finalEnergy = this.collectState().reduce((a, b) => a + b * b, 0);

    // Estimate Lyapunov exponent
    const lyapunov = Math.log(finalEnergy / (initialEnergy + 1e-10)) / testLength;

    return {
      valid: lyapunov < 0,  // Negative = stable, contracting
      lyapunovExponent: lyapunov,
    };
  }

  /**
   * Train readout layer using collected states
   */
  train(options: {
    inputSequence: number[][];
    targetSequence: number[][];
    washout?: number;
  }): TrainResult {
    const startTime = getTimestamp();
    const washout = options.washout ?? 10;
    const lossHistory: number[] = [];

    // Reset reservoir
    this.initializeReservoir();

    // Collect states
    const states: number[][] = [];
    const targets: number[][] = [];

    for (let t = 0; t < options.inputSequence.length; t++) {
      this.injectInput(options.inputSequence[t]);
      this.step();

      if (t >= washout) {
        states.push(this.collectState());
        targets.push(options.targetSequence[t]);
      }
    }

    this.reservoirStates = states;

    // Simple gradient descent for readout weights
    const numStates = states[0].length;
    const numSamples = states.length;
    const lr = 0.001;

    this.readoutWeights = new Array(numStates).fill(0);

    for (let epoch = 0; epoch < 1000; epoch++) {
      let loss = 0;

      for (let k = 0; k < numSamples; k++) {
        let prediction = 0;
        for (let i = 0; i < numStates; i++) {
          prediction += this.readoutWeights[i] * states[k][i];
        }
        const error = prediction - targets[k][0];
        loss += error * error;

        // Update weights
        for (let i = 0; i < numStates; i++) {
          this.readoutWeights[i] -= lr * error * states[k][i];
        }
      }

      loss /= numSamples;
      if (epoch % 100 === 0) lossHistory.push(loss);
    }

    return {
      lossHistory,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Run inference
   */
  infer(inputSequence: number[][]): InferResult {
    const startTime = getTimestamp();

    if (!this.readoutWeights) {
      throw new Error('Reservoir not trained - call train() first');
    }

    this.initializeReservoir();
    const outputs: number[] = [];

    for (const input of inputSequence) {
      this.injectInput(input);
      this.step();

      const state = this.collectState();
      let prediction = 0;
      for (let i = 0; i < state.length; i++) {
        prediction += this.readoutWeights[i] * state[i];
      }
      outputs.push(prediction);
    }

    return {
      output: outputs,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Get metrics
   */
  getMetrics(): NeuralMetrics {
    const fabricMetrics = this.fabric.getMetrics();
    return {
      spikesPerSecond: 0,
      powerMW: fabricMetrics.totalPowerWatts * 1000,
      accuracy: 0, // Needs external evaluation
      cycles: fabricMetrics.cycleCount,
      opsPerSecond: fabricMetrics.cycleCount * this.fabric.clockSpeed * 1e6,
      activeNeurons: this.fabric.width * this.fabric.height,
    };
  }
}

// ============================================================================
// Self-Organizing Map (Kohonen SOM)
// ============================================================================

/**
 * Self-Organizing Map native to the fabric
 *
 * Each Cognitum is a neuron in the SOM with:
 * - Weight vector stored in SRAM
 * - Winner-take-all via tile reduction
 * - Neighborhood updates via mesh connectivity
 * - Visualization = display membrane state
 */
export class SelfOrganizingMap {
  private fabric: Fabric;
  private inputDim: number;
  private learningRate: number;
  private neighborhoodRadius: number;
  private initialRadius: number;
  private timeConstant: number;
  private iteration: number = 0;

  // Memory layout: weights stored starting at offset 0
  private static readonly WEIGHTS_OFFSET = 0;

  constructor(fabric: Fabric, options: {
    inputDim?: number;
    learningRate?: number;
    neighborhoodRadius?: number;
  } = {}) {
    this.fabric = fabric;
    this.inputDim = options.inputDim ?? 3;  // e.g., RGB
    this.learningRate = options.learningRate ?? 0.5;
    this.initialRadius = options.neighborhoodRadius ?? Math.max(fabric.width, fabric.height) / 2;
    this.neighborhoodRadius = this.initialRadius;
    this.timeConstant = 1000 / Math.log(this.initialRadius);

    this.initializeWeights();
  }

  /**
   * Write a float32 to Cognitum memory
   */
  private writeFloat(cognitum: Cognitum, offset: number, value: number): void {
    const buffer = new ArrayBuffer(4);
    new Float32Array(buffer)[0] = value;
    cognitum.writeMemory(offset, new Uint8Array(buffer));
  }

  /**
   * Read a float32 from Cognitum memory
   */
  private readFloat(cognitum: Cognitum, offset: number): number {
    const data = cognitum.readMemory(offset, 4);
    return new Float32Array(data.buffer)[0];
  }

  /**
   * Initialize weight vectors randomly
   */
  private initializeWeights(): void {
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          for (let i = 0; i < this.inputDim; i++) {
            const weight = Math.random();
            this.writeFloat(cognitum, SelfOrganizingMap.WEIGHTS_OFFSET + i * 4, weight);
          }
        }
      }
    }
  }

  /**
   * Get weight vector for a neuron
   */
  getWeights(x: number, y: number): number[] {
    const cognitum = this.fabric.getCognitum(x, y);
    if (!cognitum) return [];

    const weights: number[] = [];
    for (let i = 0; i < this.inputDim; i++) {
      weights.push(this.readFloat(cognitum, SelfOrganizingMap.WEIGHTS_OFFSET + i * 4));
    }
    return weights;
  }

  /**
   * Set weight vector for a neuron
   */
  setWeights(x: number, y: number, weights: number[]): void {
    const cognitum = this.fabric.getCognitum(x, y);
    if (!cognitum) return;

    for (let i = 0; i < this.inputDim; i++) {
      this.writeFloat(cognitum, SelfOrganizingMap.WEIGHTS_OFFSET + i * 4, weights[i] ?? 0);
    }
  }

  /**
   * Calculate Euclidean distance between input and neuron weights
   */
  private distance(input: number[], weights: number[]): number {
    let sum = 0;
    for (let i = 0; i < this.inputDim; i++) {
      const diff = (input[i] ?? 0) - (weights[i] ?? 0);
      sum += diff * diff;
    }
    return Math.sqrt(sum);
  }

  /**
   * Find Best Matching Unit (BMU) using tile reduction
   */
  findBMU(input: number[]): { x: number; y: number; distance: number } {
    let minDist = Infinity;
    let bmuX = 0;
    let bmuY = 0;

    // Calculate distances and use membrane for storage (enables tile reduce)
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const weights = this.getWeights(x, y);
        const dist = this.distance(input, weights);
        const cognitum = this.fabric.getCognitum(x, y);

        if (cognitum) {
          // Store negative distance (so max = closest)
          cognitum.setMembrane(-dist);
        }

        if (dist < minDist) {
          minDist = dist;
          bmuX = x;
          bmuY = y;
        }
      }
    }

    return { x: bmuX, y: bmuY, distance: minDist };
  }

  /**
   * Neighborhood function (Gaussian)
   */
  private neighborhoodInfluence(bmuX: number, bmuY: number, x: number, y: number): number {
    const distSq = (x - bmuX) ** 2 + (y - bmuY) ** 2;
    return Math.exp(-distSq / (2 * this.neighborhoodRadius ** 2));
  }

  /**
   * Update learning parameters (decay)
   */
  private updateParameters(): void {
    this.iteration++;
    this.neighborhoodRadius = this.initialRadius * Math.exp(-this.iteration / this.timeConstant);
  }

  /**
   * Train on a single input vector
   */
  trainStep(input: number[]): { bmu: { x: number; y: number }; quantizationError: number } {
    const bmu = this.findBMU(input);

    // Update all neurons based on neighborhood
    for (let y = 0; y < this.fabric.height; y++) {
      for (let x = 0; x < this.fabric.width; x++) {
        const influence = this.neighborhoodInfluence(bmu.x, bmu.y, x, y);

        if (influence > 0.01) {  // Skip negligible updates
          const weights = this.getWeights(x, y);
          const newWeights: number[] = [];

          for (let i = 0; i < this.inputDim; i++) {
            const delta = input[i] - weights[i];
            newWeights[i] = weights[i] + this.learningRate * influence * delta;
          }

          this.setWeights(x, y, newWeights);
        }
      }
    }

    this.updateParameters();
    this.fabric.tick();

    return { bmu: { x: bmu.x, y: bmu.y }, quantizationError: bmu.distance };
  }

  /**
   * Train on dataset
   */
  train(options: {
    data: number[][];
    epochs?: number;
    shuffle?: boolean;
  }): TrainResult {
    const startTime = getTimestamp();
    const epochs = options.epochs ?? 100;
    const lossHistory: number[] = [];

    for (let epoch = 0; epoch < epochs; epoch++) {
      let epochError = 0;
      const data = options.shuffle
        ? [...options.data].sort(() => Math.random() - 0.5)
        : options.data;

      for (const sample of data) {
        const result = this.trainStep(sample);
        epochError += result.quantizationError;
      }

      lossHistory.push(epochError / data.length);
    }

    return {
      lossHistory,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Infer (find BMU) for input
   */
  infer(input: number[]): InferResult {
    const startTime = getTimestamp();
    const bmu = this.findBMU(input);

    return {
      output: [bmu.x, bmu.y],
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Get U-Matrix (unified distance matrix) for visualization
   */
  getUMatrix(): number[][] {
    const uMatrix: number[][] = [];

    for (let y = 0; y < this.fabric.height; y++) {
      uMatrix[y] = [];
      for (let x = 0; x < this.fabric.width; x++) {
        const weights = this.getWeights(x, y);
        let avgDist = 0;
        let count = 0;

        // Check all neighbors
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;

            const neighborWeights = this.getWeights(x + dx, y + dy);
            if (neighborWeights.length > 0) {
              avgDist += this.distance(weights, neighborWeights);
              count++;
            }
          }
        }

        uMatrix[y][x] = count > 0 ? avgDist / count : 0;
      }
    }

    return uMatrix;
  }

  /**
   * Visualize SOM (weights as colors for 3D input)
   */
  visualize(): number[][] {
    const image: number[][] = [];

    for (let y = 0; y < this.fabric.height; y++) {
      image[y] = [];
      for (let x = 0; x < this.fabric.width; x++) {
        const weights = this.getWeights(x, y);
        // Average of weights for grayscale
        const value = weights.slice(0, 3).reduce((a, b) => a + b, 0) / 3 * 255;
        image[y][x] = Math.round(value);

        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          cognitum.setMembrane(value / 255);
        }
      }
    }

    return image;
  }

  /**
   * Get metrics
   */
  getMetrics(): NeuralMetrics {
    const fabricMetrics = this.fabric.getMetrics();
    return {
      spikesPerSecond: 0,
      powerMW: fabricMetrics.totalPowerWatts * 1000,
      accuracy: 0,
      cycles: this.iteration,
      opsPerSecond: fabricMetrics.cycleCount * this.fabric.clockSpeed * 1e6,
      activeNeurons: this.fabric.width * this.fabric.height,
    };
  }
}

// ============================================================================
// Convolutional Layer
// ============================================================================

/**
 * Convolutional Layer for fabric-native CNN inference
 *
 * Supports:
 * - 3x3, 5x5, 7x7 kernel sizes
 * - Weight sharing across all pixels
 * - Configurable stride and padding
 * - Multiple channels via time-multiplexing
 */
export class ConvolutionalLayer {
  private fabric: Fabric;
  private kernelSize: 3 | 5 | 7;
  private kernels: number[][][];  // [outChannel][ky][kx]
  private biases: number[];
  private stride: number;
  private padding: number;
  private inChannels: number;
  private outChannels: number;
  private activation: 'relu' | 'sigmoid' | 'tanh' | 'none';

  constructor(fabric: Fabric, options: {
    kernelSize?: 3 | 5 | 7;
    inChannels?: number;
    outChannels?: number;
    stride?: number;
    padding?: number;
    activation?: 'relu' | 'sigmoid' | 'tanh' | 'none';
  } = {}) {
    this.fabric = fabric;
    this.kernelSize = options.kernelSize ?? 3;
    this.inChannels = options.inChannels ?? 1;
    this.outChannels = options.outChannels ?? 1;
    this.stride = options.stride ?? 1;
    this.padding = options.padding ?? Math.floor(this.kernelSize / 2);
    this.activation = options.activation ?? 'relu';

    // Initialize kernels (He initialization)
    this.kernels = [];
    this.biases = [];
    const scale = Math.sqrt(2 / (this.kernelSize * this.kernelSize * this.inChannels));

    for (let oc = 0; oc < this.outChannels; oc++) {
      this.kernels[oc] = [];
      for (let ky = 0; ky < this.kernelSize; ky++) {
        this.kernels[oc][ky] = [];
        for (let kx = 0; kx < this.kernelSize; kx++) {
          this.kernels[oc][ky][kx] = (Math.random() - 0.5) * 2 * scale;
        }
      }
      this.biases[oc] = 0;
    }
  }

  /**
   * Set kernel weights for a channel
   */
  setKernel(channelIdx: number, kernel: number[][]): void {
    if (kernel.length !== this.kernelSize || kernel[0].length !== this.kernelSize) {
      throw new Error(`Kernel must be ${this.kernelSize}x${this.kernelSize}`);
    }
    this.kernels[channelIdx] = kernel;
  }

  /**
   * Get kernel for a channel
   */
  getKernel(channelIdx: number): number[][] {
    return this.kernels[channelIdx];
  }

  /**
   * Set bias for a channel
   */
  setBias(channelIdx: number, bias: number): void {
    this.biases[channelIdx] = bias;
  }

  /**
   * Apply activation function
   */
  private applyActivation(value: number): number {
    switch (this.activation) {
      case 'relu':
        return Math.max(0, value);
      case 'sigmoid':
        return 1 / (1 + Math.exp(-value));
      case 'tanh':
        return Math.tanh(value);
      case 'none':
      default:
        return value;
    }
  }

  /**
   * Forward pass for single channel input
   */
  forward(input: number[][], channelIdx: number = 0): number[][] {
    const kernel = this.kernels[channelIdx];
    const bias = this.biases[channelIdx];
    const kHalf = Math.floor(this.kernelSize / 2);

    const outHeight = Math.floor((this.fabric.height + 2 * this.padding - this.kernelSize) / this.stride) + 1;
    const outWidth = Math.floor((this.fabric.width + 2 * this.padding - this.kernelSize) / this.stride) + 1;

    const output: number[][] = [];

    for (let oy = 0; oy < outHeight; oy++) {
      output[oy] = [];
      for (let ox = 0; ox < outWidth; ox++) {
        let sum = bias;

        for (let ky = 0; ky < this.kernelSize; ky++) {
          for (let kx = 0; kx < this.kernelSize; kx++) {
            const iy = oy * this.stride + ky - this.padding;
            const ix = ox * this.stride + kx - this.padding;

            let inputVal = 0;
            if (iy >= 0 && iy < input.length && ix >= 0 && ix < input[0].length) {
              inputVal = input[iy][ix];
            }

            sum += inputVal * kernel[ky][kx];
          }
        }

        output[oy][ox] = this.applyActivation(sum);
      }
    }

    // Store in fabric for potential chaining
    for (let y = 0; y < Math.min(outHeight, this.fabric.height); y++) {
      for (let x = 0; x < Math.min(outWidth, this.fabric.width); x++) {
        const cognitum = this.fabric.getCognitum(x, y);
        if (cognitum) {
          cognitum.setMembrane(output[y][x]);
        }
      }
    }

    this.fabric.tick();
    return output;
  }

  /**
   * Forward pass with multiple input channels (time-multiplexed)
   */
  forwardMultiChannel(inputs: number[][][]): number[][][] {
    const outputs: number[][][] = [];

    for (let oc = 0; oc < this.outChannels; oc++) {
      const outHeight = Math.floor((this.fabric.height + 2 * this.padding - this.kernelSize) / this.stride) + 1;
      const outWidth = Math.floor((this.fabric.width + 2 * this.padding - this.kernelSize) / this.stride) + 1;

      const channelOutput: number[][] = [];
      for (let y = 0; y < outHeight; y++) {
        channelOutput[y] = new Array(outWidth).fill(this.biases[oc]);
      }

      // For each input channel, accumulate contribution
      for (let ic = 0; ic < Math.min(inputs.length, this.inChannels); ic++) {
        const partial = this.forward(inputs[ic], oc);
        for (let y = 0; y < outHeight; y++) {
          for (let x = 0; x < outWidth; x++) {
            channelOutput[y][x] += partial[y][x] - this.biases[oc];
          }
        }
      }

      // Apply activation
      for (let y = 0; y < outHeight; y++) {
        for (let x = 0; x < outWidth; x++) {
          channelOutput[y][x] = this.applyActivation(channelOutput[y][x]);
        }
      }

      outputs.push(channelOutput);
    }

    return outputs;
  }

  /**
   * Train layer using backpropagation (gradient descent)
   */
  train(options: {
    inputs: number[][][];
    targets: number[][][];
    epochs?: number;
    learningRate?: number;
  }): TrainResult {
    const startTime = getTimestamp();
    const epochs = options.epochs ?? 100;
    const lr = options.learningRate ?? 0.01;
    const lossHistory: number[] = [];

    for (let epoch = 0; epoch < epochs; epoch++) {
      let epochLoss = 0;

      for (let sample = 0; sample < options.inputs.length; sample++) {
        const input = options.inputs[sample];
        const target = options.targets[sample];

        // Forward pass
        const output = this.forwardMultiChannel([input]);

        // Calculate loss and gradients (MSE)
        const kernelGrads: number[][][] = this.kernels.map(() =>
          Array(this.kernelSize).fill(0).map(() => Array(this.kernelSize).fill(0))
        );
        const biasGrads: number[] = new Array(this.outChannels).fill(0);

        for (let oc = 0; oc < this.outChannels; oc++) {
          // Target is a 2D array [height][width], not [channel][height][width]
          // Use first channel as target for all output channels if target has fewer channels
          const targetChannel: number[][] = target;
          const outputChannel = output[oc];

          for (let y = 0; y < outputChannel.length; y++) {
            for (let x = 0; x < outputChannel[y].length; x++) {
              const targetVal = targetChannel[y]?.[x] ?? 0;
              const error = outputChannel[y][x] - targetVal;
              epochLoss += error * error;

              biasGrads[oc] += error;

              for (let ky = 0; ky < this.kernelSize; ky++) {
                for (let kx = 0; kx < this.kernelSize; kx++) {
                  const iy = y * this.stride + ky - this.padding;
                  const ix = x * this.stride + kx - this.padding;

                  if (iy >= 0 && iy < input.length && ix >= 0 && ix < input[0].length) {
                    kernelGrads[oc][ky][kx] += error * input[iy][ix];
                  }
                }
              }
            }
          }
        }

        // Update weights
        for (let oc = 0; oc < this.outChannels; oc++) {
          this.biases[oc] -= lr * biasGrads[oc];
          for (let ky = 0; ky < this.kernelSize; ky++) {
            for (let kx = 0; kx < this.kernelSize; kx++) {
              this.kernels[oc][ky][kx] -= lr * kernelGrads[oc][ky][kx];
            }
          }
        }
      }

      lossHistory.push(epochLoss / options.inputs.length);
    }

    return {
      lossHistory,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Run inference
   */
  infer(input: number[][]): InferResult {
    const startTime = getTimestamp();
    const output = this.forward(input);

    return {
      output,
      metrics: this.getMetrics(),
      durationMs: getTimestamp() - startTime,
    };
  }

  /**
   * Get metrics
   */
  getMetrics(): NeuralMetrics {
    const fabricMetrics = this.fabric.getMetrics();
    const opsPerConv = this.kernelSize * this.kernelSize * this.fabric.width * this.fabric.height;

    return {
      spikesPerSecond: 0,
      powerMW: fabricMetrics.totalPowerWatts * 1000,
      accuracy: 0,
      cycles: fabricMetrics.cycleCount,
      opsPerSecond: opsPerConv * this.fabric.clockSpeed * 1e6,
      activeNeurons: this.fabric.width * this.fabric.height,
    };
  }

  /**
   * Get layer configuration
   */
  getConfig(): {
    kernelSize: number;
    inChannels: number;
    outChannels: number;
    stride: number;
    padding: number;
    activation: string;
  } {
    return {
      kernelSize: this.kernelSize,
      inChannels: this.inChannels,
      outChannels: this.outChannels,
      stride: this.stride,
      padding: this.padding,
      activation: this.activation,
    };
  }

  /**
   * Common convolution kernels
   */
  static readonly Kernels = {
    sobel_x: [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]],
    sobel_y: [[-1, -2, -1], [0, 0, 0], [1, 2, 1]],
    laplacian: [[0, 1, 0], [1, -4, 1], [0, 1, 0]],
    gaussian3x3: [[1/16, 2/16, 1/16], [2/16, 4/16, 2/16], [1/16, 2/16, 1/16]],
    sharpen: [[0, -1, 0], [-1, 5, -1], [0, -1, 0]],
    emboss: [[-2, -1, 0], [-1, 1, 1], [0, 1, 2]],
  };
}

// ============================================================================
// Exports
// ============================================================================

export default {
  CellularNeuralNetwork,
  SpikingNeuralNetwork,
  ReservoirComputer,
  SelfOrganizingMap,
  ConvolutionalLayer,
};
