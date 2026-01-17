//! Water Box Simulation Example (Simplified)
//!
//! This example demonstrates molecular dynamics simulation of a simplified
//! water-like system using Lennard-Jones interactions in FXNN's Reality Stack.
//!
//! # Note
//!
//! This is a simplified model demonstrating the composite force field API.
//! For production water simulations, use a validated force field with
//! proper electrostatics (Ewald summation) and bonded interactions.
//!
//! # Running
//!
//! ```bash
//! cargo run --example water_box --release
//! ```

use fxnn::{
    Simulation, SimulationBox, LennardJones, Langevin, ForceField,
    generators::fcc_lattice,
    observable,
};

fn main() {
    println!("=======================================================");
    println!("  FXNN Water-like System Simulation");
    println!("  Composite Force Fields in the Reality Stack");
    println!("=======================================================\n");

    // =========================================================================
    // Layer 1: SUBSTRATE
    // Create particles in a cubic box
    // =========================================================================

    println!("[Layer 1: SUBSTRATE]");

    // Create a small FCC lattice (simulating water-like density)
    let mut atoms = fcc_lattice(3, 3, 3, 1.2);
    println!("  Created {} particles", atoms.len());

    // Simulation box
    let box_size = 3.6;
    let box_ = SimulationBox::cubic(box_size);
    println!("  Box dimensions: {:.2} x {:.2} x {:.2}",
             box_size, box_size, box_size);

    // Initialize velocities at target temperature
    let target_temp = 2.5; // Reduced units
    initialize_velocities(&mut atoms, target_temp);
    observable::remove_com_velocity(&mut atoms);
    println!("  Initialized velocities at T = {:.1} (reduced)", target_temp);

    // =========================================================================
    // Layer 2: FORCES
    // Lennard-Jones force field
    // =========================================================================

    println!("\n[Layer 2: FORCES]");

    // Lennard-Jones interactions
    let lj = LennardJones::argon();
    println!("  Lennard-Jones: epsilon={:.4}, sigma={:.4}, cutoff={:.2}",
             lj.epsilon(), lj.sigma(), lj.cutoff());

    // =========================================================================
    // Layer 3: DYNAMICS
    // Langevin thermostat for NVT ensemble
    // =========================================================================

    println!("\n[Layer 3: DYNAMICS]");

    // Langevin dynamics for temperature control
    let gamma = 1.0;  // Friction coefficient
    let integrator = Langevin::reduced_units(target_temp, gamma)
        .with_seed(42);  // For reproducibility
    println!("  Integrator: Langevin (stochastic NVT)");
    println!("  Target temperature: {:.2}", target_temp);
    println!("  Friction coefficient: {:.2}", gamma);

    // Create simulation
    let timestep = 0.001;
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(timestep);
    println!("  Timestep: {:.5} tau", timestep);

    // =========================================================================
    // Initial State
    // =========================================================================

    println!("\n[Initial State]");
    println!("  --------------------------------------------------");
    println!("  Kinetic Energy:    {:>12.4}", sim.kinetic_energy());
    println!("  Potential Energy:  {:>12.4}", sim.potential_energy());
    println!("  Total Energy:      {:>12.4}", sim.total_energy());
    println!("  Temperature:       {:>12.4}", sim.temperature());
    println!("  --------------------------------------------------");

    // =========================================================================
    // Equilibration Phase
    // =========================================================================

    println!("\n[Equilibration]");
    println!("  Running 2000 steps for equilibration...");

    sim.run(2000);

    println!("  Equilibration complete.");
    println!("  Temperature after equilibration: {:.4}", sim.temperature());

    // =========================================================================
    // Production Phase
    // =========================================================================

    println!("\n[Production Run]");
    println!("  Running 3000 steps with temperature monitoring...\n");

    println!("  {:>8} {:>12} {:>12} {:>12} {:>10}",
             "Step", "KE", "PE", "Total E", "Temp");
    println!("  {}", "-".repeat(58));

    let mut temps = Vec::new();
    let intervals = 3;
    let steps_per_interval = 1000;

    for i in 0..intervals {
        sim.run(steps_per_interval);
        let temp = sim.temperature();
        temps.push(temp);
        println!("  {:>8} {:>12.4} {:>12.4} {:>12.4} {:>10.4}",
                 (i + 1) * steps_per_interval,
                 sim.kinetic_energy(),
                 sim.potential_energy(),
                 sim.total_energy(),
                 temp);
    }

    // =========================================================================
    // Final Analysis
    // =========================================================================

    println!("\n[Final Analysis]");
    println!("  --------------------------------------------------");

    let avg_temp: f32 = temps.iter().sum::<f32>() / temps.len() as f32;
    let temp_std: f32 = (temps.iter()
        .map(|t| (t - avg_temp).powi(2))
        .sum::<f32>() / temps.len() as f32)
        .sqrt();

    println!("  Target temperature:  {:>12.4}", target_temp);
    println!("  Average temperature: {:>12.4}", avg_temp);
    println!("  Temperature std dev: {:>12.4}", temp_std);
    println!("  Final temperature:   {:>12.4}", sim.temperature());
    println!("  --------------------------------------------------");

    // Interpret results
    println!("\n[Interpretation]");

    let temp_error = (avg_temp - target_temp).abs() / target_temp * 100.0;
    if temp_error < 10.0 {
        println!("  Temperature control: GOOD (<10% error)");
    } else if temp_error < 20.0 {
        println!("  Temperature control: ACCEPTABLE (<20% error)");
    } else {
        println!("  Temperature control: NEEDS ADJUSTMENT");
    }

    println!("\n=======================================================");
    println!("  Simulation Complete");
    println!("  FXNN - Where Physics Meets Intelligence");
    println!("=======================================================\n");
}

/// Initialize particle velocities with Maxwell-Boltzmann distribution
fn initialize_velocities(atoms: &mut [fxnn::types::Atom], temperature: f32) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for atom in atoms.iter_mut() {
        let scale = (temperature / atom.mass).sqrt();
        atom.velocity = [
            rng.gen::<f32>() * 2.0 * scale - scale,
            rng.gen::<f32>() * 2.0 * scale - scale,
            rng.gen::<f32>() * 2.0 * scale - scale,
        ];
    }
}
