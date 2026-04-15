//! # Benchmark Module
//!
//! This module provides benchmarks for validating the Reality Stack:
//!
//! - **Physics Closure**: Verify conservation laws hold over long simulations
//! - **Agency**: Benchmark agent decision-making and action execution
//! - **Emergence**: Detect and measure emergent behaviors
//!
//! ## Benchmark Categories
//!
//! 1. **Conservation Benchmarks**: Energy, momentum, angular momentum drift
//! 2. **Perception Benchmarks**: Observation generation, bandwidth utilization
//! 3. **Memory Benchmarks**: SONA adaptation speed, EWC protection effectiveness
//! 4. **Governance Benchmarks**: Action validation throughput, audit overhead
//! 5. **Emergence Benchmarks**: Pattern detection, complexity metrics

pub mod physics_closure;
pub mod agency;
pub mod emergence;

pub use physics_closure::{ConservationBenchmark, ConservationResult};
pub use agency::{AgencyBenchmark, AgencyResult};
pub use emergence::{EmergenceBenchmark, EmergenceMetrics};

use std::time::{Duration, Instant};

/// Result of a benchmark run
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Whether benchmark passed
    pub passed: bool,
    /// Execution time
    pub duration: Duration,
    /// Metric values
    pub metrics: Vec<(String, f64)>,
    /// Detailed message
    pub message: String,
}

impl BenchmarkResult {
    /// Create a new benchmark result
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            duration: Duration::ZERO,
            metrics: Vec::new(),
            message: String::new(),
        }
    }

    /// Mark as passed
    pub fn pass(mut self, message: &str) -> Self {
        self.passed = true;
        self.message = message.to_string();
        self
    }

    /// Mark as failed
    pub fn fail(mut self, message: &str) -> Self {
        self.passed = false;
        self.message = message.to_string();
        self
    }

    /// Add a metric
    pub fn with_metric(mut self, name: &str, value: f64) -> Self {
        self.metrics.push((name.to_string(), value));
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

/// Benchmark suite for running multiple benchmarks
pub struct BenchmarkSuite {
    /// Suite name
    name: String,
    /// Benchmarks to run
    benchmarks: Vec<Box<dyn Benchmark>>,
    /// Results
    results: Vec<BenchmarkResult>,
}

impl BenchmarkSuite {
    /// Create a new benchmark suite
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            benchmarks: Vec::new(),
            results: Vec::new(),
        }
    }

    /// Add a benchmark
    pub fn add(&mut self, benchmark: Box<dyn Benchmark>) {
        self.benchmarks.push(benchmark);
    }

    /// Run all benchmarks
    pub fn run(&mut self) -> &[BenchmarkResult] {
        self.results.clear();

        for benchmark in &self.benchmarks {
            let start = Instant::now();
            let mut result = benchmark.run();
            result.duration = start.elapsed();
            self.results.push(result);
        }

        &self.results
    }

    /// Get summary
    pub fn summary(&self) -> BenchmarkSummary {
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let total_time: Duration = self.results.iter().map(|r| r.duration).sum();

        BenchmarkSummary {
            suite_name: self.name.clone(),
            total,
            passed,
            failed,
            total_time,
        }
    }
}

/// Summary of benchmark suite run
#[derive(Debug, Clone)]
pub struct BenchmarkSummary {
    /// Suite name
    pub suite_name: String,
    /// Total benchmarks
    pub total: usize,
    /// Passed benchmarks
    pub passed: usize,
    /// Failed benchmarks
    pub failed: usize,
    /// Total execution time
    pub total_time: Duration,
}

impl std::fmt::Display for BenchmarkSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}/{} passed ({} failed) in {:?}",
            self.suite_name,
            self.passed,
            self.total,
            self.failed,
            self.total_time
        )
    }
}

/// Trait for individual benchmarks
pub trait Benchmark: Send + Sync {
    /// Get benchmark name
    fn name(&self) -> &str;

    /// Run the benchmark
    fn run(&self) -> BenchmarkResult;

    /// Get description
    fn description(&self) -> &str {
        ""
    }
}

/// Configuration for benchmark runs
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations
    pub iterations: usize,
    /// Warmup iterations
    pub warmup: usize,
    /// Timeout per benchmark
    pub timeout: Duration,
    /// Verbose output
    pub verbose: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup: 10,
            timeout: Duration::from_secs(60),
            verbose: false,
        }
    }
}

/// Run the standard Reality Stack benchmark suite
pub fn run_standard_suite() -> BenchmarkSummary {
    let mut suite = BenchmarkSuite::new("Reality Stack Standard");

    // Add conservation benchmarks
    suite.add(Box::new(physics_closure::EnergyConservationBenchmark::new(1000)));
    suite.add(Box::new(physics_closure::MomentumConservationBenchmark::new(1000)));

    // Add agency benchmarks
    suite.add(Box::new(agency::PolicyThroughputBenchmark::new(1000)));
    suite.add(Box::new(agency::SensorBandwidthBenchmark::new(1000)));

    // Add emergence benchmarks
    suite.add(Box::new(emergence::ComplexityBenchmark::new(1000)));

    suite.run();
    suite.summary()
}
