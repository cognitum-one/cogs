/**
 * FIXEL Test Framework
 * A lightweight, self-contained test runner with no external dependencies
 *
 * Features:
 * - describe/it/expect pattern
 * - Async test support
 * - Timing measurements
 * - Colored output
 * - Summary report
 */

// Test result types
interface TestResult {
  name: string;
  passed: boolean;
  duration: number;
  error?: string;
}

interface SuiteResult {
  name: string;
  tests: TestResult[];
  duration: number;
}

// Global state
const suites: SuiteResult[] = [];
let currentSuite: SuiteResult | null = null;
let currentTest: string = '';

// Colors for terminal output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  dim: '\x1b[2m',
  bold: '\x1b[1m',
};

/**
 * High-resolution timer
 */
function now(): number {
  if (typeof performance !== 'undefined') {
    return performance.now();
  }
  const [sec, nsec] = process.hrtime();
  return sec * 1000 + nsec / 1e6;
}

/**
 * Describe a test suite
 */
export function describe(name: string, fn: () => void | Promise<void>): void {
  console.log(`\n${colors.bold}${colors.blue}Suite: ${name}${colors.reset}`);
  console.log(colors.dim + '─'.repeat(50) + colors.reset);

  currentSuite = {
    name,
    tests: [],
    duration: 0,
  };

  const start = now();

  try {
    const result = fn();
    if (result instanceof Promise) {
      // Handle async describe - not common but supported
      result.then(() => {
        currentSuite!.duration = now() - start;
        suites.push(currentSuite!);
        currentSuite = null;
      }).catch((err) => {
        console.error(`${colors.red}Suite error: ${err}${colors.reset}`);
      });
    } else {
      currentSuite.duration = now() - start;
      suites.push(currentSuite);
      currentSuite = null;
    }
  } catch (err) {
    console.error(`${colors.red}Suite error: ${err}${colors.reset}`);
    currentSuite = null;
  }
}

/**
 * Define a test case
 */
export function it(name: string, fn: () => void | Promise<void>): void {
  if (!currentSuite) {
    throw new Error('it() must be called within describe()');
  }

  currentTest = name;
  const start = now();

  try {
    const result = fn();
    if (result instanceof Promise) {
      // Async test - we need to handle this differently
      result.then(() => {
        const duration = now() - start;
        console.log(`  ${colors.green}PASS${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
        currentSuite!.tests.push({ name, passed: true, duration });
      }).catch((err: Error) => {
        const duration = now() - start;
        console.log(`  ${colors.red}FAIL${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
        console.log(`       ${colors.red}${err.message}${colors.reset}`);
        currentSuite!.tests.push({ name, passed: false, duration, error: err.message });
      });
    } else {
      const duration = now() - start;
      console.log(`  ${colors.green}PASS${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
      currentSuite!.tests.push({ name, passed: true, duration });
    }
  } catch (err: any) {
    const duration = now() - start;
    console.log(`  ${colors.red}FAIL${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
    console.log(`       ${colors.red}${err.message}${colors.reset}`);
    currentSuite!.tests.push({ name, passed: false, duration, error: err.message });
  }

  currentTest = '';
}

/**
 * Async test wrapper
 */
export async function itAsync(name: string, fn: () => Promise<void>): Promise<void> {
  if (!currentSuite) {
    throw new Error('itAsync() must be called within describe()');
  }

  currentTest = name;
  const start = now();

  try {
    await fn();
    const duration = now() - start;
    console.log(`  ${colors.green}PASS${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
    currentSuite.tests.push({ name, passed: true, duration });
  } catch (err: any) {
    const duration = now() - start;
    console.log(`  ${colors.red}FAIL${colors.reset} ${name} ${colors.dim}(${duration.toFixed(2)}ms)${colors.reset}`);
    console.log(`       ${colors.red}${err.message}${colors.reset}`);
    currentSuite.tests.push({ name, passed: false, duration, error: err.message });
  }

  currentTest = '';
}

/**
 * Expectation builder
 */
export function expect<T>(actual: T) {
  return {
    toBe(expected: T): void {
      if (actual !== expected) {
        throw new Error(`Expected ${expected}, got ${actual}`);
      }
    },

    toEqual(expected: T): void {
      const actualStr = JSON.stringify(actual);
      const expectedStr = JSON.stringify(expected);
      if (actualStr !== expectedStr) {
        throw new Error(`Expected ${expectedStr}, got ${actualStr}`);
      }
    },

    toBeCloseTo(expected: number, tolerance = 0.001): void {
      const diff = Math.abs((actual as unknown as number) - expected);
      if (diff > tolerance) {
        throw new Error(`Expected ${expected} (+/- ${tolerance}), got ${actual} (diff: ${diff})`);
      }
    },

    toBeGreaterThan(expected: number): void {
      if ((actual as unknown as number) <= expected) {
        throw new Error(`Expected ${actual} to be greater than ${expected}`);
      }
    },

    toBeGreaterThanOrEqual(expected: number): void {
      if ((actual as unknown as number) < expected) {
        throw new Error(`Expected ${actual} to be >= ${expected}`);
      }
    },

    toBeLessThan(expected: number): void {
      if ((actual as unknown as number) >= expected) {
        throw new Error(`Expected ${actual} to be less than ${expected}`);
      }
    },

    toBeLessThanOrEqual(expected: number): void {
      if ((actual as unknown as number) > expected) {
        throw new Error(`Expected ${actual} to be <= ${expected}`);
      }
    },

    toBeTruthy(): void {
      if (!actual) {
        throw new Error(`Expected truthy value, got ${actual}`);
      }
    },

    toBeFalsy(): void {
      if (actual) {
        throw new Error(`Expected falsy value, got ${actual}`);
      }
    },

    toBeNull(): void {
      if (actual !== null) {
        throw new Error(`Expected null, got ${actual}`);
      }
    },

    toBeUndefined(): void {
      if (actual !== undefined) {
        throw new Error(`Expected undefined, got ${actual}`);
      }
    },

    toBeDefined(): void {
      if (actual === undefined) {
        throw new Error(`Expected defined value, got undefined`);
      }
    },

    toBeInstanceOf(expected: any): void {
      if (!(actual instanceof expected)) {
        throw new Error(`Expected instance of ${expected.name}`);
      }
    },

    toContain(expected: any): void {
      if (Array.isArray(actual)) {
        if (!actual.includes(expected)) {
          throw new Error(`Expected array to contain ${expected}`);
        }
      } else if (typeof actual === 'string') {
        if (!actual.includes(expected)) {
          throw new Error(`Expected string to contain "${expected}"`);
        }
      } else {
        throw new Error(`toContain() requires array or string`);
      }
    },

    toHaveLength(expected: number): void {
      const length = (actual as any).length;
      if (length !== expected) {
        throw new Error(`Expected length ${expected}, got ${length}`);
      }
    },

    toThrow(expectedMessage?: string): void {
      if (typeof actual !== 'function') {
        throw new Error('toThrow() requires a function');
      }
      try {
        (actual as Function)();
        throw new Error('Expected function to throw');
      } catch (err: any) {
        if (expectedMessage && !err.message.includes(expectedMessage)) {
          throw new Error(`Expected error message to contain "${expectedMessage}", got "${err.message}"`);
        }
      }
    },

    toBeInRange(min: number, max: number): void {
      const val = actual as unknown as number;
      if (val < min || val > max) {
        throw new Error(`Expected ${val} to be in range [${min}, ${max}]`);
      }
    },

    not: {
      toBe(expected: T): void {
        if (actual === expected) {
          throw new Error(`Expected ${actual} not to be ${expected}`);
        }
      },

      toEqual(expected: T): void {
        const actualStr = JSON.stringify(actual);
        const expectedStr = JSON.stringify(expected);
        if (actualStr === expectedStr) {
          throw new Error(`Expected not to equal ${expectedStr}`);
        }
      },

      toBeNull(): void {
        if (actual === null) {
          throw new Error(`Expected not to be null`);
        }
      },

      toBeUndefined(): void {
        if (actual === undefined) {
          throw new Error(`Expected not to be undefined`);
        }
      },
    },
  };
}

/**
 * Generate and print test summary
 */
export function printSummary(): void {
  console.log('\n' + colors.bold + '═'.repeat(60) + colors.reset);
  console.log(colors.bold + colors.cyan + '                    TEST SUMMARY' + colors.reset);
  console.log(colors.bold + '═'.repeat(60) + colors.reset + '\n');

  let totalTests = 0;
  let totalPassed = 0;
  let totalFailed = 0;
  let totalDuration = 0;

  for (const suite of suites) {
    const passed = suite.tests.filter(t => t.passed).length;
    const failed = suite.tests.filter(t => !t.passed).length;
    const status = failed === 0 ? colors.green + 'PASS' : colors.red + 'FAIL';

    console.log(`${status}${colors.reset} ${suite.name}`);
    console.log(`     ${colors.dim}${passed} passed, ${failed} failed, ${suite.duration.toFixed(2)}ms${colors.reset}`);

    // Show failed tests
    for (const test of suite.tests) {
      if (!test.passed) {
        console.log(`     ${colors.red}- ${test.name}: ${test.error}${colors.reset}`);
      }
    }

    totalTests += suite.tests.length;
    totalPassed += passed;
    totalFailed += failed;
    totalDuration += suite.duration;
  }

  console.log('\n' + colors.bold + '─'.repeat(60) + colors.reset);
  console.log(`\n${colors.bold}Total:${colors.reset} ${totalTests} tests`);
  console.log(`${colors.green}Passed:${colors.reset} ${totalPassed}`);
  console.log(`${colors.red}Failed:${colors.reset} ${totalFailed}`);
  console.log(`${colors.dim}Duration:${colors.reset} ${totalDuration.toFixed(2)}ms`);

  const passRate = totalTests > 0 ? (totalPassed / totalTests * 100).toFixed(1) : '0';
  console.log(`\n${colors.bold}Pass Rate: ${passRate}%${colors.reset}`);

  if (totalFailed === 0) {
    console.log(`\n${colors.green}${colors.bold}All tests passed!${colors.reset}\n`);
  } else {
    console.log(`\n${colors.red}${colors.bold}${totalFailed} test(s) failed.${colors.reset}\n`);
    process.exitCode = 1;
  }
}

/**
 * Benchmark utility
 */
export function benchmark(name: string, fn: () => void, iterations = 1000): { opsPerSecond: number; avgTime: number } {
  // Warmup
  for (let i = 0; i < 10; i++) {
    fn();
  }

  const start = now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const elapsed = now() - start;

  const avgTime = elapsed / iterations;
  const opsPerSecond = 1000 / avgTime;

  console.log(`  ${colors.cyan}BENCH${colors.reset} ${name}`);
  console.log(`        ${colors.dim}${opsPerSecond.toFixed(0)} ops/s, ${avgTime.toFixed(3)}ms avg${colors.reset}`);

  return { opsPerSecond, avgTime };
}

/**
 * Delay utility for async tests
 */
export function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Assert utility for quick checks
 */
export function assert(condition: boolean, message = 'Assertion failed'): void {
  if (!condition) {
    throw new Error(message);
  }
}

// Export for module usage
export default {
  describe,
  it,
  itAsync,
  expect,
  printSummary,
  benchmark,
  delay,
  assert,
};
