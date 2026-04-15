/**
 * FIXEL Neural Processing Classes
 * Spiking Neural Networks, Reservoir Computing, and ML layers
 */

import {
  DensityTier,
  DENSITY_SPECS,
  ActivationFunction,
  SpikeEvent,
  ReservoirState,
  PerformanceMetrics,
  LearningConfig,
} from './types.js';
import { Fabric } from './fabric.js';

/**
 * Spiking neuron layer using Leaky Integrate-and-Fire (LIF) model
 */
export class SpikingLayer {
  private membranes: Float32Array;
  private thresholds: Float32Array;
  private refractory: Uint8Array;
  private weights: Float32Array;
  private spikes: SpikeEvent[] = [];
  private time: number = 0;
  private layerId: number;

  readonly size: number;
  readonly tau: number;       // Membrane time constant
  readonly refPeriod: number; // Refractory period in timesteps

  constructor(
    size: number,
    layerId: number = 0,
    options: {
      tau?: number;
      threshold?: number;
      refPeriod?: number;
    } = {}
  ) {
    this.size = size;
    this.layerId = layerId;
    this.tau = options.tau ?? 20;
    this.refPeriod = options.refPeriod ?? 5;

    this.membranes = new Float32Array(size);
    this.thresholds = new Float32Array(size).fill(options.threshold ?? 1.0);
    this.refractory = new Uint8Array(size);
    this.weights = new Float32Array(size * size);

    this.initializeWeights();
  }

  private initializeWeights(): void {
    // Xavier initialization
    const scale = Math.sqrt(2.0 / this.size);
    for (let i = 0; i < this.weights.length; i++) {
      this.weights[i] = (Math.random() - 0.5) * 2 * scale;
    }
  }

  /**
   * Process input spikes and return output spikes
   */
  forward(input: Float32Array): Float32Array {
    const output = new Float32Array(this.size);
    this.spikes = [];

    for (let i = 0; i < this.size; i++) {
      // Skip if in refractory period
      if (this.refractory[i] > 0) {
        this.refractory[i]--;
        continue;
      }

      // Integrate input current
      let current = 0;
      for (let j = 0; j < input.length; j++) {
        current += input[j] * this.weights[i * input.length + j];
      }

      // Leaky integrate
      this.membranes[i] = this.membranes[i] * (1 - 1 / this.tau) + current;

      // Fire if above threshold
      if (this.membranes[i] >= this.thresholds[i]) {
        output[i] = 1.0;
        this.membranes[i] = 0;
        this.refractory[i] = this.refPeriod;
        this.spikes.push({
          neuronId: i,
          time: this.time,
          layer: this.layerId,
        });
      }
    }

    this.time++;
    return output;
  }

  /**
   * Get spikes from this layer
   */
  getSpikes(): SpikeEvent[] {
    return [...this.spikes];
  }

  /**
   * Get membrane potentials
   */
  getMembranes(): Float32Array {
    return new Float32Array(this.membranes);
  }

  /**
   * Reset the layer
   */
  reset(): void {
    this.membranes.fill(0);
    this.refractory.fill(0);
    this.spikes = [];
    this.time = 0;
  }

  /**
   * Set weights from array
   */
  setWeights(weights: Float32Array): void {
    this.weights.set(weights.slice(0, this.weights.length));
  }

  /**
   * Get weights
   */
  getWeights(): Float32Array {
    return new Float32Array(this.weights);
  }
}

/**
 * Spiking Neural Network with multiple layers
 */
export class SpikingNetwork {
  private layers: SpikingLayer[] = [];
  private cycleCount: bigint = 0n;
  private startTime: number = 0;
  private densityTier: DensityTier;

  constructor(
    layerSizes: number[],
    densityTier: DensityTier = 'medium'
  ) {
    this.densityTier = densityTier;
    this.startTime = performance.now();

    for (let i = 0; i < layerSizes.length; i++) {
      this.layers.push(new SpikingLayer(layerSizes[i], i));
    }
  }

  /**
   * Encode input values as spike trains
   */
  encode(values: Float32Array, timesteps: number): Float32Array[] {
    const spikeTrain: Float32Array[] = [];

    for (let t = 0; t < timesteps; t++) {
      const frame = new Float32Array(values.length);
      for (let i = 0; i < values.length; i++) {
        // Poisson encoding: higher value = higher spike probability
        if (Math.random() < values[i]) {
          frame[i] = 1.0;
        }
      }
      spikeTrain.push(frame);
    }

    return spikeTrain;
  }

  /**
   * Process input through all layers
   */
  forward(input: Float32Array): Float32Array {
    let current = input;

    for (const layer of this.layers) {
      current = layer.forward(current);
      this.cycleCount += BigInt(layer.size);
    }

    return current;
  }

  /**
   * Process a spike train and return spike counts per output neuron
   */
  process(spikeTrain: Float32Array[]): Float32Array {
    const outputSize = this.layers[this.layers.length - 1].size;
    const spikeCounts = new Float32Array(outputSize);

    this.reset();

    for (const frame of spikeTrain) {
      const output = this.forward(frame);
      for (let i = 0; i < output.length; i++) {
        spikeCounts[i] += output[i];
      }
    }

    return spikeCounts;
  }

  /**
   * Classify input (returns predicted class)
   */
  classify(input: Float32Array, timesteps: number = 100): number {
    const spikeTrain = this.encode(input, timesteps);
    const spikeCounts = this.process(spikeTrain);

    let maxIdx = 0;
    let maxCount = spikeCounts[0];
    for (let i = 1; i < spikeCounts.length; i++) {
      if (spikeCounts[i] > maxCount) {
        maxCount = spikeCounts[i];
        maxIdx = i;
      }
    }

    return maxIdx;
  }

  /**
   * Get all spike events from all layers
   */
  getAllSpikes(): SpikeEvent[] {
    const allSpikes: SpikeEvent[] = [];
    for (const layer of this.layers) {
      allSpikes.push(...layer.getSpikes());
    }
    return allSpikes;
  }

  /**
   * Reset all layers
   */
  reset(): void {
    for (const layer of this.layers) {
      layer.reset();
    }
  }

  /**
   * Get performance metrics
   */
  getMetrics(): PerformanceMetrics {
    const elapsedMs = performance.now() - this.startTime;
    const densitySpec = DENSITY_SPECS[this.densityTier];
    const totalNeurons = this.layers.reduce((sum, l) => sum + l.size, 0);

    return {
      totalCycles: this.cycleCount,
      activeTiles: totalNeurons,
      powerMw: totalNeurons * densitySpec.powerPerPixelMw,
      throughputOpsPerSec: Number(this.cycleCount) / (elapsedMs / 1000),
      memoryBandwidthGbps: 0,
      utilizationPercent: 100,
    };
  }
}

/**
 * Echo State Network / Liquid State Machine for reservoir computing
 */
export class ReservoirComputer {
  private state: Float32Array;
  private weights: Float32Array;
  private inputWeights: Float32Array;
  private outputWeights: Float32Array;
  private spectralRadius: number;
  private leakRate: number;
  private cycleCount: bigint = 0n;
  private startTime: number = 0;
  private densityTier: DensityTier;

  readonly reservoirSize: number;
  readonly inputSize: number;
  readonly outputSize: number;

  constructor(
    reservoirSize: number,
    inputSize: number,
    outputSize: number,
    options: {
      spectralRadius?: number;
      leakRate?: number;
      sparsity?: number;
      densityTier?: DensityTier;
    } = {}
  ) {
    this.reservoirSize = reservoirSize;
    this.inputSize = inputSize;
    this.outputSize = outputSize;
    this.spectralRadius = options.spectralRadius ?? 0.9;
    this.leakRate = options.leakRate ?? 0.3;
    this.densityTier = options.densityTier ?? 'medium';

    this.state = new Float32Array(reservoirSize);
    this.weights = new Float32Array(reservoirSize * reservoirSize);
    this.inputWeights = new Float32Array(reservoirSize * inputSize);
    this.outputWeights = new Float32Array(outputSize * reservoirSize);

    this.initializeWeights(options.sparsity ?? 0.1);
    this.startTime = performance.now();
  }

  private initializeWeights(sparsity: number): void {
    // Initialize sparse reservoir weights
    for (let i = 0; i < this.weights.length; i++) {
      if (Math.random() < sparsity) {
        this.weights[i] = (Math.random() - 0.5) * 2;
      }
    }

    // Scale to spectral radius (simplified)
    const scale = this.spectralRadius / Math.sqrt(this.reservoirSize * sparsity);
    for (let i = 0; i < this.weights.length; i++) {
      this.weights[i] *= scale;
    }

    // Initialize input weights
    for (let i = 0; i < this.inputWeights.length; i++) {
      this.inputWeights[i] = (Math.random() - 0.5) * 2;
    }

    // Output weights start at zero (will be trained)
    this.outputWeights.fill(0);
  }

  /**
   * Update reservoir state with new input
   */
  update(input: Float32Array): Float32Array {
    const newState = new Float32Array(this.reservoirSize);

    for (let i = 0; i < this.reservoirSize; i++) {
      // Input contribution
      let inputSum = 0;
      for (let j = 0; j < this.inputSize; j++) {
        inputSum += input[j] * this.inputWeights[i * this.inputSize + j];
      }

      // Recurrent contribution
      let recurrentSum = 0;
      for (let j = 0; j < this.reservoirSize; j++) {
        recurrentSum += this.state[j] * this.weights[i * this.reservoirSize + j];
      }

      // Leaky integration with tanh nonlinearity
      newState[i] = (1 - this.leakRate) * this.state[i] +
                    this.leakRate * Math.tanh(inputSum + recurrentSum);
    }

    this.state = newState;
    this.cycleCount += BigInt(this.reservoirSize * (this.inputSize + this.reservoirSize));

    return new Float32Array(this.state);
  }

  /**
   * Read output from reservoir using trained weights
   */
  readout(): Float32Array {
    const output = new Float32Array(this.outputSize);

    for (let i = 0; i < this.outputSize; i++) {
      let sum = 0;
      for (let j = 0; j < this.reservoirSize; j++) {
        sum += this.state[j] * this.outputWeights[i * this.reservoirSize + j];
      }
      output[i] = sum;
    }

    return output;
  }

  /**
   * Train output weights using ridge regression
   */
  train(
    inputs: Float32Array[],
    targets: Float32Array[],
    regularization: number = 1e-6
  ): number {
    const washout = 100;
    const states: Float32Array[] = [];

    this.reset();

    // Collect states (after washout)
    for (let t = 0; t < inputs.length; t++) {
      this.update(inputs[t]);
      if (t >= washout) {
        states.push(new Float32Array(this.state));
      }
    }

    // Ridge regression: W = Y * X^T * (X * X^T + lambda * I)^-1
    // Simplified: use gradient descent
    const learningRate = 0.001;
    const epochs = 100;
    let mse = 0;

    for (let epoch = 0; epoch < epochs; epoch++) {
      mse = 0;
      for (let t = 0; t < states.length; t++) {
        const targetIdx = t + washout;
        if (targetIdx >= targets.length) break;

        // Forward pass
        this.state = states[t];
        const output = this.readout();

        // Compute error
        for (let i = 0; i < this.outputSize; i++) {
          const error = output[i] - targets[targetIdx][i];
          mse += error * error;

          // Update weights
          for (let j = 0; j < this.reservoirSize; j++) {
            this.outputWeights[i * this.reservoirSize + j] -=
              learningRate * error * states[t][j];
          }
        }
      }

      mse /= states.length * this.outputSize;
    }

    return mse;
  }

  /**
   * Predict next values in a time series
   */
  predict(steps: number): Float32Array[] {
    const predictions: Float32Array[] = [];

    for (let t = 0; t < steps; t++) {
      const output = this.readout();
      predictions.push(output);

      // Feed output back as input
      this.update(output);
    }

    return predictions;
  }

  /**
   * Get current reservoir state
   */
  getState(): ReservoirState {
    return {
      nodes: new Float32Array(this.state),
      connections: new Float32Array(this.weights),
      spectralRadius: this.spectralRadius,
      leakRate: this.leakRate,
    };
  }

  /**
   * Reset reservoir state
   */
  reset(): void {
    this.state.fill(0);
  }

  /**
   * Get performance metrics
   */
  getMetrics(): PerformanceMetrics {
    const elapsedMs = performance.now() - this.startTime;
    const densitySpec = DENSITY_SPECS[this.densityTier];

    return {
      totalCycles: this.cycleCount,
      activeTiles: this.reservoirSize,
      powerMw: this.reservoirSize * densitySpec.powerPerPixelMw,
      throughputOpsPerSec: Number(this.cycleCount) / (elapsedMs / 1000),
      memoryBandwidthGbps: 0,
      utilizationPercent: 100,
    };
  }
}

/**
 * Simple dense layer for comparison
 */
export class DenseLayer {
  private weights: Float32Array;
  private biases: Float32Array;
  private activation: ActivationFunction;

  readonly inputSize: number;
  readonly outputSize: number;

  constructor(
    inputSize: number,
    outputSize: number,
    activation: ActivationFunction = 'relu'
  ) {
    this.inputSize = inputSize;
    this.outputSize = outputSize;
    this.activation = activation;

    this.weights = new Float32Array(inputSize * outputSize);
    this.biases = new Float32Array(outputSize);

    this.initializeWeights();
  }

  private initializeWeights(): void {
    const scale = Math.sqrt(2.0 / this.inputSize);
    for (let i = 0; i < this.weights.length; i++) {
      this.weights[i] = (Math.random() - 0.5) * 2 * scale;
    }
    this.biases.fill(0);
  }

  forward(input: Float32Array): Float32Array {
    const output = new Float32Array(this.outputSize);

    for (let i = 0; i < this.outputSize; i++) {
      let sum = this.biases[i];
      for (let j = 0; j < this.inputSize; j++) {
        sum += input[j] * this.weights[i * this.inputSize + j];
      }
      output[i] = this.applyActivation(sum);
    }

    return output;
  }

  private applyActivation(x: number): number {
    switch (this.activation) {
      case 'relu':
        return Math.max(0, x);
      case 'sigmoid':
        return 1 / (1 + Math.exp(-x));
      case 'tanh':
        return Math.tanh(x);
      default:
        return x;
    }
  }

  setWeights(weights: Float32Array, biases?: Float32Array): void {
    this.weights.set(weights.slice(0, this.weights.length));
    if (biases) {
      this.biases.set(biases.slice(0, this.biases.length));
    }
  }
}
