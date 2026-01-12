//! FXNN Flagship Benchmarks - Reality Substrate Verification
//!
//! This module implements the THREE MANDATORY FLAGSHIP BENCHMARKS from ADR-001
//! that prove FXNN is a genuine simulated reality substrate, not just a physics engine.
//!
//! # Overview
//!
//! | Benchmark | Proves | Runtime | Key Metric |
//! |-----------|--------|---------|------------|
//! | **A: Physics Closure** | System cannot remain invalid | <30s | Time-to-recovery, max drift |
//! | **B: Partial Observation** | Agents learn under uncertainty | <60s | Learning curve slope |
//! | **C: Emergence Falsifiability** | Behaviors are genuinely emergent | <60s | Cooperation collapse ratio |
//!
//! # Usage
//!
//! ```rust,no_run
//! use fxnn::benchmark::{run_all_benchmarks, BenchmarkConfig};
//!
//! let config = BenchmarkConfig::default();
//! let results = run_all_benchmarks(&config);
//!
//! for result in &results {
//!     println!("{}: {}", result.name, if result.passed { "PASS" } else { "FAIL" });
//! }
//! ```
//!
//! # ADR-001 Compliance
//!
//! These benchmarks convert philosophy into engineering. Each benchmark maps to:
//! - **Invariant**: The property being tested
//! - **Metric**: How the property is measured
//! - **Failing Test**: What constitutes failure
//!
//! If any benchmark fails, the closure claim is false.

pub mod physics_closure;
pub mod partial_observation;
pub mod emergence;
pub mod determinism;

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for running benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Seed for random number generation (determinism)
    pub seed: u64,
    /// Maximum allowed runtime per benchmark
    pub max_runtime: Duration,
    /// Verbose logging
    pub verbose: bool,
    /// Physics closure specific config
    pub physics: PhysicsClosureConfig,
    /// Partial observation specific config
    pub observation: PartialObservationConfig,
    /// Emergence specific config
    pub emergence: EmergenceConfig,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            max_runtime: Duration::from_secs(60),
            verbose: false,
            physics: PhysicsClosureConfig::default(),
            observation: PartialObservationConfig::default(),
            emergence: EmergenceConfig::default(),
        }
    }
}

/// Configuration for Physics Closure benchmark (Benchmark A)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsClosureConfig {
    /// Number of particles at equilibrium
    pub n_particles: usize,
    /// Number of adversarially overlapping pairs to inject
    pub n_overlapping_pairs: usize,
    /// Velocity multiplier for high-energy particle (100x thermal)
    pub high_energy_multiplier: f32,
    /// Total simulation ticks
    pub total_ticks: usize,
    /// Maximum ticks allowed to resolve overlaps
    pub max_overlap_resolution_ticks: usize,
    /// Maximum energy drift allowed (as fraction)
    pub max_energy_drift: f64,
    /// Sigma parameter for LJ potential
    pub sigma: f32,
}

impl Default for PhysicsClosureConfig {
    fn default() -> Self {
        Self {
            n_particles: 100,
            n_overlapping_pairs: 10,
            high_energy_multiplier: 100.0,
            total_ticks: 1000,
            max_overlap_resolution_ticks: 50,
            max_energy_drift: 0.01, // 1%
            sigma: 1.0,
        }
    }
}

/// Configuration for Partial Observation benchmark (Benchmark B)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialObservationConfig {
    /// Maze width in cells
    pub maze_width: usize,
    /// Maze height in cells
    pub maze_height: usize,
    /// Agent field of view in degrees
    pub fov_degrees: f32,
    /// Agent sensor range in cells
    pub sensor_range: f32,
    /// Sensor noise standard deviation
    pub sensor_noise_sigma: f32,
    /// Number of episodes to run
    pub n_episodes: usize,
    /// Maximum steps per episode
    pub max_steps_per_episode: usize,
    /// Required success rate by final episode
    pub required_success_rate: f32,
}

impl Default for PartialObservationConfig {
    fn default() -> Self {
        Self {
            maze_width: 10,
            maze_height: 10,
            fov_degrees: 90.0,
            sensor_range: 3.0,
            sensor_noise_sigma: 0.1,
            n_episodes: 100,
            max_steps_per_episode: 200,
            required_success_rate: 0.80,
        }
    }
}

/// Configuration for Emergence Falsifiability benchmark (Benchmark C)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergenceConfig {
    /// Number of agents in cooperative foraging
    pub n_agents: usize,
    /// Episodes with communication enabled
    pub episodes_with_comm: usize,
    /// Episodes with communication disabled
    pub episodes_without_comm: usize,
    /// Required cooperation drop when communication disabled
    pub required_cooperation_drop: f32,
    /// P-value threshold for statistical significance
    pub p_value_threshold: f64,
}

impl Default for EmergenceConfig {
    fn default() -> Self {
        Self {
            n_agents: 4,
            episodes_with_comm: 100,
            episodes_without_comm: 100,
            required_cooperation_drop: 0.50, // >50% drop required
            p_value_threshold: 0.01,
        }
    }
}

/// A single witness record for debugging and audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Tick when event occurred
    pub tick: u64,
    /// Type of event witnessed
    pub event_type: WitnessEventType,
    /// Entity IDs involved
    pub entity_ids: Vec<u64>,
    /// Constraint or rule that fired
    pub constraint_fired: String,
    /// Magnitude of correction applied
    pub delta_magnitude: f64,
    /// Description of what happened
    pub description: String,
}

/// Types of witness events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WitnessEventType {
    /// Two bodies were overlapping and pushed apart
    OverlapCorrection,
    /// Energy exceeded budget and was corrected
    EnergyDriftCorrection,
    /// Constraint was violated (joint limit, wall)
    ConstraintViolation,
    /// Force exceeded maximum and was clipped
    ForceClipping,
    /// Governance denied an action
    ActionRejected,
    /// State was rolled back from checkpoint
    RollbackTriggered,
    /// Numeric issue detected (NaN/Inf)
    NumericInstability,
}

/// Result of running a single benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// Name of the benchmark
    pub name: String,
    /// Whether the benchmark passed all criteria
    pub passed: bool,
    /// Detailed pass/fail for each criterion
    pub criteria: Vec<CriterionResult>,
    /// Runtime duration
    pub duration: Duration,
    /// Metrics collected during the run
    pub metrics: BenchmarkMetrics,
    /// Witness log of correction events
    pub witness_log: Vec<WitnessRecord>,
    /// Human-readable summary
    pub summary: String,
}

/// Result of evaluating a single criterion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    /// Name of the criterion
    pub name: String,
    /// Whether it passed
    pub passed: bool,
    /// Expected value or condition
    pub expected: String,
    /// Actual value observed
    pub actual: String,
}

/// Metrics collected during benchmark execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    // Physics closure metrics
    /// Maximum penetration depth observed
    pub max_penetration_depth: Option<f64>,
    /// Energy drift trajectory (energy at each checkpoint)
    pub energy_trajectory: Vec<f64>,
    /// Time (ticks) to resolve all overlaps
    pub overlap_resolution_ticks: Option<u64>,
    /// Final energy drift as fraction
    pub final_energy_drift: Option<f64>,

    // Partial observation metrics
    /// Success rate per episode bucket
    pub learning_curve: Vec<f32>,
    /// Belief entropy over time
    pub belief_entropy: Vec<f64>,
    /// Policy update magnitudes
    pub policy_update_magnitudes: Vec<f64>,

    // Emergence metrics
    /// Cooperation index with communication
    pub cooperation_with_comm: Option<f64>,
    /// Cooperation index without communication
    pub cooperation_without_comm: Option<f64>,
    /// Message-event mutual information
    pub message_mutual_info: Option<f64>,
    /// P-value from statistical test
    pub p_value: Option<f64>,
    /// T-statistic from t-test
    pub t_statistic: Option<f64>,
}

/// Run all flagship benchmarks
pub fn run_all_benchmarks(config: &BenchmarkConfig) -> Vec<BenchmarkReport> {
    let mut results = Vec::with_capacity(4);

    // Benchmark A: Physics Closure
    let physics_result = physics_closure::run_benchmark(config);
    results.push(physics_result);

    // Benchmark B: Partial Observation Agency
    let observation_result = partial_observation::run_benchmark(config);
    results.push(observation_result);

    // Benchmark C: Emergence Falsifiability
    let emergence_result = emergence::run_benchmark(config);
    results.push(emergence_result);

    // Determinism Test (foundational)
    let determinism_result = determinism::run_benchmark(config);
    results.push(determinism_result);

    results
}

/// Print a formatted report of all benchmark results
pub fn print_benchmark_summary(results: &[BenchmarkReport]) {
    println!("\n{}", "=".repeat(80));
    println!("FXNN FLAGSHIP BENCHMARK RESULTS");
    println!("{}", "=".repeat(80));

    let all_passed = results.iter().all(|r| r.passed);
    let total_duration: Duration = results.iter().map(|r| r.duration).sum();

    for result in results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        let icon = if result.passed { "[OK]" } else { "[!!]" };
        println!(
            "\n{} {} - {} ({:.2}s)",
            icon,
            result.name,
            status,
            result.duration.as_secs_f64()
        );

        for criterion in &result.criteria {
            let crit_icon = if criterion.passed { "  [+]" } else { "  [-]" };
            println!(
                "{}  {}: expected {}, got {}",
                crit_icon, criterion.name, criterion.expected, criterion.actual
            );
        }

        if !result.witness_log.is_empty() {
            println!("  Witness events: {}", result.witness_log.len());
        }
    }

    println!("\n{}", "-".repeat(80));
    println!(
        "OVERALL: {} | Total time: {:.2}s",
        if all_passed {
            "ALL BENCHMARKS PASSED"
        } else {
            "SOME BENCHMARKS FAILED"
        },
        total_duration.as_secs_f64()
    );
    println!("{}", "=".repeat(80));
}

/// Utility function to check for NaN or Inf in simulation state
pub fn check_numeric_stability(values: &[f32]) -> Result<(), String> {
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            return Err(format!("NaN detected at index {}", i));
        }
        if v.is_infinite() {
            return Err(format!("Inf detected at index {}", i));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BenchmarkConfig::default();
        assert_eq!(config.physics.n_particles, 100);
        assert_eq!(config.observation.maze_width, 10);
        assert_eq!(config.emergence.n_agents, 4);
    }

    #[test]
    fn test_witness_record_serialization() {
        let record = WitnessRecord {
            tick: 42,
            event_type: WitnessEventType::OverlapCorrection,
            entity_ids: vec![1, 2],
            constraint_fired: "LJ_repulsion".to_string(),
            delta_magnitude: 0.1,
            description: "Atoms 1 and 2 were overlapping".to_string(),
        };

        let json = serde_json::to_string(&record).unwrap();
        let parsed: WitnessRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tick, 42);
        assert_eq!(parsed.event_type, WitnessEventType::OverlapCorrection);
    }
}
