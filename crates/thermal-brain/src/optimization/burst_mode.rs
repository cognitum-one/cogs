//! Burst Mode for Peak Performance
//!
//! Implements temporary maximum performance mode with:
//! - Time-limited turbo operation
//! - Thermal budget management
//! - Spike activity-driven scaling
//!
//! Reference: SpiNNaker-2 adaptive power management

use crate::types::ThermalZone;

/// Burst mode configuration
#[derive(Clone, Copy, Debug)]
pub struct BurstConfig {
    /// Maximum burst duration (ms)
    pub max_duration_ms: u32,
    /// Cooldown period after burst (ms)
    pub cooldown_ms: u32,
    /// Temperature threshold to exit burst (Celsius)
    pub temp_exit_c: f32,
    /// Spike rate threshold to trigger burst
    pub spike_rate_trigger: f32,
    /// Performance multiplier during burst
    pub perf_multiplier: f32,
    /// Energy budget per burst (relative units)
    pub energy_budget: f32,
}

impl Default for BurstConfig {
    fn default() -> Self {
        Self {
            max_duration_ms: 500,
            cooldown_ms: 2000,
            temp_exit_c: 50.0,
            spike_rate_trigger: 0.8,
            perf_multiplier: 2.0,
            energy_budget: 100.0,
        }
    }
}

/// Burst mode state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurstState {
    /// Normal operation
    Idle,
    /// Burst mode active
    Active,
    /// Cooling down after burst
    Cooldown,
    /// Burst disabled (thermal limit)
    Disabled,
}

/// Burst mode controller
pub struct BurstController {
    config: BurstConfig,
    state: BurstState,
    /// Time in current state (ms)
    state_time_ms: u32,
    /// Energy consumed in current burst
    energy_consumed: f32,
    /// Number of bursts performed
    burst_count: u32,
    /// Total burst time (ms)
    total_burst_ms: u64,
    /// Spike rate EMA
    spike_rate_ema: f32,
}

impl BurstController {
    pub fn new(config: BurstConfig) -> Self {
        Self {
            config,
            state: BurstState::Idle,
            state_time_ms: 0,
            energy_consumed: 0.0,
            burst_count: 0,
            total_burst_ms: 0,
            spike_rate_ema: 0.0,
        }
    }

    /// Update burst state
    ///
    /// # Arguments
    /// * `spike_rate` - Current spike rate (0.0 to 1.0)
    /// * `temp_c` - Current temperature
    /// * `zone` - Current thermal zone
    /// * `dt_ms` - Time delta
    ///
    /// # Returns
    /// Performance multiplier (1.0 = normal, >1.0 = boosted)
    pub fn update(
        &mut self,
        spike_rate: f32,
        temp_c: f32,
        zone: ThermalZone,
        dt_ms: u32,
    ) -> f32 {
        // Update spike rate EMA
        self.spike_rate_ema = 0.9 * self.spike_rate_ema + 0.1 * spike_rate.clamp(0.0, 1.0);
        self.state_time_ms += dt_ms;

        // State machine
        match self.state {
            BurstState::Idle => {
                // Check if we should enter burst
                if self.should_trigger_burst(zone) {
                    self.enter_burst();
                }
                1.0 // Normal performance
            }
            BurstState::Active => {
                // Check exit conditions
                if self.should_exit_burst(temp_c, zone) {
                    self.exit_burst();
                    1.0
                } else {
                    // Track energy
                    self.energy_consumed += self.config.perf_multiplier * (dt_ms as f32 / 1000.0) * 10.0;
                    self.total_burst_ms += dt_ms as u64;
                    self.config.perf_multiplier
                }
            }
            BurstState::Cooldown => {
                if self.state_time_ms >= self.config.cooldown_ms {
                    self.state = BurstState::Idle;
                    self.state_time_ms = 0;
                }
                1.0
            }
            BurstState::Disabled => {
                // Re-enable if temperature drops
                if zone == ThermalZone::Cool && temp_c < self.config.temp_exit_c - 10.0 {
                    self.state = BurstState::Idle;
                    self.state_time_ms = 0;
                }
                1.0
            }
        }
    }

    /// Check if burst should be triggered
    fn should_trigger_burst(&self, zone: ThermalZone) -> bool {
        zone == ThermalZone::Cool
            && self.spike_rate_ema > self.config.spike_rate_trigger
    }

    /// Check if burst should exit
    fn should_exit_burst(&self, temp_c: f32, zone: ThermalZone) -> bool {
        // Time limit
        if self.state_time_ms >= self.config.max_duration_ms {
            return true;
        }

        // Thermal limit
        if temp_c > self.config.temp_exit_c || zone == ThermalZone::Hot {
            return true;
        }

        // Energy budget
        if self.energy_consumed >= self.config.energy_budget {
            return true;
        }

        false
    }

    /// Enter burst mode
    fn enter_burst(&mut self) {
        self.state = BurstState::Active;
        self.state_time_ms = 0;
        self.energy_consumed = 0.0;
        self.burst_count += 1;
    }

    /// Exit burst mode
    fn exit_burst(&mut self) {
        self.state = BurstState::Cooldown;
        self.state_time_ms = 0;
    }

    /// Manually trigger burst (if allowed)
    pub fn trigger(&mut self) -> bool {
        if self.state == BurstState::Idle {
            self.enter_burst();
            true
        } else {
            false
        }
    }

    /// Force exit burst
    pub fn force_exit(&mut self) {
        if self.state == BurstState::Active {
            self.exit_burst();
        }
    }

    /// Disable burst mode (thermal emergency)
    pub fn disable(&mut self) {
        self.state = BurstState::Disabled;
        self.state_time_ms = 0;
    }

    /// Get current state
    pub fn state(&self) -> BurstState {
        self.state
    }

    /// Check if burst is active
    pub fn is_active(&self) -> bool {
        self.state == BurstState::Active
    }

    /// Get burst count
    pub fn burst_count(&self) -> u32 {
        self.burst_count
    }

    /// Get total burst time
    pub fn total_burst_ms(&self) -> u64 {
        self.total_burst_ms
    }

    /// Get remaining burst time (if active)
    pub fn remaining_ms(&self) -> u32 {
        if self.state == BurstState::Active {
            self.config.max_duration_ms.saturating_sub(self.state_time_ms)
        } else {
            0
        }
    }

    /// Get remaining cooldown time
    pub fn cooldown_remaining_ms(&self) -> u32 {
        if self.state == BurstState::Cooldown {
            self.config.cooldown_ms.saturating_sub(self.state_time_ms)
        } else {
            0
        }
    }
}

/// Spike-triggered burst optimizer
pub struct SpikeBurstOptimizer {
    /// Spike history for burst prediction
    spike_history: heapless::Vec<f32, 32>,
    /// Predicted optimal burst timing
    predicted_burst_time: u32,
    /// Burst effectiveness score
    effectiveness: f32,
}

impl SpikeBurstOptimizer {
    pub fn new() -> Self {
        Self {
            spike_history: heapless::Vec::new(),
            predicted_burst_time: 0,
            effectiveness: 0.5,
        }
    }

    /// Record spike rate sample
    pub fn record(&mut self, spike_rate: f32) {
        if self.spike_history.is_full() {
            // Remove oldest
            for i in 0..self.spike_history.len() - 1 {
                self.spike_history[i] = self.spike_history[i + 1];
            }
            self.spike_history.pop();
        }
        let _ = self.spike_history.push(spike_rate);
    }

    /// Predict if burst will be effective
    pub fn predict_effectiveness(&self) -> f32 {
        if self.spike_history.len() < 3 {
            return 0.5;
        }

        // Check if spike rate is trending up
        let recent: f32 = self.spike_history.iter()
            .rev()
            .take(3)
            .sum::<f32>() / 3.0;

        let older: f32 = self.spike_history.iter()
            .take(3)
            .sum::<f32>() / 3.0;

        // Higher effectiveness if spike rate is increasing
        let trend = recent - older;
        (0.5 + trend).clamp(0.0, 1.0)
    }

    /// Update effectiveness based on burst result
    pub fn update_effectiveness(&mut self, burst_successful: bool) {
        let alpha = 0.2;
        let target = if burst_successful { 1.0 } else { 0.0 };
        self.effectiveness = self.effectiveness * (1.0 - alpha) + target * alpha;
    }

    /// Get overall effectiveness score
    pub fn effectiveness(&self) -> f32 {
        self.effectiveness
    }
}

impl Default for SpikeBurstOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_burst_controller_idle() {
        let config = BurstConfig::default();
        let controller = BurstController::new(config);

        assert_eq!(controller.state(), BurstState::Idle);
        assert!(!controller.is_active());
    }

    #[test]
    fn test_burst_trigger() {
        let config = BurstConfig::default();
        let mut controller = BurstController::new(config);

        // Trigger manually
        assert!(controller.trigger());
        assert_eq!(controller.state(), BurstState::Active);
        assert!(controller.is_active());
    }

    #[test]
    fn test_burst_auto_trigger() {
        let config = BurstConfig::default();
        let mut controller = BurstController::new(config);

        // High spike rate should trigger burst (need many iterations to build up EMA)
        for _ in 0..50 {
            controller.update(0.95, 30.0, ThermalZone::Cool, 10);
        }

        let state = controller.state();
        assert!(
            state == BurstState::Active || state == BurstState::Cooldown,
            "Expected Active or Cooldown, got {:?}", state
        );
    }

    #[test]
    fn test_burst_thermal_exit() {
        let config = BurstConfig::default();
        let mut controller = BurstController::new(config);

        controller.trigger();
        assert!(controller.is_active());

        // High temp should exit
        controller.update(0.9, 55.0, ThermalZone::Hot, 10);
        assert!(!controller.is_active());
    }

    #[test]
    fn test_burst_duration_limit() {
        let config = BurstConfig {
            max_duration_ms: 100,
            ..Default::default()
        };
        let mut controller = BurstController::new(config);

        controller.trigger();

        // Run past limit
        for _ in 0..15 {
            controller.update(0.5, 30.0, ThermalZone::Cool, 10);
        }

        assert!(!controller.is_active());
    }

    #[test]
    fn test_spike_burst_optimizer() {
        let mut optimizer = SpikeBurstOptimizer::new();

        // Record increasing spike rates
        for i in 0..10 {
            optimizer.record(i as f32 / 10.0);
        }

        let effectiveness = optimizer.predict_effectiveness();
        assert!(effectiveness > 0.5);
    }
}
