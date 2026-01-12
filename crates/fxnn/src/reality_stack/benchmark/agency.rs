//! Agency benchmarks
//!
//! These benchmarks measure agent decision-making performance including
//! policy throughput, sensor bandwidth, and action execution.

use super::{Benchmark, BenchmarkResult};
use crate::reality_stack::agency::{
    Agent, AgentId, Policy, RandomPolicy, NeuralPolicy,
    Sensor, DistanceSensor, ForceSensor, SensorReading,
    Actuator, ForceActuator, ProposedAction, ActionKind,
};
use std::time::Instant;

/// Result of an agency benchmark
#[derive(Debug, Clone)]
pub struct AgencyResult {
    /// Decisions per second
    pub decisions_per_second: f64,
    /// Average decision latency (microseconds)
    pub avg_latency_us: f64,
    /// Sensor readings per second
    pub readings_per_second: f64,
    /// Actions per second
    pub actions_per_second: f64,
}

/// Generic agency benchmark
pub struct AgencyBenchmark {
    /// Number of iterations
    n_iterations: usize,
    /// Number of agents
    n_agents: usize,
    /// Number of sensors per agent
    n_sensors: usize,
}

impl AgencyBenchmark {
    /// Create a new agency benchmark
    pub fn new(n_iterations: usize) -> Self {
        Self {
            n_iterations,
            n_agents: 10,
            n_sensors: 4,
        }
    }

    /// Set number of agents
    pub fn with_agents(mut self, n: usize) -> Self {
        self.n_agents = n;
        self
    }

    /// Run detailed benchmark
    pub fn run_detailed(&self) -> AgencyResult {
        let policy = RandomPolicy::new().with_n_outputs(3);
        let mut total_decisions = 0;
        let mut total_latency_us = 0u64;

        let start = Instant::now();

        for _ in 0..self.n_iterations {
            let readings: Vec<SensorReading> = (0..self.n_sensors)
                .map(|i| SensorReading {
                    sensor_id: crate::reality_stack::agency::sensor::SensorId(i as u32),
                    kind: crate::reality_stack::agency::sensor::SensorKind::Proprioceptive,
                    values: vec![0.0; 16],
                    timestamp: 0,
                    noise_level: 0.0,
                })
                .collect();

            let decision_start = Instant::now();
            let _ = policy.decide(&readings, &[1.0]);
            total_latency_us += decision_start.elapsed().as_micros() as u64;
            total_decisions += 1;
        }

        let elapsed = start.elapsed().as_secs_f64();

        AgencyResult {
            decisions_per_second: total_decisions as f64 / elapsed,
            avg_latency_us: total_latency_us as f64 / total_decisions as f64,
            readings_per_second: (total_decisions * self.n_sensors) as f64 / elapsed,
            actions_per_second: total_decisions as f64 / elapsed,
        }
    }
}

impl Benchmark for AgencyBenchmark {
    fn name(&self) -> &str {
        "Agency Performance"
    }

    fn run(&self) -> BenchmarkResult {
        let result = self.run_detailed();

        BenchmarkResult::new(self.name())
            .with_metric("decisions_per_second", result.decisions_per_second)
            .with_metric("avg_latency_us", result.avg_latency_us)
            .with_metric("readings_per_second", result.readings_per_second)
            .pass(&format!(
                "{:.0} decisions/s, {:.1}us latency",
                result.decisions_per_second, result.avg_latency_us
            ))
    }

    fn description(&self) -> &str {
        "Measures agent decision-making throughput and latency"
    }
}

/// Policy throughput benchmark
pub struct PolicyThroughputBenchmark {
    n_iterations: usize,
    input_size: usize,
    output_size: usize,
}

impl PolicyThroughputBenchmark {
    /// Create new policy throughput benchmark
    pub fn new(n_iterations: usize) -> Self {
        Self {
            n_iterations,
            input_size: 64,
            output_size: 8,
        }
    }
}

impl Benchmark for PolicyThroughputBenchmark {
    fn name(&self) -> &str {
        "Policy Throughput"
    }

    fn run(&self) -> BenchmarkResult {
        // Test different policy types
        let random_policy = RandomPolicy::new().with_n_outputs(self.output_size);
        let neural_policy = NeuralPolicy::new(self.input_size, self.output_size);

        let readings: Vec<SensorReading> = vec![SensorReading {
            sensor_id: crate::reality_stack::agency::sensor::SensorId(0),
            kind: crate::reality_stack::agency::sensor::SensorKind::Proprioceptive,
            values: vec![0.5; self.input_size],
            timestamp: 0,
            noise_level: 0.0,
        }];

        // Benchmark random policy
        let start = Instant::now();
        for _ in 0..self.n_iterations {
            let _ = random_policy.decide(&readings, &[1.0]);
        }
        let random_time = start.elapsed();
        let random_throughput = self.n_iterations as f64 / random_time.as_secs_f64();

        // Benchmark neural policy
        let start = Instant::now();
        for _ in 0..self.n_iterations {
            let _ = neural_policy.decide(&readings, &[1.0]);
        }
        let neural_time = start.elapsed();
        let neural_throughput = self.n_iterations as f64 / neural_time.as_secs_f64();

        BenchmarkResult::new(self.name())
            .with_metric("random_throughput", random_throughput)
            .with_metric("neural_throughput", neural_throughput)
            .with_metric("random_latency_us", random_time.as_micros() as f64 / self.n_iterations as f64)
            .with_metric("neural_latency_us", neural_time.as_micros() as f64 / self.n_iterations as f64)
            .pass(&format!(
                "Random: {:.0}/s, Neural: {:.0}/s",
                random_throughput, neural_throughput
            ))
    }
}

/// Sensor bandwidth benchmark
pub struct SensorBandwidthBenchmark {
    n_iterations: usize,
    n_atoms: usize,
}

impl SensorBandwidthBenchmark {
    /// Create new sensor bandwidth benchmark
    pub fn new(n_iterations: usize) -> Self {
        Self {
            n_iterations,
            n_atoms: 1000,
        }
    }
}

impl Benchmark for SensorBandwidthBenchmark {
    fn name(&self) -> &str {
        "Sensor Bandwidth"
    }

    fn run(&self) -> BenchmarkResult {
        use crate::types::{Atom, SimulationBox};

        // Create test atoms
        let atoms: Vec<Atom> = (0..self.n_atoms)
            .map(|i| {
                Atom::new(i as u32, 0, 1.0)
                    .with_position(
                        (i % 10) as f32,
                        ((i / 10) % 10) as f32,
                        (i / 100) as f32,
                    )
            })
            .collect();
        let box_ = SimulationBox::cubic(10.0);

        // Create sensors
        let distance_sensor = DistanceSensor::new(5.0);
        let force_sensor = ForceSensor::new(0.01);

        let agent_pos = [5.0, 5.0, 5.0];

        // Benchmark distance sensor
        let start = Instant::now();
        let mut total_values = 0usize;
        for _ in 0..self.n_iterations {
            let reading = distance_sensor.read(agent_pos, &atoms, &box_);
            total_values += reading.values.len();
        }
        let distance_time = start.elapsed();
        let distance_bandwidth = total_values as f64 / distance_time.as_secs_f64();

        // Benchmark force sensor
        let start = Instant::now();
        total_values = 0;
        for _ in 0..self.n_iterations {
            let reading = force_sensor.read(agent_pos, &atoms, &box_);
            total_values += reading.values.len();
        }
        let force_time = start.elapsed();
        let force_bandwidth = total_values as f64 / force_time.as_secs_f64();

        BenchmarkResult::new(self.name())
            .with_metric("distance_values_per_second", distance_bandwidth)
            .with_metric("force_values_per_second", force_bandwidth)
            .with_metric("distance_latency_us", distance_time.as_micros() as f64 / self.n_iterations as f64)
            .with_metric("force_latency_us", force_time.as_micros() as f64 / self.n_iterations as f64)
            .pass(&format!(
                "Distance: {:.0} vals/s, Force: {:.0} vals/s",
                distance_bandwidth, force_bandwidth
            ))
    }
}

/// Action execution benchmark
pub struct ActionExecutionBenchmark {
    n_iterations: usize,
}

impl ActionExecutionBenchmark {
    /// Create new action execution benchmark
    pub fn new(n_iterations: usize) -> Self {
        Self { n_iterations }
    }
}

impl Benchmark for ActionExecutionBenchmark {
    fn name(&self) -> &str {
        "Action Execution"
    }

    fn run(&self) -> BenchmarkResult {
        // Create actuator
        let actuator = ForceActuator::new(10.0);

        // Benchmark action generation
        let start = Instant::now();
        let mut actions = Vec::with_capacity(self.n_iterations);
        for i in 0..self.n_iterations {
            let action = actuator.generate_action(i as u32, 0.5);
            actions.push(action);
        }
        let generation_time = start.elapsed();
        let generation_rate = self.n_iterations as f64 / generation_time.as_secs_f64();

        // Benchmark action creation with full metadata
        let start = Instant::now();
        for i in 0..self.n_iterations {
            let _ = ProposedAction::new(
                AgentId(0),
                ActionKind::ApplyForce {
                    atom_id: i as u32,
                    force: [1.0, 0.0, 0.0],
                },
            ).with_priority(128)
             .with_rationale("Benchmark action");
        }
        let proposal_time = start.elapsed();
        let proposal_rate = self.n_iterations as f64 / proposal_time.as_secs_f64();

        BenchmarkResult::new(self.name())
            .with_metric("generation_rate", generation_rate)
            .with_metric("proposal_rate", proposal_rate)
            .with_metric("generation_latency_ns", generation_time.as_nanos() as f64 / self.n_iterations as f64)
            .pass(&format!(
                "{:.0} generations/s, {:.0} proposals/s",
                generation_rate, proposal_rate
            ))
    }
}

/// Multi-agent coordination benchmark
pub struct MultiAgentBenchmark {
    n_agents: usize,
    n_steps: usize,
}

impl MultiAgentBenchmark {
    /// Create new multi-agent benchmark
    pub fn new(n_agents: usize, n_steps: usize) -> Self {
        Self { n_agents, n_steps }
    }
}

impl Benchmark for MultiAgentBenchmark {
    fn name(&self) -> &str {
        "Multi-Agent Coordination"
    }

    fn run(&self) -> BenchmarkResult {
        // Create agents with random policies
        let policies: Vec<RandomPolicy> = (0..self.n_agents)
            .map(|_| RandomPolicy::new().with_n_outputs(3))
            .collect();

        let readings = vec![SensorReading {
            sensor_id: crate::reality_stack::agency::sensor::SensorId(0),
            kind: crate::reality_stack::agency::sensor::SensorKind::Proprioceptive,
            values: vec![0.5; 16],
            timestamp: 0,
            noise_level: 0.0,
        }];

        let start = Instant::now();
        let mut total_actions = 0;

        for _ in 0..self.n_steps {
            for policy in &policies {
                let _ = policy.decide(&readings, &[1.0]);
                total_actions += 1;
            }
        }

        let elapsed = start.elapsed();
        let actions_per_second = total_actions as f64 / elapsed.as_secs_f64();
        let steps_per_second = self.n_steps as f64 / elapsed.as_secs_f64();

        BenchmarkResult::new(self.name())
            .with_metric("agents", self.n_agents as f64)
            .with_metric("steps", self.n_steps as f64)
            .with_metric("actions_per_second", actions_per_second)
            .with_metric("steps_per_second", steps_per_second)
            .pass(&format!(
                "{} agents: {:.0} steps/s, {:.0} actions/s",
                self.n_agents, steps_per_second, actions_per_second
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_throughput() {
        let bench = PolicyThroughputBenchmark::new(100);
        let result = bench.run();
        assert!(result.passed);
    }

    #[test]
    fn test_sensor_bandwidth() {
        let bench = SensorBandwidthBenchmark::new(100);
        let result = bench.run();
        assert!(result.passed);
    }
}
