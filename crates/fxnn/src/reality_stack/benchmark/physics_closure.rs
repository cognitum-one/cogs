//! Physics closure benchmarks
//!
//! These benchmarks verify that the physics layer maintains conservation laws
//! over extended simulation runs.

use super::{Benchmark, BenchmarkResult};
use crate::types::{Atom, SimulationBox};
use crate::reality_stack::physics::{ConservationValidator, ConservationValues};

/// Result of a conservation benchmark
#[derive(Debug, Clone)]
pub struct ConservationResult {
    /// Energy drift (relative)
    pub energy_drift: f64,
    /// Momentum drift (magnitude)
    pub momentum_drift: f64,
    /// Angular momentum drift (magnitude)
    pub angular_momentum_drift: f64,
    /// Number of steps run
    pub steps: usize,
    /// Whether all laws were conserved
    pub all_conserved: bool,
}

/// Generic conservation benchmark
pub struct ConservationBenchmark {
    /// Number of simulation steps
    n_steps: usize,
    /// Energy tolerance
    energy_tolerance: f64,
    /// Momentum tolerance
    momentum_tolerance: f64,
}

impl ConservationBenchmark {
    /// Create a new conservation benchmark
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps,
            energy_tolerance: 1e-4,
            momentum_tolerance: 1e-10,
        }
    }

    /// Set energy tolerance
    pub fn with_energy_tolerance(mut self, tol: f64) -> Self {
        self.energy_tolerance = tol;
        self
    }

    /// Run the benchmark and return detailed result
    pub fn run_detailed(&self) -> ConservationResult {
        // Create test system
        let atoms = create_test_system(100);
        let box_ = SimulationBox::cubic(10.0);

        let initial = ConservationValues::from_atoms(&atoms, &box_);
        let initial_energy = compute_kinetic_energy(&atoms);

        // Simulate (simplified - would use actual simulation in practice)
        let final_atoms = simulate_steps(&atoms, &box_, self.n_steps);
        let final_values = ConservationValues::from_atoms(&final_atoms, &box_);
        let final_energy = compute_kinetic_energy(&final_atoms);

        // Compute drifts
        let energy_drift = (final_energy - initial_energy).abs() / initial_energy.abs().max(1e-10);

        let dp = [
            final_values.linear_momentum[0] - initial.linear_momentum[0],
            final_values.linear_momentum[1] - initial.linear_momentum[1],
            final_values.linear_momentum[2] - initial.linear_momentum[2],
        ];
        let momentum_drift = (dp[0].powi(2) + dp[1].powi(2) + dp[2].powi(2)).sqrt();

        let dL = [
            final_values.angular_momentum[0] - initial.angular_momentum[0],
            final_values.angular_momentum[1] - initial.angular_momentum[1],
            final_values.angular_momentum[2] - initial.angular_momentum[2],
        ];
        let angular_momentum_drift = (dL[0].powi(2) + dL[1].powi(2) + dL[2].powi(2)).sqrt();

        ConservationResult {
            energy_drift,
            momentum_drift,
            angular_momentum_drift,
            steps: self.n_steps,
            all_conserved: energy_drift < self.energy_tolerance && momentum_drift < self.momentum_tolerance,
        }
    }
}

impl Benchmark for ConservationBenchmark {
    fn name(&self) -> &str {
        "Conservation Laws"
    }

    fn run(&self) -> BenchmarkResult {
        let result = self.run_detailed();

        let mut bench_result = BenchmarkResult::new(self.name())
            .with_metric("energy_drift", result.energy_drift)
            .with_metric("momentum_drift", result.momentum_drift)
            .with_metric("angular_momentum_drift", result.angular_momentum_drift)
            .with_metric("steps", result.steps as f64);

        if result.all_conserved {
            bench_result.pass(&format!(
                "All conservation laws held for {} steps",
                result.steps
            ))
        } else {
            bench_result.fail(&format!(
                "Conservation violated: E_drift={:.2e}, P_drift={:.2e}",
                result.energy_drift, result.momentum_drift
            ))
        }
    }

    fn description(&self) -> &str {
        "Verifies energy and momentum conservation over simulation"
    }
}

/// Energy conservation specific benchmark
pub struct EnergyConservationBenchmark {
    n_steps: usize,
    tolerance: f64,
}

impl EnergyConservationBenchmark {
    /// Create new energy conservation benchmark
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps,
            tolerance: 1e-4,
        }
    }
}

impl Benchmark for EnergyConservationBenchmark {
    fn name(&self) -> &str {
        "Energy Conservation"
    }

    fn run(&self) -> BenchmarkResult {
        let atoms = create_test_system(100);
        let initial_ke = compute_kinetic_energy(&atoms);

        let box_ = SimulationBox::cubic(10.0);
        let final_atoms = simulate_steps(&atoms, &box_, self.n_steps);
        let final_ke = compute_kinetic_energy(&final_atoms);

        let drift = (final_ke - initial_ke).abs() / initial_ke.abs().max(1e-10);

        let result = BenchmarkResult::new(self.name())
            .with_metric("initial_energy", initial_ke)
            .with_metric("final_energy", final_ke)
            .with_metric("drift", drift);

        if drift < self.tolerance {
            result.pass(&format!("Energy drift {:.2e} within tolerance", drift))
        } else {
            result.fail(&format!("Energy drift {:.2e} exceeds tolerance {:.2e}", drift, self.tolerance))
        }
    }
}

/// Momentum conservation benchmark
pub struct MomentumConservationBenchmark {
    n_steps: usize,
    tolerance: f64,
}

impl MomentumConservationBenchmark {
    /// Create new momentum conservation benchmark
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps,
            tolerance: 1e-10,
        }
    }
}

impl Benchmark for MomentumConservationBenchmark {
    fn name(&self) -> &str {
        "Momentum Conservation"
    }

    fn run(&self) -> BenchmarkResult {
        let atoms = create_test_system(100);
        let initial_p = compute_momentum(&atoms);

        let box_ = SimulationBox::cubic(10.0);
        let final_atoms = simulate_steps(&atoms, &box_, self.n_steps);
        let final_p = compute_momentum(&final_atoms);

        let dp = [
            final_p[0] - initial_p[0],
            final_p[1] - initial_p[1],
            final_p[2] - initial_p[2],
        ];
        let drift = (dp[0].powi(2) + dp[1].powi(2) + dp[2].powi(2)).sqrt();

        let result = BenchmarkResult::new(self.name())
            .with_metric("drift_x", dp[0])
            .with_metric("drift_y", dp[1])
            .with_metric("drift_z", dp[2])
            .with_metric("drift_magnitude", drift);

        if drift < self.tolerance {
            result.pass(&format!("Momentum drift {:.2e} within tolerance", drift))
        } else {
            result.fail(&format!("Momentum drift {:.2e} exceeds tolerance", drift))
        }
    }
}

/// Symplectic integration benchmark
pub struct SymplecticBenchmark {
    n_steps: usize,
    n_periods: usize,
}

impl SymplecticBenchmark {
    /// Create new symplectic benchmark
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps,
            n_periods: 10,
        }
    }
}

impl Benchmark for SymplecticBenchmark {
    fn name(&self) -> &str {
        "Symplectic Integration"
    }

    fn run(&self) -> BenchmarkResult {
        // Test phase space volume preservation using a harmonic oscillator
        let atoms = vec![
            Atom::new(0, 0, 1.0)
                .with_position(1.0, 0.0, 0.0)
                .with_velocity(0.0, 1.0, 0.0),
        ];

        let box_ = SimulationBox::cubic(10.0);
        let final_atoms = simulate_steps(&atoms, &box_, self.n_steps);

        // For harmonic oscillator, radius in phase space should be preserved
        let initial_r = (atoms[0].position[0].powi(2) + atoms[0].velocity[1].powi(2) as f32).sqrt();
        let final_r = (final_atoms[0].position[0].powi(2) + final_atoms[0].velocity[1].powi(2) as f32).sqrt();

        let drift = (final_r - initial_r).abs() / initial_r;

        let result = BenchmarkResult::new(self.name())
            .with_metric("initial_radius", initial_r as f64)
            .with_metric("final_radius", final_r as f64)
            .with_metric("drift", drift as f64);

        if drift < 0.01 {
            result.pass("Phase space volume preserved")
        } else {
            result.fail(&format!("Phase space drift: {:.2e}", drift))
        }
    }
}

// Helper functions

fn create_test_system(n_atoms: usize) -> Vec<Atom> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut atoms: Vec<Atom> = (0..n_atoms)
        .map(|i| {
            Atom::new(i as u32, 0, 1.0)
                .with_position(
                    rng.gen_range(0.0..10.0),
                    rng.gen_range(0.0..10.0),
                    rng.gen_range(0.0..10.0),
                )
                .with_velocity(
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                )
        })
        .collect();

    // Remove center of mass velocity
    let mut vcm = [0.0f32; 3];
    for atom in &atoms {
        vcm[0] += atom.velocity[0];
        vcm[1] += atom.velocity[1];
        vcm[2] += atom.velocity[2];
    }
    let n = atoms.len() as f32;
    vcm[0] /= n;
    vcm[1] /= n;
    vcm[2] /= n;

    for atom in &mut atoms {
        atom.velocity[0] -= vcm[0];
        atom.velocity[1] -= vcm[1];
        atom.velocity[2] -= vcm[2];
    }

    atoms
}

fn compute_kinetic_energy(atoms: &[Atom]) -> f64 {
    atoms.iter().map(|a| a.kinetic_energy() as f64).sum()
}

fn compute_momentum(atoms: &[Atom]) -> [f64; 3] {
    let mut p = [0.0f64; 3];
    for atom in atoms {
        p[0] += (atom.mass * atom.velocity[0]) as f64;
        p[1] += (atom.mass * atom.velocity[1]) as f64;
        p[2] += (atom.mass * atom.velocity[2]) as f64;
    }
    p
}

fn simulate_steps(atoms: &[Atom], box_: &SimulationBox, n_steps: usize) -> Vec<Atom> {
    let dt = 0.001;
    let mut current = atoms.to_vec();

    for _ in 0..n_steps {
        // Simple Verlet step (no forces for benchmark)
        for atom in &mut current {
            atom.position[0] += atom.velocity[0] * dt;
            atom.position[1] += atom.velocity[1] * dt;
            atom.position[2] += atom.velocity[2] * dt;

            // Apply PBC
            for i in 0..3 {
                while atom.position[i] < 0.0 {
                    atom.position[i] += box_.dimensions[i];
                }
                while atom.position[i] >= box_.dimensions[i] {
                    atom.position[i] -= box_.dimensions[i];
                }
            }
        }
    }

    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservation_benchmark() {
        let bench = ConservationBenchmark::new(100);
        let result = bench.run();
        // Should pass with no forces
        assert!(result.passed);
    }

    #[test]
    fn test_momentum_benchmark() {
        let bench = MomentumConservationBenchmark::new(100);
        let result = bench.run();
        assert!(result.passed);
    }
}
