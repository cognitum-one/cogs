//! Comprehensive benchmarks for the FXNN molecular dynamics crate
//!
//! This benchmark suite measures performance across:
//! - Force computation (Lennard-Jones, Coulomb)
//! - Neighbor list building (Cell List vs Verlet List)
//! - SIMD distance calculations
//! - Full simulation step timing
//! - Neural force field inference (when neural feature enabled)
//!
//! Run with: `cargo bench`
//! Generate HTML reports in: `target/criterion/`

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use fxnn::{
    generators, Atom, CellList, Coulomb, ForceField, LennardJones, NeighborList, Simulation,
    SimulationBox, VelocityVerlet, VerletList,
};
use fxnn::neighbor::NeighborSearch;
use fxnn::simd::{
    accumulate_forces, distances_with_cutoff, minimum_image_displacement,
    pairwise_distances_squared, zero_forces,
};

// ============================================================================
// Test System Generation
// ============================================================================

/// Generate a test system with the specified number of atoms
/// Uses FCC lattice to create a realistic atomic arrangement
fn generate_test_system(n_atoms: usize) -> (Vec<Atom>, SimulationBox) {
    // Calculate lattice dimensions to get approximately n_atoms
    // FCC has 4 atoms per unit cell
    let cells_per_dim = ((n_atoms as f32 / 4.0).powf(1.0 / 3.0)).ceil() as usize;
    let lattice_constant = 1.5; // Reasonable for reduced LJ units

    let mut atoms = generators::fcc_lattice(cells_per_dim, cells_per_dim, cells_per_dim, lattice_constant);

    // Truncate to exact number if needed
    atoms.truncate(n_atoms);

    // Create box that fits the lattice
    let box_size = cells_per_dim as f32 * lattice_constant;
    let sim_box = SimulationBox::cubic(box_size);

    // Initialize velocities for realistic simulation
    generators::maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    (atoms, sim_box)
}

/// Generate atoms with charges for Coulomb benchmarks
fn generate_charged_system(n_atoms: usize) -> (Vec<Atom>, SimulationBox) {
    let (mut atoms, sim_box) = generate_test_system(n_atoms);

    // Assign alternating charges (+1, -1) for neutral system
    for (i, atom) in atoms.iter_mut().enumerate() {
        atom.charge = if i % 2 == 0 { 1.0 } else { -1.0 };
    }

    (atoms, sim_box)
}

// ============================================================================
// Force Computation Benchmarks
// ============================================================================

/// Benchmark Lennard-Jones force computation
fn bench_lennard_jones_forces(c: &mut Criterion) {
    let mut group = c.benchmark_group("LJ_Forces");

    // Test different system sizes
    for n_atoms in [1000, 10_000, 100_000] {
        let (mut atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let cutoff = lj.cutoff();

        // Build neighbor list once
        let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
        verlet.build(&atoms, &sim_box, cutoff);
        let neighbor_list = verlet.neighbor_list();

        // Calculate expected pairs for throughput
        let num_pairs = neighbor_list.num_pairs();

        group.throughput(Throughput::Elements(num_pairs as u64));

        group.bench_with_input(
            BenchmarkId::new("with_neighbor_list", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    // Zero forces before computation
                    for atom in atoms.iter_mut() {
                        atom.zero_force();
                    }
                    lj.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                });
            },
        );

        // Only benchmark direct summation for smaller systems (O(N^2))
        if n_atoms <= 1000 {
            let n_pairs_direct = n_atoms * (n_atoms - 1) / 2;
            group.throughput(Throughput::Elements(n_pairs_direct as u64));

            group.bench_with_input(
                BenchmarkId::new("direct_summation", n_atoms),
                &n_atoms,
                |b, _| {
                    b.iter(|| {
                        for atom in atoms.iter_mut() {
                            atom.zero_force();
                        }
                        lj.compute_forces(black_box(&mut atoms), black_box(&sim_box), None);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark Lennard-Jones potential energy calculation
fn bench_lennard_jones_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("LJ_Energy");

    for n_atoms in [1000, 10_000, 100_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let cutoff = lj.cutoff();

        let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
        verlet.build(&atoms, &sim_box, cutoff);
        let neighbor_list = verlet.neighbor_list();

        let num_pairs = neighbor_list.num_pairs();
        group.throughput(Throughput::Elements(num_pairs as u64));

        group.bench_with_input(
            BenchmarkId::new("potential_energy", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    lj.potential_energy(black_box(&atoms), black_box(&sim_box), Some(neighbor_list))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Coulomb electrostatic force computation
fn bench_coulomb_forces(c: &mut Criterion) {
    let mut group = c.benchmark_group("Coulomb_Forces");

    for n_atoms in [1000, 10_000, 100_000] {
        let (mut atoms, sim_box) = generate_charged_system(n_atoms);
        let coulomb = Coulomb::reduced_units(2.5);
        let cutoff = coulomb.cutoff();

        let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
        verlet.build(&atoms, &sim_box, cutoff);
        let neighbor_list = verlet.neighbor_list();

        let num_pairs = neighbor_list.num_pairs();
        group.throughput(Throughput::Elements(num_pairs as u64));

        group.bench_with_input(
            BenchmarkId::new("reaction_field", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    for atom in atoms.iter_mut() {
                        atom.zero_force();
                    }
                    coulomb.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark combined LJ + Coulomb forces
fn bench_combined_forces(c: &mut Criterion) {
    let mut group = c.benchmark_group("Combined_Forces");

    for n_atoms in [1000, 10_000] {
        let (mut atoms, sim_box) = generate_charged_system(n_atoms);

        let lj = LennardJones::argon();
        let coulomb = Coulomb::reduced_units(2.5);
        let cutoff = lj.cutoff().max(coulomb.cutoff());

        let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
        verlet.build(&atoms, &sim_box, cutoff);
        let neighbor_list = verlet.neighbor_list();

        let num_pairs = neighbor_list.num_pairs();
        group.throughput(Throughput::Elements(num_pairs as u64 * 2)); // Two force fields

        group.bench_with_input(
            BenchmarkId::new("LJ_plus_Coulomb", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    for atom in atoms.iter_mut() {
                        atom.zero_force();
                    }
                    lj.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                    coulomb.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Neighbor List Benchmarks
// ============================================================================

/// Benchmark Cell List construction
fn bench_cell_list_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("CellList_Build");

    for n_atoms in [1000, 10_000, 100_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let cutoff = 2.5;
        let skin = 0.5;

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("build", n_atoms),
            &n_atoms,
            |b, _| {
                let mut cell_list = CellList::new(n_atoms, cutoff, skin);
                b.iter(|| {
                    cell_list.build(black_box(&atoms), black_box(&sim_box), cutoff);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Verlet List construction
fn bench_verlet_list_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("VerletList_Build");

    for n_atoms in [1000, 10_000, 100_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let cutoff = 2.5;
        let skin = 0.5;

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("with_cell_list", n_atoms),
            &n_atoms,
            |b, _| {
                let mut verlet = VerletList::new(n_atoms, cutoff, skin);
                b.iter(|| {
                    verlet.build(black_box(&atoms), black_box(&sim_box), cutoff);
                });
            },
        );

        // Direct O(N^2) build only for smaller systems
        if n_atoms <= 10_000 {
            group.bench_with_input(
                BenchmarkId::new("direct_O_N2", n_atoms),
                &n_atoms,
                |b, _| {
                    let mut verlet = VerletList::new_direct(n_atoms, cutoff, skin);
                    b.iter(|| {
                        verlet.build(black_box(&atoms), black_box(&sim_box), cutoff);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Compare Cell List vs Direct neighbor list building
fn bench_neighbor_list_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("NeighborList_Comparison");

    for n_atoms in [500, 1000, 2000, 5000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let cutoff = 2.5;
        let skin = 0.5;

        group.throughput(Throughput::Elements(n_atoms as u64));

        // Cell List approach (O(N) expected)
        group.bench_with_input(
            BenchmarkId::new("cell_list", n_atoms),
            &n_atoms,
            |b, _| {
                let mut cell_list = CellList::new(n_atoms, cutoff, skin);
                b.iter(|| {
                    cell_list.build(black_box(&atoms), black_box(&sim_box), cutoff);
                });
            },
        );

        // Direct approach (O(N^2))
        group.bench_with_input(
            BenchmarkId::new("direct", n_atoms),
            &n_atoms,
            |b, _| {
                let mut nl = NeighborList::new(n_atoms, cutoff, skin);
                b.iter(|| {
                    nl.build_direct(black_box(&atoms), black_box(&sim_box));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark neighbor list rebuild detection
fn bench_neighbor_list_needs_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("NeighborList_NeedsRebuild");

    for n_atoms in [1000, 10_000, 100_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let cutoff = 2.5;
        let skin = 0.5;

        let mut verlet = VerletList::new(n_atoms, cutoff, skin);
        verlet.build(&atoms, &sim_box, cutoff);

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("check_rebuild", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    verlet.needs_rebuild(black_box(&atoms), black_box(&sim_box))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// SIMD Distance Calculation Benchmarks
// ============================================================================

/// Benchmark pairwise distance calculations
fn bench_simd_pairwise_distances(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD_PairwiseDistances");

    for n_pairs in [100, 1000, 10_000, 100_000] {
        let sim_box = SimulationBox::cubic(100.0);

        // Generate random coordinate arrays
        let mut rng = rand::thread_rng();
        use rand::Rng;

        let x1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let y1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let z1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let x2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let y2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let z2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 100.0).collect();
        let mut out = vec![0.0f32; n_pairs];

        group.throughput(Throughput::Elements(n_pairs as u64));

        group.bench_with_input(
            BenchmarkId::new("squared_distances", n_pairs),
            &n_pairs,
            |b, _| {
                b.iter(|| {
                    pairwise_distances_squared(
                        black_box(&x1),
                        black_box(&y1),
                        black_box(&z1),
                        black_box(&x2),
                        black_box(&y2),
                        black_box(&z2),
                        black_box(&sim_box),
                        black_box(&mut out),
                    );
                });
            },
        );
    }

    group.finish();
}

/// Benchmark minimum image displacement calculations
fn bench_simd_minimum_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD_MinimumImage");

    for n_pairs in [100, 1000, 10_000, 100_000] {
        let sim_box = SimulationBox::cubic(10.0);

        let mut rng = rand::thread_rng();
        use rand::Rng;

        // Positions that require wrapping
        let x1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();
        let y1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();
        let z1: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();
        let x2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();
        let y2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();
        let z2: Vec<f32> = (0..n_pairs).map(|_| rng.gen::<f32>() * 10.0).collect();

        let mut dx_out = vec![0.0f32; n_pairs];
        let mut dy_out = vec![0.0f32; n_pairs];
        let mut dz_out = vec![0.0f32; n_pairs];

        group.throughput(Throughput::Elements(n_pairs as u64));

        group.bench_with_input(
            BenchmarkId::new("displacement_vectors", n_pairs),
            &n_pairs,
            |b, _| {
                b.iter(|| {
                    minimum_image_displacement(
                        black_box(&x1),
                        black_box(&y1),
                        black_box(&z1),
                        black_box(&x2),
                        black_box(&y2),
                        black_box(&z2),
                        black_box(&sim_box),
                        black_box(&mut dx_out),
                        black_box(&mut dy_out),
                        black_box(&mut dz_out),
                    );
                });
            },
        );
    }

    group.finish();
}

/// Benchmark distance with cutoff check
fn bench_simd_distance_cutoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD_DistanceCutoff");

    for n_atoms in [100, 1000, 10_000] {
        let sim_box = SimulationBox::cubic(10.0);
        let cutoff = 2.5;
        let cutoff2 = cutoff * cutoff;

        let mut rng = rand::thread_rng();
        use rand::Rng;

        // Central atom position
        let x1 = 5.0f32;
        let y1 = 5.0f32;
        let z1 = 5.0f32;

        // Other atom positions
        let x2: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>() * 10.0).collect();
        let y2: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>() * 10.0).collect();
        let z2: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>() * 10.0).collect();

        let mut d2_out = vec![0.0f32; n_atoms];
        let mut mask_out = vec![false; n_atoms];

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("with_cutoff_mask", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    distances_with_cutoff(
                        black_box(x1),
                        black_box(y1),
                        black_box(z1),
                        black_box(&x2),
                        black_box(&y2),
                        black_box(&z2),
                        black_box(&sim_box),
                        black_box(cutoff2),
                        black_box(&mut d2_out),
                        black_box(&mut mask_out),
                    );
                });
            },
        );
    }

    group.finish();
}

/// Benchmark force accumulation
fn bench_simd_force_accumulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD_ForceAccumulation");

    for n_atoms in [1000, 10_000, 100_000] {
        let mut rng = rand::thread_rng();
        use rand::Rng;

        let mut fx: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();
        let mut fy: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();
        let mut fz: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();

        let dfx: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();
        let dfy: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();
        let dfz: Vec<f32> = (0..n_atoms).map(|_| rng.gen::<f32>()).collect();

        group.throughput(Throughput::Elements(n_atoms as u64 * 3)); // 3 components

        group.bench_with_input(
            BenchmarkId::new("accumulate", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    accumulate_forces(
                        black_box(&mut fx),
                        black_box(&mut fy),
                        black_box(&mut fz),
                        black_box(&dfx),
                        black_box(&dfy),
                        black_box(&dfz),
                    );
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("zero_forces", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    zero_forces(black_box(&mut fx), black_box(&mut fy), black_box(&mut fz));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Full Simulation Step Benchmarks
// ============================================================================

/// Benchmark full simulation step (integration + force computation)
fn bench_simulation_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("Simulation_Step");

    for n_atoms in [1000, 10_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let mut sim = Simulation::new(atoms, sim_box, lj, integrator)
            .with_timestep(0.001);

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("single_step", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    sim.step_forward();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark multiple simulation steps (amortizes neighbor list rebuild)
fn bench_simulation_multiple_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("Simulation_MultiStep");

    for n_atoms in [1000, 10_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let mut sim = Simulation::new(atoms.clone(), sim_box, lj.clone(), integrator)
            .with_timestep(0.001);

        let steps = 100;
        group.throughput(Throughput::Elements((n_atoms * steps) as u64));

        group.bench_with_input(
            BenchmarkId::new("100_steps", n_atoms),
            &n_atoms,
            |b, _| {
                // Reset simulation for each iteration
                sim = Simulation::new(atoms.clone(), sim_box, lj.clone(), integrator)
                    .with_timestep(0.001);
                b.iter(|| {
                    for _ in 0..steps {
                        sim.step_forward();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark energy calculations
fn bench_simulation_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("Simulation_Energy");

    for n_atoms in [1000, 10_000] {
        let (atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let sim = Simulation::new(atoms, sim_box, lj, integrator);

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("kinetic_energy", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    black_box(sim.kinetic_energy())
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("potential_energy", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    black_box(sim.potential_energy())
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("total_energy", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    black_box(sim.total_energy())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Scaling Analysis Benchmarks
// ============================================================================

/// Analyze scaling behavior of force computation
fn bench_force_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Scaling_Forces");

    // Logarithmic scaling from 100 to 50000 atoms
    let sizes = [100, 200, 500, 1000, 2000, 5000, 10_000, 20_000, 50_000];

    for &n_atoms in &sizes {
        let (mut atoms, sim_box) = generate_test_system(n_atoms);
        let lj = LennardJones::argon();
        let cutoff = lj.cutoff();

        let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
        verlet.build(&atoms, &sim_box, cutoff);
        let neighbor_list = verlet.neighbor_list();

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("LJ_forces", n_atoms),
            &n_atoms,
            |b, _| {
                b.iter(|| {
                    for atom in atoms.iter_mut() {
                        atom.zero_force();
                    }
                    lj.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                });
            },
        );
    }

    group.finish();
}

/// Analyze scaling behavior of neighbor list building
fn bench_neighbor_list_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Scaling_NeighborList");

    let sizes = [100, 200, 500, 1000, 2000, 5000, 10_000, 20_000, 50_000];
    let cutoff = 2.5;
    let skin = 0.5;

    for &n_atoms in &sizes {
        let (atoms, sim_box) = generate_test_system(n_atoms);

        group.throughput(Throughput::Elements(n_atoms as u64));

        group.bench_with_input(
            BenchmarkId::new("cell_list_build", n_atoms),
            &n_atoms,
            |b, _| {
                let mut cell_list = CellList::new(n_atoms, cutoff, skin);
                b.iter(|| {
                    cell_list.build(black_box(&atoms), black_box(&sim_box), cutoff);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Neural Force Field Benchmarks (conditional on feature)
// ============================================================================

#[cfg(feature = "neural")]
mod neural_benchmarks {
    use super::*;
    use fxnn::force_field::neural::{NeuralForceField, SchNetModel, RadialBasisFunctions};

    /// Benchmark neural force field energy inference
    pub fn bench_neural_energy(c: &mut Criterion) {
        let mut group = c.benchmark_group("Neural_Energy");

        // Smaller systems due to computational cost
        for n_atoms in [50, 100, 200] {
            let (atoms, sim_box) = generate_test_system(n_atoms);
            let cutoff = 5.0;

            // Create a smaller model for benchmarking
            let model = SchNetModel::new(
                32,  // num_features (smaller)
                2,   // num_interactions
                10,  // num_rbf
                cutoff,
                10,  // max_z
            );
            let nff = NeuralForceField::new(model);

            // Build neighbor list
            let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
            verlet.build(&atoms, &sim_box, cutoff);
            let neighbor_list = verlet.neighbor_list();

            group.throughput(Throughput::Elements(n_atoms as u64));

            group.bench_with_input(
                BenchmarkId::new("forward_pass", n_atoms),
                &n_atoms,
                |b, _| {
                    b.iter(|| {
                        nff.potential_energy(black_box(&atoms), black_box(&sim_box), Some(neighbor_list))
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark neural force field force inference
    pub fn bench_neural_forces(c: &mut Criterion) {
        let mut group = c.benchmark_group("Neural_Forces");

        // Very small systems due to numerical gradient cost (6N forward passes)
        for n_atoms in [10, 25, 50] {
            let (mut atoms, sim_box) = generate_test_system(n_atoms);
            let cutoff = 5.0;

            let model = SchNetModel::new(32, 2, 10, cutoff, 10);
            let nff = NeuralForceField::new(model);

            let mut verlet = VerletList::new(n_atoms, cutoff, 0.5);
            verlet.build(&atoms, &sim_box, cutoff);
            let neighbor_list = verlet.neighbor_list();

            group.throughput(Throughput::Elements(n_atoms as u64));

            group.bench_with_input(
                BenchmarkId::new("numerical_gradient", n_atoms),
                &n_atoms,
                |b, _| {
                    b.iter(|| {
                        for atom in atoms.iter_mut() {
                            atom.zero_force();
                        }
                        nff.compute_forces(black_box(&mut atoms), black_box(&sim_box), Some(neighbor_list));
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark RBF expansion (key component)
    pub fn bench_rbf_expansion(c: &mut Criterion) {
        let mut group = c.benchmark_group("Neural_RBF");

        for num_basis in [10, 25, 50, 100] {
            let rbf = RadialBasisFunctions::new(num_basis, 5.0);

            // Test at various distances
            let distances: Vec<f32> = (0..1000).map(|i| (i as f32) * 0.005).collect();

            group.throughput(Throughput::Elements(1000));

            group.bench_with_input(
                BenchmarkId::new("expand", num_basis),
                &num_basis,
                |b, _| {
                    b.iter(|| {
                        for &d in &distances {
                            black_box(rbf.expand(d));
                        }
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("expand_gradient", num_basis),
                &num_basis,
                |b, _| {
                    b.iter(|| {
                        for &d in &distances {
                            black_box(rbf.expand_gradient(d));
                        }
                    });
                },
            );
        }

        group.finish();
    }
}

// ============================================================================
// Benchmark Groups
// ============================================================================

// Main benchmark groups
criterion_group!(
    force_benchmarks,
    bench_lennard_jones_forces,
    bench_lennard_jones_energy,
    bench_coulomb_forces,
    bench_combined_forces,
);

criterion_group!(
    neighbor_benchmarks,
    bench_cell_list_build,
    bench_verlet_list_build,
    bench_neighbor_list_comparison,
    bench_neighbor_list_needs_rebuild,
);

criterion_group!(
    simd_benchmarks,
    bench_simd_pairwise_distances,
    bench_simd_minimum_image,
    bench_simd_distance_cutoff,
    bench_simd_force_accumulation,
);

criterion_group!(
    simulation_benchmarks,
    bench_simulation_step,
    bench_simulation_multiple_steps,
    bench_simulation_energy,
);

criterion_group!(
    scaling_benchmarks,
    bench_force_scaling,
    bench_neighbor_list_scaling,
);

// Neural benchmarks (conditional)
#[cfg(feature = "neural")]
criterion_group!(
    neural_benchmarks_group,
    neural_benchmarks::bench_neural_energy,
    neural_benchmarks::bench_neural_forces,
    neural_benchmarks::bench_rbf_expansion,
);

// Main entry point
#[cfg(not(feature = "neural"))]
criterion_main!(
    force_benchmarks,
    neighbor_benchmarks,
    simd_benchmarks,
    simulation_benchmarks,
    scaling_benchmarks,
);

#[cfg(feature = "neural")]
criterion_main!(
    force_benchmarks,
    neighbor_benchmarks,
    simd_benchmarks,
    simulation_benchmarks,
    scaling_benchmarks,
    neural_benchmarks_group,
);
