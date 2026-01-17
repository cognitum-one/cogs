//! WebAssembly bindings for ThermalBrain
//!
//! This module provides JavaScript-friendly APIs for using ThermalBrain
//! in web browsers and Node.js environments.

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use crate::{ThermalBrain, ThermalBrainConfig, ThermalZone};

/// ThermalBrain WASM wrapper
#[wasm_bindgen]
pub struct WasmThermalBrain {
    brain: ThermalBrain,
}

#[wasm_bindgen]
impl WasmThermalBrain {
    /// Create a new ThermalBrain instance with default configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            brain: ThermalBrain::default_config(),
        }
    }

    /// Create with custom configuration
    #[wasm_bindgen]
    pub fn with_config(
        target_temp_c: f32,
        ema_alpha: f32,
        num_neurons: usize,
        max_patterns: usize,
    ) -> Self {
        let config = ThermalBrainConfig {
            thermal: crate::config::ThermalConfig {
                target_temp_c,
                ema_alpha,
                ..Default::default()
            },
            neural: crate::config::NeuralConfig {
                num_neurons,
                ..Default::default()
            },
            storage: crate::config::StorageConfig {
                max_patterns,
                ..Default::default()
            },
            ..Default::default()
        };
        Self {
            brain: ThermalBrain::new(config),
        }
    }

    /// Push a temperature sample
    #[wasm_bindgen]
    pub fn push_sample(&mut self, temperature_c: f32) {
        self.brain.push_sample(temperature_c);
    }

    /// Run one processing cycle
    ///
    /// Returns JSON string with match result or null
    #[wasm_bindgen]
    pub fn process(&mut self) -> Option<String> {
        match self.brain.process() {
            Some(result) => {
                let json = format!(
                    r#"{{"label":"{}","confidence":{},"pattern_id":{}}}"#,
                    result.label,
                    result.confidence,
                    result.pattern_id
                );
                Some(json)
            }
            None => None,
        }
    }

    /// Learn a new pattern from current state
    ///
    /// Returns pattern ID on success, or -1 on error
    #[wasm_bindgen]
    pub fn learn(&mut self, label: &str) -> i32 {
        match self.brain.learn(label) {
            Ok(id) => id as i32,
            Err(_) => -1,
        }
    }

    /// Delete a learned pattern
    #[wasm_bindgen]
    pub fn forget(&mut self, pattern_id: u32) -> bool {
        self.brain.forget(pattern_id).is_ok()
    }

    /// Get current thermal zone (0=Cool, 1=Warm, 2=Hot, 3=Critical, 4=Emergency)
    #[wasm_bindgen]
    pub fn thermal_zone(&self) -> u8 {
        self.brain.thermal_zone() as u8
    }

    /// Get current temperature
    #[wasm_bindgen]
    pub fn temperature(&self) -> f32 {
        self.brain.status().temperature_c
    }

    /// Get pattern count
    #[wasm_bindgen]
    pub fn pattern_count(&self) -> usize {
        self.brain.pattern_count()
    }

    /// Get recommended sleep duration in milliseconds
    #[wasm_bindgen]
    pub fn recommended_sleep_ms(&self) -> u32 {
        self.brain.recommended_sleep_ms()
    }

    /// Get spike threshold
    #[wasm_bindgen]
    pub fn spike_threshold(&self) -> f32 {
        self.brain.status().spike_threshold
    }

    /// Get inference count
    #[wasm_bindgen]
    pub fn inference_count(&self) -> u64 {
        self.brain.status().inference_count
    }

    /// Get spike count
    #[wasm_bindgen]
    pub fn spike_count(&self) -> u64 {
        self.brain.status().spike_count
    }

    /// Reset all neurons
    #[wasm_bindgen]
    pub fn reset_neurons(&mut self) {
        self.brain.reset_neurons();
    }

    /// Get status as JSON
    #[wasm_bindgen]
    pub fn status_json(&self) -> String {
        let status = self.brain.status();
        format!(
            r#"{{"zone":{},"temperature_c":{},"spike_threshold":{},"pattern_count":{},"inference_count":{},"spike_count":{}}}"#,
            status.zone as u8,
            status.temperature_c,
            status.spike_threshold,
            status.pattern_count,
            status.inference_count,
            status.spike_count
        )
    }

    /// Get last sparse vector as JSON
    #[wasm_bindgen]
    pub fn last_sparse_json(&self) -> String {
        let sparse = self.brain.last_sparse();
        let indices: Vec<u8> = sparse.indices.iter().copied().collect();
        let values: Vec<i8> = sparse.values.iter().copied().collect();

        format!(
            r#"{{"indices":{:?},"values":{:?},"threshold":{},"nnz":{}}}"#,
            indices,
            values,
            sparse.threshold,
            sparse.nnz()
        )
    }

    /// Get last features as JSON array
    #[wasm_bindgen]
    pub fn last_features_json(&self) -> String {
        let features = self.brain.last_features();
        format!("{:?}", features.as_slice())
    }

    /// Get HNSW stats as JSON
    #[wasm_bindgen]
    pub fn hnsw_stats_json(&self) -> String {
        let stats = self.brain.hnsw_stats();
        format!(
            r#"{{"num_vectors":{},"num_layers":{},"memory_bytes":{},"avg_search_hops":{}}}"#,
            stats.num_vectors,
            stats.num_layers,
            stats.memory_bytes,
            stats.avg_search_hops
        )
    }
}

impl Default for WasmThermalBrain {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize WASM module (called automatically)
#[wasm_bindgen(start)]
pub fn init() {
    // Set up console_error_panic_hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get library version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get platform info
#[wasm_bindgen]
pub fn platform() -> String {
    "WASM".to_string()
}

/// Thermal zone name
#[wasm_bindgen]
pub fn thermal_zone_name(zone: u8) -> String {
    match zone {
        0 => "Cool".to_string(),
        1 => "Warm".to_string(),
        2 => "Hot".to_string(),
        3 => "Critical".to_string(),
        4 => "Emergency".to_string(),
        _ => "Unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_thermal_brain() {
        let mut brain = WasmThermalBrain::new();

        // Push samples
        for i in 0..100 {
            brain.push_sample(25.0 + i as f32 * 0.1);
        }

        assert_eq!(brain.thermal_zone(), 0); // Cool

        // Learn pattern
        let id = brain.learn("test_pattern");
        assert!(id >= 0);

        // Check pattern count
        assert_eq!(brain.pattern_count(), 1);
    }
}
