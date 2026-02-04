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
    /// Convert raw ADC counts to millivolts.
    pub fn adc_to_mv(&self, counts: u16) -> u16 {
        let max_counts = (1u32 << self.adc_resolution_bits) - 1;
        ((counts as u32 * self.adc_vref_mv as u32) / max_counts) as u16
    }

    /// Estimate energy consumed during one active window (µJ).
    pub fn estimate_active_energy_uj(&self) -> u32 {
        // E = I × V × t
        // I in µA, V in mV, t in ms → E in pJ / 1_000_000 → µJ
        let i_ua = self.active_current_ua as u64;
        let v_mv = self.mcu_vdd_mv as u64;
        let t_ms = self.max_active_ms as u64;
        // i_ua * v_mv * t_ms gives pico-joule-seconds / 1000
        // = (µA × mV × ms) = (1e-6 × 1e-3 × 1e-3) = 1e-12 → need /1 to get µJ
        // Actually: µA × mV = nW, nW × ms = nW·ms = µJ × 1e-3... let me be precise:
        // I(A) = I_ua * 1e-6
        // V(V) = V_mv * 1e-3
        // t(s) = t_ms * 1e-3
        // E(J) = I*V*t = I_ua * V_mv * t_ms * 1e-12
        // E(µJ) = I_ua * V_mv * t_ms * 1e-6
        // = (I_ua * V_mv * t_ms) / 1_000_000
        ((i_ua * v_mv * t_ms) / 1_000_000) as u32
    }

    /// Estimate energy consumed during one sleep period (µJ).
    pub fn estimate_sleep_energy_uj(&self) -> u32 {
        let i_na = self.sleep_current_na as u64;
        let v_mv = self.mcu_vdd_mv as u64;
        let t_ms = self.duty_period_ms as u64;
        // I(A) = i_na * 1e-9, V(V) = v_mv * 1e-3, t(s) = t_ms * 1e-3
        // E(J) = i_na * v_mv * t_ms * 1e-15
        // E(µJ) = i_na * v_mv * t_ms * 1e-9
        // = (i_na * v_mv * t_ms) / 1_000_000_000
        ((i_na * v_mv * t_ms) / 1_000_000_000) as u32
    }

    /// Total estimated energy per duty cycle (µJ).
    pub fn estimate_cycle_energy_uj(&self) -> u32 {
        self.estimate_active_energy_uj()
            .saturating_add(self.estimate_sleep_energy_uj())
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
}
