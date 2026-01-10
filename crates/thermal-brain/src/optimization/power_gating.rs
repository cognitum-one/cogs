//! Power Gating for Neuron Banks
//!
//! Completely power-off idle neuron banks to achieve zero leakage power.
//! Based on fine-grained power gating techniques from neuromorphic research.
//!
//! Key features:
//! - Per-bank power control
//! - Wake-on-spike capability
//! - Retention state for fast wake-up
//! - Activity-based automatic gating

use heapless::Vec as HVec;

/// Maximum neuron banks
const MAX_BANKS: usize = 16;

/// Neurons per bank
const NEURONS_PER_BANK: usize = 8;

/// Power state for a neuron bank
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerState {
    /// Fully powered, normal operation
    Active,
    /// Clock gated, state retained, fast wake-up (~10 cycles)
    ClockGated,
    /// Power gated with retention, medium wake-up (~100 cycles)
    RetentionSleep,
    /// Fully power gated, slow wake-up (~1000 cycles), state lost
    DeepSleep,
}

impl PowerState {
    /// Get relative power consumption (0.0 to 1.0)
    pub fn power_factor(&self) -> f32 {
        match self {
            PowerState::Active => 1.0,
            PowerState::ClockGated => 0.3,      // Only leakage
            PowerState::RetentionSleep => 0.05, // Minimal retention
            PowerState::DeepSleep => 0.001,     // Near zero
        }
    }

    /// Get wake-up latency in cycles
    pub fn wake_latency(&self) -> u32 {
        match self {
            PowerState::Active => 0,
            PowerState::ClockGated => 10,
            PowerState::RetentionSleep => 100,
            PowerState::DeepSleep => 1000,
        }
    }
}

/// Neuron bank state
#[derive(Clone, Copy, Debug)]
struct BankState {
    /// Current power state
    power: PowerState,
    /// Activity counter (spikes in recent window)
    activity: u16,
    /// Cycles since last activity
    idle_cycles: u32,
    /// Retained membrane potentials (for retention sleep)
    retained_state: [i16; NEURONS_PER_BANK],
    /// Bank is enabled
    enabled: bool,
}

impl Default for BankState {
    fn default() -> Self {
        Self {
            power: PowerState::Active,
            activity: 0,
            idle_cycles: 0,
            retained_state: [0; NEURONS_PER_BANK],
            enabled: true,
        }
    }
}

/// Power gating configuration
#[derive(Clone, Copy, Debug)]
pub struct PowerGatingConfig {
    /// Cycles before clock gating
    pub clock_gate_threshold: u32,
    /// Cycles before retention sleep
    pub retention_threshold: u32,
    /// Cycles before deep sleep
    pub deep_sleep_threshold: u32,
    /// Activity threshold to stay active
    pub activity_threshold: u16,
    /// Enable automatic power gating
    pub auto_gate: bool,
    /// Enable wake-on-spike
    pub wake_on_spike: bool,
}

impl Default for PowerGatingConfig {
    fn default() -> Self {
        Self {
            clock_gate_threshold: 100,
            retention_threshold: 1000,
            deep_sleep_threshold: 10000,
            activity_threshold: 5,
            auto_gate: true,
            wake_on_spike: true,
        }
    }
}

/// Power gating controller
///
/// Manages power states for multiple neuron banks to minimize
/// power consumption while maintaining responsiveness.
pub struct PowerGatingController {
    config: PowerGatingConfig,
    /// Bank states
    banks: HVec<BankState, MAX_BANKS>,
    /// Total power savings (accumulated)
    total_savings: f32,
    /// Current cycle count
    cycle_count: u64,
    /// Wake events counter
    wake_events: u32,
}

impl PowerGatingController {
    /// Create a new power gating controller
    pub fn new(config: PowerGatingConfig, num_banks: usize) -> Self {
        let mut banks = HVec::new();
        for _ in 0..num_banks.min(MAX_BANKS) {
            let _ = banks.push(BankState::default());
        }

        Self {
            config,
            banks,
            total_savings: 0.0,
            cycle_count: 0,
            wake_events: 0,
        }
    }

    /// Update power states for all banks
    ///
    /// Call this every cycle to manage power gating
    pub fn update(&mut self) {
        self.cycle_count += 1;

        if !self.config.auto_gate {
            return;
        }

        // Collect transitions needed (to avoid borrow conflicts)
        let mut transitions: heapless::Vec<(usize, PowerState), 16> = heapless::Vec::new();

        for (idx, bank) in self.banks.iter_mut().enumerate() {
            if !bank.enabled {
                continue;
            }

            // Update idle counter
            if bank.activity == 0 {
                bank.idle_cycles += 1;
            } else {
                bank.idle_cycles = 0;
            }

            // Decay activity counter
            if self.cycle_count % 100 == 0 {
                bank.activity = bank.activity.saturating_sub(1);
            }

            // Determine target power state
            let target = if bank.activity >= self.config.activity_threshold {
                PowerState::Active
            } else if bank.idle_cycles < self.config.clock_gate_threshold {
                PowerState::Active
            } else if bank.idle_cycles < self.config.retention_threshold {
                PowerState::ClockGated
            } else if bank.idle_cycles < self.config.deep_sleep_threshold {
                PowerState::RetentionSleep
            } else {
                PowerState::DeepSleep
            };

            // Record transition if needed
            if target != bank.power {
                let _ = transitions.push((idx, target));
            }
        }

        // Apply transitions
        for (bank_idx, target) in transitions {
            self.transition_bank_by_id(bank_idx, target);
        }

        // Update power savings estimate
        self.update_savings();
    }

    /// Transition a bank to a new power state
    fn transition_bank(&mut self, bank: &mut BankState, target: PowerState) {
        match (&bank.power, &target) {
            // Going to deeper sleep - save state if needed
            (PowerState::Active, PowerState::RetentionSleep) |
            (PowerState::ClockGated, PowerState::RetentionSleep) => {
                // State is retained automatically in retention sleep
            }
            (_, PowerState::DeepSleep) => {
                // Clear retained state - will be lost
                bank.retained_state = [0; NEURONS_PER_BANK];
            }
            // Waking up
            (PowerState::DeepSleep, _) => {
                // State was lost, start fresh
                bank.retained_state = [0; NEURONS_PER_BANK];
                self.wake_events += 1;
            }
            (PowerState::RetentionSleep, PowerState::Active) |
            (PowerState::RetentionSleep, PowerState::ClockGated) => {
                // Restore from retention
                self.wake_events += 1;
            }
            _ => {}
        }

        bank.power = target;
    }

    /// Record spike activity for a bank
    pub fn record_activity(&mut self, bank_id: usize, spike_count: u16) {
        if let Some(bank) = self.banks.get_mut(bank_id) {
            bank.activity = bank.activity.saturating_add(spike_count);
            bank.idle_cycles = 0;

            // Wake up if sleeping and wake-on-spike enabled
            if self.config.wake_on_spike && bank.power != PowerState::Active {
                self.wake_bank(bank_id);
            }
        }
    }

    /// Manually wake a bank
    pub fn wake_bank(&mut self, bank_id: usize) {
        if let Some(bank) = self.banks.get_mut(bank_id) {
            if bank.power != PowerState::Active {
                self.transition_bank_by_id(bank_id, PowerState::Active);
            }
        }
    }

    /// Helper to transition bank by ID
    fn transition_bank_by_id(&mut self, bank_id: usize, target: PowerState) {
        if let Some(bank) = self.banks.get_mut(bank_id) {
            let old_power = bank.power;

            match (&old_power, &target) {
                (PowerState::DeepSleep, _) |
                (PowerState::RetentionSleep, PowerState::Active) |
                (PowerState::RetentionSleep, PowerState::ClockGated) => {
                    self.wake_events += 1;
                }
                _ => {}
            }

            if target == PowerState::DeepSleep {
                bank.retained_state = [0; NEURONS_PER_BANK];
            }

            bank.power = target;
        }
    }

    /// Manually sleep a bank
    pub fn sleep_bank(&mut self, bank_id: usize, state: PowerState) {
        self.transition_bank_by_id(bank_id, state);
    }

    /// Get power state of a bank
    pub fn bank_power_state(&self, bank_id: usize) -> Option<PowerState> {
        self.banks.get(bank_id).map(|b| b.power)
    }

    /// Check if bank is active
    pub fn is_bank_active(&self, bank_id: usize) -> bool {
        self.banks.get(bank_id)
            .map(|b| b.power == PowerState::Active)
            .unwrap_or(false)
    }

    /// Get number of active banks
    pub fn active_bank_count(&self) -> usize {
        self.banks.iter().filter(|b| b.power == PowerState::Active).count()
    }

    /// Get number of sleeping banks
    pub fn sleeping_bank_count(&self) -> usize {
        self.banks.iter().filter(|b| b.power != PowerState::Active).count()
    }

    /// Calculate current power consumption factor
    pub fn current_power_factor(&self) -> f32 {
        if self.banks.is_empty() {
            return 1.0;
        }

        let total: f32 = self.banks.iter()
            .filter(|b| b.enabled)
            .map(|b| b.power.power_factor())
            .sum();

        total / self.banks.iter().filter(|b| b.enabled).count() as f32
    }

    /// Update power savings calculation
    fn update_savings(&mut self) {
        let savings = 1.0 - self.current_power_factor();
        self.total_savings += savings;
    }

    /// Get average power savings
    pub fn average_power_savings(&self) -> f32 {
        if self.cycle_count == 0 {
            0.0
        } else {
            self.total_savings / self.cycle_count as f32
        }
    }

    /// Get wake latency for a bank (cycles)
    pub fn wake_latency(&self, bank_id: usize) -> u32 {
        self.banks.get(bank_id)
            .map(|b| b.power.wake_latency())
            .unwrap_or(0)
    }

    /// Get total wake events
    pub fn wake_events(&self) -> u32 {
        self.wake_events
    }

    /// Enable/disable a bank
    pub fn set_bank_enabled(&mut self, bank_id: usize, enabled: bool) {
        if let Some(bank) = self.banks.get_mut(bank_id) {
            bank.enabled = enabled;
            if !enabled {
                bank.power = PowerState::DeepSleep;
            }
        }
    }

    /// Save state for a bank (before retention sleep)
    pub fn save_bank_state(&mut self, bank_id: usize, state: &[i16]) {
        if let Some(bank) = self.banks.get_mut(bank_id) {
            for (i, &v) in state.iter().take(NEURONS_PER_BANK).enumerate() {
                bank.retained_state[i] = v;
            }
        }
    }

    /// Restore state for a bank (after wake from retention)
    pub fn restore_bank_state(&self, bank_id: usize) -> Option<[i16; NEURONS_PER_BANK]> {
        self.banks.get(bank_id).map(|b| b.retained_state)
    }

    /// Get number of banks
    pub fn num_banks(&self) -> usize {
        self.banks.len()
    }

    /// Force all banks to a specific state
    pub fn force_all_state(&mut self, state: PowerState) {
        for i in 0..self.banks.len() {
            self.transition_bank_by_id(i, state);
        }
    }

    /// Reset controller
    pub fn reset(&mut self) {
        for bank in self.banks.iter_mut() {
            bank.power = PowerState::Active;
            bank.activity = 0;
            bank.idle_cycles = 0;
            bank.retained_state = [0; NEURONS_PER_BANK];
        }
        self.total_savings = 0.0;
        self.cycle_count = 0;
        self.wake_events = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_gating_controller() {
        let config = PowerGatingConfig::default();
        let controller = PowerGatingController::new(config, 4);

        assert_eq!(controller.num_banks(), 4);
        assert_eq!(controller.active_bank_count(), 4);
    }

    #[test]
    fn test_power_states() {
        assert_eq!(PowerState::Active.power_factor(), 1.0);
        assert!(PowerState::DeepSleep.power_factor() < 0.01);
        assert_eq!(PowerState::Active.wake_latency(), 0);
        assert!(PowerState::DeepSleep.wake_latency() > 100);
    }

    #[test]
    fn test_auto_gating() {
        let config = PowerGatingConfig {
            clock_gate_threshold: 10,
            retention_threshold: 50,
            deep_sleep_threshold: 100,
            auto_gate: true,
            ..Default::default()
        };
        let mut controller = PowerGatingController::new(config, 2);

        // Run many cycles without activity
        for _ in 0..200 {
            controller.update();
        }

        // Banks should be in deep sleep
        assert_eq!(controller.bank_power_state(0), Some(PowerState::DeepSleep));
    }

    #[test]
    fn test_wake_on_spike() {
        let config = PowerGatingConfig {
            clock_gate_threshold: 5,
            wake_on_spike: true,
            auto_gate: true,
            ..Default::default()
        };
        let mut controller = PowerGatingController::new(config, 2);

        // Let bank go to sleep
        for _ in 0..20 {
            controller.update();
        }

        // Record activity should wake it
        controller.record_activity(0, 10);

        assert_eq!(controller.bank_power_state(0), Some(PowerState::Active));
    }

    #[test]
    fn test_power_savings() {
        let config = PowerGatingConfig {
            clock_gate_threshold: 5,
            retention_threshold: 10,
            deep_sleep_threshold: 20,
            auto_gate: true,
            ..Default::default()
        };
        let mut controller = PowerGatingController::new(config, 4);

        // Initial - all active
        assert_eq!(controller.current_power_factor(), 1.0);

        // Let banks sleep
        for _ in 0..50 {
            controller.update();
        }

        // Should have significant power savings
        assert!(controller.current_power_factor() < 0.5);
    }

    #[test]
    fn test_state_retention() {
        let config = PowerGatingConfig::default();
        let mut controller = PowerGatingController::new(config, 2);

        // Save state
        let state = [100i16, 200, 300, 400, 0, 0, 0, 0];
        controller.save_bank_state(0, &state);

        // Sleep with retention
        controller.sleep_bank(0, PowerState::RetentionSleep);

        // Wake and restore
        controller.wake_bank(0);
        let restored = controller.restore_bank_state(0).unwrap();

        assert_eq!(restored[0], 100);
        assert_eq!(restored[1], 200);
    }
}
