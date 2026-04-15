//! Dynamic Voltage and Frequency Scaling (DVFS)
//!
//! Based on SpiNNaker-2 research achieving:
//! - 75% total PE power reduction
//! - 80% baseline power reduction
//! - 50% energy reduction per neuron/synapse computation
//! - Per-core scaling within <100ns
//!
//! Reference: IEEE Dynamic Power Management for Neuromorphic Many-Core Systems

use crate::types::ThermalZone;
use heapless::Vec as HVec;

/// Maximum performance levels
const MAX_PERF_LEVELS: usize = 8;

/// Performance level configuration
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerfLevel {
    /// Frequency multiplier (0.1 to 2.0)
    pub freq_mult: f32,
    /// Voltage scaling (0.6 to 1.2)
    pub voltage_scale: f32,
    /// Power consumption estimate (relative)
    pub power_factor: f32,
    /// Name identifier
    pub name: &'static str,
}

impl PerfLevel {
    pub const fn new(freq_mult: f32, voltage_scale: f32, name: &'static str) -> Self {
        // Power ~ V^2 * F (quadratic voltage effect)
        let power_factor = voltage_scale * voltage_scale * freq_mult;
        Self { freq_mult, voltage_scale, power_factor, name }
    }
}

/// Predefined performance levels (SpiNNaker-2 inspired)
pub const PERF_LEVELS: [PerfLevel; 8] = [
    PerfLevel::new(0.125, 0.60, "ultra_low"),    // Sleep mode
    PerfLevel::new(0.25, 0.70, "very_low"),      // Deep idle
    PerfLevel::new(0.50, 0.80, "low"),           // Power saving
    PerfLevel::new(0.75, 0.90, "medium_low"),    // Balanced low
    PerfLevel::new(1.00, 1.00, "nominal"),       // Default
    PerfLevel::new(1.25, 1.05, "medium_high"),   // Mild boost
    PerfLevel::new(1.50, 1.10, "high"),          // Performance
    PerfLevel::new(2.00, 1.20, "turbo"),         // Maximum overclock
];

/// DVFS Controller state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DvfsState {
    /// Stable at current level
    Stable,
    /// Transitioning to higher performance
    RampingUp,
    /// Transitioning to lower performance
    RampingDown,
    /// Emergency throttling
    Throttling,
    /// Burst mode active
    Burst,
}

/// DVFS Controller configuration
#[derive(Clone, Copy, Debug)]
pub struct DvfsConfig {
    /// Minimum performance level index
    pub min_level: usize,
    /// Maximum performance level index (for overclocking)
    pub max_level: usize,
    /// Ramp-up rate (levels per second)
    pub ramp_up_rate: f32,
    /// Ramp-down rate (levels per second)
    pub ramp_down_rate: f32,
    /// Load threshold for upscaling (0.0-1.0)
    pub upscale_threshold: f32,
    /// Load threshold for downscaling (0.0-1.0)
    pub downscale_threshold: f32,
    /// Temperature-based throttle threshold (Celsius)
    pub throttle_temp_c: f32,
    /// Enable aggressive overclocking
    pub enable_overclock: bool,
    /// Burst duration limit (ms)
    pub burst_limit_ms: u32,
}

impl Default for DvfsConfig {
    fn default() -> Self {
        Self {
            min_level: 0,
            max_level: 7,  // Allow turbo
            ramp_up_rate: 4.0,
            ramp_down_rate: 2.0,
            upscale_threshold: 0.75,
            downscale_threshold: 0.25,
            throttle_temp_c: 55.0,
            enable_overclock: true,
            burst_limit_ms: 500,
        }
    }
}

/// DVFS Controller - SpiNNaker-2 style per-core management
pub struct DvfsController {
    config: DvfsConfig,
    current_level: usize,
    target_level: usize,
    state: DvfsState,
    /// Exponential moving average of load
    load_ema: f32,
    /// Time spent in burst mode (ms)
    burst_time_ms: u32,
    /// Last transition timestamp
    last_transition_ms: u64,
    /// Accumulated energy estimate
    energy_used: f32,
}

impl DvfsController {
    /// Create new DVFS controller
    pub fn new(config: DvfsConfig) -> Self {
        Self {
            config,
            current_level: 4, // Start at nominal
            target_level: 4,
            state: DvfsState::Stable,
            load_ema: 0.5,
            burst_time_ms: 0,
            last_transition_ms: 0,
            energy_used: 0.0,
        }
    }

    /// Update DVFS state based on load and temperature
    ///
    /// # Arguments
    /// * `load` - Current computational load (0.0 to 1.0)
    /// * `temp_c` - Current temperature in Celsius
    /// * `zone` - Current thermal zone
    /// * `dt_ms` - Time delta in milliseconds
    ///
    /// # Returns
    /// Current performance level
    pub fn update(
        &mut self,
        load: f32,
        temp_c: f32,
        zone: ThermalZone,
        dt_ms: u32,
    ) -> &PerfLevel {
        // Update load EMA (alpha = 0.2)
        self.load_ema = 0.8 * self.load_ema + 0.2 * load.clamp(0.0, 1.0);

        // Check thermal throttling first
        if temp_c > self.config.throttle_temp_c || zone == ThermalZone::Critical || zone == ThermalZone::Emergency {
            self.state = DvfsState::Throttling;
            self.target_level = self.config.min_level;
            self.burst_time_ms = 0;
        } else if self.state == DvfsState::Throttling && temp_c < self.config.throttle_temp_c - 5.0 {
            // Exit throttling with hysteresis
            self.state = DvfsState::Stable;
        }

        // Handle burst mode timeout
        if self.state == DvfsState::Burst {
            self.burst_time_ms += dt_ms;
            if self.burst_time_ms > self.config.burst_limit_ms {
                self.state = DvfsState::RampingDown;
                self.target_level = 4; // Return to nominal
                self.burst_time_ms = 0;
            }
        }

        // Determine target level based on load (if not throttling)
        if self.state != DvfsState::Throttling {
            let new_target = self.calculate_target_level();
            if new_target != self.target_level {
                self.target_level = new_target;
                self.state = if new_target > self.current_level {
                    DvfsState::RampingUp
                } else {
                    DvfsState::RampingDown
                };
            }
        }

        // Apply ramping
        self.apply_ramping(dt_ms);

        // Track energy consumption
        let perf = &PERF_LEVELS[self.current_level];
        self.energy_used += perf.power_factor * (dt_ms as f32 / 1000.0);

        perf
    }

    /// Calculate optimal target level based on load
    fn calculate_target_level(&self) -> usize {
        let max = if self.config.enable_overclock {
            self.config.max_level
        } else {
            4 // Cap at nominal
        };

        if self.load_ema > 0.95 {
            max // Maximum performance
        } else if self.load_ema > self.config.upscale_threshold {
            ((self.current_level + 1).min(max)) // Step up
        } else if self.load_ema < self.config.downscale_threshold {
            self.current_level.saturating_sub(1).max(self.config.min_level) // Step down
        } else {
            self.current_level // Maintain
        }
    }

    /// Apply ramping transitions
    fn apply_ramping(&mut self, dt_ms: u32) {
        let dt_s = dt_ms as f32 / 1000.0;

        match self.state {
            DvfsState::RampingUp => {
                let steps = (self.config.ramp_up_rate * dt_s) as usize;
                if steps > 0 && self.current_level < self.target_level {
                    self.current_level = (self.current_level + steps).min(self.target_level);
                }
                if self.current_level >= self.target_level {
                    self.state = DvfsState::Stable;
                }
            }
            DvfsState::RampingDown | DvfsState::Throttling => {
                let steps = (self.config.ramp_down_rate * dt_s) as usize;
                if steps > 0 && self.current_level > self.target_level {
                    self.current_level = self.current_level.saturating_sub(steps).max(self.target_level);
                }
                if self.current_level <= self.target_level && self.state == DvfsState::RampingDown {
                    self.state = DvfsState::Stable;
                }
            }
            _ => {}
        }
    }

    /// Enter burst mode for maximum temporary performance
    pub fn enter_burst(&mut self) -> bool {
        if !self.config.enable_overclock {
            return false;
        }
        if self.state == DvfsState::Throttling {
            return false;
        }

        self.state = DvfsState::Burst;
        self.target_level = self.config.max_level;
        self.burst_time_ms = 0;
        true
    }

    /// Exit burst mode
    pub fn exit_burst(&mut self) {
        if self.state == DvfsState::Burst {
            self.state = DvfsState::RampingDown;
            self.target_level = 4; // Nominal
            self.burst_time_ms = 0;
        }
    }

    /// Get current performance level
    pub fn current_level(&self) -> &PerfLevel {
        &PERF_LEVELS[self.current_level]
    }

    /// Get current state
    pub fn state(&self) -> DvfsState {
        self.state
    }

    /// Get frequency multiplier
    pub fn freq_mult(&self) -> f32 {
        PERF_LEVELS[self.current_level].freq_mult
    }

    /// Get voltage scale
    pub fn voltage_scale(&self) -> f32 {
        PERF_LEVELS[self.current_level].voltage_scale
    }

    /// Get accumulated energy usage
    pub fn energy_used(&self) -> f32 {
        self.energy_used
    }

    /// Reset energy counter
    pub fn reset_energy(&mut self) {
        self.energy_used = 0.0;
    }

    /// Get current load EMA
    pub fn load(&self) -> f32 {
        self.load_ema
    }

    /// Force specific level (for testing/calibration)
    pub fn force_level(&mut self, level: usize) {
        let level = level.min(PERF_LEVELS.len() - 1);
        self.current_level = level;
        self.target_level = level;
        self.state = DvfsState::Stable;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvfs_initial_state() {
        let config = DvfsConfig::default();
        let dvfs = DvfsController::new(config);

        assert_eq!(dvfs.state(), DvfsState::Stable);
        assert_eq!(dvfs.current_level().name, "nominal");
    }

    #[test]
    fn test_dvfs_high_load() {
        let config = DvfsConfig {
            ramp_up_rate: 10.0, // Faster ramping for test
            ..Default::default()
        };
        let mut dvfs = DvfsController::new(config);

        // Simulate high load for longer to let EMA and ramp converge
        for _ in 0..100 {
            dvfs.update(0.99, 35.0, ThermalZone::Cool, 500); // 500ms dt for fast ramping
        }

        // Should have ramped up
        assert!(dvfs.freq_mult() > 1.0, "Freq mult: {}, expected > 1.0", dvfs.freq_mult());
    }

    #[test]
    fn test_dvfs_thermal_throttling() {
        let config = DvfsConfig::default();
        let mut dvfs = DvfsController::new(config);

        // High temp should throttle
        dvfs.update(0.9, 60.0, ThermalZone::Hot, 10);

        assert_eq!(dvfs.state(), DvfsState::Throttling);
    }

    #[test]
    fn test_dvfs_burst_mode() {
        let config = DvfsConfig::default();
        let mut dvfs = DvfsController::new(config);

        assert!(dvfs.enter_burst());
        assert_eq!(dvfs.state(), DvfsState::Burst);

        // Run burst for a while with larger dt to ramp up
        for _ in 0..50 {
            dvfs.update(0.9, 35.0, ThermalZone::Cool, 50);
        }

        // Should still be in burst or ramping
        let state = dvfs.state();
        assert!(
            state == DvfsState::Burst || state == DvfsState::RampingUp,
            "State: {:?}", state
        );
    }

    #[test]
    fn test_dvfs_burst_timeout() {
        let config = DvfsConfig {
            burst_limit_ms: 100,
            ..Default::default()
        };
        let mut dvfs = DvfsController::new(config);

        dvfs.enter_burst();

        // Exceed burst limit
        for _ in 0..15 {
            dvfs.update(0.9, 35.0, ThermalZone::Cool, 10);
        }

        assert_ne!(dvfs.state(), DvfsState::Burst);
    }
}
