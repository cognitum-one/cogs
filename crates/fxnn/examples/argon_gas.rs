//! Argon gas simulation example

use fxnn::{
    Simulation, SimulationBox, LennardJones, VelocityVerlet,
    generators::{fcc_lattice, maxwell_boltzmann_velocities},
};

fn main() {
    println!("FXNN Argon Gas Simulation");
    println!("=========================\n");

    // Create FCC lattice of argon atoms
    let mut atoms = fcc_lattice(4, 4, 4, 1.5);
    let box_ = SimulationBox::cubic(6.0);

    println!("Number of atoms: {}", atoms.len());
    println!("Box size: {:?}", box_.dimensions);

    // Initialize velocities at T=1.0 (reduced units)
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    // Set up force field and integrator
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    // Create simulation
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001);

    // Initial energies
    println!("\nInitial state:");
    println!("  KE: {:.4}", sim.kinetic_energy());
    println!("  PE: {:.4}", sim.potential_energy());
    println!("  Total: {:.4}", sim.total_energy());
    println!("  Temperature: {:.4}", sim.temperature());

    // Run equilibration
    println!("\nRunning 1000 steps...");
    let e0 = sim.total_energy();
    sim.run(1000);
    let e1 = sim.total_energy();

    println!("\nFinal state:");
    println!("  KE: {:.4}", sim.kinetic_energy());
    println!("  PE: {:.4}", sim.potential_energy());
    println!("  Total: {:.4}", sim.total_energy());
    println!("  Temperature: {:.4}", sim.temperature());
    println!("  Energy drift: {:.6}%", ((e1 - e0) / e0.abs() * 100.0));

    println!("\nSimulation complete!");
}
