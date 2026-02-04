//! Power management controller wrapping PMIC GPIO load enable and MCU sleep modes.
//!
//! Controls the load switch between the supercapacitor and the MCU compute core.
//! On real hardware, this toggles a GPIO pin connected to the PMIC load enable
//! or a discrete MOSFET switch.

/// State of the load switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub enum LoadState {
    /// Load disconnected — harvesting mode, minimal current.
    Disabled,
    /// Load connected — compute core powered.
    Enabled,
}

/// Tracks cumulative load enable/disable transitions for telemetry.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct PowerStats {
    /// Total number of load enable events.
    pub enable_count: u32,
    /// Total number of load disable events.
    pub disable_count: u32,
    /// Total number of emergency cutoffs.
    pub emergency_count: u32,
    /// Total number of watchdog-triggered cutoffs.
    pub watchdog_count: u32,
}

/// Power manager controlling the PMIC load switch and MCU sleep.
pub struct PowerManager {
    /// Current load switch state.
    state: LoadState,
    /// Cumulative statistics.
    stats: PowerStats,
    /// Whether a watchdog timer is armed.
    watchdog_armed: bool,
    /// Watchdog timeout in ms (mirrors config.max_active_ms).
    watchdog_timeout_ms: u16,
    /// Simulated elapsed time since load enable (ms), for host testing.
    #[cfg(feature = "std")]
    sim_elapsed_ms: u32,
}

impl PowerManager {
    /// Create a new power manager. Load starts disabled.
    pub fn new(watchdog_timeout_ms: u16) -> Self {
        Self {
            state: LoadState::Disabled,
            stats: PowerStats::default(),
            watchdog_armed: false,
            watchdog_timeout_ms,
            #[cfg(feature = "std")]
            sim_elapsed_ms: 0,
        }
    }

    /// Enable the compute load (connect VSTOR to MCU core).
    ///
    /// On hardware: sets GPIO high → MOSFET/PMIC load enable.
    /// Arms the watchdog timer.
    pub fn enable_core(&mut self) {
        if self.state == LoadState::Disabled {
            self.state = LoadState::Enabled;
            self.stats.enable_count = self.stats.enable_count.saturating_add(1);
            self.watchdog_armed = true;
            #[cfg(feature = "std")]
            {
                self.sim_elapsed_ms = 0;
            }
            // On hardware:
            // gpio_load_en.set_high().unwrap();
            // watchdog.start(self.watchdog_timeout_ms);
        }
    }

    /// Disable the compute load (disconnect VSTOR from MCU core).
    ///
    /// On hardware: sets GPIO low → MOSFET/PMIC load disable.
    /// Disarms the watchdog.
    pub fn disable_core(&mut self) {
        if self.state == LoadState::Enabled {
            self.state = LoadState::Disabled;
            self.stats.disable_count = self.stats.disable_count.saturating_add(1);
            self.watchdog_armed = false;
            // On hardware:
            // gpio_load_en.set_low().unwrap();
            // watchdog.cancel();
        }
    }

    /// Emergency load cutoff — immediate disconnect regardless of state.
    ///
    /// Called when VSTOR drops below TH_CRITICAL during execution.
    pub fn emergency_cutoff(&mut self) {
        self.state = LoadState::Disabled;
        self.watchdog_armed = false;
        self.stats.emergency_count = self.stats.emergency_count.saturating_add(1);
        // On hardware:
        // gpio_load_en.set_low().unwrap();
        // watchdog.cancel();
    }

    /// Record a watchdog-triggered cutoff (called by ISR or duty cycle controller).
    pub fn watchdog_cutoff(&mut self) {
        self.state = LoadState::Disabled;
        self.watchdog_armed = false;
        self.stats.watchdog_count = self.stats.watchdog_count.saturating_add(1);
    }

    /// Enter MCU low-power sleep mode for the given duration.
    ///
    /// On hardware: enters Stop/Standby mode with RTC wakeup.
    /// In simulation: no-op (returns immediately).
    pub fn sleep_ms(&self, _duration_ms: u32) {
        // On hardware:
        // rtc.set_wakeup(duration_ms);
        // pwr.enter_stop_mode();
        // — MCU halts here until RTC interrupt —

        // In std simulation: this is a no-op; the duty cycle controller
        // simulates time advancement.
    }

    /// Get current load switch state.
    pub fn state(&self) -> LoadState {
        self.state
    }

    /// Get cumulative power statistics.
    pub fn stats(&self) -> PowerStats {
        self.stats
    }

    /// Check if watchdog is currently armed.
    pub fn watchdog_armed(&self) -> bool {
        self.watchdog_armed
    }

    /// Get watchdog timeout setting.
    pub fn watchdog_timeout_ms(&self) -> u16 {
        self.watchdog_timeout_ms
    }

    /// Check if execution has exceeded watchdog timeout (simulation only).
    #[cfg(feature = "std")]
    pub fn check_watchdog(&mut self, elapsed_ms: u32) -> bool {
        if self.watchdog_armed && elapsed_ms >= self.watchdog_timeout_ms as u32 {
            self.watchdog_cutoff();
            true
        } else {
            false
        }
    }

    /// Reset statistics counters.
    pub fn reset_stats(&mut self) {
        self.stats = PowerStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_disabled() {
        let pm = PowerManager::new(50);
        assert_eq!(pm.state(), LoadState::Disabled);
        assert!(!pm.watchdog_armed());
    }

    #[test]
    fn enable_disable_cycle() {
        let mut pm = PowerManager::new(50);

        pm.enable_core();
        assert_eq!(pm.state(), LoadState::Enabled);
        assert!(pm.watchdog_armed());
        assert_eq!(pm.stats().enable_count, 1);

        pm.disable_core();
        assert_eq!(pm.state(), LoadState::Disabled);
        assert!(!pm.watchdog_armed());
        assert_eq!(pm.stats().disable_count, 1);
    }

    #[test]
    fn emergency_cutoff_records_event() {
        let mut pm = PowerManager::new(50);
        pm.enable_core();
        pm.emergency_cutoff();

        assert_eq!(pm.state(), LoadState::Disabled);
        assert_eq!(pm.stats().emergency_count, 1);
    }

    #[test]
    fn watchdog_timeout() {
        let mut pm = PowerManager::new(50);
        pm.enable_core();

        assert!(!pm.check_watchdog(49));
        assert!(pm.check_watchdog(50));
        assert_eq!(pm.state(), LoadState::Disabled);
        assert_eq!(pm.stats().watchdog_count, 1);
    }

    #[test]
    fn double_enable_is_idempotent() {
        let mut pm = PowerManager::new(50);
        pm.enable_core();
        pm.enable_core(); // second call should be no-op
        assert_eq!(pm.stats().enable_count, 1);
    }

    #[test]
    fn double_disable_is_idempotent() {
        let mut pm = PowerManager::new(50);
        pm.enable_core();
        pm.disable_core();
        pm.disable_core(); // second call should be no-op
        assert_eq!(pm.stats().disable_count, 1);
    }
}
