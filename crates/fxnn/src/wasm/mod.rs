//! WebAssembly bindings for FXNN molecular dynamics simulation.
//!
//! This module provides JavaScript-friendly wrappers around the core FXNN
//! simulation engine, enabling high-performance molecular dynamics simulations
//! to run directly in web browsers.
//!
//! # Overview
//!
//! The WASM bindings expose:
//!
//! - [`WasmSimulation`]: Main simulation wrapper with step/run controls
//! - [`WasmVisualization`]: Data export for WebGL/Three.js rendering
//! - [`WasmBenchmark`]: Browser-based performance benchmarks
//!
//! # Usage from JavaScript
//!
//! ```javascript
//! import init, { WasmSimulation, WasmVisualization } from 'fxnn';
//!
//! async function runSimulation() {
//!     await init();
//!
//!     // Create simulation with 256 atoms in FCC lattice
//!     const sim = WasmSimulation.new_fcc(4, 4, 4, 1.5, 1.0);
//!     sim.set_timestep(0.001);
//!
//!     // Run 1000 steps
//!     sim.run(1000);
//!
//!     // Get data for visualization
//!     const positions = sim.get_positions();  // Float32Array
//!     const velocities = sim.get_velocities(); // Float32Array
//!     console.log(`Energy: ${sim.get_total_energy()}`);
//! }
//! ```
//!
//! # Performance
//!
//! WASM performance is typically 50-80% of native Rust performance,
//! significantly faster than equivalent JavaScript implementations.
//! For large systems (1000+ atoms), the performance advantage grows.

mod simulation;
mod visualization;
mod benchmark;
mod mcp;

pub use simulation::WasmSimulation;
pub use visualization::{WasmVisualization, AtomRenderData, TrajectoryFrame};
pub use benchmark::{WasmBenchmark, BenchmarkResult};
pub use mcp::McpHandler;

use wasm_bindgen::prelude::*;

/// Initialize the WASM module with panic hook for better error messages.
///
/// This should be called once at the start of your application.
/// It sets up proper panic handling so that Rust panics are converted
/// to JavaScript exceptions with readable stack traces.
///
/// # Example
///
/// ```javascript
/// import init, { wasm_init } from 'fxnn';
///
/// async function main() {
///     await init();
///     wasm_init();  // Enable better error messages
///     // ... your code
/// }
/// ```
#[wasm_bindgen]
pub fn wasm_init() {
    console_error_panic_hook::set_once();
}

/// Get the current performance timestamp in milliseconds.
///
/// Uses `performance.now()` when available in the browser,
/// falling back to a simple counter otherwise.
///
/// # Returns
///
/// High-resolution timestamp in milliseconds.
#[wasm_bindgen]
pub fn get_performance_now() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(performance) = window.performance().ok_or(()) {
                return performance.now();
            }
        }
    }
    0.0
}

/// Log a message to the browser console.
///
/// # Arguments
///
/// * `msg` - Message to log
#[wasm_bindgen]
pub fn console_log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

/// Log an error message to the browser console.
///
/// # Arguments
///
/// * `msg` - Error message to log
#[wasm_bindgen]
pub fn console_error(msg: &str) {
    web_sys::console::error_1(&JsValue::from_str(msg));
}

/// Get the library version.
///
/// # Returns
///
/// Version string in semver format.
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if SIMD is available in the WASM runtime.
///
/// Note: This checks compile-time SIMD feature, not runtime support.
/// For actual SIMD detection, use JavaScript feature detection.
///
/// # Returns
///
/// `true` if compiled with SIMD support.
#[wasm_bindgen]
pub fn has_simd_support() -> bool {
    cfg!(feature = "simd")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = get_version();
        assert!(!version.is_empty());
    }
}
