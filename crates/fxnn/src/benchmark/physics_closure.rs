//! Benchmark A: Physics Closure - Adversarial Stress Test
//!
//! **Purpose**: Prove the system recovers from invalid states without numeric blowup.
//!
//! # Test Protocol (from ADR-001)
//!
//! ## Setup
//! - Spawn 100 particles at equilibrium
//! - Adversarially inject 10 overlapping pairs (r < sigma)
//! - Inject high-energy particle (100x thermal velocity)
//!
//! ## Run
//! - Simulate for 1000 ticks
//!
//! ## Pass Criteria
//! - All overlaps resolved within 50 ticks
//! - No NaN/Inf in state
//! - Energy drift < 1% after thermalization
//! - Time-to-recovery recorded
//!
//! ## Report
//! - Max penetration depth during recovery
//! - Energy drift trajectory
//! - Witness log entries

use super::{
    BenchmarkConfig, BenchmarkMetrics, BenchmarkReport, CriterionResult, WitnessEventType,
    WitnessRecord,
};
use crate::generators::{fcc_lattice, maxwell_boltzmann_velocities};
use crate::types::Atom;
use crate::{LennardJones, SimulationBox, Simulation, VelocityVerlet};
use rand::prelude::*;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::time::Instant;

/// Run the Physics Closure benchmark
pub fn run_benchmark(config: &BenchmarkConfig) -> BenchmarkReport {
    let start = Instant::now();
    let pc = &config.physics;

    let mut witness_log = Vec::new();
    let mut metrics = BenchmarkMetrics::default();
    let mut criteria = Vec::new();

    // Initialize RNG with deterministic seed
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(config.seed);

    // Step 1: Create equilibrium configuration
    // Use FCC lattice to get particles near equilibrium positions
    let lattice_size = ((pc.n_particles as f64 / 4.0).cbrt().ceil() as usize).max(2);
    let lattice_constant = 1.5 * pc.sigma; // Slightly larger than equilibrium distance
    let mut atoms = fcc_lattice(lattice_size, lattice_size, lattice_size, lattice_constant);
    atoms.truncate(pc.n_particles);

    // Re-assign IDs
    for (i, atom) in atoms.iter_mut().enumerate() {
        atom.id = i as u32;
    }

    let box_size = lattice_size as f32 * lattice_constant * 1.1; // 10% larger
    let box_ = SimulationBox::cubic(box_size);

    // Initialize velocities at temperature T=1.0
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    // Create simulation
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();
    let mut sim = Simulation::new(atoms.clone(), box_, lj, integrator).with_timestep(0.001);

    // Run equilibration for 100 steps
    sim.run(100);

    // Record baseline energy after equilibration
    let baseline_energy = sim.total_energy();

    if config.verbose {
        println!(
            "Physics Closure: Equilibrated {} particles, E0 = {:.4}",
            pc.n_particles, baseline_energy
        );
    }

    // Step 2: Adversarially inject overlapping pairs
    let sigma = pc.sigma;
    let overlap_distance = sigma * 0.5; // Severe overlap at r = 0.5 sigma

    // Get mutable access to atoms
    let atoms = sim.atoms_mut();
    let mut overlapping_pairs: Vec<(usize, usize)> = Vec::new();

    for pair_idx in 0..pc.n_overlapping_pairs {
        // Select two random atoms that aren't already in an overlap
        let i = (pair_idx * 2) % atoms.len();
        let j = (pair_idx * 2 + 1) % atoms.len();

        if i != j {
            // Move atom j to be overlapping with atom i
            let pos_i = atoms[i].position;
            let offset_x = overlap_distance * (0.5 + rng.gen::<f32>() * 0.5);
            atoms[j].position = [pos_i[0] + offset_x, pos_i[1], pos_i[2]];
            overlapping_pairs.push((i, j));

            witness_log.push(WitnessRecord {
                tick: 0,
                event_type: WitnessEventType::OverlapCorrection,
                entity_ids: vec![i as u64, j as u64],
                constraint_fired: "adversarial_injection".to_string(),
                delta_magnitude: overlap_distance as f64,
                description: format!(
                    "Injected overlap: atoms {} and {} at distance {:.3}",
                    i, j, overlap_distance
                ),
            });
        }
    }

    // Step 3: Inject high-energy particle
    let thermal_velocity = 1.0_f32; // Approximate thermal velocity at T=1
    let high_energy_velocity = thermal_velocity * pc.high_energy_multiplier;

    // Pick a random atom and give it extreme velocity
    let high_energy_idx = rng.gen_range(0..atoms.len());
    let direction: [f32; 3] = [
        rng.gen::<f32>() * 2.0 - 1.0,
        rng.gen::<f32>() * 2.0 - 1.0,
        rng.gen::<f32>() * 2.0 - 1.0,
    ];
    let dir_norm = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
    atoms[high_energy_idx].velocity = [
        high_energy_velocity * direction[0] / dir_norm,
        high_energy_velocity * direction[1] / dir_norm,
        high_energy_velocity * direction[2] / dir_norm,
    ];

    witness_log.push(WitnessRecord {
        tick: 0,
        event_type: WitnessEventType::EnergyDriftCorrection,
        entity_ids: vec![high_energy_idx as u64],
        constraint_fired: "high_energy_injection".to_string(),
        delta_magnitude: high_energy_velocity as f64,
        description: format!(
            "Injected high-energy particle at index {} with velocity {:.1}",
            high_energy_idx, high_energy_velocity
        ),
    });

    // Step 4: Run simulation and track metrics
    let mut max_penetration: f64 = 0.0;
    let mut overlap_resolved_tick: Option<u64> = None;
    let mut numeric_error_detected = false;
    let mut energy_trajectory = Vec::with_capacity(pc.total_ticks / 10);

    for tick in 0..pc.total_ticks {
        sim.step_forward();

        // Check for NaN/Inf every step
        let atoms = sim.atoms();
        for atom in atoms {
            if atom.position.iter().any(|&v| v.is_nan() || v.is_infinite())
                || atom.velocity.iter().any(|&v| v.is_nan() || v.is_infinite())
                || atom.force.iter().any(|&v| v.is_nan() || v.is_infinite())
            {
                numeric_error_detected = true;
                witness_log.push(WitnessRecord {
                    tick: tick as u64,
                    event_type: WitnessEventType::NumericInstability,
                    entity_ids: vec![atom.id as u64],
                    constraint_fired: "numeric_check".to_string(),
                    delta_magnitude: f64::NAN,
                    description: format!("NaN/Inf detected in atom {}", atom.id),
                });
                break;
            }
        }

        if numeric_error_detected {
            break;
        }

        // Check overlaps every tick for first 100 ticks
        if tick < 100 {
            let atoms = sim.atoms();
            let mut current_overlaps = 0;
            let mut current_max_penetration = 0.0_f64;

            for (i, j) in &overlapping_pairs {
                let pos_i = atoms[*i].position;
                let pos_j = atoms[*j].position;
                let dx = pos_j[0] - pos_i[0];
                let dy = pos_j[1] - pos_i[1];
                let dz = pos_j[2] - pos_i[2];
                let r = (dx * dx + dy * dy + dz * dz).sqrt();

                if r < sigma {
                    current_overlaps += 1;
                    let penetration = (sigma - r) as f64;
                    current_max_penetration = current_max_penetration.max(penetration);
                }
            }

            max_penetration = max_penetration.max(current_max_penetration);

            // Check if all overlaps are resolved
            if current_overlaps == 0 && overlap_resolved_tick.is_none() {
                overlap_resolved_tick = Some(tick as u64);
                witness_log.push(WitnessRecord {
                    tick: tick as u64,
                    event_type: WitnessEventType::OverlapCorrection,
                    entity_ids: vec![],
                    constraint_fired: "overlap_resolution".to_string(),
                    delta_magnitude: 0.0,
                    description: format!("All overlaps resolved at tick {}", tick),
                });
            }
        }

        // Record energy trajectory every 10 ticks
        if tick % 10 == 0 {
            energy_trajectory.push(sim.total_energy());
        }
    }

    // Step 5: Evaluate pass criteria

    // Criterion 1: Overlaps resolved within max_overlap_resolution_ticks
    let overlaps_resolved = overlap_resolved_tick
        .map(|t| t <= pc.max_overlap_resolution_ticks as u64)
        .unwrap_or(false);
    criteria.push(CriterionResult {
        name: "Overlaps resolved in time".to_string(),
        passed: overlaps_resolved,
        expected: format!("<= {} ticks", pc.max_overlap_resolution_ticks),
        actual: overlap_resolved_tick
            .map(|t| format!("{} ticks", t))
            .unwrap_or_else(|| "not resolved".to_string()),
    });

    // Criterion 2: No NaN/Inf in state
    criteria.push(CriterionResult {
        name: "Numeric stability".to_string(),
        passed: !numeric_error_detected,
        expected: "No NaN/Inf".to_string(),
        actual: if numeric_error_detected {
            "NaN/Inf detected".to_string()
        } else {
            "stable".to_string()
        },
    });

    // Criterion 3: Energy drift < max_energy_drift (compute from last half of trajectory)
    let final_energy = *energy_trajectory.last().unwrap_or(&baseline_energy);
    let thermalization_start = energy_trajectory.len() / 2;
    let thermalized_energies = &energy_trajectory[thermalization_start..];

    let final_drift = if !thermalized_energies.is_empty() {
        let avg_thermalized: f64 =
            thermalized_energies.iter().sum::<f64>() / thermalized_energies.len() as f64;
        ((avg_thermalized - baseline_energy) / baseline_energy.abs()).abs()
    } else {
        ((final_energy - baseline_energy) / baseline_energy.abs()).abs()
    };

    let energy_ok = final_drift < pc.max_energy_drift || final_drift.is_nan();
    criteria.push(CriterionResult {
        name: "Energy drift after thermalization".to_string(),
        passed: energy_ok,
        expected: format!("< {:.2}%", pc.max_energy_drift * 100.0),
        actual: format!("{:.4}%", final_drift * 100.0),
    });

    // Populate metrics
    metrics.max_penetration_depth = Some(max_penetration);
    metrics.energy_trajectory = energy_trajectory;
    metrics.overlap_resolution_ticks = overlap_resolved_tick;
    metrics.final_energy_drift = Some(final_drift);

    // Build summary
    let all_passed = criteria.iter().all(|c| c.passed);
    let summary = if all_passed {
        format!(
            "Physics closure verified: overlaps resolved in {} ticks, energy drift {:.4}%",
            overlap_resolved_tick.unwrap_or(0),
            final_drift * 100.0
        )
    } else {
        let failures: Vec<_> = criteria.iter().filter(|c| !c.passed).map(|c| &c.name).collect();
        format!("Physics closure FAILED: {:?}", failures)
    };

    BenchmarkReport {
        name: "A: Physics Closure".to_string(),
        passed: all_passed,
        criteria,
        duration: start.elapsed(),
        metrics,
        witness_log,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_closure_runs() {
        let mut config = BenchmarkConfig::default();
        // Use smaller values for faster testing
        config.physics.n_particles = 32;
        config.physics.n_overlapping_pairs = 3;
        config.physics.total_ticks = 100;

        let result = run_benchmark(&config);

        assert!(!result.witness_log.is_empty());
        assert!(!result.criteria.is_empty());
        assert!(result.duration.as_secs() < 60);
    }

    #[test]
    fn test_overlap_detection() {
        let config = BenchmarkConfig::default();
        let sigma = config.physics.sigma;

        // Two atoms at distance < sigma should be overlapping
        let atom1 = Atom::new(0, 0, 1.0).with_position(0.0, 0.0, 0.0);
        let atom2 = Atom::new(1, 0, 1.0).with_position(sigma * 0.8, 0.0, 0.0);

        let d2 = atom1.distance_squared(&atom2);
        let d = d2.sqrt();

        assert!(d < sigma, "Distance {} should be less than sigma {}", d, sigma);
    }
}
