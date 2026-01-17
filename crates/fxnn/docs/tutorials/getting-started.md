# Getting Started with FXNN

This tutorial walks you through creating your first molecular dynamics simulation.

## Prerequisites

- Rust 1.70 or later
- Basic understanding of molecular dynamics concepts

## Installation

Add FXNN to your `Cargo.toml`:

```toml
[dependencies]
fxnn = "0.1"
```

## Your First Simulation

### Step 1: Create Atoms

```rust
use fxnn::generators::fcc_lattice;

// Create a face-centered cubic lattice
// 4x4x4 unit cells with 1.5 reduced units spacing
let mut atoms = fcc_lattice(4, 4, 4, 1.5);
println!("Created {} atoms", atoms.len()); // 256 atoms
```

### Step 2: Initialize Velocities

```rust
use fxnn::generators::maxwell_boltzmann_velocities;

// Initialize velocities at temperature T=1.0 (reduced units)
maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
```

### Step 3: Set Up the Simulation

```rust
use fxnn::{SimulationBox, Simulation};
use fxnn::force_field::LennardJones;
use fxnn::integrator::VelocityVerlet;

// Create simulation box
let box_ = SimulationBox::cubic(6.0);

// Use Lennard-Jones force field (argon parameters)
let lj = LennardJones::argon();

// Velocity Verlet integrator for NVE ensemble
let integrator = VelocityVerlet::new();

// Build simulation
let mut sim = Simulation::new(atoms, box_, lj, integrator)
    .with_timestep(0.001);
```

### Step 4: Run the Simulation

```rust
// Equilibrate for 10,000 steps
sim.run(10_000);

// Production run
for _ in 0..100 {
    sim.run(100);
    println!(
        "Step {}: E = {:.4}, T = {:.4}",
        sim.step(),
        sim.total_energy(),
        sim.temperature()
    );
}
```

## Complete Example

```rust
use fxnn::{Simulation, SimulationBox};
use fxnn::force_field::LennardJones;
use fxnn::integrator::VelocityVerlet;
use fxnn::generators::{fcc_lattice, maxwell_boltzmann_velocities};

fn main() {
    // Setup
    let mut atoms = fcc_lattice(4, 4, 4, 1.5);
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    let box_ = SimulationBox::cubic(6.0);
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001);

    // Run
    println!("Initial: E = {:.4}", sim.total_energy());
    sim.run(10_000);
    println!("Final: E = {:.4}", sim.total_energy());
}
```

## Next Steps

- [Force Field Guide](force-fields.md) - Learn about different force fields
- [WASM/MCP Guide](wasm-mcp.md) - Run simulations in the browser
- [Performance Guide](../guides/performance.md) - Optimize your simulations
