//! Configuration constants and tunable parameters for the energy harvester.
//!
//! All voltage thresholds are in millivolts (mV).
//! All time values are in milliseconds (ms).
//! All energy values are in microjoules (µJ) stored as u32.

/// Main configuration for the energy harvester micro-agent.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct HarvesterConfig {
    /// Voltage threshold to permit a wake+execute cycle (mV).
    pub th_wake_mv: u16,

    /// Voltage threshold to return to sleep after execution (mV).
    pub th_sleep_mv: u16,

    /// Critical voltage: immediately cut load (mV).
    pub th_critical_mv: u16,

    /// Hysteresis band applied to thresholds (mV).
    pub hysteresis_mv: u16,

    /// Duty cycle period — time between wake attempts (ms).
    pub duty_period_ms: u32,

    /// Maximum active execution time before watchdog fires (ms).
    pub max_active_ms: u16,

    /// Number of ADC samples to average per reading.
    pub adc_oversampling: u8,

    /// Energy budget safety margin (fixed-point ×100, e.g. 110 = 1.10×).
    pub budget_margin_pct: u8,

    /// Rolling window length for energy ledger (number of slots).
    pub ledger_slots: u16,

    /// Sustainability ratio threshold (×100). Below this, reduce wake frequency.
    pub sustainability_ratio_pct: u16,

    /// Surplus ratio threshold (×100). Above this, increase wake frequency or enable radio.
    pub surplus_ratio_pct: u16,

    /// ADC reference voltage (mV). Used to convert ADC counts to voltage.
    pub adc_vref_mv: u16,

    /// ADC resolution in bits.
    pub adc_resolution_bits: u8,

    /// Estimated active current draw during execution (µA).
    pub active_current_ua: u32,

    /// Estimated sleep current draw (nA, stored as u32 for fixed-point).
    pub sleep_current_na: u32,

    /// MCU supply voltage during execution (mV).
    pub mcu_vdd_mv: u16,
}

/// Precomputed values derived from config — computed once, used every cycle.
#[derive(Clone, Copy, Debug)]
pub struct ConfigDerived {
    /// ADC max count: `(1 << resolution_bits) - 1`.
    pub adc_max_counts: u32,
    /// Precomputed active energy per cycle (µJ).
    pub active_energy_uj: u32,
    /// Precomputed sleep energy per cycle (µJ).
    pub sleep_energy_uj: u32,
    /// Total estimated energy per duty cycle (µJ).
    pub cycle_energy_uj: u32,
}

impl Default for HarvesterConfig {
    fn default() -> Self {
        Self {
            th_wake_mv: 3300,
            th_sleep_mv: 2800,
            th_critical_mv: 2200,
            hysteresis_mv: 200,
            duty_period_ms: 300_000, // 5 minutes
            max_active_ms: 50,
            adc_oversampling: 4,
            budget_margin_pct: 110, // 1.10×
            ledger_slots: 2016,     // 7 days × 24h × 12/h = 2016 slots at 5-min intervals
            sustainability_ratio_pct: 110,
            surplus_ratio_pct: 200,
            adc_vref_mv: 3300,
            adc_resolution_bits: 12,
            active_current_ua: 5000, // 5 mA
            sleep_current_na: 500,   // 0.5 µA
            mcu_vdd_mv: 3300,
        }
    }
}

impl HarvesterConfig {
    /// Precompute derived values from this config.
    ///
    /// Call once at init and pass to subsystems that need fast-path access.
    #[inline]
    pub fn derive(&self) -> ConfigDerived {
        let adc_max_counts = (1u32 << self.adc_resolution_bits) - 1;
        let active_energy_uj = self.compute_active_energy_uj();
        let sleep_energy_uj = self.compute_sleep_energy_uj();
        ConfigDerived {
            adc_max_counts,
            active_energy_uj,
            sleep_energy_uj,
            cycle_energy_uj: active_energy_uj.saturating_add(sleep_energy_uj),
        }
    }

    /// Convert raw ADC counts to millivolts using precomputed max_counts.
    #[inline]
    pub fn adc_to_mv(&self, counts: u16) -> u16 {
        let max_counts = (1u32 << self.adc_resolution_bits) - 1;
        ((counts as u32 * self.adc_vref_mv as u32) / max_counts) as u16
    }

    /// Convert raw ADC counts to millivolts using precomputed derived values.
    #[inline]
    pub fn adc_to_mv_fast(&self, counts: u16, derived: &ConfigDerived) -> u16 {
        ((counts as u32 * self.adc_vref_mv as u32) / derived.adc_max_counts) as u16
    }

    /// Estimate energy consumed during one active window (µJ).
    pub fn estimate_active_energy_uj(&self) -> u32 {
        self.compute_active_energy_uj()
    }

    /// Estimate energy consumed during one sleep period (µJ).
    pub fn estimate_sleep_energy_uj(&self) -> u32 {
        self.compute_sleep_energy_uj()
    }

    /// Total estimated energy per duty cycle (µJ).
    pub fn estimate_cycle_energy_uj(&self) -> u32 {
        self.compute_active_energy_uj()
            .saturating_add(self.compute_sleep_energy_uj())
    }

    /// Internal: compute active energy (µJ).
    /// E(µJ) = I(µA) × V(mV) × t(ms) / 1_000_000
    #[inline]
    fn compute_active_energy_uj(&self) -> u32 {
        let i_ua = self.active_current_ua as u64;
        let v_mv = self.mcu_vdd_mv as u64;
        let t_ms = self.max_active_ms as u64;
        ((i_ua * v_mv * t_ms) / 1_000_000) as u32
    }

    /// Internal: compute sleep energy (µJ).
    /// E(µJ) = I(nA) × V(mV) × t(ms) / 1_000_000_000
    #[inline]
    fn compute_sleep_energy_uj(&self) -> u32 {
        let i_na = self.sleep_current_na as u64;
        let v_mv = self.mcu_vdd_mv as u64;
        let t_ms = self.duty_period_ms as u64;
        ((i_na * v_mv * t_ms) / 1_000_000_000) as u32
    }

    /// Check if wake threshold respects hysteresis relative to sleep threshold.
    pub fn validate(&self) -> bool {
        self.th_wake_mv > self.th_sleep_mv
            && self.th_sleep_mv > self.th_critical_mv
            && (self.th_wake_mv - self.th_sleep_mv) >= self.hysteresis_mv
            && self.max_active_ms > 0
            && self.duty_period_ms > self.max_active_ms as u32
            && self.adc_oversampling > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = HarvesterConfig::default();
        assert!(cfg.validate());
    }

    #[test]
    fn adc_to_mv_conversion() {
        let cfg = HarvesterConfig::default();
        // 12-bit ADC, 3300 mV reference
        assert_eq!(cfg.adc_to_mv(0), 0);
        assert_eq!(cfg.adc_to_mv(4095), 3300);
        assert_eq!(cfg.adc_to_mv(2048), 1650); // ~midpoint
    }

    #[test]
    fn energy_estimates_reasonable() {
        let cfg = HarvesterConfig::default();
        // Active: 5mA × 3.3V × 50ms = 825 µJ
        assert_eq!(cfg.estimate_active_energy_uj(), 825);
        // Sleep: 0.5µA × 3.3V × 300s = 495 µJ
        assert_eq!(cfg.estimate_sleep_energy_uj(), 495);
        // Total per cycle
        assert_eq!(cfg.estimate_cycle_energy_uj(), 1320);
    }

    #[test]
    fn invalid_config_detected() {
        let mut cfg = HarvesterConfig::default();
        cfg.th_wake_mv = cfg.th_sleep_mv; // wake == sleep violates ordering
        assert!(!cfg.validate());
    }

    #[test]
    fn derived_values_match_computed() {
        let cfg = HarvesterConfig::default();
        let derived = cfg.derive();
        assert_eq!(derived.adc_max_counts, 4095);
        assert_eq!(derived.active_energy_uj, cfg.estimate_active_energy_uj());
        assert_eq!(derived.sleep_energy_uj, cfg.estimate_sleep_energy_uj());
        assert_eq!(derived.cycle_energy_uj, cfg.estimate_cycle_energy_uj());
    }

    #[test]
    fn fast_adc_conversion_matches_standard() {
        let cfg = HarvesterConfig::default();
        let derived = cfg.derive();
        for counts in [0u16, 1, 100, 2048, 4095] {
            assert_eq!(cfg.adc_to_mv(counts), cfg.adc_to_mv_fast(counts, &derived));
        }
    }
}
