/**
 * FIXEL Test Suite Entry Point
 *
 * Runs all FIXEL tests and generates a summary report.
 *
 * Usage:
 *   npx ts-node tests/index.ts
 *   npx tsx tests/index.ts
 *
 * Or compile first:
 *   npx tsc tests/*.ts --outDir dist --esModuleInterop
 *   node dist/index.js
 */

import { printSummary } from './test-runner';

// Import test modules to register their tests
import './cognitum.test';
import './fabric.test';
import './neural.test';
import './benchmark.test';

// Print header
console.log(`
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║   ███████╗██╗██╗  ██╗███████╗██╗                             ║
║   ██╔════╝██║╚██╗██╔╝██╔════╝██║                             ║
║   █████╗  ██║ ╚███╔╝ █████╗  ██║                             ║
║   ██╔══╝  ██║ ██╔██╗ ██╔══╝  ██║                             ║
║   ██║     ██║██╔╝ ██╗███████╗███████╗                        ║
║   ╚═╝     ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝                        ║
║                                                              ║
║   Finn Pixel Cognitive Fabric - Test Suite                   ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
`);

console.log('Running FIXEL comprehensive test suite...\n');
console.log('Test Configuration:');
console.log('  - Cognitum Unit Tests');
console.log('  - Fabric Integration Tests');
console.log('  - Neural Network Tests');
console.log('  - Performance Benchmarks');
console.log('');

// Give time for async tests to complete
setTimeout(() => {
  printSummary();
}, 100);
