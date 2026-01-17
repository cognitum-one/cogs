//! Browser-based benchmarking utilities.
//!
//! This module provides performance benchmarking capabilities that run
//! in the browser, using `performance.now()` for high-resolution timing.

use wasm_bindgen::prelude::*;
use js_sys::Function;
use serde::{Serialize, Deserialize};

use crate::wasm::simulation::WasmSimulation;

/// Result of a benchmark run.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct BenchmarkResult {
    /// Benchmark name
    name: String,
    /// Number of atoms in the simulation
    n_atoms: usize,
    /// Number of steps run
    n_steps: usize,
    /// Total time in milliseconds
    total_time_ms: f64,
    /// Time per step in milliseconds
    time_per_step_ms: f64,
    /// Steps per second
    steps_per_second: f64,
    /// Atom-steps per second (n_atoms * steps_per_second)
    atom_steps_per_second: f64,
}

#[wasm_bindgen]
impl BenchmarkResult {
    /// Get the benchmark name.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get the number of atoms.
    #[wasm_bindgen(getter)]
    pub fn n_atoms(&self) -> usize {
        self.n_atoms
    }

    /// Get the number of steps.
    #[wasm_bindgen(getter)]
    pub fn n_steps(&self) -> usize {
        self.n_steps
    }

    /// Get total time in milliseconds.
    #[wasm_bindgen(getter)]
    pub fn total_time_ms(&self) -> f64 {
        self.total_time_ms
    }

    /// Get time per step in milliseconds.
    #[wasm_bindgen(getter)]
    pub fn time_per_step_ms(&self) -> f64 {
        self.time_per_step_ms
    }

    /// Get steps per second.
    #[wasm_bindgen(getter)]
    pub fn steps_per_second(&self) -> f64 {
        self.steps_per_second
    }

    /// Get atom-steps per second.
    #[wasm_bindgen(getter)]
    pub fn atom_steps_per_second(&self) -> f64 {
        self.atom_steps_per_second
    }

    /// Convert to JSON string.
    #[wasm_bindgen]
    pub fn to_json(&self) -> Result<String, JsValue> {
        let data = BenchmarkResultData {
            name: self.name.clone(),
            n_atoms: self.n_atoms,
            n_steps: self.n_steps,
            total_time_ms: self.total_time_ms,
            time_per_step_ms: self.time_per_step_ms,
            steps_per_second: self.steps_per_second,
            atom_steps_per_second: self.atom_steps_per_second,
        };

        serde_json::to_string(&data)
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }
}

/// Serializable benchmark result.
#[derive(Serialize, Deserialize)]
struct BenchmarkResultData {
    name: String,
    n_atoms: usize,
    n_steps: usize,
    total_time_ms: f64,
    time_per_step_ms: f64,
    steps_per_second: f64,
    atom_steps_per_second: f64,
}

/// Browser benchmark runner for molecular dynamics simulations.
///
/// Runs various benchmarks to measure simulation performance in the
/// browser environment.
#[wasm_bindgen]
pub struct WasmBenchmark {
    /// Results from completed benchmarks
    results: Vec<BenchmarkResult>,
}

#[wasm_bindgen]
impl WasmBenchmark {
    /// Create a new benchmark runner.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmBenchmark {
        WasmBenchmark {
            results: Vec::new(),
        }
    }

    /// Clear all stored results.
    #[wasm_bindgen]
    pub fn clear_results(&mut self) {
        self.results.clear();
    }

    /// Get the number of stored results.
    #[wasm_bindgen]
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Get a specific result by index.
    #[wasm_bindgen]
    pub fn get_result(&self, index: usize) -> Option<BenchmarkResult> {
        self.results.get(index).cloned()
    }

    /// Run a simple benchmark with configurable parameters.
    ///
    /// # Arguments
    ///
    /// * `name` - Benchmark name for identification
    /// * `n_atoms` - Number of atoms (will create FCC lattice)
    /// * `n_steps` - Number of simulation steps to run
    /// * `warmup_steps` - Steps to run before timing (cache warmup)
    ///
    /// # Returns
    ///
    /// BenchmarkResult with timing information.
    #[wasm_bindgen]
    pub fn run_benchmark(
        &mut self,
        name: &str,
        n_atoms_target: usize,
        n_steps: usize,
        warmup_steps: usize,
    ) -> BenchmarkResult {
        // Calculate FCC lattice size to get approximately n_atoms_target atoms
        // FCC has 4 atoms per unit cell
        let n_cells = ((n_atoms_target as f64 / 4.0).cbrt().ceil() as usize).max(1);
        let actual_n_atoms = 4 * n_cells * n_cells * n_cells;

        // Create simulation
        let mut sim = WasmSimulation::new_fcc(n_cells, n_cells, n_cells, 1.5, 1.0);

        // Warmup
        sim.run(warmup_steps);

        // Timed run
        let start = get_performance_now();
        sim.run(n_steps);
        let end = get_performance_now();

        let total_time_ms = end - start;
        let time_per_step_ms = total_time_ms / n_steps as f64;
        let steps_per_second = 1000.0 / time_per_step_ms;
        let atom_steps_per_second = steps_per_second * actual_n_atoms as f64;

        let result = BenchmarkResult {
            name: name.to_string(),
            n_atoms: actual_n_atoms,
            n_steps,
            total_time_ms,
            time_per_step_ms,
            steps_per_second,
            atom_steps_per_second,
        };

        self.results.push(result.clone());
        result
    }

    /// Run a scaling benchmark to test performance across different system sizes.
    ///
    /// # Arguments
    ///
    /// * `n_steps` - Steps to run for each size
    /// * `progress_callback` - Optional callback called with (current_index, total) progress
    ///
    /// # Returns
    ///
    /// Array of BenchmarkResult for sizes: 32, 108, 256, 500, 864, 1372, 2048, 4000
    #[wasm_bindgen]
    pub fn run_scaling_benchmark(
        &mut self,
        n_steps: usize,
        progress_callback: Option<Function>,
    ) -> js_sys::Array {
        // Standard benchmark sizes (FCC lattice sizes)
        // n_cells: 2, 3, 4, 5, 6, 7, 8, 10 -> atoms: 32, 108, 256, 500, 864, 1372, 2048, 4000
        let sizes = [2, 3, 4, 5, 6, 7, 8, 10];
        let results = js_sys::Array::new();

        for (i, &n_cells) in sizes.iter().enumerate() {
            let n_atoms = 4 * n_cells * n_cells * n_cells;
            let name = format!("scaling_{}atoms", n_atoms);

            let result = self.run_benchmark(&name, n_atoms, n_steps, 100);
            results.push(&JsValue::from(result));

            // Report progress
            if let Some(ref callback) = progress_callback {
                let _ = callback.call2(
                    &JsValue::NULL,
                    &JsValue::from_f64((i + 1) as f64),
                    &JsValue::from_f64(sizes.len() as f64),
                );
            }
        }

        results
    }

    /// Run a timing consistency benchmark.
    ///
    /// Runs the same benchmark multiple times to measure variance.
    ///
    /// # Arguments
    ///
    /// * `n_atoms` - Target number of atoms
    /// * `n_steps` - Steps per iteration
    /// * `iterations` - Number of iterations to run
    ///
    /// # Returns
    ///
    /// Array of BenchmarkResult for each iteration.
    #[wasm_bindgen]
    pub fn run_consistency_benchmark(
        &mut self,
        n_atoms: usize,
        n_steps: usize,
        iterations: usize,
    ) -> js_sys::Array {
        let results = js_sys::Array::new();

        for i in 0..iterations {
            let name = format!("consistency_iter{}", i);
            let result = self.run_benchmark(&name, n_atoms, n_steps, 50);
            results.push(&JsValue::from(result));
        }

        results
    }

    /// Run a quick benchmark suitable for UI feedback.
    ///
    /// Uses small system and few steps for fast results.
    ///
    /// # Returns
    ///
    /// BenchmarkResult from quick test.
    #[wasm_bindgen]
    pub fn run_quick_benchmark(&mut self) -> BenchmarkResult {
        self.run_benchmark("quick", 256, 1000, 100)
    }

    /// Run a comprehensive benchmark suite.
    ///
    /// Includes scaling, consistency, and stress tests.
    ///
    /// # Arguments
    ///
    /// * `progress_callback` - Optional callback with (stage_name, progress) updates
    ///
    /// # Returns
    ///
    /// JSON string with all benchmark results.
    #[wasm_bindgen]
    pub fn run_full_suite(&mut self, progress_callback: Option<Function>) -> Result<String, JsValue> {
        self.clear_results();

        // Report progress helper
        let report = |stage: &str, pct: f64| {
            if let Some(ref callback) = progress_callback {
                let _ = callback.call2(
                    &JsValue::NULL,
                    &JsValue::from_str(stage),
                    &JsValue::from_f64(pct),
                );
            }
        };

        // Quick warmup
        report("warmup", 0.0);
        let _ = self.run_quick_benchmark();
        self.results.pop(); // Don't include warmup in results

        // Scaling benchmarks
        report("scaling", 0.1);
        let sizes = [32, 108, 256, 500, 864];
        for (i, &n_atoms) in sizes.iter().enumerate() {
            let name = format!("scale_{}", n_atoms);
            let _ = self.run_benchmark(&name, n_atoms, 2000, 100);
            report("scaling", 0.1 + 0.4 * (i + 1) as f64 / sizes.len() as f64);
        }

        // Consistency benchmarks
        report("consistency", 0.5);
        for i in 0..5 {
            let name = format!("consistency_{}", i);
            let _ = self.run_benchmark(&name, 256, 1000, 50);
            report("consistency", 0.5 + 0.3 * (i + 1) as f64 / 5.0);
        }

        // Stress test
        report("stress", 0.8);
        let _ = self.run_benchmark("stress", 2048, 500, 50);
        report("complete", 1.0);

        // Export all results
        self.export_results_json()
    }

    /// Export all results as JSON.
    #[wasm_bindgen]
    pub fn export_results_json(&self) -> Result<String, JsValue> {
        let data: Vec<BenchmarkResultData> = self.results
            .iter()
            .map(|r| BenchmarkResultData {
                name: r.name.clone(),
                n_atoms: r.n_atoms,
                n_steps: r.n_steps,
                total_time_ms: r.total_time_ms,
                time_per_step_ms: r.time_per_step_ms,
                steps_per_second: r.steps_per_second,
                atom_steps_per_second: r.atom_steps_per_second,
            })
            .collect();

        serde_json::to_string(&data)
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }

    /// Calculate summary statistics from stored results.
    ///
    /// # Returns
    ///
    /// JSON string with min, max, mean, and median performance.
    #[wasm_bindgen]
    pub fn get_summary(&self) -> Result<String, JsValue> {
        if self.results.is_empty() {
            return Err(JsValue::from_str("No benchmark results available"));
        }

        let mut atom_steps: Vec<f64> = self.results
            .iter()
            .map(|r| r.atom_steps_per_second)
            .collect();

        atom_steps.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = atom_steps.first().copied().unwrap_or(0.0);
        let max = atom_steps.last().copied().unwrap_or(0.0);
        let mean = atom_steps.iter().sum::<f64>() / atom_steps.len() as f64;
        let median = if atom_steps.len() % 2 == 0 {
            (atom_steps[atom_steps.len() / 2 - 1] + atom_steps[atom_steps.len() / 2]) / 2.0
        } else {
            atom_steps[atom_steps.len() / 2]
        };

        let summary = BenchmarkSummary {
            n_benchmarks: self.results.len(),
            min_atom_steps_per_second: min,
            max_atom_steps_per_second: max,
            mean_atom_steps_per_second: mean,
            median_atom_steps_per_second: median,
        };

        serde_json::to_string(&summary)
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }
}

impl Default for WasmBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for benchmarks.
#[derive(Serialize, Deserialize)]
struct BenchmarkSummary {
    n_benchmarks: usize,
    min_atom_steps_per_second: f64,
    max_atom_steps_per_second: f64,
    mean_atom_steps_per_second: f64,
    median_atom_steps_per_second: f64,
}

/// Get high-resolution timestamp for benchmarking.
fn get_performance_now() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Some(performance) = window.performance() {
                return performance.now();
            }
        }
    }
    0.0
}

/// Standalone function to run a quick performance check.
///
/// Useful for sanity-checking WASM performance without creating
/// a full benchmark runner.
///
/// # Returns
///
/// Approximate atom-steps per second.
#[wasm_bindgen]
pub fn quick_performance_check() -> f64 {
    let mut sim = WasmSimulation::new_fcc(4, 4, 4, 1.5, 1.0);

    // Warmup
    sim.run(100);

    // Timed run
    let start = get_performance_now();
    sim.run(1000);
    let end = get_performance_now();

    let total_time_ms = end - start;
    let steps_per_second = 1000.0 * 1000.0 / total_time_ms;
    let n_atoms = sim.get_n_atoms();

    steps_per_second * n_atoms as f64
}

/// Compare WASM performance to expected baseline.
///
/// # Returns
///
/// Object with performance metrics and comparison to baseline.
#[wasm_bindgen]
pub fn performance_comparison() -> Result<JsValue, JsValue> {
    let atom_steps = quick_performance_check();

    // Baseline expectations (conservative estimates for modern hardware)
    // Native Rust: ~500M atom-steps/second for small systems
    // WASM typically 50-80% of native
    let expected_wasm_low = 50_000_000.0;   // 50M atom-steps/sec (conservative)
    let expected_wasm_high = 200_000_000.0; // 200M atom-steps/sec (optimistic)

    let rating = if atom_steps >= expected_wasm_high {
        "excellent"
    } else if atom_steps >= expected_wasm_low {
        "good"
    } else if atom_steps >= expected_wasm_low / 2.0 {
        "acceptable"
    } else {
        "slow"
    };

    let comparison = PerformanceComparison {
        atom_steps_per_second: atom_steps,
        expected_low: expected_wasm_low,
        expected_high: expected_wasm_high,
        rating: rating.to_string(),
        percentage_of_expected: (atom_steps / expected_wasm_low) * 100.0,
    };

    serde_wasm_bindgen::to_value(&comparison)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

#[derive(Serialize)]
struct PerformanceComparison {
    atom_steps_per_second: f64,
    expected_low: f64,
    expected_high: f64,
    rating: String,
    percentage_of_expected: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_runner_creation() {
        let bench = WasmBenchmark::new();
        assert_eq!(bench.result_count(), 0);
    }
}
