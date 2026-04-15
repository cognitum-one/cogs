//! Predictive Thermal Management
//!
//! Implements ML-based thermal prediction for proactive management:
//! - Temperature forecasting using simple neural model
//! - Preemptive throttling before thermal limits
//! - Thermal budget allocation
//! - Activity-based thermal modeling
//!
//! Reference: AI-based predictive thermal management research

use crate::types::ThermalZone;
use libm::expf;

/// Thermal prediction configuration
#[derive(Clone, Copy, Debug)]
pub struct PredictiveConfig {
    /// Prediction horizon (samples ahead)
    pub horizon_samples: usize,
    /// Temperature at which to start preemptive action
    pub preemptive_threshold_c: f32,
    /// Thermal time constant (for modeling)
    pub thermal_tau_s: f32,
    /// Ambient temperature estimate
    pub ambient_c: f32,
    /// Maximum allowed temperature
    pub max_temp_c: f32,
    /// Thermal budget per second
    pub budget_per_sec: f32,
}

impl Default for PredictiveConfig {
    fn default() -> Self {
        Self {
            horizon_samples: 10,
            preemptive_threshold_c: 45.0,
            thermal_tau_s: 30.0,
            ambient_c: 25.0,
            max_temp_c: 60.0,
            budget_per_sec: 5.0,
        }
    }
}

/// Temperature history buffer
#[derive(Clone)]
pub struct TempHistory {
    samples: [f32; 64],
    write_idx: usize,
    count: usize,
}

impl TempHistory {
    pub fn new() -> Self {
        Self {
            samples: [0.0; 64],
            write_idx: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, temp: f32) {
        self.samples[self.write_idx] = temp;
        self.write_idx = (self.write_idx + 1) % 64;
        if self.count < 64 {
            self.count += 1;
        }
    }

    pub fn last_n(&self, n: usize) -> impl Iterator<Item = f32> + '_ {
        let n = n.min(self.count);
        let start = if self.count >= 64 {
            (self.write_idx + 64 - n) % 64
        } else {
            self.count.saturating_sub(n)
        };
        (0..n).map(move |i| self.samples[(start + i) % 64])
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl Default for TempHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple thermal predictor using exponential model
pub struct ThermalPredictor {
    config: PredictiveConfig,
    history: TempHistory,
    /// Estimated thermal mass
    thermal_mass: f32,
    /// Estimated power dissipation
    power_estimate: f32,
    /// Last predicted temperature
    last_prediction: f32,
    /// Prediction error (for adaptation)
    prediction_error: f32,
}

impl ThermalPredictor {
    pub fn new(config: PredictiveConfig) -> Self {
        Self {
            config,
            history: TempHistory::new(),
            thermal_mass: 1.0,
            power_estimate: 0.0,
            last_prediction: config.ambient_c,
            prediction_error: 0.0,
        }
    }

    /// Update with new temperature reading
    pub fn update(&mut self, temp_c: f32, power_level: f32, dt_s: f32) {
        // Track prediction error for adaptation
        if self.history.len() > 0 {
            self.prediction_error = temp_c - self.last_prediction;

            // Adapt thermal model based on error
            if self.prediction_error.abs() > 0.5 {
                self.adapt_model(temp_c, dt_s);
            }
        }

        self.history.push(temp_c);
        self.power_estimate = power_level;
    }

    /// Predict temperature N steps ahead
    pub fn predict(&mut self, steps_ahead: usize, power_level: f32) -> f32 {
        let n = self.config.horizon_samples.min(steps_ahead);

        // Get recent temperature trend
        let temps: heapless::Vec<f32, 64> = self.history.last_n(10).collect();
        if temps.len() < 2 {
            return self.config.ambient_c;
        }

        let current = *temps.last().unwrap_or(&self.config.ambient_c);

        // Calculate trend (linear regression slope)
        let n_samples = temps.len() as f32;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut sum_xy = 0.0f32;
        let mut sum_xx = 0.0f32;

        for (i, &t) in temps.iter().enumerate() {
            let x = i as f32;
            sum_x += x;
            sum_y += t;
            sum_xy += x * t;
            sum_xx += x * x;
        }

        let slope = if (n_samples * sum_xx - sum_x * sum_x).abs() > 1e-10 {
            (n_samples * sum_xy - sum_x * sum_y) / (n_samples * sum_xx - sum_x * sum_x)
        } else {
            0.0
        };

        // Exponential thermal model:
        // T(t) = T_ambient + (T_current - T_ambient) * e^(-t/tau) + P * R_th * (1 - e^(-t/tau))
        let t = (n as f32) * 0.1; // Assuming 100ms per step
        let decay = expf(-t / self.config.thermal_tau_s);
        let thermal_resistance = 0.1 / self.thermal_mass;

        let delta_ambient = current - self.config.ambient_c;
        let power_contribution = power_level * thermal_resistance * (1.0 - decay);
        let trend_contribution = slope * (n as f32);

        let predicted = self.config.ambient_c
            + delta_ambient * decay
            + power_contribution
            + trend_contribution * 0.5; // Damped trend

        self.last_prediction = predicted;
        predicted
    }

    /// Adapt thermal model based on observed error
    fn adapt_model(&mut self, actual_temp: f32, dt_s: f32) {
        // Simple adaptation: adjust thermal mass estimate
        let alpha = 0.1;

        if self.prediction_error > 0.0 {
            // Heating faster than expected - reduce thermal mass
            self.thermal_mass *= 1.0 - alpha;
        } else {
            // Cooling faster than expected - increase thermal mass
            self.thermal_mass *= 1.0 + alpha;
        }

        self.thermal_mass = self.thermal_mass.clamp(0.5, 2.0);
    }

    /// Check if preemptive action is needed
    pub fn needs_preemptive_action(&mut self, power_level: f32) -> bool {
        let predicted = self.predict(self.config.horizon_samples, power_level);
        predicted > self.config.preemptive_threshold_c
    }

    /// Calculate recommended power level to stay within thermal budget
    pub fn recommended_power(&mut self, current_temp: f32) -> f32 {
        let headroom = self.config.max_temp_c - current_temp;

        if headroom <= 0.0 {
            return 0.0; // Full throttle
        }

        let budget_ratio = headroom / (self.config.max_temp_c - self.config.preemptive_threshold_c);
        budget_ratio.clamp(0.0, 1.0)
    }

    /// Get thermal margin (degrees below max)
    pub fn thermal_margin(&self) -> f32 {
        let current = self.history.last_n(1).next().unwrap_or(self.config.ambient_c);
        self.config.max_temp_c - current
    }

    /// Get prediction confidence (based on recent error)
    pub fn confidence(&self) -> f32 {
        let error_factor = 1.0 - (self.prediction_error.abs() / 10.0).min(1.0);
        let history_factor = (self.history.len() as f32 / 20.0).min(1.0);
        error_factor * history_factor
    }
}

/// Thermal budget allocator
pub struct ThermalBudget {
    /// Total budget per period
    total_budget: f32,
    /// Remaining budget
    remaining: f32,
    /// Budget period (ms)
    period_ms: u32,
    /// Time in current period
    elapsed_ms: u32,
}

impl ThermalBudget {
    pub fn new(budget: f32, period_ms: u32) -> Self {
        Self {
            total_budget: budget,
            remaining: budget,
            period_ms,
            elapsed_ms: 0,
        }
    }

    /// Consume budget
    pub fn consume(&mut self, amount: f32) -> bool {
        if amount <= self.remaining {
            self.remaining -= amount;
            true
        } else {
            false
        }
    }

    /// Update time, potentially resetting period
    pub fn tick(&mut self, dt_ms: u32) {
        self.elapsed_ms += dt_ms;
        if self.elapsed_ms >= self.period_ms {
            self.reset();
        }
    }

    /// Reset budget for new period
    pub fn reset(&mut self) {
        self.remaining = self.total_budget;
        self.elapsed_ms = 0;
    }

    /// Get remaining budget
    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// Get budget utilization
    pub fn utilization(&self) -> f32 {
        1.0 - (self.remaining / self.total_budget)
    }

    /// Check if budget allows operation
    pub fn can_afford(&self, cost: f32) -> bool {
        cost <= self.remaining
    }
}

/// Thermal state machine for predictive control
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictiveState {
    /// Normal operation
    Normal,
    /// Preemptive throttling (predicted thermal issue)
    Preemptive,
    /// Active throttling (actual thermal issue)
    Throttling,
    /// Recovery from thermal event
    Recovery,
}

/// Predictive thermal controller
pub struct PredictiveThermalController {
    predictor: ThermalPredictor,
    budget: ThermalBudget,
    state: PredictiveState,
    /// Consecutive predictions above threshold
    warning_count: u32,
    /// Time in recovery
    recovery_time_ms: u32,
}

impl PredictiveThermalController {
    pub fn new(config: PredictiveConfig) -> Self {
        Self {
            predictor: ThermalPredictor::new(config),
            budget: ThermalBudget::new(config.budget_per_sec, 1000),
            state: PredictiveState::Normal,
            warning_count: 0,
            recovery_time_ms: 0,
        }
    }

    /// Update controller state
    pub fn update(
        &mut self,
        temp_c: f32,
        power_level: f32,
        zone: ThermalZone,
        dt_ms: u32,
    ) -> f32 {
        let dt_s = dt_ms as f32 / 1000.0;
        self.predictor.update(temp_c, power_level, dt_s);
        self.budget.tick(dt_ms);

        // State transitions
        match self.state {
            PredictiveState::Normal => {
                if zone == ThermalZone::Hot || zone == ThermalZone::Critical {
                    self.state = PredictiveState::Throttling;
                } else if self.predictor.needs_preemptive_action(power_level) {
                    self.warning_count += 1;
                    if self.warning_count >= 3 {
                        self.state = PredictiveState::Preemptive;
                    }
                } else {
                    self.warning_count = 0;
                }
            }
            PredictiveState::Preemptive => {
                if zone == ThermalZone::Hot || zone == ThermalZone::Critical {
                    self.state = PredictiveState::Throttling;
                } else if !self.predictor.needs_preemptive_action(power_level * 0.8) {
                    self.state = PredictiveState::Normal;
                    self.warning_count = 0;
                }
            }
            PredictiveState::Throttling => {
                if zone == ThermalZone::Cool || zone == ThermalZone::Warm {
                    self.state = PredictiveState::Recovery;
                    self.recovery_time_ms = 0;
                }
            }
            PredictiveState::Recovery => {
                self.recovery_time_ms += dt_ms;
                if self.recovery_time_ms >= 2000 {
                    self.state = PredictiveState::Normal;
                    self.warning_count = 0;
                }
            }
        }

        // Return recommended power multiplier
        match self.state {
            PredictiveState::Normal => 1.0,
            PredictiveState::Preemptive => 0.7,
            PredictiveState::Throttling => 0.3,
            PredictiveState::Recovery => 0.5,
        }
    }

    /// Get current state
    pub fn state(&self) -> PredictiveState {
        self.state
    }

    /// Get predicted temperature
    pub fn predicted_temp(&mut self, steps: usize, power: f32) -> f32 {
        self.predictor.predict(steps, power)
    }

    /// Get prediction confidence
    pub fn confidence(&self) -> f32 {
        self.predictor.confidence()
    }

    /// Get thermal margin
    pub fn thermal_margin(&self) -> f32 {
        self.predictor.thermal_margin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_history() {
        let mut history = TempHistory::new();

        for i in 0..10 {
            history.push(25.0 + i as f32);
        }

        assert_eq!(history.len(), 10);

        let last_5: Vec<f32> = history.last_n(5).collect();
        assert_eq!(last_5.len(), 5);
        assert_eq!(*last_5.last().unwrap(), 34.0);
    }

    #[test]
    fn test_thermal_predictor() {
        let config = PredictiveConfig::default();
        let mut predictor = ThermalPredictor::new(config);

        // Feed some data
        for i in 0..20 {
            predictor.update(25.0 + i as f32 * 0.5, 0.5, 0.1);
        }

        let predicted = predictor.predict(5, 0.5);
        assert!(predicted > 30.0); // Should predict continued warming
    }

    #[test]
    fn test_thermal_budget() {
        let mut budget = ThermalBudget::new(100.0, 1000);

        assert!(budget.consume(30.0));
        assert!(budget.consume(30.0));
        assert!(budget.consume(30.0));
        assert!(!budget.consume(20.0)); // Over budget

        budget.reset();
        assert!(budget.consume(50.0));
    }

    #[test]
    fn test_predictive_controller() {
        let config = PredictiveConfig::default();
        let mut controller = PredictiveThermalController::new(config);

        // Normal operation
        let mult = controller.update(30.0, 0.5, ThermalZone::Cool, 100);
        assert_eq!(mult, 1.0);
        assert_eq!(controller.state(), PredictiveState::Normal);

        // Hot zone should throttle
        let mult = controller.update(55.0, 0.5, ThermalZone::Hot, 100);
        assert!(mult < 1.0);
    }
}
