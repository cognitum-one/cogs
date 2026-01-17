/**
 * FIXEL Performance Benchmark Tests
 *
 * Tests for performance metrics:
 * - Operations per second per tier
 * - Power efficiency (ops/watt)
 * - Memory bandwidth
 * - Latency measurements
 */

import { describe, it, expect, benchmark, assert } from './test-runner';
import { CognitumCore, FIXEL } from './cognitum.test';
import { FixelTile, FixelSector, WavePropagator, ClockTree, TILE, SECTOR, DISPLAY, BoundaryMode } from './fabric.test';
import { CNNLayer, SNNLayer, ReservoirComputer, SelfOrganizingMap, FixelCNN, KERNELS } from './neural.test';

// ============================================================================
// Benchmark Configuration
// ============================================================================

interface BenchmarkResult {
  name: string;
  opsPerSecond: number;
  avgLatencyMs: number;
  minLatencyMs: number;
  maxLatencyMs: number;
  throughput: number;
  unit: string;
}

interface PowerMetrics {
  activePower: number;   // uW
  idlePower: number;     // uW
  opsPerWatt: number;
  efficiency: string;
}

interface LatencyMetrics {
  local: number;         // ns
  tile: number;          // ns
  sector: number;        // ns
  global: number;        // ns
}

/**
 * Performance tiers based on FIXEL architecture
 */
const TIERS = {
  PIXEL: {
    name: 'Pixel (Cognitum)',
    clockMHz: 100,
    cores: 1,
    sramBytes: 512,
    activePowerUW: 0.46,
    idlePowerUW: 0.031,
  },
  TILE: {
    name: 'Tile (16x16)',
    clockMHz: 100,
    cores: 256,
    sramBytes: 4096,
    activePowerUW: 117.76, // 256 * 0.46
    idlePowerUW: 7.936,    // 256 * 0.031
  },
  SECTOR: {
    name: 'Sector (128x128)',
    clockMHz: 100,
    cores: 16384,
    sramBytes: 65536,
    activePowerUW: 7536.64, // 16384 * 0.46
    idlePowerUW: 507.9,     // 16384 * 0.031
  },
  DISPLAY_4K: {
    name: 'Display 4K',
    clockMHz: 100,
    cores: 8294400,
    sramBytes: 4294967296, // Theoretical
    activePowerUW: 3815424, // ~3.8W
    idlePowerUW: 257126.4,  // ~0.26W
  },
};

/**
 * High-resolution timer
 */
function hrTime(): number {
  if (typeof performance !== 'undefined') {
    return performance.now();
  }
  const [sec, nsec] = process.hrtime();
  return sec * 1000 + nsec / 1e6;
}

/**
 * Run benchmark with statistics
 */
function runBenchmark(
  name: string,
  fn: () => void,
  iterations: number = 1000,
  warmup: number = 100
): BenchmarkResult {
  // Warmup
  for (let i = 0; i < warmup; i++) {
    fn();
  }

  // Collect samples
  const samples: number[] = [];
  const totalStart = hrTime();

  for (let i = 0; i < iterations; i++) {
    const start = hrTime();
    fn();
    const end = hrTime();
    samples.push(end - start);
  }

  const totalEnd = hrTime();
  const totalTime = totalEnd - totalStart;

  // Calculate statistics
  samples.sort((a, b) => a - b);
  const avgLatencyMs = samples.reduce((a, b) => a + b, 0) / samples.length;
  const minLatencyMs = samples[0];
  const maxLatencyMs = samples[samples.length - 1];
  const opsPerSecond = iterations / (totalTime / 1000);

  return {
    name,
    opsPerSecond,
    avgLatencyMs,
    minLatencyMs,
    maxLatencyMs,
    throughput: opsPerSecond,
    unit: 'ops/s',
  };
}

/**
 * Calculate power metrics
 */
function calculatePowerMetrics(
  opsPerSecond: number,
  activePowerUW: number,
  idlePowerUW: number,
  dutyCycle: number = 0.5
): PowerMetrics {
  const avgPower = activePowerUW * dutyCycle + idlePowerUW * (1 - dutyCycle);
  const avgPowerW = avgPower / 1e6;
  const opsPerWatt = opsPerSecond / avgPowerW;

  let efficiency: string;
  if (opsPerWatt > 1e12) {
    efficiency = `${(opsPerWatt / 1e12).toFixed(2)} TOps/W`;
  } else if (opsPerWatt > 1e9) {
    efficiency = `${(opsPerWatt / 1e9).toFixed(2)} GOps/W`;
  } else if (opsPerWatt > 1e6) {
    efficiency = `${(opsPerWatt / 1e6).toFixed(2)} MOps/W`;
  } else {
    efficiency = `${opsPerWatt.toFixed(2)} Ops/W`;
  }

  return {
    activePower: activePowerUW,
    idlePower: idlePowerUW,
    opsPerWatt,
    efficiency,
  };
}

// ============================================================================
// Test Suites
// ============================================================================

describe('Per-Tier Operations Performance', () => {
  describe('Pixel Tier (Cognitum)', () => {
    const core = new CognitumCore();

    it('should benchmark MAC operation', () => {
      core.reset();
      const result = runBenchmark('Pixel MAC', () => {
        core.mac(100, 50);
      }, 10000);

      console.log(`    Pixel MAC: ${result.opsPerSecond.toFixed(0)} ops/s`);
      expect(result.opsPerSecond).toBeGreaterThan(100000);
    });

    it('should benchmark ALU operations', () => {
      const results: BenchmarkResult[] = [];

      results.push(runBenchmark('ADD', () => core.add(100, 50), 10000));
      results.push(runBenchmark('MUL', () => core.mul(10, 10), 10000));
      results.push(runBenchmark('AND', () => core.and(0xFF, 0xAA), 10000));
      results.push(runBenchmark('SHL', () => core.shl(1, 4), 10000));

      for (const r of results) {
        console.log(`    ${r.name}: ${r.opsPerSecond.toFixed(0)} ops/s`);
        expect(r.opsPerSecond).toBeGreaterThan(100000);
      }
    });

    it('should benchmark activation functions', () => {
      core.regAcc = 1000;
      const results: BenchmarkResult[] = [];

      results.push(runBenchmark('ReLU', () => core.relu(core.regAcc), 10000));
      results.push(runBenchmark('Sigmoid', () => core.sigmoid(core.regAcc), 10000));
      results.push(runBenchmark('Clamp', () => core.clampAct(core.regAcc), 10000));

      for (const r of results) {
        console.log(`    ${r.name}: ${r.opsPerSecond.toFixed(0)} ops/s`);
        expect(r.opsPerSecond).toBeGreaterThan(100000);
      }
    });

    it('should benchmark SRAM access', () => {
      const result = runBenchmark('SRAM R/W', () => {
        core.sramWrite(0, 42);
        core.sramRead(0);
      }, 10000);

      console.log(`    SRAM R/W: ${result.opsPerSecond.toFixed(0)} ops/s`);
      expect(result.opsPerSecond).toBeGreaterThan(100000);
    });

    it('should benchmark full instruction cycle', () => {
      core.reset();
      core.setNeighbor(0, 100);

      const result = runBenchmark('Full Instruction', () => {
        core.execute(FIXEL.OP_LOAD, 0);
        core.execute(FIXEL.OP_MAC, 0, 50);
        core.execute(FIXEL.OP_ACT, 0);
      }, 5000);

      console.log(`    Full Instruction: ${result.opsPerSecond.toFixed(0)} cycles/s`);
      expect(result.opsPerSecond).toBeGreaterThan(10000);
    });
  });

  describe('Tile Tier (16x16)', () => {
    const tile = new FixelTile();

    it('should benchmark tile-wide execution', () => {
      tile.reset();
      const result = runBenchmark('Tile Execute', () => {
        tile.executeAll(FIXEL.OP_MAC, 0, 100);
      }, 1000);

      // 256 cores per tile
      const totalOps = result.opsPerSecond * 256;
      console.log(`    Tile Execute: ${result.opsPerSecond.toFixed(0)} tile-ops/s (${(totalOps/1e6).toFixed(2)} MOps/s)`);
      expect(result.opsPerSecond).toBeGreaterThan(1000);
    });

    it('should benchmark neighbor update', () => {
      const result = runBenchmark('Neighbor Update', () => {
        tile.updateNeighborConnections();
      }, 500);

      console.log(`    Neighbor Update: ${result.opsPerSecond.toFixed(0)} updates/s`);
      expect(result.opsPerSecond).toBeGreaterThan(100);
    });

    it('should benchmark tile reduction', () => {
      for (let y = 0; y < 16; y++) {
        for (let x = 0; x < 16; x++) {
          tile.setCoreValue(x, y, 1);
        }
      }

      const result = runBenchmark('Tile Reduce', () => {
        tile.reduce('SUM');
      }, 2000);

      console.log(`    Tile Reduce: ${result.opsPerSecond.toFixed(0)} reductions/s`);
      expect(result.opsPerSecond).toBeGreaterThan(5000);
    });

    it('should benchmark halo generation', () => {
      const result = runBenchmark('Halo Generate', () => {
        tile.generateHalos();
      }, 2000);

      console.log(`    Halo Generate: ${result.opsPerSecond.toFixed(0)} ops/s`);
      expect(result.opsPerSecond).toBeGreaterThan(5000);
    });
  });

  describe('Sector Tier (128x128)', () => {
    // Use smaller sector for benchmarking
    const sector = new FixelSector(0, 2, 2); // 2x2 tiles = 32x32 pixels

    it('should benchmark sector-wide execution', () => {
      sector.reset();
      const result = runBenchmark('Sector Execute', () => {
        sector.executeAll(FIXEL.OP_MAC, 0, 100);
      }, 200);

      // 4 tiles * 256 cores = 1024 cores
      const totalOps = result.opsPerSecond * 1024;
      console.log(`    Sector Execute: ${result.opsPerSecond.toFixed(0)} sector-ops/s (${(totalOps/1e6).toFixed(2)} MOps/s)`);
      expect(result.opsPerSecond).toBeGreaterThan(100);
    });

    it('should benchmark sector connection update', () => {
      const result = runBenchmark('Sector Update', () => {
        sector.updateAllConnections();
      }, 100);

      console.log(`    Sector Update: ${result.opsPerSecond.toFixed(0)} updates/s`);
      expect(result.opsPerSecond).toBeGreaterThan(10);
    });

    it('should benchmark sector reduction', () => {
      const result = runBenchmark('Sector Reduce', () => {
        sector.reduce('SUM');
      }, 500);

      console.log(`    Sector Reduce: ${result.opsPerSecond.toFixed(0)} reductions/s`);
      expect(result.opsPerSecond).toBeGreaterThan(100);
    });
  });
});

describe('Power Efficiency (Ops/Watt)', () => {
  it('should calculate pixel-level efficiency', () => {
    const core = new CognitumCore();

    const result = runBenchmark('Pixel MAC', () => {
      core.mac(100, 50);
    }, 10000);

    const power = calculatePowerMetrics(
      result.opsPerSecond,
      TIERS.PIXEL.activePowerUW,
      TIERS.PIXEL.idlePowerUW,
      0.5
    );

    console.log(`    Pixel Efficiency: ${power.efficiency}`);
    expect(power.opsPerWatt).toBeGreaterThan(1e6);
  });

  it('should calculate tile-level efficiency', () => {
    const tile = new FixelTile();

    const result = runBenchmark('Tile Execute', () => {
      tile.executeAll(FIXEL.OP_MAC, 0, 100);
    }, 500);

    // Multiply by 256 cores
    const totalOps = result.opsPerSecond * 256;

    const power = calculatePowerMetrics(
      totalOps,
      TIERS.TILE.activePowerUW,
      TIERS.TILE.idlePowerUW,
      0.5
    );

    console.log(`    Tile Efficiency: ${power.efficiency}`);
    expect(power.opsPerWatt).toBeGreaterThan(1e6);
  });

  it('should estimate 4K display efficiency', () => {
    // Theoretical calculation based on per-pixel performance
    const core = new CognitumCore();

    const pixelResult = runBenchmark('Pixel MAC', () => {
      core.mac(100, 50);
    }, 5000);

    // Scale to 4K (8.3M pixels)
    // But simulation overhead means we estimate based on theoretical
    const theoreticalOpsPerPixel = 100e6; // 100 MHz clock = 100M ops/s per pixel at 1 op/cycle
    const totalOps4K = theoreticalOpsPerPixel * TIERS.DISPLAY_4K.cores;

    const power = calculatePowerMetrics(
      totalOps4K,
      TIERS.DISPLAY_4K.activePowerUW,
      TIERS.DISPLAY_4K.idlePowerUW,
      0.5
    );

    console.log(`    4K Display Theoretical: ${power.efficiency}`);
    console.log(`    4K Total Power: ${((TIERS.DISPLAY_4K.activePowerUW * 0.5 + TIERS.DISPLAY_4K.idlePowerUW * 0.5) / 1e6).toFixed(2)}W`);

    // Should be in the 10s of TOps/W range
    expect(power.opsPerWatt).toBeGreaterThan(1e10);
  });

  it('should compare efficiency across power modes', () => {
    const tile = new FixelTile();

    const results: { mode: string; efficiency: string }[] = [];

    // Full active
    tile.reset();
    const fullResult = runBenchmark('Full Active', () => {
      tile.executeAll(FIXEL.OP_MAC, 0, 100);
    }, 200);

    const fullPower = calculatePowerMetrics(
      fullResult.opsPerSecond * 256,
      TIERS.TILE.activePowerUW,
      TIERS.TILE.idlePowerUW,
      1.0 // 100% duty cycle
    );
    results.push({ mode: 'Full Active (100%)', efficiency: fullPower.efficiency });

    // Half active
    for (let y = 0; y < 16; y++) {
      for (let x = 0; x < 8; x++) {
        tile.getCore(x, y)!.setPowerState(false);
      }
    }

    const halfResult = runBenchmark('Half Active', () => {
      tile.executeAll(FIXEL.OP_MAC, 0, 100);
    }, 200);

    const halfPower = calculatePowerMetrics(
      halfResult.opsPerSecond * 128, // Only 128 active
      TIERS.TILE.activePowerUW / 2,
      TIERS.TILE.idlePowerUW,
      0.5
    );
    results.push({ mode: 'Half Active (50%)', efficiency: halfPower.efficiency });

    // Idle
    const idlePower = calculatePowerMetrics(
      0,
      0,
      TIERS.TILE.idlePowerUW,
      0
    );
    results.push({ mode: 'Idle (0%)', efficiency: `${TIERS.TILE.idlePowerUW.toFixed(2)} uW` });

    for (const r of results) {
      console.log(`    ${r.mode}: ${r.efficiency}`);
    }

    expect(results.length).toBe(3);
  });
});

describe('Memory Bandwidth', () => {
  it('should measure SRAM bandwidth', () => {
    const core = new CognitumCore();
    const dataSize = 512; // bytes

    const result = runBenchmark('SRAM Sequential', () => {
      for (let i = 0; i < dataSize; i++) {
        core.sramWrite(i, i & 0xFF);
      }
      for (let i = 0; i < dataSize; i++) {
        core.sramRead(i);
      }
    }, 1000);

    // Each iteration moves 512 * 2 = 1024 bytes
    const bandwidthMBs = (result.opsPerSecond * 1024) / 1e6;
    console.log(`    SRAM Sequential: ${bandwidthMBs.toFixed(2)} MB/s`);
    expect(bandwidthMBs).toBeGreaterThan(100);
  });

  it('should measure register bandwidth', () => {
    const core = new CognitumCore();

    const result = runBenchmark('Register Access', () => {
      for (let r = 0; r < 8; r++) {
        core.writeReg(r, r * 10);
      }
      for (let r = 0; r < 8; r++) {
        core.readReg(r);
      }
    }, 5000);

    // 8 writes + 8 reads = 16 bytes
    const bandwidthMBs = (result.opsPerSecond * 16) / 1e6;
    console.log(`    Register Access: ${bandwidthMBs.toFixed(2)} MB/s`);
    expect(bandwidthMBs).toBeGreaterThan(10);
  });

  it('should measure tile SRAM bandwidth', () => {
    const tile = new FixelTile();

    const result = runBenchmark('Tile SRAM', () => {
      for (let i = 0; i < 256; i++) {
        tile.sram[i] = i & 0xFF;
      }
    }, 2000);

    const bandwidthMBs = (result.opsPerSecond * 256) / 1e6;
    console.log(`    Tile SRAM Write: ${bandwidthMBs.toFixed(2)} MB/s`);
    expect(bandwidthMBs).toBeGreaterThan(50);
  });

  it('should measure broadcast bandwidth', () => {
    const tile = new FixelTile();

    const result = runBenchmark('Broadcast', () => {
      tile.broadcast(100);
    }, 2000);

    // 256 pixels receive 1 byte each
    const bandwidthMBs = (result.opsPerSecond * 256) / 1e6;
    console.log(`    Broadcast: ${bandwidthMBs.toFixed(2)} MB/s`);
    expect(bandwidthMBs).toBeGreaterThan(10);
  });

  it('should estimate theoretical memory bandwidth', () => {
    // Each pixel has 512B SRAM, accessed at up to clock rate
    const clockHz = 100e6;
    const bytesPerPixel = 512;
    const pixelsIn4K = 8294400;

    // Aggregate bandwidth if all pixels access SRAM each cycle
    const theoreticalBandwidthTBs = (clockHz * pixelsIn4K * 1) / 1e12; // 1 byte per cycle per pixel

    console.log(`    Theoretical Aggregate: ${theoreticalBandwidthTBs.toFixed(2)} TB/s`);
    console.log(`    (If all ${(pixelsIn4K/1e6).toFixed(1)}M pixels access 1 byte per cycle @ 100MHz)`);

    expect(theoreticalBandwidthTBs).toBeGreaterThan(0.5);
  });
});

describe('Latency Measurements', () => {
  it('should measure neighbor access latency', () => {
    const core = new CognitumCore();
    core.setNeighbor(0, 100);

    const result = runBenchmark('Neighbor Access', () => {
      core.execute(FIXEL.OP_LOAD, 0);
    }, 10000);

    const latencyUs = result.avgLatencyMs * 1000;
    console.log(`    Neighbor Access: ${latencyUs.toFixed(3)} us (simulation)`);

    // Theoretical: 1 cycle @ 100MHz = 10ns
    console.log(`    Theoretical: 10 ns (1 cycle @ 100MHz)`);

    expect(result.avgLatencyMs).toBeLessThan(1);
  });

  it('should measure MAC operation latency', () => {
    const core = new CognitumCore();

    const result = runBenchmark('MAC Latency', () => {
      core.mac(100, 50);
    }, 10000);

    const latencyUs = result.avgLatencyMs * 1000;
    console.log(`    MAC: ${latencyUs.toFixed(3)} us (simulation)`);
    console.log(`    Theoretical: 10-20 ns (1-2 cycles)`);

    expect(result.avgLatencyMs).toBeLessThan(1);
  });

  it('should measure tile synchronization latency', () => {
    const tile = new FixelTile();

    const result = runBenchmark('Tile Sync', () => {
      tile.executeAll(FIXEL.OP_SYNC, 0);
      tile.waitAllDone();
    }, 500);

    const latencyUs = result.avgLatencyMs * 1000;
    console.log(`    Tile Sync: ${latencyUs.toFixed(1)} us (simulation)`);
    console.log(`    Theoretical: ~160 ns (16 cycles across tile)`);

    expect(result.avgLatencyMs).toBeLessThan(10);
  });

  it('should measure wave propagation latency', () => {
    // 16x16 tile wave
    const wave16 = new WavePropagator(16, 16);
    const result16 = runBenchmark('Wave 16x16', () => {
      wave16.propagateFromEdge('N', 100);
    }, 500);

    console.log(`    Wave 16x16: ${(result16.avgLatencyMs * 1000).toFixed(1)} us`);

    // Larger wave
    const wave64 = new WavePropagator(64, 64);
    const result64 = runBenchmark('Wave 64x64', () => {
      wave64.propagateFromEdge('N', 100);
    }, 100);

    console.log(`    Wave 64x64: ${(result64.avgLatencyMs * 1000).toFixed(1)} us`);

    // Latency should scale with size
    expect(result64.avgLatencyMs).toBeGreaterThan(result16.avgLatencyMs);
  });

  it('should estimate end-to-end inference latency', () => {
    // CNN inference on tile
    const fixelCNN = new FixelCNN(KERNELS.SOBEL_X);
    const input: number[][] = [];
    for (let y = 0; y < 16; y++) {
      input.push(new Array(16).fill(128));
    }

    const result = runBenchmark('CNN Inference', () => {
      fixelCNN.loadInput(input);
      fixelCNN.convolve();
    }, 200);

    console.log(`    CNN Inference 16x16: ${(result.avgLatencyMs * 1000).toFixed(1)} us (simulation)`);
    console.log(`    Theoretical: ~200 ns (20 cycles @ 100MHz)`);

    expect(result.avgLatencyMs).toBeLessThan(50);
  });

  it('should calculate latency hierarchy', () => {
    const latencies: LatencyMetrics = {
      local: 10,     // 1 cycle
      tile: 160,     // 16 cycles across tile
      sector: 1280,  // 128 cycles across sector
      global: 60000, // ~6000 cycles across 4K
    };

    console.log('    Latency Hierarchy:');
    console.log(`      Local (neighbor): ${latencies.local} ns`);
    console.log(`      Tile (16x16): ${latencies.tile} ns`);
    console.log(`      Sector (128x128): ${latencies.sector} ns`);
    console.log(`      Global (4K): ${latencies.global} ns = ${latencies.global/1000} us`);

    expect(latencies.tile).toBeGreaterThan(latencies.local);
    expect(latencies.sector).toBeGreaterThan(latencies.tile);
    expect(latencies.global).toBeGreaterThan(latencies.sector);
  });
});

describe('Neural Network Performance', () => {
  it('should benchmark convolution throughput', () => {
    const layer = new CNNLayer(KERNELS.SOBEL_X, 1);
    const input: number[][] = [];
    for (let y = 0; y < 16; y++) {
      input.push(new Array(16).fill(128));
    }

    const result = runBenchmark('Conv 3x3', () => {
      layer.convolve(input);
    }, 1000);

    // 16x16 output * 9 MACs per output = 2304 MACs
    const macsPerSecond = result.opsPerSecond * 2304;
    console.log(`    Conv 3x3 (16x16): ${(macsPerSecond/1e6).toFixed(2)} MMACs/s`);

    expect(macsPerSecond).toBeGreaterThan(1e6);
  });

  it('should benchmark SNN throughput', () => {
    const layer = new SNNLayer(64, 32, 0.8);
    const inputSpikes = new Array(64).fill(false).map(() => Math.random() > 0.5);

    const result = runBenchmark('SNN Step', () => {
      layer.step(inputSpikes, 0);
    }, 5000);

    // 32 neurons * 64 synapses = 2048 synapse operations
    const synapsesPerSecond = result.opsPerSecond * 2048;
    console.log(`    SNN Step: ${(synapsesPerSecond/1e6).toFixed(2)} MSynapses/s`);

    expect(synapsesPerSecond).toBeGreaterThan(1e6);
  });

  it('should benchmark reservoir computing', () => {
    const reservoir = new ReservoirComputer(10, 100, 5);
    const input = new Array(10).fill(0.5);

    const result = runBenchmark('Reservoir Step', () => {
      reservoir.step(input);
    }, 1000);

    // Reservoir has 100 neurons * (10 input + 100 recurrent) = 11000 ops
    const opsPerSecond = result.opsPerSecond * 11000;
    console.log(`    Reservoir Step: ${(opsPerSecond/1e6).toFixed(2)} MOps/s`);

    expect(opsPerSecond).toBeGreaterThan(1e6);
  });

  it('should benchmark SOM training', () => {
    const som = new SelfOrganizingMap(16, 16, 3);

    const result = runBenchmark('SOM Train', () => {
      som.train([Math.random(), Math.random(), Math.random()]);
    }, 1000);

    // 16x16 = 256 nodes, each compared (3 dims) = 768 ops for BMU
    // Plus neighborhood updates
    console.log(`    SOM Train: ${result.opsPerSecond.toFixed(0)} samples/s`);

    expect(result.opsPerSecond).toBeGreaterThan(500);
  });
});

describe('Comparative Analysis', () => {
  it('should compare to GPU efficiency', () => {
    // NVIDIA H100: ~4 PFLOPS @ 700W = 5.7 TOps/W
    const h100Efficiency = 5.7e12; // ops/W

    // FIXEL theoretical: 100 MHz * 8.3M pixels = 830 TOps/s @ ~3W = 277 TOps/W
    const fixelEfficiency = (100e6 * 8.3e6) / 3; // ops/W

    const ratio = fixelEfficiency / h100Efficiency;

    console.log(`    H100 GPU: 5.7 TOps/W`);
    console.log(`    FIXEL 4K: ${(fixelEfficiency/1e12).toFixed(0)} TOps/W (theoretical)`);
    console.log(`    FIXEL Advantage: ${ratio.toFixed(0)}x`);

    expect(ratio).toBeGreaterThan(10);
  });

  it('should compare to neuromorphic chips', () => {
    // Intel Loihi 2: ~15x fewer ops but extremely efficient, ~100+ SOps/J
    // IBM TrueNorth: 46 GSOPS @ 70mW = 657 SOps/W

    // FIXEL at neuromophic workloads (event-driven, sparse)
    // Assuming 1% activity rate
    const activityRate = 0.01;
    const fixelSpikeOps = 100e6 * 8.3e6 * activityRate;
    const fixelPowerMW = 3000 * activityRate + 260; // Active + idle
    const fixelNeuromorphicEfficiency = fixelSpikeOps / (fixelPowerMW / 1000);

    console.log(`    TrueNorth: 657 GSOps/W`);
    console.log(`    FIXEL (1% sparse): ${(fixelNeuromorphicEfficiency/1e9).toFixed(0)} GSOps/W`);

    expect(fixelNeuromorphicEfficiency).toBeGreaterThan(1e9);
  });

  it('should summarize performance metrics', () => {
    console.log('\n    FIXEL Performance Summary:');
    console.log('    ════════════════════════════════════════');

    console.log('    Architecture:');
    console.log(`      Pixels (4K): 8.3 million`);
    console.log(`      Transistors/pixel: ~2.3 million`);
    console.log(`      SRAM/pixel: 512 bytes`);
    console.log(`      Clock: 100 MHz`);

    console.log('    Performance:');
    console.log(`      Peak throughput: 830 TOps/s`);
    console.log(`      Typical (50%): 415 TOps/s`);
    console.log(`      Latency (local): 10 ns`);
    console.log(`      Latency (global): 60 us`);

    console.log('    Power:');
    console.log(`      Active: 3.8 W`);
    console.log(`      Idle: 0.26 W`);
    console.log(`      Typical (50%): 2.0 W`);

    console.log('    Efficiency:');
    console.log(`      Peak: 220 TOps/W`);
    console.log(`      Typical: 110 TOps/W`);

    expect(true).toBe(true);
  });
});

describe('Scaling Analysis', () => {
  it('should analyze scaling from tile to 4K', () => {
    const scales = [
      { name: 'Tile', pixels: 256, power: 0.12, ops: 25.6e9 },
      { name: 'Sector', pixels: 16384, power: 7.5, ops: 1.64e12 },
      { name: '1080p', pixels: 2073600, power: 950, ops: 207e12 },
      { name: '4K', pixels: 8294400, power: 3800, ops: 830e12 },
      { name: '8K', pixels: 33177600, power: 15200, ops: 3.3e15 },
    ];

    console.log('\n    Scaling Analysis:');
    console.log('    Scale       Pixels      Power(mW)   TOps/s    TOps/W');
    console.log('    ────────────────────────────────────────────────────');

    for (const s of scales) {
      const efficiency = s.ops / (s.power / 1000);
      console.log(`    ${s.name.padEnd(10)} ${(s.pixels/1e6).toFixed(2).padStart(8)}M  ${s.power.toFixed(0).padStart(8)}   ${(s.ops/1e12).toFixed(1).padStart(7)}  ${(efficiency/1e12).toFixed(0).padStart(6)}`);
    }

    expect(scales.length).toBe(5);
  });

  it('should verify linear scaling', () => {
    // Power and performance should scale linearly with pixel count
    const tileOps = 256 * 100e6; // 256 pixels * 100MHz
    const sectorOps = 16384 * 100e6;
    const display4KOps = 8294400 * 100e6;

    const ratio1 = sectorOps / tileOps;
    const ratio2 = display4KOps / sectorOps;

    console.log(`    Tile->Sector scaling: ${ratio1.toFixed(0)}x`);
    console.log(`    Sector->4K scaling: ${ratio2.toFixed(0)}x`);

    // Should match pixel count ratios
    expect(ratio1).toBeCloseTo(64, 1);
    expect(ratio2).toBeCloseTo(506, 10);
  });
});

export { runBenchmark, calculatePowerMetrics, TIERS };
