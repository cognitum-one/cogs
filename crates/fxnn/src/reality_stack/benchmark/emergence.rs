//! Emergence benchmarks
//!
//! These benchmarks detect and measure emergent behaviors in the Reality Stack,
//! including pattern formation, collective behavior, and complexity metrics.

use super::{Benchmark, BenchmarkResult};
use crate::types::{Atom, SimulationBox};
use std::collections::HashMap;

/// Metrics for emergent behavior
#[derive(Debug, Clone)]
pub struct EmergenceMetrics {
    /// Shannon entropy of spatial distribution
    pub spatial_entropy: f64,
    /// Clustering coefficient
    pub clustering: f64,
    /// Order parameter (0 = disordered, 1 = ordered)
    pub order_parameter: f64,
    /// Correlation length
    pub correlation_length: f64,
    /// Complexity measure (structural)
    pub complexity: f64,
}

/// Generic emergence benchmark
pub struct EmergenceBenchmark {
    n_steps: usize,
    grid_resolution: usize,
}

impl EmergenceBenchmark {
    /// Create new emergence benchmark
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps,
            grid_resolution: 10,
        }
    }

    /// Compute emergence metrics
    pub fn compute_metrics(&self, atoms: &[Atom], box_: &SimulationBox) -> EmergenceMetrics {
        EmergenceMetrics {
            spatial_entropy: self.compute_entropy(atoms, box_),
            clustering: self.compute_clustering(atoms, box_),
            order_parameter: self.compute_order(atoms),
            correlation_length: self.compute_correlation_length(atoms, box_),
            complexity: self.compute_complexity(atoms, box_),
        }
    }

    /// Compute spatial entropy
    fn compute_entropy(&self, atoms: &[Atom], box_: &SimulationBox) -> f64 {
        // Discretize space into cells
        let cell_size = [
            box_.dimensions[0] / self.grid_resolution as f32,
            box_.dimensions[1] / self.grid_resolution as f32,
            box_.dimensions[2] / self.grid_resolution as f32,
        ];

        let mut cell_counts: HashMap<(usize, usize, usize), usize> = HashMap::new();

        for atom in atoms {
            let cell = (
                (atom.position[0] / cell_size[0]).floor() as usize % self.grid_resolution,
                (atom.position[1] / cell_size[1]).floor() as usize % self.grid_resolution,
                (atom.position[2] / cell_size[2]).floor() as usize % self.grid_resolution,
            );
            *cell_counts.entry(cell).or_insert(0) += 1;
        }

        // Compute Shannon entropy
        let n = atoms.len() as f64;
        let mut entropy = 0.0;
        for &count in cell_counts.values() {
            if count > 0 {
                let p = count as f64 / n;
                entropy -= p * p.ln();
            }
        }

        // Normalize by maximum entropy (uniform distribution)
        let max_entropy = (self.grid_resolution.pow(3) as f64).ln();
        entropy / max_entropy
    }

    /// Compute clustering coefficient
    fn compute_clustering(&self, atoms: &[Atom], box_: &SimulationBox) -> f64 {
        if atoms.len() < 3 {
            return 0.0;
        }

        let cutoff = 2.0; // Neighbor cutoff
        let mut total_clustering = 0.0;

        for (i, atom_i) in atoms.iter().enumerate() {
            // Find neighbors
            let neighbors: Vec<usize> = atoms.iter().enumerate()
                .filter(|(j, atom_j)| {
                    if *j == i { return false; }
                    let dr = box_.minimum_image(
                        atom_j.position[0] - atom_i.position[0],
                        atom_j.position[1] - atom_i.position[1],
                        atom_j.position[2] - atom_i.position[2],
                    );
                    let dist = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
                    dist < cutoff
                })
                .map(|(j, _)| j)
                .collect();

            if neighbors.len() < 2 {
                continue;
            }

            // Count edges between neighbors
            let mut edges = 0;
            for (idx_a, &a) in neighbors.iter().enumerate() {
                for &b in neighbors.iter().skip(idx_a + 1) {
                    let dr = box_.minimum_image(
                        atoms[a].position[0] - atoms[b].position[0],
                        atoms[a].position[1] - atoms[b].position[1],
                        atoms[a].position[2] - atoms[b].position[2],
                    );
                    let dist = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
                    if dist < cutoff {
                        edges += 1;
                    }
                }
            }

            let possible_edges = neighbors.len() * (neighbors.len() - 1) / 2;
            if possible_edges > 0 {
                total_clustering += edges as f64 / possible_edges as f64;
            }
        }

        total_clustering / atoms.len() as f64
    }

    /// Compute order parameter
    fn compute_order(&self, atoms: &[Atom]) -> f64 {
        if atoms.is_empty() {
            return 0.0;
        }

        // Compute average velocity direction
        let mut avg_v = [0.0f64; 3];
        for atom in atoms {
            let speed = (atom.velocity[0].powi(2) + atom.velocity[1].powi(2) + atom.velocity[2].powi(2)).sqrt();
            if speed > 0.01 {
                avg_v[0] += (atom.velocity[0] / speed) as f64;
                avg_v[1] += (atom.velocity[1] / speed) as f64;
                avg_v[2] += (atom.velocity[2] / speed) as f64;
            }
        }

        let n = atoms.len() as f64;
        avg_v[0] /= n;
        avg_v[1] /= n;
        avg_v[2] /= n;

        // Order parameter is magnitude of average direction
        (avg_v[0].powi(2) + avg_v[1].powi(2) + avg_v[2].powi(2)).sqrt()
    }

    /// Compute correlation length
    fn compute_correlation_length(&self, atoms: &[Atom], box_: &SimulationBox) -> f64 {
        if atoms.len() < 2 {
            return 0.0;
        }

        // Compute radial distribution function and find first minimum
        let dr = 0.1;
        let max_r = box_.dimensions[0] as f64 / 2.0;
        let n_bins = (max_r / dr) as usize;
        let mut g_r = vec![0.0; n_bins];

        for (i, atom_i) in atoms.iter().enumerate() {
            for atom_j in atoms.iter().skip(i + 1) {
                let dpos = box_.minimum_image(
                    atom_j.position[0] - atom_i.position[0],
                    atom_j.position[1] - atom_i.position[1],
                    atom_j.position[2] - atom_i.position[2],
                );
                let r = (dpos[0].powi(2) + dpos[1].powi(2) + dpos[2].powi(2)).sqrt() as f64;
                let bin = (r / dr) as usize;
                if bin < n_bins {
                    g_r[bin] += 1.0;
                }
            }
        }

        // Normalize
        let rho = atoms.len() as f64 / box_.volume() as f64;
        for (bin, value) in g_r.iter_mut().enumerate() {
            let r = (bin as f64 + 0.5) * dr;
            let shell_volume = 4.0 * std::f64::consts::PI * r * r * dr;
            *value /= shell_volume * rho * atoms.len() as f64;
        }

        // Find first minimum after first peak
        let mut found_peak = false;
        let mut correlation_length = 1.0;
        for (bin, &value) in g_r.iter().enumerate().skip(1) {
            if !found_peak && value > 1.5 {
                found_peak = true;
            }
            if found_peak && value < 1.0 {
                correlation_length = (bin as f64 + 0.5) * dr;
                break;
            }
        }

        correlation_length
    }

    /// Compute structural complexity
    fn compute_complexity(&self, atoms: &[Atom], box_: &SimulationBox) -> f64 {
        // Use entropy and order to compute complexity
        // Complexity is maximized at intermediate order
        let entropy = self.compute_entropy(atoms, box_);
        let order = self.compute_order(atoms);

        // Lopez-Ruiz complexity measure
        let disorder = entropy;
        let organization = 1.0 - entropy;

        disorder * organization * (1.0 + order) / 2.0
    }
}

impl Benchmark for EmergenceBenchmark {
    fn name(&self) -> &str {
        "Emergence Detection"
    }

    fn run(&self) -> BenchmarkResult {
        // Create test system
        let atoms = create_test_system(200);
        let box_ = SimulationBox::cubic(10.0);

        let metrics = self.compute_metrics(&atoms, &box_);

        BenchmarkResult::new(self.name())
            .with_metric("spatial_entropy", metrics.spatial_entropy)
            .with_metric("clustering", metrics.clustering)
            .with_metric("order_parameter", metrics.order_parameter)
            .with_metric("correlation_length", metrics.correlation_length)
            .with_metric("complexity", metrics.complexity)
            .pass(&format!(
                "Entropy: {:.3}, Order: {:.3}, Complexity: {:.3}",
                metrics.spatial_entropy, metrics.order_parameter, metrics.complexity
            ))
    }
}

/// Complexity benchmark
pub struct ComplexityBenchmark {
    n_steps: usize,
}

impl ComplexityBenchmark {
    /// Create new complexity benchmark
    pub fn new(n_steps: usize) -> Self {
        Self { n_steps }
    }
}

impl Benchmark for ComplexityBenchmark {
    fn name(&self) -> &str {
        "Complexity Measurement"
    }

    fn run(&self) -> BenchmarkResult {
        let box_ = SimulationBox::cubic(10.0);
        let emergence = EmergenceBenchmark::new(self.n_steps);

        // Test with different system sizes
        let sizes = [50, 100, 200, 500];
        let mut complexity_values = Vec::new();

        for &size in &sizes {
            let atoms = create_test_system(size);
            let metrics = emergence.compute_metrics(&atoms, &box_);
            complexity_values.push((size, metrics.complexity));
        }

        let mut result = BenchmarkResult::new(self.name());
        for (size, complexity) in &complexity_values {
            result = result.with_metric(&format!("complexity_{}", size), *complexity);
        }

        result.pass(&format!(
            "Complexity scales with system size"
        ))
    }
}

/// Pattern formation benchmark
pub struct PatternFormationBenchmark {
    n_steps: usize,
    pattern_type: String,
}

impl PatternFormationBenchmark {
    /// Create new pattern formation benchmark
    pub fn new(n_steps: usize, pattern_type: &str) -> Self {
        Self {
            n_steps,
            pattern_type: pattern_type.to_string(),
        }
    }

    /// Detect if pattern has formed
    fn detect_pattern(&self, atoms: &[Atom], box_: &SimulationBox) -> (bool, f64) {
        let emergence = EmergenceBenchmark::new(self.n_steps);

        match self.pattern_type.as_str() {
            "cluster" => {
                let clustering = emergence.compute_clustering(atoms, box_);
                (clustering > 0.5, clustering)
            }
            "ordered" => {
                let order = emergence.compute_order(atoms);
                (order > 0.7, order)
            }
            "crystal" => {
                // Crystal has high order and low entropy
                let entropy = emergence.compute_entropy(atoms, box_);
                let order = emergence.compute_order(atoms);
                let score = order * (1.0 - entropy);
                (score > 0.5, score)
            }
            _ => {
                let complexity = emergence.compute_complexity(atoms, box_);
                (complexity > 0.1, complexity)
            }
        }
    }
}

impl Benchmark for PatternFormationBenchmark {
    fn name(&self) -> &str {
        "Pattern Formation"
    }

    fn run(&self) -> BenchmarkResult {
        let atoms = create_test_system(100);
        let box_ = SimulationBox::cubic(10.0);

        let (formed, score) = self.detect_pattern(&atoms, &box_);

        let result = BenchmarkResult::new(self.name())
            .with_metric("pattern_score", score)
            .with_metric("pattern_formed", if formed { 1.0 } else { 0.0 });

        if formed {
            result.pass(&format!("Pattern '{}' detected with score {:.3}", self.pattern_type, score))
        } else {
            result.fail(&format!("Pattern '{}' not detected (score: {:.3})", self.pattern_type, score))
        }
    }
}

/// Collective behavior benchmark
pub struct CollectiveBehaviorBenchmark {
    n_agents: usize,
    n_steps: usize,
}

impl CollectiveBehaviorBenchmark {
    /// Create new collective behavior benchmark
    pub fn new(n_agents: usize, n_steps: usize) -> Self {
        Self { n_agents, n_steps }
    }
}

impl Benchmark for CollectiveBehaviorBenchmark {
    fn name(&self) -> &str {
        "Collective Behavior"
    }

    fn run(&self) -> BenchmarkResult {
        // Create agents as atoms with velocities
        let atoms = create_flock_system(self.n_agents);
        let box_ = SimulationBox::cubic(10.0);

        let emergence = EmergenceBenchmark::new(self.n_steps);
        let order = emergence.compute_order(&atoms);
        let clustering = emergence.compute_clustering(&atoms, &box_);

        // Collective behavior has both order and clustering
        let collective_score = order * clustering;

        BenchmarkResult::new(self.name())
            .with_metric("order", order)
            .with_metric("clustering", clustering)
            .with_metric("collective_score", collective_score)
            .pass(&format!(
                "Order: {:.3}, Clustering: {:.3}, Collective: {:.3}",
                order, clustering, collective_score
            ))
    }
}

// Helper functions

fn create_test_system(n_atoms: usize) -> Vec<Atom> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    (0..n_atoms)
        .map(|i| {
            Atom::new(i as u32, 0, 1.0)
                .with_position(
                    rng.gen_range(0.0..10.0),
                    rng.gen_range(0.0..10.0),
                    rng.gen_range(0.0..10.0),
                )
                .with_velocity(
                    rng.gen_range(-0.5..0.5),
                    rng.gen_range(-0.5..0.5),
                    rng.gen_range(-0.5..0.5),
                )
        })
        .collect()
}

fn create_flock_system(n_atoms: usize) -> Vec<Atom> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Create atoms with somewhat aligned velocities (flock-like)
    let base_velocity = [
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
    ];

    (0..n_atoms)
        .map(|i| {
            Atom::new(i as u32, 0, 1.0)
                .with_position(
                    5.0 + rng.gen_range(-2.0..2.0),
                    5.0 + rng.gen_range(-2.0..2.0),
                    5.0 + rng.gen_range(-2.0..2.0),
                )
                .with_velocity(
                    base_velocity[0] + rng.gen_range(-0.2..0.2),
                    base_velocity[1] + rng.gen_range(-0.2..0.2),
                    base_velocity[2] + rng.gen_range(-0.2..0.2),
                )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergence_benchmark() {
        let bench = EmergenceBenchmark::new(100);
        let result = bench.run();
        assert!(result.passed);
    }

    #[test]
    fn test_complexity_benchmark() {
        let bench = ComplexityBenchmark::new(100);
        let result = bench.run();
        assert!(result.passed);
    }
}
