//! WebGL/Three.js visualization data export.
//!
//! This module provides utilities for exporting simulation data in formats
//! suitable for 3D rendering in web browsers using WebGL, Three.js, or
//! similar graphics libraries.

use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use serde::{Serialize, Deserialize};

use crate::wasm::simulation::WasmSimulation;

/// Atom rendering data optimized for GPU upload.
///
/// Contains all per-atom data needed for visualization in a format
/// suitable for WebGL vertex buffers.
#[wasm_bindgen]
#[derive(Clone)]
pub struct AtomRenderData {
    /// Positions: [x0, y0, z0, x1, y1, z1, ...]
    positions: Vec<f32>,
    /// Colors: [r0, g0, b0, a0, r1, g1, b1, a1, ...]
    colors: Vec<f32>,
    /// Radii for each atom
    radii: Vec<f32>,
    /// Number of atoms
    n_atoms: usize,
}

#[wasm_bindgen]
impl AtomRenderData {
    /// Get positions as Float32Array for WebGL buffer.
    #[wasm_bindgen]
    pub fn positions(&self) -> Float32Array {
        Float32Array::from(self.positions.as_slice())
    }

    /// Get colors as Float32Array for WebGL buffer.
    #[wasm_bindgen]
    pub fn colors(&self) -> Float32Array {
        Float32Array::from(self.colors.as_slice())
    }

    /// Get radii as Float32Array for WebGL buffer.
    #[wasm_bindgen]
    pub fn radii(&self) -> Float32Array {
        Float32Array::from(self.radii.as_slice())
    }

    /// Get number of atoms.
    #[wasm_bindgen]
    pub fn n_atoms(&self) -> usize {
        self.n_atoms
    }
}

/// Trajectory frame for animation playback.
///
/// Stores a snapshot of atom positions at a single timestep,
/// compressed for efficient storage of long trajectories.
#[wasm_bindgen]
#[derive(Clone)]
pub struct TrajectoryFrame {
    /// Step number when this frame was captured
    step: usize,
    /// Simulation time
    time: f64,
    /// Compressed positions (relative to previous frame or absolute)
    positions: Vec<f32>,
    /// Temperature at this frame
    temperature: f32,
    /// Total energy at this frame
    energy: f64,
}

#[wasm_bindgen]
impl TrajectoryFrame {
    /// Get the step number.
    #[wasm_bindgen]
    pub fn step(&self) -> usize {
        self.step
    }

    /// Get the simulation time.
    #[wasm_bindgen]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Get positions as Float32Array.
    #[wasm_bindgen]
    pub fn positions(&self) -> Float32Array {
        Float32Array::from(self.positions.as_slice())
    }

    /// Get temperature.
    #[wasm_bindgen]
    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    /// Get total energy.
    #[wasm_bindgen]
    pub fn energy(&self) -> f64 {
        self.energy
    }
}

/// Visualization helper for molecular dynamics rendering.
///
/// Provides utilities for extracting visualization data from simulations,
/// color mapping, and trajectory recording.
#[wasm_bindgen]
pub struct WasmVisualization {
    /// Color palette for velocity-based coloring
    velocity_palette: ColorPalette,
    /// Color palette for energy-based coloring
    energy_palette: ColorPalette,
    /// Recorded trajectory frames
    trajectory: Vec<TrajectoryFrame>,
    /// Maximum trajectory length (0 = unlimited)
    max_trajectory_length: usize,
}

#[wasm_bindgen]
impl WasmVisualization {
    /// Create a new visualization helper.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmVisualization {
        WasmVisualization {
            velocity_palette: ColorPalette::viridis(),
            energy_palette: ColorPalette::plasma(),
            trajectory: Vec::new(),
            max_trajectory_length: 0,
        }
    }

    /// Set maximum trajectory length.
    ///
    /// When set to 0, trajectory length is unlimited.
    /// When set to N > 0, only the last N frames are kept.
    #[wasm_bindgen]
    pub fn set_max_trajectory_length(&mut self, max_length: usize) {
        self.max_trajectory_length = max_length;
        if max_length > 0 && self.trajectory.len() > max_length {
            let excess = self.trajectory.len() - max_length;
            self.trajectory.drain(0..excess);
        }
    }

    /// Record current frame to trajectory.
    #[wasm_bindgen]
    pub fn record_frame(&mut self, sim: &mut WasmSimulation) {
        let step = sim.get_step();
        let time = sim.get_time();
        let temperature = sim.get_temperature();
        let energy = sim.get_total_energy();

        // Get positions
        let positions_arr = sim.get_positions();
        let mut positions = vec![0.0f32; positions_arr.length() as usize];
        positions_arr.copy_to(&mut positions);

        let frame = TrajectoryFrame {
            step,
            time,
            positions,
            temperature,
            energy,
        };

        self.trajectory.push(frame);

        // Trim if needed
        if self.max_trajectory_length > 0 && self.trajectory.len() > self.max_trajectory_length {
            self.trajectory.remove(0);
        }
    }

    /// Get number of recorded frames.
    #[wasm_bindgen]
    pub fn trajectory_length(&self) -> usize {
        self.trajectory.len()
    }

    /// Get a specific frame from the trajectory.
    #[wasm_bindgen]
    pub fn get_frame(&self, index: usize) -> Option<TrajectoryFrame> {
        self.trajectory.get(index).cloned()
    }

    /// Clear recorded trajectory.
    #[wasm_bindgen]
    pub fn clear_trajectory(&mut self) {
        self.trajectory.clear();
    }

    /// Get render data with atoms colored by velocity.
    ///
    /// Fast atoms are colored with warm colors, slow atoms with cool colors.
    ///
    /// # Arguments
    ///
    /// * `sim` - The simulation to extract data from
    /// * `atom_radius` - Radius for all atoms
    /// * `max_velocity` - Maximum velocity for color scale normalization
    #[wasm_bindgen]
    pub fn get_render_data_by_velocity(
        &self,
        sim: &mut WasmSimulation,
        atom_radius: f32,
        max_velocity: f32,
    ) -> AtomRenderData {
        let n_atoms = sim.get_n_atoms();
        let positions_arr = sim.get_positions();
        let speeds_arr = sim.get_speeds();

        let mut positions = vec![0.0f32; n_atoms * 3];
        positions_arr.copy_to(&mut positions);

        let mut speeds = vec![0.0f32; n_atoms];
        speeds_arr.copy_to(&mut speeds);

        let mut colors = Vec::with_capacity(n_atoms * 4);
        let radii = vec![atom_radius; n_atoms];

        for speed in speeds {
            let t = (speed / max_velocity).min(1.0).max(0.0);
            let (r, g, b) = self.velocity_palette.sample(t);
            colors.push(r);
            colors.push(g);
            colors.push(b);
            colors.push(1.0); // Alpha
        }

        AtomRenderData {
            positions,
            colors,
            radii,
            n_atoms,
        }
    }

    /// Get render data with atoms colored by kinetic energy.
    ///
    /// # Arguments
    ///
    /// * `sim` - The simulation to extract data from
    /// * `atom_radius` - Radius for all atoms
    /// * `max_energy` - Maximum energy for color scale normalization
    #[wasm_bindgen]
    pub fn get_render_data_by_energy(
        &self,
        sim: &mut WasmSimulation,
        atom_radius: f32,
        max_energy: f32,
    ) -> AtomRenderData {
        let n_atoms = sim.get_n_atoms();
        let positions_arr = sim.get_positions();
        let energies_arr = sim.get_kinetic_energies();

        let mut positions = vec![0.0f32; n_atoms * 3];
        positions_arr.copy_to(&mut positions);

        let mut energies = vec![0.0f32; n_atoms];
        energies_arr.copy_to(&mut energies);

        let mut colors = Vec::with_capacity(n_atoms * 4);
        let radii = vec![atom_radius; n_atoms];

        for energy in energies {
            let t = (energy / max_energy).min(1.0).max(0.0);
            let (r, g, b) = self.energy_palette.sample(t);
            colors.push(r);
            colors.push(g);
            colors.push(b);
            colors.push(1.0); // Alpha
        }

        AtomRenderData {
            positions,
            colors,
            radii,
            n_atoms,
        }
    }

    /// Get render data with uniform color.
    ///
    /// # Arguments
    ///
    /// * `sim` - The simulation to extract data from
    /// * `atom_radius` - Radius for all atoms
    /// * `r`, `g`, `b` - RGB color components (0.0-1.0)
    #[wasm_bindgen]
    pub fn get_render_data_uniform(
        &self,
        sim: &mut WasmSimulation,
        atom_radius: f32,
        r: f32,
        g: f32,
        b: f32,
    ) -> AtomRenderData {
        let n_atoms = sim.get_n_atoms();
        let positions_arr = sim.get_positions();

        let mut positions = vec![0.0f32; n_atoms * 3];
        positions_arr.copy_to(&mut positions);

        let mut colors = Vec::with_capacity(n_atoms * 4);
        let radii = vec![atom_radius; n_atoms];

        for _ in 0..n_atoms {
            colors.push(r);
            colors.push(g);
            colors.push(b);
            colors.push(1.0);
        }

        AtomRenderData {
            positions,
            colors,
            radii,
            n_atoms,
        }
    }

    /// Export trajectory as JSON string.
    ///
    /// # Returns
    ///
    /// JSON string containing all trajectory frames.
    #[wasm_bindgen]
    pub fn export_trajectory_json(&self) -> Result<String, JsValue> {
        let frames: Vec<TrajectoryData> = self.trajectory
            .iter()
            .map(|f| TrajectoryData {
                step: f.step,
                time: f.time,
                positions: f.positions.clone(),
                temperature: f.temperature,
                energy: f.energy,
            })
            .collect();

        serde_json::to_string(&frames)
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }

    /// Calculate velocity distribution histogram.
    ///
    /// # Arguments
    ///
    /// * `sim` - The simulation
    /// * `n_bins` - Number of histogram bins
    /// * `max_velocity` - Maximum velocity for binning
    ///
    /// # Returns
    ///
    /// Float32Array with normalized bin counts.
    #[wasm_bindgen]
    pub fn velocity_histogram(
        &self,
        sim: &WasmSimulation,
        n_bins: usize,
        max_velocity: f32,
    ) -> Float32Array {
        let speeds_arr = sim.get_speeds();
        let n_atoms = speeds_arr.length() as usize;
        let mut speeds = vec![0.0f32; n_atoms];
        speeds_arr.copy_to(&mut speeds);

        let bin_width = max_velocity / n_bins as f32;
        let mut histogram = vec![0.0f32; n_bins];

        for speed in speeds {
            let bin = ((speed / bin_width) as usize).min(n_bins - 1);
            histogram[bin] += 1.0;
        }

        // Normalize
        let total: f32 = histogram.iter().sum();
        if total > 0.0 {
            for count in &mut histogram {
                *count /= total * bin_width; // Density normalization
            }
        }

        Float32Array::from(histogram.as_slice())
    }
}

impl Default for WasmVisualization {
    fn default() -> Self {
        Self::new()
    }
}

/// Color palette for visualization.
struct ColorPalette {
    /// Color stops as (position, r, g, b)
    stops: Vec<(f32, f32, f32, f32)>,
}

impl ColorPalette {
    /// Viridis colormap (perceptually uniform, colorblind friendly).
    fn viridis() -> Self {
        ColorPalette {
            stops: vec![
                (0.0, 0.267, 0.004, 0.329),   // Dark purple
                (0.25, 0.282, 0.140, 0.458),  // Purple
                (0.5, 0.127, 0.566, 0.551),   // Teal
                (0.75, 0.369, 0.788, 0.383),  // Green
                (1.0, 0.993, 0.906, 0.144),   // Yellow
            ],
        }
    }

    /// Plasma colormap.
    fn plasma() -> Self {
        ColorPalette {
            stops: vec![
                (0.0, 0.050, 0.030, 0.527),   // Dark blue
                (0.25, 0.417, 0.000, 0.658),  // Purple
                (0.5, 0.828, 0.211, 0.482),   // Pink
                (0.75, 0.985, 0.490, 0.245),  // Orange
                (1.0, 0.940, 0.975, 0.131),   // Yellow
            ],
        }
    }

    /// Sample color at position t (0.0-1.0).
    fn sample(&self, t: f32) -> (f32, f32, f32) {
        let t = t.max(0.0).min(1.0);

        // Find surrounding stops
        let mut lower_idx = 0;
        for (i, stop) in self.stops.iter().enumerate() {
            if stop.0 <= t {
                lower_idx = i;
            }
        }

        let upper_idx = (lower_idx + 1).min(self.stops.len() - 1);

        let lower = &self.stops[lower_idx];
        let upper = &self.stops[upper_idx];

        if (upper.0 - lower.0).abs() < 1e-6 {
            return (lower.1, lower.2, lower.3);
        }

        // Interpolate
        let local_t = (t - lower.0) / (upper.0 - lower.0);
        let r = lower.1 + local_t * (upper.1 - lower.1);
        let g = lower.2 + local_t * (upper.2 - lower.2);
        let b = lower.3 + local_t * (upper.3 - lower.3);

        (r, g, b)
    }
}

/// Serializable trajectory data for export.
#[derive(Serialize, Deserialize)]
struct TrajectoryData {
    step: usize,
    time: f64,
    positions: Vec<f32>,
    temperature: f32,
    energy: f64,
}

/// Create a bonds buffer for connected atoms.
///
/// This is a standalone function for generating bond geometry.
///
/// # Arguments
///
/// * `positions` - Flat array of atom positions
/// * `bond_pairs` - Flat array of atom index pairs [i0, j0, i1, j1, ...]
///
/// # Returns
///
/// Float32Array of line segment vertices suitable for WebGL LINE_STRIP.
#[wasm_bindgen]
pub fn create_bonds_buffer(
    positions: &[f32],
    bond_pairs: &[u32],
) -> Result<Float32Array, JsValue> {
    if bond_pairs.len() % 2 != 0 {
        return Err(JsValue::from_str("bond_pairs must have even length"));
    }

    let n_bonds = bond_pairs.len() / 2;
    let mut vertices = Vec::with_capacity(n_bonds * 6); // 2 points per bond, 3 floats each

    for i in 0..n_bonds {
        let idx_a = bond_pairs[i * 2] as usize;
        let idx_b = bond_pairs[i * 2 + 1] as usize;

        // First vertex
        vertices.push(positions[idx_a * 3]);
        vertices.push(positions[idx_a * 3 + 1]);
        vertices.push(positions[idx_a * 3 + 2]);

        // Second vertex
        vertices.push(positions[idx_b * 3]);
        vertices.push(positions[idx_b * 3 + 1]);
        vertices.push(positions[idx_b * 3 + 2]);
    }

    Ok(Float32Array::from(vertices.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_palette_viridis() {
        let palette = ColorPalette::viridis();

        // Test endpoints
        let (r, g, b) = palette.sample(0.0);
        assert!((r - 0.267).abs() < 0.01);

        let (r, g, b) = palette.sample(1.0);
        assert!((r - 0.993).abs() < 0.01);

        // Test interpolation
        let (r, g, b) = palette.sample(0.5);
        assert!(r >= 0.0 && r <= 1.0);
    }

    #[test]
    fn test_visualization_new() {
        let viz = WasmVisualization::new();
        assert_eq!(viz.trajectory_length(), 0);
    }
}
