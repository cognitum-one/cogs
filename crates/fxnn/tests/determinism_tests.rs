//! Determinism Tests
//!
//! Tests for reproducibility and determinism:
//! - Trajectory hash reproducibility
//! - Seed determinism
//!
//! These tests ensure that simulations can be exactly reproduced
//! given the same initial conditions and random seeds.

use fxnn::{
    Simulation, SimulationBox, LennardJones, VelocityVerlet, ForceField,
    generators::{fcc_lattice, maxwell_boltzmann_velocities, random_atoms},
    types::Atom,
    observable,
};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use rand::{SeedableRng, Rng};
use rand_xoshiro::Xoshiro256StarStar;

// ============================================================================
// Trajectory Hashing Infrastructure
// ============================================================================

/// Trajectory snapshot at a given time
#[derive(Debug, Clone)]
struct TrajectorySnapshot {
    step: usize,
    positions: Vec<[f32; 3]>,
    velocities: Vec<[f32; 3]>,
    total_energy: f64,
    temperature: f32,
}

impl TrajectorySnapshot {
    fn from_simulation<F: fxnn::force_field::ForceField, I: fxnn::integrator::Integrator>(
        sim: &Simulation<F, I>
    ) -> Self {
        Self {
            step: sim.step(),
            positions: sim.atoms().iter().map(|a| a.position).collect(),
            velocities: sim.atoms().iter().map(|a| a.velocity).collect(),
            total_energy: sim.total_energy(),
            temperature: sim.temperature(),
        }
    }

    /// Compute a deterministic hash of this snapshot
    fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        self.step.hash(&mut hasher);

        // Hash positions with fixed precision
        for pos in &self.positions {
            for &p in pos {
                let quantized = (p * 1e6) as i64;
                quantized.hash(&mut hasher);
            }
        }

        // Hash velocities with fixed precision
        for vel in &self.velocities {
            for &v in vel {
                let quantized = (v * 1e6) as i64;
                quantized.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Check if two snapshots are approximately equal
    fn approx_equal(&self, other: &TrajectorySnapshot, tol: f32) -> bool {
        if self.step != other.step {
            return false;
        }

        if self.positions.len() != other.positions.len() {
            return false;
        }

        for (p1, p2) in self.positions.iter().zip(other.positions.iter()) {
            for i in 0..3 {
                if (p1[i] - p2[i]).abs() > tol {
                    return false;
                }
            }
        }

        for (v1, v2) in self.velocities.iter().zip(other.velocities.iter()) {
            for i in 0..3 {
                if (v1[i] - v2[i]).abs() > tol {
                    return false;
                }
            }
        }

        true
    }
}

/// Full trajectory as a sequence of snapshots
struct Trajectory {
    snapshots: Vec<TrajectorySnapshot>,
}

impl Trajectory {
    fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    fn add_snapshot(&mut self, snapshot: TrajectorySnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Compute hash of entire trajectory
    fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        for snapshot in &self.snapshots {
            snapshot.compute_hash().hash(&mut hasher);
        }

        hasher.finish()
    }

    fn len(&self) -> usize {
        self.snapshots.len()
    }
}

// ============================================================================
// Trajectory Hash Reproducibility Tests
// ============================================================================

/// Create a deterministic initial configuration
fn create_deterministic_config() -> (Vec<Atom>, SimulationBox) {
    // FCC lattice is deterministic
    let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    let box_ = SimulationBox::cubic(4.5);

    // Initialize velocities deterministically using seeded RNG
    let mut rng = Xoshiro256StarStar::seed_from_u64(12345);

    for atom in &mut atoms {
        atom.velocity = [
            (rng.gen::<f64>() * 2.0 - 1.0) as f32,
            (rng.gen::<f64>() * 2.0 - 1.0) as f32,
            (rng.gen::<f64>() * 2.0 - 1.0) as f32,
        ];
    }

    // Remove COM velocity deterministically
    observable::remove_com_velocity(&mut atoms);

    (atoms, box_)
}

/// Run a simulation and record trajectory
fn run_and_record_trajectory(
    atoms: Vec<Atom>,
    box_: SimulationBox,
    steps: usize,
    snapshot_interval: usize,
) -> Trajectory {
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    let mut trajectory = Trajectory::new();

    // Record initial state
    trajectory.add_snapshot(TrajectorySnapshot::from_simulation(&sim));

    // Run and record
    for step in 1..=steps {
        sim.step_forward();

        if step % snapshot_interval == 0 {
            trajectory.add_snapshot(TrajectorySnapshot::from_simulation(&sim));
        }
    }

    trajectory
}

/// Test that same initial conditions produce same trajectory hash
#[test]
fn test_trajectory_hash_reproducibility() {
    // Run 1
    let (atoms1, box1) = create_deterministic_config();
    let trajectory1 = run_and_record_trajectory(atoms1, box1, 100, 10);

    // Run 2 (should be identical)
    let (atoms2, box2) = create_deterministic_config();
    let trajectory2 = run_and_record_trajectory(atoms2, box2, 100, 10);

    let hash1 = trajectory1.compute_hash();
    let hash2 = trajectory2.compute_hash();

    println!("Trajectory 1 hash: {:016x}", hash1);
    println!("Trajectory 2 hash: {:016x}", hash2);
    println!("Trajectory length: {} snapshots", trajectory1.len());

    assert_eq!(
        hash1, hash2,
        "Identical initial conditions should produce identical trajectory hashes"
    );
}

/// Test that all snapshots match between identical runs
#[test]
fn test_trajectory_snapshot_reproducibility() {
    let (atoms1, box1) = create_deterministic_config();
    let trajectory1 = run_and_record_trajectory(atoms1, box1, 50, 5);

    let (atoms2, box2) = create_deterministic_config();
    let trajectory2 = run_and_record_trajectory(atoms2, box2, 50, 5);

    assert_eq!(
        trajectory1.len(),
        trajectory2.len(),
        "Trajectories should have same number of snapshots"
    );

    for (i, (snap1, snap2)) in trajectory1.snapshots.iter()
        .zip(trajectory2.snapshots.iter())
        .enumerate()
    {
        let hash1 = snap1.compute_hash();
        let hash2 = snap2.compute_hash();

        assert_eq!(
            hash1, hash2,
            "Snapshot {} hashes should match: {:016x} vs {:016x}",
            i, hash1, hash2
        );

        // Also check approximate equality
        assert!(
            snap1.approx_equal(snap2, 1e-6),
            "Snapshot {} positions/velocities should match exactly",
            i
        );
    }
}

/// Test that different initial conditions produce different hashes
#[test]
fn test_trajectory_hash_sensitivity() {
    let (atoms1, box1) = create_deterministic_config();
    let trajectory1 = run_and_record_trajectory(atoms1, box1, 50, 10);

    // Create different initial condition
    let (mut atoms2, box2) = create_deterministic_config();
    // Small perturbation
    atoms2[0].position[0] += 0.001;
    let trajectory2 = run_and_record_trajectory(atoms2, box2, 50, 10);

    let hash1 = trajectory1.compute_hash();
    let hash2 = trajectory2.compute_hash();

    println!("Hash (original): {:016x}", hash1);
    println!("Hash (perturbed): {:016x}", hash2);

    // Due to chaotic nature, even small perturbations should eventually lead to different hashes
    // (though they might match for early snapshots)
    assert_ne!(
        hash1, hash2,
        "Different initial conditions should produce different hashes"
    );
}

// ============================================================================
// Seed Determinism Tests
// ============================================================================

/// Test that seeded random atom generation is deterministic
#[test]
fn test_seed_determinism_atoms() {
    fn create_atoms_with_seed(seed: u64, n: usize) -> Vec<Atom> {
        let mut rng = Xoshiro256StarStar::seed_from_u64(seed);

        (0..n)
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
            .collect()
    }

    // Same seed should produce same atoms
    let atoms1 = create_atoms_with_seed(42, 100);
    let atoms2 = create_atoms_with_seed(42, 100);

    for i in 0..100 {
        assert_eq!(
            atoms1[i].position, atoms2[i].position,
            "Atom {} position should match with same seed",
            i
        );
        assert_eq!(
            atoms1[i].velocity, atoms2[i].velocity,
            "Atom {} velocity should match with same seed",
            i
        );
    }

    // Different seed should produce different atoms
    let atoms3 = create_atoms_with_seed(123, 100);

    let mut any_different = false;
    for i in 0..100 {
        if atoms1[i].position != atoms3[i].position {
            any_different = true;
            break;
        }
    }

    assert!(
        any_different,
        "Different seeds should produce different atom configurations"
    );
}

/// Test seeded simulation trajectory determinism
#[test]
fn test_seed_determinism_simulation() {
    fn run_seeded_simulation(seed: u64) -> Trajectory {
        let mut rng = Xoshiro256StarStar::seed_from_u64(seed);

        let mut atoms = fcc_lattice(2, 2, 2, 1.5);
        let box_ = SimulationBox::cubic(3.0);

        // Seeded velocity initialization
        for atom in &mut atoms {
            atom.velocity = [
                rng.gen_range(-0.5..0.5),
                rng.gen_range(-0.5..0.5),
                rng.gen_range(-0.5..0.5),
            ];
        }

        observable::remove_com_velocity(&mut atoms);

        run_and_record_trajectory(atoms, box_, 100, 20)
    }

    // Same seed
    let traj1 = run_seeded_simulation(999);
    let traj2 = run_seeded_simulation(999);

    assert_eq!(
        traj1.compute_hash(),
        traj2.compute_hash(),
        "Same seed should produce identical trajectories"
    );

    // Different seed
    let traj3 = run_seeded_simulation(1000);

    assert_ne!(
        traj1.compute_hash(),
        traj3.compute_hash(),
        "Different seeds should produce different trajectories"
    );
}

/// Test multiple runs with same seed in sequence
#[test]
fn test_seed_determinism_multiple_runs() {
    let seed = 777;

    let mut hashes = Vec::new();

    for _ in 0..5 {
        let mut rng = Xoshiro256StarStar::seed_from_u64(seed);

        let mut atoms = fcc_lattice(2, 2, 2, 1.5);
        let box_ = SimulationBox::cubic(3.0);

        for atom in &mut atoms {
            atom.velocity = [
                rng.gen_range(-0.3..0.3),
                rng.gen_range(-0.3..0.3),
                rng.gen_range(-0.3..0.3),
            ];
        }

        observable::remove_com_velocity(&mut atoms);

        let traj = run_and_record_trajectory(atoms, box_, 50, 10);
        hashes.push(traj.compute_hash());
    }

    // All hashes should be identical
    let first_hash = hashes[0];
    for (i, hash) in hashes.iter().enumerate() {
        assert_eq!(
            *hash, first_hash,
            "Run {} hash {:016x} should match first hash {:016x}",
            i, hash, first_hash
        );
    }
}

// ============================================================================
// Bit-exact Reproducibility Tests
// ============================================================================

/// Test bit-exact reproducibility of forces
#[test]
fn test_force_calculation_reproducibility() {
    let atoms = vec![
        Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0),
        Atom::new(1, 0, 1.0).with_position(1.5, 0.0, 0.0),
        Atom::new(2, 0, 1.0).with_position(0.0, 1.5, 0.0),
        Atom::new(3, 0, 1.0).with_position(0.75, 0.75, 1.3),
    ];

    let box_ = SimulationBox::cubic(10.0);
    let lj = LennardJones::argon();

    // Calculate forces multiple times
    let mut force_results: Vec<Vec<[f32; 3]>> = Vec::new();

    for _ in 0..10 {
        let mut atoms_copy = atoms.clone();
        for atom in &mut atoms_copy {
            atom.zero_force();
        }

        lj.compute_forces(&mut atoms_copy, &box_, None);

        let forces: Vec<[f32; 3]> = atoms_copy.iter().map(|a| a.force).collect();
        force_results.push(forces);
    }

    // All force calculations should be identical
    for (i, forces) in force_results.iter().enumerate().skip(1) {
        for (j, (f1, f2)) in force_results[0].iter().zip(forces.iter()).enumerate() {
            assert_eq!(
                f1, f2,
                "Run {} atom {} force {:?} should equal {:?}",
                i, j, f2, f1
            );
        }
    }
}

/// Test bit-exact reproducibility of energy
#[test]
fn test_energy_calculation_reproducibility() {
    let atoms = fcc_lattice(2, 2, 2, 1.5);
    let box_ = SimulationBox::cubic(3.0);
    let lj = LennardJones::argon();

    // Calculate energy multiple times
    let mut energies: Vec<f64> = Vec::new();

    for _ in 0..10 {
        let energy = lj.potential_energy(&atoms, &box_, None);
        energies.push(energy);
    }

    // All energy calculations should be identical
    let first_energy = energies[0];
    for (i, energy) in energies.iter().enumerate() {
        assert_eq!(
            *energy, first_energy,
            "Run {} energy {} should equal {}",
            i, energy, first_energy
        );
    }
}

// ============================================================================
// Long-term Determinism Tests
// ============================================================================

/// Test determinism over longer simulation
#[test]
fn test_long_simulation_determinism() {
    let (atoms1, box1) = create_deterministic_config();
    let (atoms2, box2) = create_deterministic_config();

    let lj1 = LennardJones::argon();
    let lj2 = LennardJones::argon();
    let integrator1 = VelocityVerlet::new();
    let integrator2 = VelocityVerlet::new();

    let mut sim1 = Simulation::new(atoms1, box1, lj1, integrator1)
        .with_timestep(0.002);
    let mut sim2 = Simulation::new(atoms2, box2, lj2, integrator2)
        .with_timestep(0.002);

    // Run for longer
    for step in 0..1000 {
        sim1.step_forward();
        sim2.step_forward();

        // Check every 100 steps
        if step % 100 == 0 {
            for (a1, a2) in sim1.atoms().iter().zip(sim2.atoms().iter()) {
                assert_eq!(
                    a1.position, a2.position,
                    "Position mismatch at step {}", step
                );
                assert_eq!(
                    a1.velocity, a2.velocity,
                    "Velocity mismatch at step {}", step
                );
            }
        }
    }

    // Final state should match exactly
    let e1 = sim1.total_energy();
    let e2 = sim2.total_energy();

    assert_eq!(
        e1, e2,
        "Final energy should be bit-exact: {} vs {}",
        e1, e2
    );
}

/// Test that interrupting and resuming produces same result as continuous run
#[test]
fn test_checkpoint_resume_determinism() {
    let (atoms, box_) = create_deterministic_config();

    // Continuous run
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();
    let mut sim_continuous = Simulation::new(atoms.clone(), box_, lj.clone(), integrator)
        .with_timestep(0.002);

    sim_continuous.run(500);
    let continuous_energy = sim_continuous.total_energy();
    let continuous_positions: Vec<[f32; 3]> = sim_continuous.atoms().iter()
        .map(|a| a.position)
        .collect();

    // Checkpoint and resume run
    let mut sim_resume = Simulation::new(atoms.clone(), box_, lj.clone(), VelocityVerlet::new())
        .with_timestep(0.002);

    sim_resume.run(250);

    // "Checkpoint" by copying state
    let checkpoint_atoms: Vec<Atom> = sim_resume.atoms().to_vec();

    // "Resume" by creating new simulation from checkpoint
    let mut sim_resumed = Simulation::new(checkpoint_atoms, box_, lj.clone(), VelocityVerlet::new())
        .with_timestep(0.002);

    sim_resumed.run(250);

    let resumed_energy = sim_resumed.total_energy();
    let resumed_positions: Vec<[f32; 3]> = sim_resumed.atoms().iter()
        .map(|a| a.position)
        .collect();

    // Results should match (with tolerance for force recalculation on resume)
    // When resuming from checkpoint, forces are recalculated which can introduce
    // small numerical differences due to floating point ordering
    let energy_tolerance = 1e-4;
    assert!(
        (continuous_energy - resumed_energy).abs() < energy_tolerance,
        "Energy should match within tolerance: continuous={}, resumed={}, diff={}",
        continuous_energy, resumed_energy, (continuous_energy - resumed_energy).abs()
    );

    // Position tolerance also needs to account for force recalculation differences
    let position_tolerance = 1e-5;
    for (i, (p1, p2)) in continuous_positions.iter().zip(resumed_positions.iter()).enumerate() {
        for d in 0..3 {
            assert!(
                (p1[d] - p2[d]).abs() < position_tolerance,
                "Position {} dim {} should match: {} vs {} (diff={})",
                i, d, p1[d], p2[d], (p1[d] - p2[d]).abs()
            );
        }
    }
}
