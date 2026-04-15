//! Integration Tests
//!
//! End-to-end integration tests for the FXNN molecular dynamics engine:
//! - Full simulation loop
//! - Multi-agent interaction
//!
//! These tests verify that all components work together correctly.

use fxnn::{
    Simulation, SimulationBox, LennardJones, Coulomb, CompositeForceField,
    VelocityVerlet, Langevin,
    generators::{fcc_lattice, maxwell_boltzmann_velocities, random_atoms},
    types::{Atom, Topology},
    observable,
    neighbor::{VerletList, CellList, NeighborSearch},
};
use proptest::prelude::*;

// ============================================================================
// Full Simulation Loop Tests
// ============================================================================

/// Test complete simulation workflow
#[test]
fn test_full_simulation_loop() {
    // Step 1: Create initial configuration
    let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    let box_ = SimulationBox::cubic(4.5);

    // Step 2: Initialize velocities
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
    observable::remove_com_velocity(&mut atoms);

    // Step 3: Create force field
    let lj = LennardJones::argon();

    // Step 4: Create integrator
    let integrator = VelocityVerlet::new();

    // Step 5: Create simulation
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    // Step 6: Record initial state
    let initial_energy = sim.total_energy();
    let initial_temp = sim.temperature();

    println!("Initial state:");
    println!("  Atoms: {}", sim.n_atoms());
    println!("  Energy: {:.4}", initial_energy);
    println!("  Temperature: {:.4}", initial_temp);

    // Step 7: Equilibration run
    sim.run(1000);

    let equil_energy = sim.total_energy();
    let equil_temp = sim.temperature();

    println!("After equilibration (1000 steps):");
    println!("  Energy: {:.4}", equil_energy);
    println!("  Temperature: {:.4}", equil_temp);
    println!("  Energy drift: {:.4}%", (equil_energy - initial_energy) / initial_energy.abs() * 100.0);

    // Step 8: Production run with monitoring
    let mut energies = Vec::new();
    let mut temperatures = Vec::new();

    for _ in 0..100 {
        sim.run(10);
        energies.push(sim.total_energy());
        temperatures.push(sim.temperature());
    }

    let avg_energy: f64 = energies.iter().sum::<f64>() / energies.len() as f64;
    let avg_temp: f32 = temperatures.iter().sum::<f32>() / temperatures.len() as f32;

    println!("Production run statistics:");
    println!("  Average energy: {:.4}", avg_energy);
    println!("  Average temperature: {:.4}", avg_temp);

    // Step 9: Verify physical correctness
    // Energy should be reasonably conserved
    let energy_std: f64 = energies.iter()
        .map(|e| (e - avg_energy).powi(2))
        .sum::<f64>() / energies.len() as f64;
    let energy_fluctuation = energy_std.sqrt() / avg_energy.abs();

    println!("  Energy fluctuation: {:.4}%", energy_fluctuation * 100.0);

    assert!(
        energy_fluctuation < 0.01,
        "Energy fluctuation {:.4}% too large",
        energy_fluctuation * 100.0
    );
}

/// Test simulation with Langevin thermostat (NVT)
#[test]
fn test_full_simulation_loop_nvt() {
    let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    let box_ = SimulationBox::cubic(4.5);

    // Start at wrong temperature
    maxwell_boltzmann_velocities(&mut atoms, 0.5, 1.0);
    observable::remove_com_velocity(&mut atoms);

    let target_temp = 1.0f32;

    let lj = LennardJones::argon();
    let langevin = Langevin::reduced_units(1.0, target_temp);

    let mut sim = Simulation::new(atoms, box_, lj, langevin)
        .with_timestep(0.002);

    // Run equilibration
    sim.run(2000);

    // Sample temperatures
    let mut temperatures = Vec::new();
    for _ in 0..100 {
        sim.run(10);
        temperatures.push(sim.temperature());
    }

    let avg_temp: f32 = temperatures.iter().sum::<f32>() / temperatures.len() as f32;

    println!("NVT simulation:");
    println!("  Target temperature: {:.4}", target_temp);
    println!("  Average temperature: {:.4}", avg_temp);
    println!("  Temperature deviation: {:.4}", (avg_temp - target_temp).abs());

    // Temperature should be close to target
    assert!(
        (avg_temp - target_temp).abs() < 0.2,
        "Temperature {:.4} too far from target {:.4}",
        avg_temp, target_temp
    );
}

/// Test simulation with composite force field
#[test]
fn test_full_simulation_loop_composite_forcefield() {
    // Create charged particles at well-spaced positions
    let mut atoms = Vec::new();
    let n_pairs = 8;

    for i in 0..n_pairs {
        let x = (i % 4) as f32 * 3.0 + 2.0;
        let y = (i / 4) as f32 * 3.0 + 2.0;

        // Positive ion
        atoms.push(
            Atom::new(i * 2, 0, 1.0)
                .with_position(x, y, 5.0)
                .with_charge(0.5) // Reduced charge for stability
        );
        // Negative ion - at equilibrium distance
        atoms.push(
            Atom::new(i * 2 + 1, 1, 1.0)
                .with_position(x + 1.5, y, 5.0) // Well separated
                .with_charge(-0.5)
        );
    }

    let box_ = SimulationBox::cubic(15.0);

    maxwell_boltzmann_velocities(&mut atoms, 0.2, 1.0);
    observable::remove_com_velocity(&mut atoms);

    // Composite force field: LJ + Coulomb
    // Need LJ with 2 types since we have atom types 0 and 1
    use fxnn::force_field::CoulombMethod;
    let mut lj = LennardJones::new(2, 2.5);
    lj.set_parameters(0, 0, 1.0, 1.0);
    lj.set_parameters(0, 1, 1.0, 1.0);
    lj.set_parameters(1, 1, 1.0, 1.0);

    let ff = CompositeForceField::new()
        .add(lj)
        .add(Coulomb::reduced_units(5.0).with_method(CoulombMethod::Cutoff));

    let integrator = VelocityVerlet::new();
    let mut sim = Simulation::new(atoms, box_, ff, integrator)
        .with_timestep(0.0005); // Smaller timestep for stability

    let initial_energy = sim.total_energy();

    // Run simulation
    sim.run(1000);

    let final_energy = sim.total_energy();
    let energy_drift = ((final_energy - initial_energy) / initial_energy.abs() * 100.0).abs();

    println!("Composite force field simulation:");
    println!("  Initial energy: {:.4}", initial_energy);
    println!("  Final energy: {:.4}", final_energy);
    println!("  Energy drift: {:.4}%", energy_drift);

    // Composite force fields with Coulomb interactions have higher energy drift
    // due to the long-range nature of electrostatics and cutoff discontinuities
    // Note: 12% threshold accounts for numerical variance across different runs
    assert!(
        energy_drift < 12.0,
        "Energy drift {:.4}% too large for composite force field",
        energy_drift
    );
}

// ============================================================================
// Multi-Agent Interaction Tests
// ============================================================================

/// Test multiple "agents" (atoms with distinct behaviors) interacting
#[test]
fn test_multi_agent_interaction() {
    let n_agents = 10;

    // Create agents with different properties
    let mut atoms: Vec<Atom> = Vec::new();

    for i in 0..n_agents {
        atoms.push(
            Atom::new(i as u32, i as u16 % 3, 1.0 + (i as f32) * 0.1)
                .with_position(
                    5.0 + (i as f32 * 0.5),
                    5.0 + ((i as f32 * 0.7).sin() * 2.0),
                    5.0,
                )
                .with_velocity(
                    (i as f32 * 0.3).cos() * 0.5,
                    (i as f32 * 0.3).sin() * 0.5,
                    0.0,
                )
        );
    }

    let box_ = SimulationBox::cubic(15.0);
    observable::remove_com_velocity(&mut atoms);

    // LJ with 3 types since we use atom types 0, 1, 2 (i % 3)
    let mut lj = LennardJones::new(3, 2.5);
    for t1 in 0..3 {
        for t2 in t1..3 {
            lj.set_parameters(t1, t2, 1.0, 1.0);
        }
    }
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    // Track interactions
    let mut collision_events = Vec::new();

    for step in 0..500 {
        sim.step_forward();

        // Check for close approaches (collisions)
        let atoms = sim.atoms();
        for i in 0..n_agents {
            for j in (i + 1)..n_agents {
                let dist = sim.box_().distance(&atoms[i].position, &atoms[j].position);
                if dist < 1.2 { // Approximate collision distance
                    collision_events.push((step, i, j, dist));
                }
            }
        }
    }

    println!("Multi-agent simulation:");
    println!("  Agents: {}", n_agents);
    println!("  Steps: 500");
    println!("  Close approach events: {}", collision_events.len());

    // Should have some interactions
    assert!(
        collision_events.len() > 0,
        "Agents should have close interactions"
    );
}

/// Test agent swarm dynamics
#[test]
fn test_agent_swarm_dynamics() {
    // Create a swarm of agents in a cluster
    let n_agents = 50;
    let mut atoms = random_atoms(n_agents, &SimulationBox::cubic(3.0));
    let box_ = SimulationBox::cubic(10.0);

    // Shift to center of larger box
    for atom in &mut atoms {
        atom.position[0] += 3.5;
        atom.position[1] += 3.5;
        atom.position[2] += 3.5;
    }

    // Give radial velocities (expansion)
    let center = [5.0, 5.0, 5.0];
    for atom in &mut atoms {
        let dx = atom.position[0] - center[0];
        let dy = atom.position[1] - center[1];
        let dz = atom.position[2] - center[2];
        let r = (dx*dx + dy*dy + dz*dz).sqrt().max(0.1);

        atom.velocity = [
            dx / r * 0.5,
            dy / r * 0.5,
            dz / r * 0.5,
        ];
    }

    observable::remove_com_velocity(&mut atoms);

    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    // Track swarm radius over time
    let mut radii = Vec::new();

    for _ in 0..200 {
        sim.run(5);

        // Calculate average distance from center
        let avg_radius: f32 = sim.atoms().iter()
            .map(|a| {
                let dx = a.position[0] - center[0];
                let dy = a.position[1] - center[1];
                let dz = a.position[2] - center[2];
                (dx*dx + dy*dy + dz*dz).sqrt()
            })
            .sum::<f32>() / n_agents as f32;

        radii.push(avg_radius);
    }

    let initial_radius = radii[0];
    let final_radius = *radii.last().unwrap();

    println!("Swarm dynamics:");
    println!("  Initial radius: {:.4}", initial_radius);
    println!("  Final radius: {:.4}", final_radius);

    // Swarm should have expanded (initial radial velocities)
    assert!(
        final_radius > initial_radius,
        "Swarm should expand with radial velocities"
    );
}

/// Test interaction of different atom types
#[test]
fn test_multi_type_interaction() {
    // Create a system with two types of atoms
    let mut atoms = Vec::new();

    // Type 0: Heavy, slow - well-spaced (spacing > sigma)
    for i in 0..10 {
        atoms.push(
            Atom::new(i, 0, 2.0)
                .with_position(i as f32 * 1.2, 5.0, 5.0)
        );
    }

    // Type 1: Light, fast - well-spaced, starting above type 0
    for i in 0..10 {
        atoms.push(
            Atom::new(10 + i, 1, 0.5)
                .with_position(i as f32 * 1.2, 8.0, 5.0)
                .with_velocity(0.0, -0.3, 0.0) // Moving towards type 0
        );
    }

    let box_ = SimulationBox::cubic(15.0);
    observable::remove_com_velocity(&mut atoms);

    // Create LJ with different parameters for types
    let mut lj = LennardJones::new(2, 2.5);
    lj.set_parameters(0, 0, 1.0, 1.0);  // Type 0-0
    lj.set_parameters(1, 1, 0.5, 0.8);  // Type 1-1
    lj.set_parameters(0, 1, 0.7, 0.9);  // Type 0-1 (mixed)

    let integrator = VelocityVerlet::new();
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001);

    // Run and track type separation
    sim.run(500);

    // Calculate center of mass for each type
    let atoms = sim.atoms();
    let (mut com0, mut com1) = ([0.0f32; 3], [0.0f32; 3]);
    let (mut count0, mut count1) = (0, 0);

    for atom in atoms {
        if atom.atom_type == 0 {
            com0[0] += atom.position[0];
            com0[1] += atom.position[1];
            com0[2] += atom.position[2];
            count0 += 1;
        } else {
            com1[0] += atom.position[0];
            com1[1] += atom.position[1];
            com1[2] += atom.position[2];
            count1 += 1;
        }
    }

    com0[0] /= count0 as f32;
    com0[1] /= count0 as f32;
    com0[2] /= count0 as f32;
    com1[0] /= count1 as f32;
    com1[1] /= count1 as f32;
    com1[2] /= count1 as f32;

    let type_separation = sim.box_().distance(&com0, &com1);

    println!("Multi-type interaction:");
    println!("  Type 0 COM: [{:.2}, {:.2}, {:.2}]", com0[0], com0[1], com0[2]);
    println!("  Type 1 COM: [{:.2}, {:.2}, {:.2}]", com1[0], com1[1], com1[2]);
    println!("  Type separation: {:.4}", type_separation);

    // Types should have mixed somewhat (not completely separated)
    assert!(
        type_separation < 5.0,
        "Types should interact and mix"
    );
}

// ============================================================================
// Neighbor List Integration Tests
// ============================================================================

/// Test simulation with explicit neighbor list management
#[test]
fn test_simulation_with_cell_list() {
    let atoms = fcc_lattice(4, 4, 4, 1.5);
    let box_ = SimulationBox::cubic(6.0);
    let cutoff = 2.5;
    let skin = 0.5;

    let mut cell_list = CellList::new(atoms.len(), cutoff, skin);
    cell_list.build(&atoms, &box_, cutoff);

    // Verify cell list built correctly
    let stats = cell_list.stats();
    println!("Cell list stats:");
    println!("  Cells: {}x{}x{}", stats.n_cells[0], stats.n_cells[1], stats.n_cells[2]);
    println!("  Max atoms per cell: {}", stats.max_atoms_per_cell);
    println!("  Avg atoms per cell: {:.2}", stats.avg_atoms_per_cell);

    // Verify all atoms have neighbors (dense system)
    let neighbor_list = cell_list.neighbor_list();
    for i in 0..atoms.len() {
        let neighbors = neighbor_list.get_neighbors(i);
        assert!(
            neighbors.len() > 0,
            "Atom {} should have neighbors in FCC lattice",
            i
        );
    }
}

/// Test neighbor list updates during simulation
#[test]
fn test_neighbor_list_updates() {
    let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    let box_ = SimulationBox::cubic(4.5);

    maxwell_boltzmann_velocities(&mut atoms, 1.5, 1.0);
    observable::remove_com_velocity(&mut atoms);

    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    // Track energy to verify neighbor list is working correctly
    let initial_energy = sim.total_energy();

    // Run enough steps to require neighbor list rebuilds
    sim.run(1000);

    let final_energy = sim.total_energy();
    let energy_drift = ((final_energy - initial_energy) / initial_energy.abs() * 100.0).abs();

    println!("Neighbor list update test:");
    println!("  Energy drift: {:.4}%", energy_drift);

    // Good energy conservation indicates neighbor list is working
    assert!(
        energy_drift < 0.1,
        "Energy drift {:.4}% suggests neighbor list issues",
        energy_drift
    );
}

// ============================================================================
// Property-Based Integration Tests
// ============================================================================

proptest! {
    /// Test that simulations with positive parameters don't crash
    /// Uses an FCC lattice to ensure well-spaced initial configuration
    #[test]
    fn test_simulation_robustness(
        nx in 2usize..5,
        ny in 2usize..5,
        nz in 2usize..5,
        temperature in 0.1f32..1.5,
        n_steps in 10usize..50,
    ) {
        // Use FCC lattice with spacing > sigma to avoid initial overlaps
        let lattice_spacing = 1.3; // > sigma = 1.0
        let mut atoms = fcc_lattice(nx, ny, nz, lattice_spacing);
        let box_size = (nx.max(ny).max(nz) as f32 + 1.0) * lattice_spacing * 1.5;
        let box_ = SimulationBox::cubic(box_size);

        maxwell_boltzmann_velocities(&mut atoms, temperature, 1.0);
        observable::remove_com_velocity(&mut atoms);

        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let mut sim = Simulation::new(atoms, box_, lj, integrator)
            .with_timestep(0.001);

        // Should not panic
        sim.run(n_steps);

        // Should produce finite values
        let energy = sim.total_energy();
        let temp = sim.temperature();

        prop_assert!(energy.is_finite(), "Energy should be finite");
        prop_assert!(temp.is_finite(), "Temperature should be finite");
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

/// Test with large system
#[test]
#[ignore] // Takes a while - run with `cargo test --ignored`
fn test_large_system() {
    let atoms = fcc_lattice(10, 10, 10, 1.5);
    let box_ = SimulationBox::cubic(15.0);

    println!("Large system test: {} atoms", atoms.len());

    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.002);

    let start = std::time::Instant::now();
    sim.run(100);
    let elapsed = start.elapsed();

    println!(
        "  100 steps in {:.2}ms ({:.2} steps/sec)",
        elapsed.as_millis(),
        100.0 / elapsed.as_secs_f64()
    );

    assert!(sim.total_energy().is_finite());
}

/// Test with high temperature (fast dynamics)
#[test]
fn test_high_temperature() {
    let mut atoms = fcc_lattice(3, 3, 3, 1.5);
    let box_ = SimulationBox::cubic(4.5);

    // High temperature
    maxwell_boltzmann_velocities(&mut atoms, 5.0, 1.0);
    observable::remove_com_velocity(&mut atoms);

    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    // Need smaller timestep for high-speed dynamics
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.0005);

    let initial_energy = sim.total_energy();
    sim.run(500);
    let final_energy = sim.total_energy();

    let drift = ((final_energy - initial_energy) / initial_energy.abs() * 100.0).abs();

    println!("High temperature test:");
    println!("  Energy drift: {:.4}%", drift);

    // Should still conserve energy reasonably
    assert!(
        drift < 1.0,
        "Energy drift {:.4}% too large for high-T simulation",
        drift
    );
}
