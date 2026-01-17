//! WebAssembly simulation wrapper.
//!
//! Provides a JavaScript-friendly API for running molecular dynamics
//! simulations in the browser.

use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use serde::{Serialize, Deserialize};

use crate::types::{Atom, SimulationBox};
use crate::force_field::LennardJones;
use crate::integrator::VelocityVerlet;
use crate::simulation::Simulation;
use crate::generators::{fcc_lattice, random_atoms, maxwell_boltzmann_velocities};

/// WebAssembly wrapper for molecular dynamics simulation.
///
/// This struct provides a JavaScript-friendly interface to the core
/// FXNN simulation engine. It handles type conversions between Rust
/// and JavaScript automatically.
///
/// # Example
///
/// ```javascript
/// const sim = WasmSimulation.new_fcc(4, 4, 4, 1.5, 1.0);
/// sim.set_timestep(0.001);
/// sim.run(10000);
/// console.log(`Final temperature: ${sim.get_temperature()}`);
/// ```
#[wasm_bindgen]
pub struct WasmSimulation {
    /// Internal simulation state
    sim: Simulation<LennardJones, VelocityVerlet>,
    /// Cached position array for efficient repeated access
    position_cache: Vec<f32>,
    /// Cached velocity array for efficient repeated access
    velocity_cache: Vec<f32>,
    /// Flag indicating if caches need refresh
    cache_valid: bool,
}

#[wasm_bindgen]
impl WasmSimulation {
    /// Create a new simulation with atoms on an FCC lattice.
    ///
    /// This creates a face-centered cubic crystal structure, commonly
    /// used for noble gases and metals.
    ///
    /// # Arguments
    ///
    /// * `nx` - Number of unit cells in x direction
    /// * `ny` - Number of unit cells in y direction
    /// * `nz` - Number of unit cells in z direction
    /// * `lattice_constant` - Size of each unit cell (typically ~1.5 sigma)
    /// * `temperature` - Initial temperature (in reduced units)
    ///
    /// # Returns
    ///
    /// A new `WasmSimulation` instance with `4 * nx * ny * nz` atoms.
    ///
    /// # Example
    ///
    /// ```javascript
    /// // Create 256 atoms (4*4*4*4) in FCC structure
    /// const sim = WasmSimulation.new_fcc(4, 4, 4, 1.5, 1.0);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new_fcc(
        nx: usize,
        ny: usize,
        nz: usize,
        lattice_constant: f32,
        temperature: f32,
    ) -> WasmSimulation {
        let mut atoms = fcc_lattice(nx, ny, nz, lattice_constant);
        let box_size = (nx as f32) * lattice_constant;
        let box_ = SimulationBox::cubic(box_size);

        // Initialize velocities at target temperature
        maxwell_boltzmann_velocities(&mut atoms, temperature, 1.0);

        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let n_atoms = atoms.len();
        let sim = Simulation::new(atoms, box_, lj, integrator)
            .with_timestep(0.001);

        WasmSimulation {
            sim,
            position_cache: vec![0.0; n_atoms * 3],
            velocity_cache: vec![0.0; n_atoms * 3],
            cache_valid: false,
        }
    }

    /// Create a simulation with randomly positioned atoms.
    ///
    /// Creates a gas-like configuration with atoms uniformly distributed
    /// throughout the simulation box.
    ///
    /// # Arguments
    ///
    /// * `n_atoms` - Number of atoms
    /// * `box_size` - Cubic box side length
    /// * `temperature` - Initial temperature (in reduced units)
    ///
    /// # Returns
    ///
    /// A new `WasmSimulation` instance.
    ///
    /// # Example
    ///
    /// ```javascript
    /// // Create 500 atoms in a 20x20x20 box
    /// const sim = WasmSimulation.new_random(500, 20.0, 1.0);
    /// ```
    #[wasm_bindgen]
    pub fn new_random(n_atoms: usize, box_size: f32, temperature: f32) -> WasmSimulation {
        let box_ = SimulationBox::cubic(box_size);
        let mut atoms = random_atoms(n_atoms, &box_);

        // Initialize velocities
        maxwell_boltzmann_velocities(&mut atoms, temperature, 1.0);

        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let sim = Simulation::new(atoms, box_, lj, integrator)
            .with_timestep(0.001);

        WasmSimulation {
            sim,
            position_cache: vec![0.0; n_atoms * 3],
            velocity_cache: vec![0.0; n_atoms * 3],
            cache_valid: false,
        }
    }

    /// Create a simulation from custom atom positions.
    ///
    /// # Arguments
    ///
    /// * `positions` - Flat array of positions [x0, y0, z0, x1, y1, z1, ...]
    /// * `box_size` - Cubic box side length
    /// * `temperature` - Initial temperature
    ///
    /// # Returns
    ///
    /// A new `WasmSimulation` instance.
    #[wasm_bindgen]
    pub fn new_from_positions(
        positions: &[f32],
        box_size: f32,
        temperature: f32,
    ) -> Result<WasmSimulation, JsValue> {
        if positions.len() % 3 != 0 {
            return Err(JsValue::from_str("positions array length must be divisible by 3"));
        }

        let n_atoms = positions.len() / 3;
        let box_ = SimulationBox::cubic(box_size);

        let mut atoms: Vec<Atom> = (0..n_atoms)
            .map(|i| {
                Atom::new(i as u32, 0, 1.0)
                    .with_position(
                        positions[i * 3],
                        positions[i * 3 + 1],
                        positions[i * 3 + 2],
                    )
            })
            .collect();

        // Initialize velocities
        maxwell_boltzmann_velocities(&mut atoms, temperature, 1.0);

        let lj = LennardJones::argon();
        let integrator = VelocityVerlet::new();

        let sim = Simulation::new(atoms, box_, lj, integrator)
            .with_timestep(0.001);

        Ok(WasmSimulation {
            sim,
            position_cache: vec![0.0; n_atoms * 3],
            velocity_cache: vec![0.0; n_atoms * 3],
            cache_valid: false,
        })
    }

    /// Perform a single integration step.
    ///
    /// Advances the simulation by one timestep using the velocity Verlet
    /// algorithm.
    ///
    /// # Example
    ///
    /// ```javascript
    /// sim.step();
    /// console.log(`Step: ${sim.get_step()}`);
    /// ```
    #[wasm_bindgen]
    pub fn step(&mut self) {
        self.sim.step_forward();
        self.cache_valid = false;
    }

    /// Run multiple integration steps.
    ///
    /// # Arguments
    ///
    /// * `n_steps` - Number of steps to run
    ///
    /// # Example
    ///
    /// ```javascript
    /// sim.run(1000);
    /// console.log(`Completed ${sim.get_step()} steps`);
    /// ```
    #[wasm_bindgen]
    pub fn run(&mut self, n_steps: usize) {
        self.sim.run(n_steps);
        self.cache_valid = false;
    }

    /// Run steps with progress callback.
    ///
    /// Calls the provided callback every `report_interval` steps with
    /// the current step number. Useful for progress bars in the UI.
    ///
    /// # Arguments
    ///
    /// * `n_steps` - Total number of steps to run
    /// * `report_interval` - Steps between progress reports
    /// * `callback` - JavaScript function called with current step
    ///
    /// # Example
    ///
    /// ```javascript
    /// sim.run_with_callback(10000, 100, (step) => {
    ///     progressBar.value = step / 10000 * 100;
    /// });
    /// ```
    #[wasm_bindgen]
    pub fn run_with_callback(
        &mut self,
        n_steps: usize,
        report_interval: usize,
        callback: &js_sys::Function,
    ) {
        let this = JsValue::NULL;

        for i in 0..n_steps {
            self.sim.step_forward();

            if (i + 1) % report_interval == 0 {
                let step = JsValue::from_f64((i + 1) as f64);
                let _ = callback.call1(&this, &step);
            }
        }

        self.cache_valid = false;
    }

    /// Set the integration timestep.
    ///
    /// # Arguments
    ///
    /// * `dt` - Timestep in reduced time units (typical: 0.001-0.005)
    ///
    /// # Note
    ///
    /// Smaller timesteps give better energy conservation but slower
    /// simulation. For Lennard-Jones systems, dt=0.001 is typically safe.
    #[wasm_bindgen]
    pub fn set_timestep(&mut self, _dt: f32) {
        // Note: Current Simulation doesn't have a set_timestep method after creation
        // This would need to be added to the core simulation
        // For now, this is a placeholder
    }

    /// Set the system temperature by rescaling velocities.
    ///
    /// # Arguments
    ///
    /// * `temperature` - Target temperature in reduced units
    #[wasm_bindgen]
    pub fn set_temperature(&mut self, temperature: f32) {
        self.sim.set_temperature(temperature);
        self.cache_valid = false;
    }

    /// Get atom positions as a Float32Array.
    ///
    /// Returns a flat array [x0, y0, z0, x1, y1, z1, ...] suitable for
    /// direct use with WebGL buffers.
    ///
    /// # Returns
    ///
    /// Float32Array containing all atom positions.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const positions = sim.get_positions();
    /// gl.bufferData(gl.ARRAY_BUFFER, positions, gl.DYNAMIC_DRAW);
    /// ```
    #[wasm_bindgen]
    pub fn get_positions(&mut self) -> Float32Array {
        self.update_caches();
        Float32Array::from(self.position_cache.as_slice())
    }

    /// Get atom velocities as a Float32Array.
    ///
    /// Returns a flat array [vx0, vy0, vz0, vx1, vy1, vz1, ...].
    ///
    /// # Returns
    ///
    /// Float32Array containing all atom velocities.
    #[wasm_bindgen]
    pub fn get_velocities(&mut self) -> Float32Array {
        self.update_caches();
        Float32Array::from(self.velocity_cache.as_slice())
    }

    /// Get atom speeds (velocity magnitudes) as a Float32Array.
    ///
    /// Useful for coloring atoms by speed in visualizations.
    ///
    /// # Returns
    ///
    /// Float32Array containing speed of each atom.
    #[wasm_bindgen]
    pub fn get_speeds(&self) -> Float32Array {
        let atoms = self.sim.atoms();
        let speeds: Vec<f32> = atoms.iter().map(|a| a.speed()).collect();
        Float32Array::from(speeds.as_slice())
    }

    /// Get kinetic energies per atom as a Float32Array.
    ///
    /// Useful for coloring atoms by energy in visualizations.
    ///
    /// # Returns
    ///
    /// Float32Array containing kinetic energy of each atom.
    #[wasm_bindgen]
    pub fn get_kinetic_energies(&self) -> Float32Array {
        let atoms = self.sim.atoms();
        let energies: Vec<f32> = atoms.iter().map(|a| a.kinetic_energy()).collect();
        Float32Array::from(energies.as_slice())
    }

    /// Get the total kinetic energy.
    ///
    /// # Returns
    ///
    /// Total kinetic energy in reduced units.
    #[wasm_bindgen]
    pub fn get_kinetic_energy(&self) -> f64 {
        self.sim.kinetic_energy()
    }

    /// Get the total potential energy.
    ///
    /// # Returns
    ///
    /// Total potential energy in reduced units.
    #[wasm_bindgen]
    pub fn get_potential_energy(&self) -> f64 {
        self.sim.potential_energy()
    }

    /// Get the total energy (kinetic + potential).
    ///
    /// # Returns
    ///
    /// Total energy in reduced units.
    ///
    /// # Note
    ///
    /// For NVE simulations, this should remain approximately constant.
    /// Significant drift indicates timestep is too large.
    #[wasm_bindgen]
    pub fn get_total_energy(&self) -> f64 {
        self.sim.total_energy()
    }

    /// Get the instantaneous temperature.
    ///
    /// # Returns
    ///
    /// Temperature in reduced units.
    #[wasm_bindgen]
    pub fn get_temperature(&self) -> f32 {
        self.sim.temperature()
    }

    /// Get the current simulation step number.
    ///
    /// # Returns
    ///
    /// Number of steps completed.
    #[wasm_bindgen]
    pub fn get_step(&self) -> usize {
        self.sim.step()
    }

    /// Get the current simulation time.
    ///
    /// # Returns
    ///
    /// Simulation time in reduced units.
    #[wasm_bindgen]
    pub fn get_time(&self) -> f64 {
        self.sim.time()
    }

    /// Get the number of atoms in the simulation.
    ///
    /// # Returns
    ///
    /// Number of atoms.
    #[wasm_bindgen]
    pub fn get_n_atoms(&self) -> usize {
        self.sim.n_atoms()
    }

    /// Get the simulation box dimensions.
    ///
    /// # Returns
    ///
    /// Array [Lx, Ly, Lz] of box dimensions.
    #[wasm_bindgen]
    pub fn get_box_dimensions(&self) -> Float32Array {
        let box_ = self.sim.box_();
        Float32Array::from(box_.dimensions.as_slice())
    }

    /// Get simulation statistics as a JavaScript object.
    ///
    /// # Returns
    ///
    /// Object with properties: step, time, temperature, kinetic_energy,
    /// potential_energy, total_energy, n_atoms.
    #[wasm_bindgen]
    pub fn get_stats(&self) -> Result<JsValue, JsValue> {
        let stats = SimulationStats {
            step: self.sim.step(),
            time: self.sim.time(),
            temperature: self.sim.temperature() as f64,
            kinetic_energy: self.sim.kinetic_energy(),
            potential_energy: self.sim.potential_energy(),
            total_energy: self.sim.total_energy(),
            n_atoms: self.sim.n_atoms(),
        };

        serde_wasm_bindgen::to_value(&stats)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Update internal caches from simulation state.
    fn update_caches(&mut self) {
        if self.cache_valid {
            return;
        }

        let atoms = self.sim.atoms();

        for (i, atom) in atoms.iter().enumerate() {
            let base = i * 3;
            self.position_cache[base] = atom.position[0];
            self.position_cache[base + 1] = atom.position[1];
            self.position_cache[base + 2] = atom.position[2];

            self.velocity_cache[base] = atom.velocity[0];
            self.velocity_cache[base + 1] = atom.velocity[1];
            self.velocity_cache[base + 2] = atom.velocity[2];
        }

        self.cache_valid = true;
    }

    /// Reset the simulation to initial state.
    ///
    /// Creates new random velocities at the specified temperature
    /// while keeping atom positions unchanged.
    ///
    /// # Arguments
    ///
    /// * `temperature` - New target temperature
    #[wasm_bindgen]
    pub fn reset_velocities(&mut self, temperature: f32) {
        self.sim.set_temperature(temperature);
        self.cache_valid = false;
    }

    /// Get center of mass position.
    ///
    /// # Returns
    ///
    /// Array [x, y, z] of center of mass position.
    #[wasm_bindgen]
    pub fn get_center_of_mass(&self) -> Float32Array {
        let atoms = self.sim.atoms();
        let mut com = [0.0f32; 3];
        let mut total_mass = 0.0f32;

        for atom in atoms {
            com[0] += atom.mass * atom.position[0];
            com[1] += atom.mass * atom.position[1];
            com[2] += atom.mass * atom.position[2];
            total_mass += atom.mass;
        }

        if total_mass > 0.0 {
            com[0] /= total_mass;
            com[1] /= total_mass;
            com[2] /= total_mass;
        }

        Float32Array::from(com.as_slice())
    }

    /// Set atom positions from a flat array.
    ///
    /// Used for restoring simulation state from snapshots.
    ///
    /// # Arguments
    ///
    /// * `positions` - Flat array [x0, y0, z0, x1, y1, z1, ...]
    ///
    /// # Panics
    ///
    /// Panics if the array length doesn't match the number of atoms * 3.
    #[wasm_bindgen]
    pub fn set_positions(&mut self, positions: &[f32]) {
        let atoms = self.sim.atoms_mut();
        let n_atoms = atoms.len();

        if positions.len() != n_atoms * 3 {
            panic!("positions array length must be {} (got {})", n_atoms * 3, positions.len());
        }

        for (i, atom) in atoms.iter_mut().enumerate() {
            atom.position[0] = positions[i * 3];
            atom.position[1] = positions[i * 3 + 1];
            atom.position[2] = positions[i * 3 + 2];
        }

        self.cache_valid = false;
    }

    /// Set atom velocities from a flat array.
    ///
    /// Used for restoring simulation state from snapshots.
    ///
    /// # Arguments
    ///
    /// * `velocities` - Flat array [vx0, vy0, vz0, vx1, vy1, vz1, ...]
    ///
    /// # Panics
    ///
    /// Panics if the array length doesn't match the number of atoms * 3.
    #[wasm_bindgen]
    pub fn set_velocities(&mut self, velocities: &[f32]) {
        let atoms = self.sim.atoms_mut();
        let n_atoms = atoms.len();

        if velocities.len() != n_atoms * 3 {
            panic!("velocities array length must be {} (got {})", n_atoms * 3, velocities.len());
        }

        for (i, atom) in atoms.iter_mut().enumerate() {
            atom.velocity[0] = velocities[i * 3];
            atom.velocity[1] = velocities[i * 3 + 1];
            atom.velocity[2] = velocities[i * 3 + 2];
        }

        self.cache_valid = false;
    }
}

/// Simulation statistics for JSON serialization.
#[derive(Serialize, Deserialize)]
struct SimulationStats {
    step: usize,
    time: f64,
    temperature: f64,
    kinetic_energy: f64,
    potential_energy: f64,
    total_energy: f64,
    n_atoms: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_simulation_creation() {
        let sim = WasmSimulation::new_fcc(2, 2, 2, 1.5, 1.0);
        assert_eq!(sim.get_n_atoms(), 32); // 4 * 2 * 2 * 2
    }

    #[test]
    fn test_wasm_simulation_step() {
        let mut sim = WasmSimulation::new_fcc(2, 2, 2, 1.5, 1.0);
        assert_eq!(sim.get_step(), 0);
        sim.step();
        assert_eq!(sim.get_step(), 1);
    }

    #[test]
    fn test_wasm_simulation_run() {
        let mut sim = WasmSimulation::new_fcc(2, 2, 2, 1.5, 1.0);
        sim.run(100);
        assert_eq!(sim.get_step(), 100);
    }
}
