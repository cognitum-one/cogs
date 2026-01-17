//! Thermal governor - temperature-based adaptive control
//!
//! Regulates system behavior based on temperature to:
//! - Prevent overheating
//! - Optimize power consumption
//! - Maintain stable operation

use crate::config::ThermalConfig;
use crate::types::ThermalZone;

/// Thermal governor - controls system behavior based on temperature
pub struct ThermalGovernor {
    /// Current instantaneous temperature
    current_temp: f32,
    /// Exponential moving average temperature
    avg_temp: f32,
    /// Current thermal zone
    zone: ThermalZone,
    /// Previous thermal zone (for change detection)
    prev_zone: ThermalZone,
    /// Configuration
    config: ThermalConfig,
    /// Cumulative processing time (for duty cycle tracking)
    processing_time_ms: u64,
    /// Total time (for duty cycle tracking)
    total_time_ms: u64,
    /// Emergency flag
    emergency: bool,
}

impl ThermalGovernor {
    /// Create a new thermal governor
    pub fn new(config: ThermalConfig) -> Self {
        Self {
            current_temp: 25.0, // Room temperature default
            avg_temp: 25.0,
            zone: ThermalZone::Cool,
            prev_zone: ThermalZone::Cool,
            config,
            processing_time_ms: 0,
            total_time_ms: 0,
            emergency: false,
        }
    }

    /// Update with new temperature reading
    ///
    /// # Arguments
    /// * `temp_c` - Temperature in Celsius
    pub fn update(&mut self, temp_c: f32) {
        self.current_temp = temp_c;

        // Update EMA
        self.avg_temp = self.config.ema_alpha * temp_c
            + (1.0 - self.config.ema_alpha) * self.avg_temp;

        // Update zone with hysteresis
        self.prev_zone = self.zone;
        self.zone = self.compute_zone(self.avg_temp);

        // Check emergency condition
        if self.zone == ThermalZone::Emergency {
            self.emergency = true;
        } else if self.zone != ThermalZone::Emergency {
            // Clear emergency when no longer in Emergency zone
            self.emergency = false;
        }
    }

    /// Compute thermal zone with hysteresis
    fn compute_zone(&self, temp: f32) -> ThermalZone {
        let t = &self.config.zone_thresholds_c;
        let h = self.config.hysteresis_c;

        match self.zone {
            ThermalZone::Cool => {
                if temp >= t[0] {
                    ThermalZone::Warm
                } else {
                    ThermalZone::Cool
                }
            }
            ThermalZone::Warm => {
                if temp >= t[1] {
                    ThermalZone::Hot
                } else if temp < t[0] - h {
                    ThermalZone::Cool
                } else {
                    ThermalZone::Warm
                }
            }
            ThermalZone::Hot => {
                if temp >= t[2] {
                    ThermalZone::Critical
                } else if temp < t[1] - h {
                    ThermalZone::Warm
                } else {
                    ThermalZone::Hot
                }
            }
            ThermalZone::Critical => {
                if temp >= t[3] {
                    ThermalZone::Emergency
                } else if temp < t[2] - h {
                    ThermalZone::Hot
                } else {
                    ThermalZone::Critical
                }
            }
            ThermalZone::Emergency => {
                if temp < t[3] - h {
                    ThermalZone::Critical
                } else {
                    ThermalZone::Emergency
                }
            }
        }
    }

    /// Get current thermal zone
    pub fn zone(&self) -> ThermalZone {
        self.zone
    }

    /// Check if zone has changed since last update
    pub fn zone_changed(&self) -> bool {
        self.zone != self.prev_zone
    }

    /// Get current temperature (instantaneous)
    pub fn current_temp(&self) -> f32 {
        self.current_temp
    }

    /// Get average temperature (EMA)
    pub fn avg_temp(&self) -> f32 {
        self.avg_temp
    }

    /// Get spike threshold for current zone
    pub fn spike_threshold(&self) -> f32 {
        self.zone.spike_threshold()
    }

    /// Get refractory period for current zone (ms)
    pub fn refractory_ms(&self) -> u32 {
        self.zone.refractory_ms()
    }

    /// Get recommended sleep duration for current zone (ms)
    pub fn sleep_ms(&self) -> u32 {
        self.zone.sleep_ms()
    }

    /// Check if processing is allowed
    ///
    /// Returns false in emergency state
    pub fn can_process(&self) -> bool {
        !self.emergency
    }

    /// Check if in emergency state
    pub fn is_emergency(&self) -> bool {
        self.emergency
    }

    /// Get target temperature
    pub fn target_temp(&self) -> f32 {
        self.config.target_temp_c
    }

    /// Get temperature delta from target
    pub fn temp_delta(&self) -> f32 {
        self.avg_temp - self.config.target_temp_c
    }

    /// Record processing time for duty cycle tracking
    pub fn record_processing(&mut self, processing_ms: u64, total_ms: u64) {
        self.processing_time_ms += processing_ms;
        self.total_time_ms += total_ms;
    }

    /// Get duty cycle (0.0 to 1.0)
    pub fn duty_cycle(&self) -> f32 {
        if self.total_time_ms == 0 {
            0.0
        } else {
            self.processing_time_ms as f32 / self.total_time_ms as f32
        }
    }

    /// Reset duty cycle tracking
    pub fn reset_duty_cycle(&mut self) {
        self.processing_time_ms = 0;
        self.total_time_ms = 0;
    }

    /// Get recommended CPU frequency scaling factor
    ///
    /// Returns a value between 0.0 (minimum) and 1.0 (maximum)
    pub fn cpu_scaling(&self) -> f32 {
        match self.zone {
            ThermalZone::Cool => 1.0,
            ThermalZone::Warm => 0.8,
            ThermalZone::Hot => 0.5,
            ThermalZone::Critical => 0.3,
            ThermalZone::Emergency => 0.1,
        }
    }

    /// Get recommended WiFi/BLE scanning interval (seconds)
    pub fn scan_interval_s(&self) -> u32 {
        match self.zone {
            ThermalZone::Cool => 30,
            ThermalZone::Warm => 60,
            ThermalZone::Hot => 120,
            ThermalZone::Critical => 300,
            ThermalZone::Emergency => 0, // Disabled
        }
    }

    /// Check if WiFi/BLE scanning is allowed
    pub fn can_scan(&self) -> bool {
        self.zone <= ThermalZone::Critical
    }

    /// Get thermal headroom (degrees before next zone)
    pub fn headroom(&self) -> f32 {
        let t = &self.config.zone_thresholds_c;
        match self.zone {
            ThermalZone::Cool => t[0] - self.avg_temp,
            ThermalZone::Warm => t[1] - self.avg_temp,
            ThermalZone::Hot => t[2] - self.avg_temp,
            ThermalZone::Critical => t[3] - self.avg_temp,
            ThermalZone::Emergency => 0.0,
        }
    }
}

/// Thermal event for logging/notification
#[derive(Clone, Debug)]
pub enum ThermalEvent {
    /// Zone transition
    ZoneChange {
        from: ThermalZone,
        to: ThermalZone,
        temp_c: f32,
    },
    /// Emergency triggered
    EmergencyEntered {
        temp_c: f32,
    },
    /// Emergency cleared
    EmergencyCleared {
        temp_c: f32,
    },
    /// Thermal shock (rapid temperature change)
    ThermalShock {
        delta_c: f32,
        duration_ms: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ThermalConfig {
        ThermalConfig {
            target_temp_c: 50.0,
            ema_alpha: 1.0, // Instant response for testing (no smoothing)
            hysteresis_c: 2.0,
            zone_thresholds_c: [40.0, 50.0, 60.0, 70.0],
        }
    }

    #[test]
    fn test_zone_transitions() {
        let mut gov = ThermalGovernor::new(make_config());

        // Start cool
        gov.update(30.0);
        assert_eq!(gov.zone(), ThermalZone::Cool);

        // Warm up
        gov.update(45.0);
        assert_eq!(gov.zone(), ThermalZone::Warm);

        // Hot
        gov.update(55.0);
        assert_eq!(gov.zone(), ThermalZone::Hot);
    }

    #[test]
    fn test_hysteresis() {
        let mut gov = ThermalGovernor::new(make_config());

        // Get to Warm zone
        gov.update(45.0);
        assert_eq!(gov.zone(), ThermalZone::Warm);

        // Drop below threshold but within hysteresis
        gov.update(39.0); // 40 - 2 = 38, so 39 is still in hysteresis
        assert_eq!(gov.zone(), ThermalZone::Warm);

        // Drop below hysteresis
        gov.update(37.0);
        assert_eq!(gov.zone(), ThermalZone::Cool);
    }

    #[test]
    fn test_emergency() {
        let mut gov = ThermalGovernor::new(make_config());

        // Heat to emergency (zone transitions one step at a time)
        // Cool→Warm→Hot→Critical→Emergency
        for _ in 0..5 {
            gov.update(75.0);
        }
        assert_eq!(gov.zone(), ThermalZone::Emergency);
        assert!(gov.is_emergency());
        assert!(!gov.can_process());

        // Cool down slightly (still in emergency due to hysteresis)
        gov.update(69.0);
        assert_eq!(gov.zone(), ThermalZone::Emergency);

        // Cool below hysteresis (70 - 2 = 68)
        for _ in 0..3 {
            gov.update(65.0);
        }
        assert_eq!(gov.zone(), ThermalZone::Critical);
        assert!(!gov.is_emergency());
        assert!(gov.can_process());
    }

    #[test]
    fn test_spike_threshold() {
        let mut gov = ThermalGovernor::new(make_config());

        gov.update(30.0);
        assert_eq!(gov.spike_threshold(), 0.30);

        // Transition through zones to get to Hot (55°C)
        for _ in 0..3 {
            gov.update(55.0);
        }
        assert_eq!(gov.spike_threshold(), 0.70);
    }

    #[test]
    fn test_cpu_scaling() {
        let mut gov = ThermalGovernor::new(make_config());

        gov.update(30.0);
        assert_eq!(gov.cpu_scaling(), 1.0);

        // Transition to Hot zone
        for _ in 0..3 {
            gov.update(55.0);
        }
        assert_eq!(gov.cpu_scaling(), 0.5);

        // Transition to Emergency zone
        for _ in 0..3 {
            gov.update(75.0);
        }
        assert_eq!(gov.cpu_scaling(), 0.1);
    }

    #[test]
    fn test_ema_smoothing() {
        let config = ThermalConfig {
            ema_alpha: 0.1, // Slow response
            ..make_config()
        };
        let mut gov = ThermalGovernor::new(config);

        // Start at 25
        gov.update(25.0);
        let _first = gov.avg_temp();

        // Spike to 50
        gov.update(50.0);
        let second = gov.avg_temp();

        // EMA should smooth the spike
        assert!(second < 40.0); // Should not jump to 50

        // Continue at 50
        for _ in 0..20 {
            gov.update(50.0);
        }

        // Should approach 50
        assert!(gov.avg_temp() > 45.0);
    }

    #[test]
    fn test_headroom() {
        let mut gov = ThermalGovernor::new(make_config());

        gov.update(35.0);
        // In Cool zone, threshold is 40
        assert!((gov.headroom() - 5.0).abs() < 0.5);

        gov.update(45.0);
        // In Warm zone, threshold is 50
        assert!((gov.headroom() - 5.0).abs() < 1.0);
    }
}
