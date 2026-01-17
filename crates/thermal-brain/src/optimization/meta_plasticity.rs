//! Meta-Plasticity with Homeostatic Adaptation
//!
//! Implements adaptive threshold mechanisms inspired by:
//! - Biological homeostatic plasticity
//! - Intrinsic plasticity for spike rate regulation
//! - BCM (Bienenstock-Cooper-Munro) learning rule
//! - Activity-dependent threshold adaptation
//!
//! Reference: Neuromorphic homeostatic mechanisms

use libm::{expf, fabsf};

/// Meta-plasticity configuration
#[derive(Clone, Copy, Debug)]
pub struct MetaPlasticityConfig {
    /// Target spike rate (spikes per second)
    pub target_rate_hz: f32,
    /// Threshold adaptation rate
    pub threshold_eta: f32,
    /// Time constant for rate estimation (ms)
    pub rate_tau_ms: f32,
    /// Minimum threshold
    pub min_threshold: f32,
    /// Maximum threshold
    pub max_threshold: f32,
    /// BCM sliding threshold rate
    pub bcm_rate: f32,
    /// Enable intrinsic plasticity
    pub intrinsic_enabled: bool,
}

impl Default for MetaPlasticityConfig {
    fn default() -> Self {
        Self {
            target_rate_hz: 10.0,
            threshold_eta: 0.01,
            rate_tau_ms: 1000.0,
            min_threshold: 0.3,
            max_threshold: 2.0,
            bcm_rate: 0.001,
            intrinsic_enabled: true,
        }
    }
}

/// Single neuron meta-plasticity state
#[derive(Clone, Copy, Debug)]
pub struct NeuronMetaState {
    /// Current threshold
    pub threshold: f32,
    /// Estimated firing rate (Hz)
    pub estimated_rate: f32,
    /// BCM modification threshold
    pub bcm_theta: f32,
    /// Intrinsic excitability
    pub excitability: f32,
    /// Spike count for rate estimation
    spike_count: u32,
    /// Time window for rate estimation (ms)
    window_ms: u32,
}

impl NeuronMetaState {
    pub fn new(initial_threshold: f32) -> Self {
        Self {
            threshold: initial_threshold,
            estimated_rate: 0.0,
            bcm_theta: 0.5,
            excitability: 1.0,
            spike_count: 0,
            window_ms: 0,
        }
    }

    /// Record a spike
    pub fn record_spike(&mut self) {
        self.spike_count += 1;
    }

    /// Update rate estimation
    pub fn update_rate(&mut self, dt_ms: u32, tau_ms: f32) {
        self.window_ms += dt_ms;

        // Exponential moving average of rate
        let dt_s = dt_ms as f32 / 1000.0;
        let alpha = 1.0 - expf(-dt_s / (tau_ms / 1000.0));

        // Instantaneous rate in Hz
        let instant_rate = if self.window_ms > 0 {
            (self.spike_count as f32) / (self.window_ms as f32 / 1000.0)
        } else {
            0.0
        };

        self.estimated_rate = self.estimated_rate * (1.0 - alpha) + instant_rate * alpha;

        // Reset window periodically
        if self.window_ms >= tau_ms as u32 {
            self.spike_count = 0;
            self.window_ms = 0;
        }
    }
}

/// Meta-plasticity controller for neuron population
pub struct MetaPlasticityController {
    config: MetaPlasticityConfig,
    /// Per-neuron meta states
    states: heapless::Vec<NeuronMetaState, 64>,
    /// Population average rate
    population_rate: f32,
    /// Global threshold adjustment
    global_adjustment: f32,
}

impl MetaPlasticityController {
    pub fn new(config: MetaPlasticityConfig) -> Self {
        Self {
            config,
            states: heapless::Vec::new(),
            population_rate: 0.0,
            global_adjustment: 0.0,
        }
    }

    /// Initialize neurons with threshold
    pub fn init_neurons(&mut self, count: usize, initial_threshold: f32) {
        self.states.clear();
        for _ in 0..count.min(64) {
            let _ = self.states.push(NeuronMetaState::new(initial_threshold));
        }
    }

    /// Record spike for neuron
    pub fn record_spike(&mut self, neuron_idx: usize) {
        if let Some(state) = self.states.get_mut(neuron_idx) {
            state.record_spike();
        }
    }

    /// Update all neuron meta-states
    pub fn update(&mut self, dt_ms: u32) {
        let tau_ms = self.config.rate_tau_ms;
        let intrinsic_enabled = self.config.intrinsic_enabled;
        let target_rate_hz = self.config.target_rate_hz;
        let threshold_eta = self.config.threshold_eta;
        let min_threshold = self.config.min_threshold;
        let max_threshold = self.config.max_threshold;
        let bcm_rate = self.config.bcm_rate;

        // Update individual neurons
        for state in self.states.iter_mut() {
            state.update_rate(dt_ms, tau_ms);

            // Homeostatic threshold adaptation
            if intrinsic_enabled {
                // Adapt threshold based on firing rate
                let rate_error = state.estimated_rate - target_rate_hz;
                let delta = threshold_eta * rate_error;
                state.threshold = (state.threshold + delta)
                    .clamp(min_threshold, max_threshold);

                // Update BCM sliding threshold
                let activity = state.estimated_rate / target_rate_hz;
                state.bcm_theta = state.bcm_theta * (1.0 - bcm_rate) + activity.powi(2) * bcm_rate;
                state.excitability = 1.0 / (1.0 + state.bcm_theta);
            }
        }

        // Update population statistics
        self.update_population_stats();
    }

    /// Update population-level statistics
    fn update_population_stats(&mut self) {
        if self.states.is_empty() {
            return;
        }

        // Average firing rate
        let total_rate: f32 = self.states.iter().map(|s| s.estimated_rate).sum();
        self.population_rate = total_rate / self.states.len() as f32;

        // Global adjustment based on population rate
        let population_error = self.population_rate - self.config.target_rate_hz;
        self.global_adjustment = population_error / self.config.target_rate_hz;
    }

    /// Get threshold for neuron
    pub fn get_threshold(&self, neuron_idx: usize) -> f32 {
        self.states
            .get(neuron_idx)
            .map(|s| s.threshold * (1.0 + self.global_adjustment * 0.1))
            .unwrap_or(1.0)
    }

    /// Get effective input gain (excitability)
    pub fn get_gain(&self, neuron_idx: usize) -> f32 {
        self.states
            .get(neuron_idx)
            .map(|s| s.excitability)
            .unwrap_or(1.0)
    }

    /// Get neuron state
    pub fn get_state(&self, neuron_idx: usize) -> Option<&NeuronMetaState> {
        self.states.get(neuron_idx)
    }

    /// Get population firing rate
    pub fn population_rate(&self) -> f32 {
        self.population_rate
    }

    /// Check if homeostasis is achieved
    pub fn is_homeostatic(&self) -> bool {
        let error = fabsf(self.population_rate - self.config.target_rate_hz);
        error < self.config.target_rate_hz * 0.2 // Within 20%
    }

    /// Get statistics
    pub fn stats(&self) -> MetaPlasticityStats {
        let thresholds: heapless::Vec<f32, 64> =
            self.states.iter().map(|s| s.threshold).collect();

        let rates: heapless::Vec<f32, 64> =
            self.states.iter().map(|s| s.estimated_rate).collect();

        let avg_threshold = if thresholds.is_empty() {
            1.0
        } else {
            thresholds.iter().sum::<f32>() / thresholds.len() as f32
        };

        let avg_rate = if rates.is_empty() {
            0.0
        } else {
            rates.iter().sum::<f32>() / rates.len() as f32
        };

        MetaPlasticityStats {
            avg_threshold,
            avg_rate,
            population_rate: self.population_rate,
            is_homeostatic: self.is_homeostatic(),
            neuron_count: self.states.len(),
        }
    }
}

/// Meta-plasticity statistics
#[derive(Clone, Copy, Debug)]
pub struct MetaPlasticityStats {
    pub avg_threshold: f32,
    pub avg_rate: f32,
    pub population_rate: f32,
    pub is_homeostatic: bool,
    pub neuron_count: usize,
}

/// Spike-Timing Dependent Plasticity (STDP) with meta-plasticity
pub struct MetaSTDP {
    /// Base learning rate
    pub eta: f32,
    /// Potentiation time constant (ms)
    pub tau_plus: f32,
    /// Depression time constant (ms)
    pub tau_minus: f32,
    /// Meta-plasticity scaling
    pub meta_scale: f32,
}

impl MetaSTDP {
    pub fn new(eta: f32, tau_plus: f32, tau_minus: f32) -> Self {
        Self {
            eta,
            tau_plus,
            tau_minus,
            meta_scale: 1.0,
        }
    }

    /// Calculate weight change based on spike timing
    ///
    /// # Arguments
    /// * `delta_t` - Time difference (post - pre) in ms
    /// * `pre_rate` - Presynaptic firing rate
    /// * `post_rate` - Postsynaptic firing rate
    /// * `bcm_theta` - BCM sliding threshold
    pub fn weight_change(
        &self,
        delta_t: f32,
        pre_rate: f32,
        post_rate: f32,
        bcm_theta: f32,
    ) -> f32 {
        let stdp = if delta_t > 0.0 {
            // LTP: post fires after pre
            self.eta * expf(-delta_t / self.tau_plus)
        } else {
            // LTD: pre fires after post
            -self.eta * expf(delta_t / self.tau_minus)
        };

        // BCM modulation: LTP only if post_rate > bcm_theta
        let bcm_mod = if delta_t > 0.0 {
            (post_rate - bcm_theta).max(0.0) / (1.0 + post_rate)
        } else {
            1.0
        };

        stdp * self.meta_scale * bcm_mod
    }

    /// Update meta-scale based on overall activity
    pub fn update_meta_scale(&mut self, activity_level: f32, target: f32) {
        let error = activity_level - target;
        // Reduce learning rate if too active, increase if too quiet
        self.meta_scale = (1.0 - error * 0.1).clamp(0.5, 2.0);
    }
}

impl Default for MetaSTDP {
    fn default() -> Self {
        Self::new(0.01, 20.0, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neuron_meta_state() {
        let mut state = NeuronMetaState::new(1.0);

        state.record_spike();
        state.record_spike();
        state.update_rate(100, 1000.0);

        assert!(state.estimated_rate > 0.0);
    }

    #[test]
    fn test_meta_plasticity_controller() {
        let config = MetaPlasticityConfig::default();
        let mut controller = MetaPlasticityController::new(config);

        controller.init_neurons(10, 1.0);

        // Simulate activity
        for _ in 0..100 {
            controller.record_spike(0);
            controller.record_spike(0);
            controller.record_spike(1);
            controller.update(10);
        }

        // Neuron 0 should have higher threshold (more active)
        let thresh0 = controller.get_threshold(0);
        let thresh9 = controller.get_threshold(9);
        assert!(thresh0 > thresh9);
    }

    #[test]
    fn test_stdp() {
        let stdp = MetaSTDP::new(0.01, 20.0, 20.0);

        // LTP: post fires after pre
        let ltp = stdp.weight_change(5.0, 10.0, 15.0, 10.0);
        assert!(ltp > 0.0);

        // LTD: pre fires after post
        let ltd = stdp.weight_change(-5.0, 10.0, 15.0, 10.0);
        assert!(ltd < 0.0);
    }

    #[test]
    fn test_homeostasis() {
        let config = MetaPlasticityConfig {
            target_rate_hz: 10.0,
            ..Default::default()
        };
        let mut controller = MetaPlasticityController::new(config);
        controller.init_neurons(5, 1.0);

        // Simulate stable activity
        for _ in 0..1000 {
            for i in 0..5 {
                if i % 2 == 0 {
                    controller.record_spike(i);
                }
            }
            controller.update(100);
        }

        let stats = controller.stats();
        assert!(stats.avg_threshold > 0.0);
    }
}
