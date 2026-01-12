//! Argon Gas Simulation Example
//!
//! This example demonstrates a basic molecular dynamics simulation of argon gas
//! using the Lennard-Jones potential in FXNN's Five-Layer Reality Stack.
//!
//! # Physical System
//!
//! Argon is a noble gas with simple, spherically symmetric interactions, making it
//! ideal for testing MD codes. The Lennard-Jones 12-6 potential accurately captures
//! the van der Waals interactions between argon atoms:
//!
//! ```text
//! V(r) = 4 * epsilon * [ (sigma/r)^12 - (sigma/r)^6 ]
//! ```
//!
//! Parameters for argon:
//! - epsilon = 1.0 kJ/mol (119.8 K in temperature units)
//! - sigma = 0.34 nm (3.4 Angstroms)
//! - mass = 39.948 amu
//!
//! # Reduced Units
//!
//! This example uses reduced (dimensionless) Lennard-Jones units:
//! - Length: sigma (1.0 = 0.34 nm)
//! - Energy: epsilon (1.0 = 1.0 kJ/mol)
//! - Temperature: epsilon/k_B (1.0 = 119.8 K)
//! - Time: tau = sigma * sqrt(m/epsilon) ~ 2.2 ps
//!
//! # Reality Stack Layers
//!
//! This example exercises the first three layers:
//!
//! 1. **Substrate**: FCC lattice of atoms, cubic simulation box with PBC
//! 2. **Forces**: Lennard-Jones potential with cutoff, cell list neighbor search
//! 3. **Dynamics**: Velocity Verlet integrator (NVE ensemble)
//!
//! # Expected Results
//!
//! For a well-equilibrated system at T=1.0 (reduced):
//! - Temperature should fluctuate around 1.0
//! - Total energy should be conserved (drift << 0.01% per 1000 steps)
//! - Kinetic and potential energies should fluctuate but sum to constant
//!
//! # Running
//!
//! ```bash
//! cargo run --example argon_gas --release
//! ```
//!
//! # Output Interpretation
//!
//! The simulation prints energy components and temperature at each stage.
//! A small energy drift (<0.01%) indicates correct implementation.
//! Larger drifts may indicate:
//! - Timestep too large (try 0.0005 instead of 0.001)
//! - Neighbor list not updating frequently enough
//! - Numerical precision issues
//!
//! # Extending This Example
//!
//! Try modifying:
//! - `fcc_lattice(nx, ny, nz, spacing)`: Change system size
//! - `maxwell_boltzmann_velocities(&mut atoms, T, kb)`: Change temperature
//! - `with_timestep(dt)`: Adjust integration timestep
//! - `sim.run(steps)`: Change simulation length
//!
//! For temperature control, replace VelocityVerlet with Langevin integrator.

use fxnn::{
    Simulation, SimulationBox, LennardJones, VelocityVerlet, ForceField,
    generators::{fcc_lattice, maxwell_boltzmann_velocities},
    observable,
};

fn main() {
    println!("=======================================================");
    println!("  FXNN Argon Gas Simulation");
    println!("  A Five-Layer Reality Stack Demonstration");
    println!("=======================================================\n");

    // =========================================================================
    // Layer 1: SUBSTRATE
    // Create the physical foundation: atoms on a crystal lattice in a box
    // =========================================================================

    println!("[Layer 1: SUBSTRATE]");

    // Create FCC lattice of argon atoms
    // FCC (Face-Centered Cubic) is the equilibrium crystal structure for solid argon
    // Parameters: nx, ny, nz unit cells, lattice spacing
    let mut atoms = fcc_lattice(4, 4, 4, 1.5);
    println!("  Created {} atoms on FCC lattice", atoms.len());

    // Simulation box with periodic boundary conditions
    // Box must be larger than 2*cutoff to avoid self-interaction
    let box_ = SimulationBox::cubic(6.0);
    println!("  Simulation box: {:.2} x {:.2} x {:.2} sigma^3",
             box_.dimensions[0], box_.dimensions[1], box_.dimensions[2]);
    println!("  Density: {:.4} (atoms/sigma^3)",
             atoms.len() as f64 / box_.volume() as f64);

    // Initialize velocities from Maxwell-Boltzmann distribution at T=1.0
    // This sets kinetic energy to (3/2)*N*k_B*T
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
    println!("  Initialized velocities at T = 1.0 (reduced)");

    // Remove center-of-mass velocity to prevent system drift
    observable::remove_com_velocity(&mut atoms);
    println!("  Removed center-of-mass velocity");

    // =========================================================================
    // Layer 2: FORCES
    // Define how particles interact via the Lennard-Jones potential
    // =========================================================================

    println!("\n[Layer 2: FORCES]");

    // Lennard-Jones force field with argon parameters
    let lj = LennardJones::argon();
    println!("  Force field: Lennard-Jones 12-6");
    println!("  Parameters: epsilon = {:.3}, sigma = {:.3}",
             lj.epsilon(), lj.sigma());
    println!("  Cutoff: {:.3} sigma", lj.cutoff());

    // =========================================================================
    // Layer 3: DYNAMICS
    // Time integration with the velocity Verlet algorithm (NVE ensemble)
    // =========================================================================

    println!("\n[Layer 3: DYNAMICS]");

    // Velocity Verlet integrator - symplectic, time-reversible
    let integrator = VelocityVerlet::new();
    println!("  Integrator: Velocity Verlet (symplectic)");
    println!("  Ensemble: NVE (microcanonical)");

    // Create simulation with all components
    let timestep = 0.001;
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(timestep);
    println!("  Timestep: {:.4} tau ({:.2} fs)", timestep, timestep * 2200.0);

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
    println!("  Running 1000 steps for equilibration...");

    let e0 = sim.total_energy();
    sim.run(1000);
    let e1 = sim.total_energy();

    println!("  Equilibration complete.");

    // =========================================================================
    // Production Phase with Energy Monitoring
    // =========================================================================

    println!("\n[Production Run]");
    println!("  Running 5000 steps with energy monitoring...\n");

    println!("  {:>8} {:>12} {:>12} {:>12} {:>10}",
             "Step", "KE", "PE", "Total E", "Temp");
    println!("  {}", "-".repeat(58));

    let e_start = sim.total_energy();
    let intervals = 5;
    let steps_per_interval = 1000;

    for i in 0..intervals {
        sim.run(steps_per_interval);
        println!("  {:>8} {:>12.4} {:>12.4} {:>12.4} {:>10.4}",
                 (i + 1) * steps_per_interval,
                 sim.kinetic_energy(),
                 sim.potential_energy(),
                 sim.total_energy(),
                 sim.temperature());
    }

    let e_end = sim.total_energy();
    let drift_percent = (e_end - e_start).abs() / e_start.abs() * 100.0;

    // =========================================================================
    // Final Analysis
    // =========================================================================

    println!("\n[Final Analysis]");
    println!("  --------------------------------------------------");
    println!("  Initial total energy: {:>12.6}", e0);
    println!("  Final total energy:   {:>12.6}", sim.total_energy());
    println!("  Energy drift:         {:>12.6}%", drift_percent);
    println!("  Final temperature:    {:>12.4}", sim.temperature());
    println!("  --------------------------------------------------");

    // Interpret results
    println!("\n[Interpretation]");
    if drift_percent < 0.01 {
        println!("  Energy conservation: EXCELLENT (<0.01% drift)");
    } else if drift_percent < 0.1 {
        println!("  Energy conservation: GOOD (<0.1% drift)");
    } else if drift_percent < 1.0 {
        println!("  Energy conservation: ACCEPTABLE (<1% drift)");
        println!("  Consider reducing timestep for better conservation.");
    } else {
        println!("  Energy conservation: POOR (>1% drift)");
        println!("  Reduce timestep or check neighbor list settings.");
    }

    println!("\n=======================================================");
    println!("  Simulation Complete");
    println!("  FXNN - Where Physics Meets Intelligence");
    println!("=======================================================\n");
}
