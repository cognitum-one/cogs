# FXNN - Fast Molecular Dynamics in Rust

A high-performance molecular dynamics library written in Rust, designed for computational chemistry and physics research.

## Overview

FXNN (pronounced "finn" or "fiction") provides a complete toolkit for running classical and neural network-based molecular dynamics simulations. Built with performance as a primary goal, it leverages Rust's zero-cost abstractions, SIMD vectorization, and cache-friendly algorithms to deliver competitive performance with established MD packages.

### Key Features

- **Classical Force Fields**: Lennard-Jones, Coulomb, bonded interactions (bonds, angles, dihedrals)
- **Neural Network Potentials**: SchNet-style continuous-filter convolutions with online learning
- **Efficient Neighbor Lists**: Cell list and Verlet list algorithms for O(N) scaling
- **Symplectic Integrators**: Velocity Verlet for NVE, Langevin for NVT ensembles
- **SIMD Optimization**: Vectorized force calculations using the `wide` crate
- **Parallel Execution**: Optional Rayon-based parallelization for large systems

## Installation

Add FXNN to your `Cargo.toml`:

```toml
[dependencies]
fxnn = "0.1"
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `simd` | Yes | SIMD-optimized force calculations |
| `parallel` | No | Multi-threaded execution via Rayon |
| `neural` | No | Neural network force fields (SchNet) |
| `serde` | Yes | Serialization support for checkpointing |

Enable features as needed:

```toml
[dependencies]
fxnn = { version = "0.1", features = ["parallel", "neural"] }
```

## Quick Start

### Simple Lennard-Jones Simulation

```rust
use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
use fxnn::generators::{fcc_lattice, maxwell_boltzmann_velocities};

fn main() {
    // Create a face-centered cubic lattice of atoms
    let mut atoms = fcc_lattice(4, 4, 4, 1.5);  // 4x4x4 unit cells

    // Initialize velocities at temperature T=1.0
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    // Create simulation box and force field
    let box_ = SimulationBox::cubic(6.0);
    let lj = LennardJones::argon();  // Reduced units: epsilon=1, sigma=1
    let integrator = VelocityVerlet::new();

    // Build simulation
    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001);

    // Print initial state
    println!("Initial energy: {:.4}", sim.total_energy());
    println!("Initial temperature: {:.4}", sim.temperature());

    // Run equilibration (10,000 steps)
    for i in 0..100 {
        sim.run(100);
        println!(
            "Step {:5}: E = {:8.4}, T = {:6.4}",
            sim.step(),
            sim.total_energy(),
            sim.temperature()
        );
    }

    println!("Final energy: {:.4}", sim.total_energy());
}
```

### NVT Simulation with Langevin Thermostat

```rust
use fxnn::{Simulation, SimulationBox, LennardJones, Langevin};
use fxnn::generators::random_atoms;

fn main() {
    // Random initial configuration
    let box_ = SimulationBox::cubic(10.0);
    let atoms = random_atoms(500, &box_);

    // Langevin thermostat at T=1.5 with friction gamma=1.0
    let thermostat = Langevin::reduced_units(1.0, 1.5);
    let lj = LennardJones::argon();

    let mut sim = Simulation::new(atoms, box_, lj, thermostat)
        .with_timestep(0.002);

    // Equilibrate
    sim.run(5000);

    // Production run - temperature should fluctuate around 1.5
    for _ in 0..10 {
        sim.run(1000);
        println!("T = {:.3}", sim.temperature());
    }
}
```

## Architecture

### Core Components

```
fxnn
├── types/           # Core data structures (Atom, SimulationBox, Topology)
├── force_field/     # Force field implementations
│   ├── LennardJones
│   ├── Coulomb
│   ├── BondedForces (bonds, angles, dihedrals)
│   ├── CompositeForceField
│   └── neural/ (optional)
├── integrator/      # Time integration schemes
│   ├── VelocityVerlet (NVE)
│   └── Langevin (NVT)
├── neighbor/        # Neighbor list algorithms
│   ├── CellList
│   └── VerletList
├── observable/      # Thermodynamic calculators
├── generators/      # System setup utilities
└── simulation.rs    # Main simulation engine
```

### Data Flow

```
           ┌─────────────────┐
           │    Simulation   │
           │  (orchestrator) │
           └────────┬────────┘
                    │
    ┌───────────────┼───────────────┐
    │               │               │
    ▼               ▼               ▼
┌────────┐   ┌────────────┐   ┌──────────┐
│ Atoms  │   │ ForceField │   │Integrator│
└────────┘   └────────────┘   └──────────┘
    │               │               │
    │               ▼               │
    │        ┌────────────┐        │
    └───────►│NeighborList│◄───────┘
             └────────────┘
```

## Units

FXNN supports both reduced (dimensionless) and real units:

### Reduced Units (Default)

| Quantity | Unit | Typical Value |
|----------|------|---------------|
| Length | sigma | 1.0 |
| Energy | epsilon | 1.0 |
| Mass | m | 1.0 |
| Time | tau = sigma * sqrt(m/epsilon) | ~2.15 ps |
| Temperature | epsilon/k_B | ~120 K for argon |

### Real Units

| Quantity | Unit | Example |
|----------|------|---------|
| Length | nm | 0.34 nm (sigma for argon) |
| Energy | kJ/mol | 0.996 kJ/mol (epsilon for argon) |
| Mass | amu | 39.948 amu (argon) |
| Time | ps | 0.001 ps |
| Temperature | K | 300 K |

## Performance

FXNN achieves high performance through several optimizations:

### Algorithmic Complexity

| Operation | Complexity |
|-----------|------------|
| Force calculation (with neighbor list) | O(N) |
| Neighbor list build (cell list) | O(N) |
| Energy calculation | O(N) |

### SIMD Vectorization

Force calculations are vectorized using 4-wide SIMD (SSE/NEON) or 8-wide (AVX2):

```rust
// Automatically vectorized LJ force loop
// Processes 4 atom pairs per iteration
for chunk in pairs.chunks(4) {
    // SIMD distance calculation
    // SIMD force computation
    // SIMD force accumulation
}
```

### Cache Optimization

- Atom data uses `#[repr(C)]` for predictable memory layout
- Structure-of-Arrays (SoA) layout available for maximum SIMD efficiency
- Cell list uses half-shell iteration to minimize memory writes

### Benchmark Results

Typical performance on a modern CPU (single-threaded):

| System Size | Steps/second |
|-------------|--------------|
| 1,000 atoms | ~50,000 |
| 10,000 atoms | ~5,000 |
| 100,000 atoms | ~400 |

## Examples

See the `examples/` directory for complete working examples:

- `argon_gas.rs` - Simple LJ simulation of argon
- `water_box.rs` - TIP3P water with bonds and angles
- `neural_training.rs` - Training a neural network potential

Run examples with:

```bash
cargo run --example argon_gas --release
```

## Documentation

Full API documentation is available via:

```bash
cargo doc --open
```

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

FXNN draws inspiration from established MD packages including:
- GROMACS
- OpenMM
- LAMMPS
- ASE (Atomic Simulation Environment)

## Citation

If you use FXNN in your research, please cite:

```bibtex
@software{fxnn,
  title = {FXNN: Fast Molecular Dynamics in Rust},
  url = {https://github.com/your-org/fxnn},
  version = {0.1.0},
  year = {2024}
}
```
