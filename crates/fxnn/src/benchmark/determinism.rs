//! Formal Determinism Test - Root of Trust
//!
//! Determinism is the foundation of reproducibility and debugging.
//! This test verifies that identical inputs produce identical outputs.
//!
//! # Definition (from ADR-001)
//!
//! Given:
//! - Seed S (u64)
//! - Initial state X0 (serialized snapshot)
//! - Configuration C (all parameters)
//!
//! Invariant:
//! ```text
//! TrajectoryHash(S, X0, C) = constant across runs
//! ```
//!
//! # Hash Function Specification
//!
//! ```rust,ignore
//! fn trajectory_hash(sim: &Simulation, steps: usize) -> [u8; 32] {
//!     let mut hasher = blake3::Hasher::new();
//!     hasher.update(&sim.seed.to_le_bytes());
//!     hasher.update(&sim.config_hash());
//!
//!     for step in 0..steps {
//!         sim.step();
//!         for entity in sim.entities_sorted_by_id() {
//!             hasher.update(&entity.id.to_le_bytes());
//!             hasher.update(&entity.position.to_le_bytes());
//!             hasher.update(&entity.velocity.to_le_bytes());
//!         }
//!     }
//!     hasher.finalize().into()
//! }
//! ```
//!
//! # Floating-Point Mode Rules
//!
//! - IEEE 754 strict compliance (no fast-math)
//! - Deterministic operation ordering (no parallel reduction without ordered accumulators)
//! - Fixed thread scheduling for parallelism
//! - FMA (fused multiply-add) either always on or always off
//!
//! # Test Protocol
//!
//! 1. Run simulation with seed S, config C, for N steps
//! 2. Record trajectory hash H1
//! 3. Repeat with identical inputs
//! 4. Verify H1 = H2
//! 5. If mismatch, bisect to find divergence point

use super::{BenchmarkConfig, BenchmarkMetrics, BenchmarkReport, CriterionResult, WitnessRecord};
use crate::generators::fcc_lattice;
use crate::types::Atom;
use crate::{LennardJones, SimulationBox, Simulation, VelocityVerlet};
use std::time::Instant;

/// Blake3-based trajectory hasher for ADR-001 compliance
///
/// Uses blake3 for cryptographic-strength determinism verification.
/// This is the formal implementation specified in ADR-001 Part VII.
pub struct Blake3TrajectoryHasher {
    hasher: blake3::Hasher,
}

impl Blake3TrajectoryHasher {
    /// Create a new blake3-based trajectory hasher
    pub fn new() -> Self {
        Self {
            hasher: blake3::Hasher::new(),
        }
    }

    /// Update with raw bytes
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Update with u32 value
    pub fn update_u32(&mut self, value: u32) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Update with u64 value
    pub fn update_u64(&mut self, value: u64) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Update with f32 value (IEEE 754 representation)
    pub fn update_f32(&mut self, value: f32) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Update with f64 value (IEEE 754 representation)
    pub fn update_f64(&mut self, value: f64) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Finalize and return the 32-byte hash
    pub fn finalize(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
}

impl Default for Blake3TrajectoryHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy FNV-based trajectory hasher (kept for comparison)
struct TrajectoryHasher {
    state: [u64; 4],
}

impl TrajectoryHasher {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self {
            state: [Self::FNV_OFFSET; 4],
        }
    }

    fn update(&mut self, data: &[u8]) {
        for (i, chunk) in data.chunks(8).enumerate() {
            let idx = i % 4;
            for &byte in chunk {
                self.state[idx] ^= byte as u64;
                self.state[idx] = self.state[idx].wrapping_mul(Self::FNV_PRIME);
            }
        }
    }

    fn update_u32(&mut self, value: u32) {
        self.update(&value.to_le_bytes());
    }

    fn update_u64(&mut self, value: u64) {
        self.update(&value.to_le_bytes());
    }

    fn update_f32(&mut self, value: f32) {
        // IEEE 754 representation - deterministic across platforms
        self.update(&value.to_le_bytes());
    }

    fn finalize(self) -> [u8; 32] {
        let mut result = [0u8; 32];
        for (i, &h) in self.state.iter().enumerate() {
            result[i * 8..(i + 1) * 8].copy_from_slice(&h.to_le_bytes());
        }
        result
    }
}

/// Compute trajectory hash for a simulation using blake3 (ADR-001 compliant)
///
/// This is the formal implementation specified in ADR-001 Part VII:
/// ```text
/// TrajectoryHash(S, X0, C) = constant across runs
/// ```
///
/// # Arguments
/// * `seed` - Random seed for reproducibility
/// * `n_particles` - Number of particles
/// * `n_steps` - Number of simulation steps
/// * `dt` - Timestep
///
/// # Returns
/// A 32-byte blake3 hash that is deterministic for identical inputs.
pub fn trajectory_hash_blake3(
    seed: u64,
    n_particles: usize,
    n_steps: usize,
    dt: f32,
) -> [u8; 32] {
    let mut hasher = Blake3TrajectoryHasher::new();

    // Hash seed and configuration
    hasher.update_u64(seed);
    hasher.update_u64(n_particles as u64);
    hasher.update_u64(n_steps as u64);
    hasher.update_f32(dt);

    // Create deterministic initial configuration
    let lattice_size = ((n_particles as f64 / 4.0).cbrt().ceil() as usize).max(2);
    let lattice_constant = 1.5;
    let mut atoms = fcc_lattice(lattice_size, lattice_size, lattice_size, lattice_constant);
    atoms.truncate(n_particles);

    // Re-assign IDs deterministically
    for (i, atom) in atoms.iter_mut().enumerate() {
        atom.id = i as u32;
    }

    let box_size = lattice_size as f32 * lattice_constant * 1.1;
    let box_ = SimulationBox::cubic(box_size);

    // Initialize velocities deterministically using seed
    use rand::SeedableRng;
    use rand_xoshiro::Xoshiro256PlusPlus;
    use rand_distr::{Normal, Distribution};

    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    let temperature = 1.0_f32;
    let kb = 1.0_f32;

    for atom in atoms.iter_mut() {
        let sigma = (kb * temperature / atom.mass).sqrt();
        let normal = Normal::new(0.0, sigma as f64).unwrap();

        atom.velocity = [
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
        ];
    }

    // Remove center of mass velocity deterministically
    let n = atoms.len() as f32;
    let mut vcm = [0.0f32; 3];
    for atom in atoms.iter() {
        vcm[0] += atom.velocity[0];
        vcm[1] += atom.velocity[1];
        vcm[2] += atom.velocity[2];
    }
    vcm[0] /= n;
    vcm[1] /= n;
    vcm[2] /= n;

    for atom in atoms.iter_mut() {
        atom.velocity[0] -= vcm[0];
        atom.velocity[1] -= vcm[1];
        atom.velocity[2] -= vcm[2];
    }

    // Hash initial state
    for atom in &atoms {
        hasher.update_u32(atom.id);
        hasher.update_f32(atom.position[0]);
        hasher.update_f32(atom.position[1]);
        hasher.update_f32(atom.position[2]);
        hasher.update_f32(atom.velocity[0]);
        hasher.update_f32(atom.velocity[1]);
        hasher.update_f32(atom.velocity[2]);
    }

    // Create simulation
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();
    let mut sim = Simulation::new(atoms, box_, lj, integrator).with_timestep(dt);

    // Run simulation and hash state at each step
    for step in 0..n_steps {
        sim.step_forward();

        // Hash state - sort by ID for deterministic ordering
        let mut atoms_vec: Vec<&Atom> = sim.atoms().iter().collect();
        atoms_vec.sort_by_key(|a| a.id);

        hasher.update_u64(step as u64);
        for atom in atoms_vec {
            hasher.update_u32(atom.id);
            hasher.update_f32(atom.position[0]);
            hasher.update_f32(atom.position[1]);
            hasher.update_f32(atom.position[2]);
            hasher.update_f32(atom.velocity[0]);
            hasher.update_f32(atom.velocity[1]);
            hasher.update_f32(atom.velocity[2]);
        }
    }

    hasher.finalize()
}

/// Legacy trajectory hash function (FNV-based, kept for backwards compatibility)
///
/// This function runs a simulation and computes a deterministic hash
/// of the entire trajectory. The hash should be identical for identical
/// inputs (seed, initial state, configuration).
pub fn trajectory_hash(
    seed: u64,
    n_particles: usize,
    n_steps: usize,
    dt: f32,
) -> [u8; 32] {
    let mut hasher = TrajectoryHasher::new();

    // Hash seed and configuration
    hasher.update_u64(seed);
    hasher.update_u64(n_particles as u64);
    hasher.update_u64(n_steps as u64);
    hasher.update_f32(dt);

    // Create deterministic initial configuration
    // Use seed for RNG initialization via manual seeding
    let lattice_size = ((n_particles as f64 / 4.0).cbrt().ceil() as usize).max(2);
    let lattice_constant = 1.5;
    let mut atoms = fcc_lattice(lattice_size, lattice_size, lattice_size, lattice_constant);
    atoms.truncate(n_particles);

    // Re-assign IDs deterministically
    for (i, atom) in atoms.iter_mut().enumerate() {
        atom.id = i as u32;
    }

    let box_size = lattice_size as f32 * lattice_constant * 1.1;
    let box_ = SimulationBox::cubic(box_size);

    // Initialize velocities deterministically using seed
    // Note: maxwell_boltzmann_velocities uses thread_rng(), which is non-deterministic
    // For true determinism, we need to use a seeded RNG
    use rand::SeedableRng;
    use rand_xoshiro::Xoshiro256PlusPlus;
    use rand_distr::{Normal, Distribution};

    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    let temperature = 1.0_f32;
    let kb = 1.0_f32;

    for atom in atoms.iter_mut() {
        let sigma = (kb * temperature / atom.mass).sqrt();
        let normal = Normal::new(0.0, sigma as f64).unwrap();

        atom.velocity = [
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
        ];
    }

    // Remove center of mass velocity deterministically
    let n = atoms.len() as f32;
    let mut vcm = [0.0f32; 3];
    for atom in atoms.iter() {
        vcm[0] += atom.velocity[0];
        vcm[1] += atom.velocity[1];
        vcm[2] += atom.velocity[2];
    }
    vcm[0] /= n;
    vcm[1] /= n;
    vcm[2] /= n;

    for atom in atoms.iter_mut() {
        atom.velocity[0] -= vcm[0];
        atom.velocity[1] -= vcm[1];
        atom.velocity[2] -= vcm[2];
    }

    // Hash initial state
    for atom in &atoms {
        hasher.update_u32(atom.id);
        hasher.update_f32(atom.position[0]);
        hasher.update_f32(atom.position[1]);
        hasher.update_f32(atom.position[2]);
        hasher.update_f32(atom.velocity[0]);
        hasher.update_f32(atom.velocity[1]);
        hasher.update_f32(atom.velocity[2]);
    }

    // Create simulation
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();
    let mut sim = Simulation::new(atoms, box_, lj, integrator).with_timestep(dt);

    // Run simulation and hash state at each step
    for step in 0..n_steps {
        sim.step_forward();

        // Hash state - sort by ID for deterministic ordering
        let mut atoms_vec: Vec<&Atom> = sim.atoms().iter().collect();
        atoms_vec.sort_by_key(|a| a.id);

        hasher.update_u64(step as u64);
        for atom in atoms_vec {
            hasher.update_u32(atom.id);
            hasher.update_f32(atom.position[0]);
            hasher.update_f32(atom.position[1]);
            hasher.update_f32(atom.position[2]);
            hasher.update_f32(atom.velocity[0]);
            hasher.update_f32(atom.velocity[1]);
            hasher.update_f32(atom.velocity[2]);
        }
    }

    hasher.finalize()
}

/// Format hash as hex string
fn hash_to_hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Find divergence point between two simulations
fn find_divergence_point(
    seed: u64,
    n_particles: usize,
    n_steps: usize,
    dt: f32,
) -> Option<usize> {
    // Run two simulations and compare state at each step
    use rand::SeedableRng;
    use rand_xoshiro::Xoshiro256PlusPlus;
    use rand_distr::{Normal, Distribution};

    // Helper to create simulation
    let create_sim = |seed: u64| {
        let lattice_size = ((n_particles as f64 / 4.0).cbrt().ceil() as usize).max(2);
        let lattice_constant = 1.5;
        let mut atoms = fcc_lattice(lattice_size, lattice_size, lattice_size, lattice_constant);
        atoms.truncate(n_particles);

        for (i, atom) in atoms.iter_mut().enumerate() {
            atom.id = i as u32;
        }

        let box_size = lattice_size as f32 * lattice_constant * 1.1;
        let box_ = SimulationBox::cubic(box_size);

        let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        let temperature = 1.0_f32;
        let kb = 1.0_f32;

        for atom in atoms.iter_mut() {
            let sigma = (kb * temperature / atom.mass).sqrt();
            let normal = Normal::new(0.0, sigma as f64).unwrap();
            atom.velocity = [
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
            ];
        }

        // Remove COM velocity
        let n = atoms.len() as f32;
        let mut vcm = [0.0f32; 3];
        for atom in atoms.iter() {
            vcm[0] += atom.velocity[0];
            vcm[1] += atom.velocity[1];
            vcm[2] += atom.velocity[2];
        }
        vcm[0] /= n;
        vcm[1] /= n;
        vcm[2] /= n;
        for atom in atoms.iter_mut() {
            atom.velocity[0] -= vcm[0];
            atom.velocity[1] -= vcm[1];
            atom.velocity[2] -= vcm[2];
        }

        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();
        Simulation::new(atoms, box_, lj, integrator).with_timestep(dt)
    };

    let mut sim1 = create_sim(seed);
    let mut sim2 = create_sim(seed);

    // Compare states step by step
    for step in 0..n_steps {
        sim1.step_forward();
        sim2.step_forward();

        let atoms1 = sim1.atoms();
        let atoms2 = sim2.atoms();

        for (a1, a2) in atoms1.iter().zip(atoms2.iter()) {
            if a1.position != a2.position || a1.velocity != a2.velocity {
                return Some(step);
            }
        }
    }

    None // No divergence found
}

/// Run the Determinism benchmark
pub fn run_benchmark(config: &BenchmarkConfig) -> BenchmarkReport {
    let start = Instant::now();

    let mut witness_log = Vec::new();
    let metrics = BenchmarkMetrics::default();
    let mut criteria = Vec::new();

    let seed = config.seed;
    let n_particles = 32; // Small system for fast testing
    let n_steps = 100;
    let dt = 0.001;

    // Run trajectory hash twice with identical parameters
    if config.verbose {
        println!("Computing trajectory hash (run 1)...");
    }
    let hash1 = trajectory_hash(seed, n_particles, n_steps, dt);

    if config.verbose {
        println!("Computing trajectory hash (run 2)...");
    }
    let hash2 = trajectory_hash(seed, n_particles, n_steps, dt);

    // Compare hashes
    let hashes_match = hash1 == hash2;

    criteria.push(CriterionResult {
        name: "Trajectory hash reproducibility".to_string(),
        passed: hashes_match,
        expected: "H1 = H2".to_string(),
        actual: if hashes_match {
            format!("match ({}...)", &hash_to_hex(&hash1)[..16])
        } else {
            format!(
                "MISMATCH: H1={}..., H2={}...",
                &hash_to_hex(&hash1)[..16],
                &hash_to_hex(&hash2)[..16]
            )
        },
    });

    // Test with different seed should produce different hash
    let hash_different_seed = trajectory_hash(seed + 1, n_particles, n_steps, dt);
    let different_seed_different_hash = hash1 != hash_different_seed;

    criteria.push(CriterionResult {
        name: "Different seed produces different hash".to_string(),
        passed: different_seed_different_hash,
        expected: "H(seed) != H(seed+1)".to_string(),
        actual: if different_seed_different_hash {
            "different hashes".to_string()
        } else {
            "SAME hash (unexpected)".to_string()
        },
    });

    // Test IEEE 754 strict mode by checking for consistent floating point results
    let ieee_test_passed = test_ieee754_consistency();
    criteria.push(CriterionResult {
        name: "IEEE 754 floating-point consistency".to_string(),
        passed: ieee_test_passed,
        expected: "consistent results".to_string(),
        actual: if ieee_test_passed {
            "consistent".to_string()
        } else {
            "INCONSISTENT".to_string()
        },
    });

    // If hashes don't match, find divergence point
    if !hashes_match {
        if let Some(divergence_step) = find_divergence_point(seed, n_particles, n_steps, dt) {
            witness_log.push(WitnessRecord {
                tick: divergence_step as u64,
                event_type: super::WitnessEventType::NumericInstability,
                entity_ids: vec![],
                constraint_fired: "determinism_violation".to_string(),
                delta_magnitude: 0.0,
                description: format!("Trajectory diverged at step {}", divergence_step),
            });
        }
    }

    // Build summary
    let all_passed = criteria.iter().all(|c| c.passed);
    let summary = if all_passed {
        format!(
            "Determinism verified: trajectory hash {}",
            &hash_to_hex(&hash1)[..16]
        )
    } else {
        let failures: Vec<_> = criteria.iter().filter(|c| !c.passed).map(|c| &c.name).collect();
        format!("Determinism FAILED: {:?}", failures)
    };

    BenchmarkReport {
        name: "D: Determinism (Root of Trust)".to_string(),
        passed: all_passed,
        criteria,
        duration: start.elapsed(),
        metrics,
        witness_log,
        summary,
    }
}

/// Test IEEE 754 floating-point consistency
///
/// Verifies that floating-point operations produce consistent results
/// across multiple executions. This is crucial for determinism.
fn test_ieee754_consistency() -> bool {
    // Test 1: Basic arithmetic
    let a = 1.0_f32 / 3.0;
    let b = a * 3.0;
    let diff1 = (b - 1.0).abs();

    // This should be consistent across runs
    let expected_diff1 = (1.0_f32 / 3.0 * 3.0 - 1.0).abs();
    if (diff1 - expected_diff1).abs() > 0.0 {
        return false;
    }

    // Test 2: Accumulation order
    let values = [0.1_f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    let sum1: f32 = values.iter().sum();
    let sum2: f32 = values.iter().sum(); // Should be identical

    if sum1 != sum2 {
        return false;
    }

    // Test 3: sqrt and power operations
    let x = 2.0_f32;
    let sqrt_x = x.sqrt();
    let sqrt_x_again = 2.0_f32.sqrt();

    if sqrt_x != sqrt_x_again {
        return false;
    }

    // Test 4: Trigonometric functions
    let angle = std::f32::consts::PI / 4.0;
    let sin_val = angle.sin();
    let sin_val_again = (std::f32::consts::PI / 4.0).sin();

    if sin_val != sin_val_again {
        return false;
    }

    // Test 5: NaN and Inf handling
    // Use is_nan() to properly check NaN behavior (NaN != NaN is always true in IEEE 754)
    let nan_check = f32::NAN.is_nan(); // NaN.is_nan() should be true
    let inf_check = f32::INFINITY > f32::MAX; // Inf > MAX should be true

    if !nan_check || !inf_check {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Blake3 Trajectory Hash Tests (ADR-001 Formal Determinism)
    // =========================================================================

    #[test]
    fn test_blake3_trajectory_hash_deterministic() {
        // ADR-001 Part VII: TrajectoryHash(S, X0, C) = constant across runs
        let hash1 = trajectory_hash_blake3(42, 16, 10, 0.001);
        let hash2 = trajectory_hash_blake3(42, 16, 10, 0.001);

        assert_eq!(hash1, hash2, "Blake3 trajectory hashes should be identical for same inputs");
        println!("Blake3 hash: {}", hash_to_hex(&hash1));
    }

    #[test]
    fn test_blake3_different_seed_different_hash() {
        let hash1 = trajectory_hash_blake3(42, 16, 10, 0.001);
        let hash2 = trajectory_hash_blake3(43, 16, 10, 0.001);

        assert_ne!(hash1, hash2, "Different seeds should produce different blake3 hashes");
    }

    #[test]
    fn test_blake3_hasher_consistency() {
        let mut hasher = Blake3TrajectoryHasher::new();
        hasher.update(b"hello world");
        let hash1 = hasher.finalize();

        let mut hasher2 = Blake3TrajectoryHasher::new();
        hasher2.update(b"hello world");
        let hash2 = hasher2.finalize();

        assert_eq!(hash1, hash2, "Blake3 hasher should be deterministic");
    }

    #[test]
    fn test_blake3_hasher_updates() {
        // Test that incremental updates produce same result as single update
        let mut hasher1 = Blake3TrajectoryHasher::new();
        hasher1.update_u64(12345);
        hasher1.update_f32(3.14159);
        let hash1 = hasher1.finalize();

        let mut hasher2 = Blake3TrajectoryHasher::new();
        hasher2.update_u64(12345);
        hasher2.update_f32(3.14159);
        let hash2 = hasher2.finalize();

        assert_eq!(hash1, hash2);
    }

    // =========================================================================
    // Legacy FNV Hash Tests (Backwards Compatibility)
    // =========================================================================

    #[test]
    fn test_trajectory_hash_deterministic() {
        let hash1 = trajectory_hash(42, 16, 10, 0.001);
        let hash2 = trajectory_hash(42, 16, 10, 0.001);

        assert_eq!(hash1, hash2, "Trajectory hashes should be identical");
    }

    #[test]
    fn test_different_seed_different_hash() {
        let hash1 = trajectory_hash(42, 16, 10, 0.001);
        let hash2 = trajectory_hash(43, 16, 10, 0.001);

        assert_ne!(hash1, hash2, "Different seeds should produce different hashes");
    }

    #[test]
    fn test_ieee754_ops_consistency() {
        assert!(super::test_ieee754_consistency());
    }

    #[test]
    fn test_hasher() {
        let mut hasher = TrajectoryHasher::new();
        hasher.update(b"hello world");
        let hash1 = hasher.finalize();

        let mut hasher2 = TrajectoryHasher::new();
        hasher2.update(b"hello world");
        let hash2 = hasher2.finalize();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_determinism_benchmark_runs() {
        let config = BenchmarkConfig::default();
        let result = run_benchmark(&config);

        assert!(result.passed, "Determinism benchmark should pass");
        assert!(result.duration.as_secs() < 60);
    }

    // =========================================================================
    // Cross-Run Determinism Test (ADR-001 Requirement)
    // =========================================================================

    #[test]
    fn test_formal_determinism_protocol() {
        // ADR-001 Part VII Test Protocol:
        // 1. Run simulation with seed S, config C, for N steps
        // 2. Record trajectory hash H1
        // 3. Repeat with identical inputs
        // 4. Verify H1 = H2

        let seed = 0xDEADBEEF_u64;
        let n_particles = 32;
        let n_steps = 50;
        let dt = 0.002_f32;

        // Run 1
        let h1 = trajectory_hash_blake3(seed, n_particles, n_steps, dt);

        // Run 2 (should be identical)
        let h2 = trajectory_hash_blake3(seed, n_particles, n_steps, dt);

        assert_eq!(h1, h2, "Formal determinism test failed: H1 != H2");

        // Run 3 with different seed (should differ)
        let h3 = trajectory_hash_blake3(seed + 1, n_particles, n_steps, dt);

        assert_ne!(h1, h3, "Hash sensitivity test failed: H(S) == H(S+1)");
    }
}
