/**
 * FIXEL Simulations Index
 * Export all simulation modules for easy import
 */

// Core types and interfaces
export * from './types.js';

// Fabric and Neural processing classes
export { Fabric } from './fabric.js';
export {
  SpikingLayer,
  SpikingNetwork,
  ReservoirComputer,
  DenseLayer,
} from './neural.js';

// Simulations
export {
  runEdgeDetection,
  detectEdges,
  printEdgeMap,
  main as runEdgeDetectionDemo,
  type EdgeDetectionConfig,
  type EdgeDetectionResult,
} from './edge-detection.js';

export {
  runGameOfLife,
  runWithPattern,
  printGrid,
  PATTERNS,
  main as runGameOfLifeDemo,
  type GameOfLifeConfig,
  type GameOfLifeResult,
} from './game-of-life.js';

export {
  runSpikingMnist,
  analyzeSpikes,
  main as runSpikingMnistDemo,
  type SpikingMnistConfig,
  type SpikingMnistResult,
} from './spiking-mnist.js';

export {
  runReservoirPrediction,
  runFabricReservoir,
  TIME_SERIES,
  main as runReservoirDemo,
  type ReservoirConfig,
  type ReservoirResult,
} from './reservoir-timeseries.js';

export {
  runFluidSimulation,
  computePoiseuilleReference,
  compareToReference,
  main as runFluidDemo,
  type FluidConfig,
  type FluidResult,
} from './fluid-simulation.js';

/**
 * Run all simulation demos
 */
export async function runAllDemos(): Promise<void> {
  const { main: edgeDemo } = await import('./edge-detection.js');
  const { main: lifeDemo } = await import('./game-of-life.js');
  const { main: mnistDemo } = await import('./spiking-mnist.js');
  const { main: reservoirDemo } = await import('./reservoir-timeseries.js');
  const { main: fluidDemo } = await import('./fluid-simulation.js');

  console.log('\n' + '='.repeat(60));
  console.log('Running All FIXEL Simulations');
  console.log('='.repeat(60) + '\n');

  await edgeDemo();
  console.log('\n' + '-'.repeat(60) + '\n');

  await lifeDemo();
  console.log('\n' + '-'.repeat(60) + '\n');

  await mnistDemo();
  console.log('\n' + '-'.repeat(60) + '\n');

  await reservoirDemo();
  console.log('\n' + '-'.repeat(60) + '\n');

  await fluidDemo();

  console.log('\n' + '='.repeat(60));
  console.log('All simulations complete');
  console.log('='.repeat(60) + '\n');
}

/**
 * Quick benchmark across all simulations
 */
export async function runBenchmark(
  densityTier: import('./types.js').DensityTier = 'medium'
): Promise<{
  edgeDetection: { cycles: bigint; powerMw: number; wallTimeMs: number };
  gameOfLife: { generations: number; gensPerSec: number; powerMw: number };
  spikingMnist: { accuracy: number; powerMw: number; energyPerInference: number };
  reservoir: { rmse: number; powerMw: number };
  fluidSim: { timesteps: number; throughput: number; powerMw: number };
}> {
  const { runEdgeDetection } = await import('./edge-detection.js');
  const { runGameOfLife } = await import('./game-of-life.js');
  const { runSpikingMnist } = await import('./spiking-mnist.js');
  const { runReservoirPrediction } = await import('./reservoir-timeseries.js');
  const { runFluidSimulation } = await import('./fluid-simulation.js');

  console.log(`\nBenchmarking all simulations at ${densityTier.toUpperCase()} tier...\n`);

  const edgeResult = await runEdgeDetection({
    width: 128,
    height: 128,
    densityTier,
  });

  const lifeResult = await runGameOfLife({
    width: 128,
    height: 128,
    densityTier,
  }, 1000);

  const mnistResult = await runSpikingMnist({
    densityTier,
    hiddenLayers: [256, 128],
    timesteps: 50,
  }, 100);

  const reservoirResult = await runReservoirPrediction({
    reservoirSize: 500,
    densityTier,
  }, 'mackeyGlass', 2000, 50);

  const fluidResult = await runFluidSimulation({
    width: 100,
    height: 40,
    densityTier,
    obstacleType: 'cylinder',
  }, 500);

  return {
    edgeDetection: {
      cycles: edgeResult.cycles,
      powerMw: edgeResult.powerMw,
      wallTimeMs: edgeResult.wallTimeMs,
    },
    gameOfLife: {
      generations: lifeResult.generations,
      gensPerSec: lifeResult.generationsPerSecond,
      powerMw: lifeResult.powerMw,
    },
    spikingMnist: {
      accuracy: mnistResult.accuracy!,
      powerMw: mnistResult.powerMw,
      energyPerInference: mnistResult.energyPerInference,
    },
    reservoir: {
      rmse: reservoirResult.rmse,
      powerMw: reservoirResult.powerMw,
    },
    fluidSim: {
      timesteps: fluidResult.timesteps,
      throughput: fluidResult.metrics.throughputOpsPerSec,
      powerMw: fluidResult.powerMw,
    },
  };
}

// CLI entry point
if (typeof require !== 'undefined' && require.main === module) {
  const args = process.argv.slice(2);

  if (args.includes('--benchmark') || args.includes('-b')) {
    const tier = (args.find(a => a.startsWith('--tier='))?.split('=')[1] || 'medium') as import('./types.js').DensityTier;
    runBenchmark(tier)
      .then(results => {
        console.log('\nBenchmark Results:');
        console.log(JSON.stringify(results, (k, v) =>
          typeof v === 'bigint' ? v.toString() : v, 2));
      })
      .catch(console.error);
  } else if (args.includes('--help') || args.includes('-h')) {
    console.log(`
FIXEL Simulation Suite

Usage:
  npx ts-node index.ts [options]

Options:
  --help, -h          Show this help message
  --benchmark, -b     Run quick benchmark of all simulations
  --tier=<tier>       Set density tier (low, medium, high, ultra)
  (no args)           Run all simulation demos

Examples:
  npx ts-node index.ts
  npx ts-node index.ts --benchmark --tier=high
`);
  } else {
    runAllDemos().catch(console.error);
  }
}
