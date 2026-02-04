//! ADC abstraction for reading VSTOR voltage and harvester current.
//!
//! Uses `embedded-hal` traits for portability across MCU families.
//! Supports oversampling with averaging for noise reduction.

use crate::config::HarvesterConfig;

/// Raw ADC reading with metadata.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct AdcReading {
    /// Averaged ADC count (after oversampling).
    pub counts: u16,
    /// Converted voltage in millivolts.
    pub voltage_mv: u16,
    /// Number of samples averaged.
    pub samples: u8,
}

/// Abstraction over the MCU's ADC peripheral.
///
/// On real hardware, this wraps an `embedded-hal::adc::OneShot` channel.
/// In simulation mode, it can be driven by software-injected values.
pub struct AdcReader {
    /// Current VSTOR reading (charge reservoir voltage).
    vstor: AdcReading,
    /// Current harvester input current reading (µA).
    harvest_current_ua: u32,
    /// Configuration reference.
    oversampling: u8,
    adc_vref_mv: u16,
    adc_resolution_bits: u8,
    /// Simulated ADC values for host testing.
    #[cfg(feature = "std")]
    sim_vstor_mv: u16,
    #[cfg(feature = "std")]
    sim_current_ua: u32,
}

impl AdcReader {
    /// Create a new ADC reader from configuration.
    pub fn new(config: &HarvesterConfig) -> Self {
        Self {
            vstor: AdcReading::default(),
            harvest_current_ua: 0,
            oversampling: config.adc_oversampling,
            adc_vref_mv: config.adc_vref_mv,
            adc_resolution_bits: config.adc_resolution_bits,
            #[cfg(feature = "std")]
            sim_vstor_mv: config.th_wake_mv, // start at wake threshold
            #[cfg(feature = "std")]
            sim_current_ua: 100, // default 100 µA harvest
        }
    }

    /// Read VSTOR voltage with oversampling.
    ///
    /// On real hardware, this performs N ADC conversions and averages.
    /// In simulation, returns the injected value.
    pub fn read_vstor(&mut self) -> AdcReading {
        #[cfg(feature = "std")]
        {
            let mv = self.sim_vstor_mv;
            let max_counts = (1u32 << self.adc_resolution_bits) - 1;
            let counts = ((mv as u32 * max_counts) / self.adc_vref_mv as u32) as u16;
            self.vstor = AdcReading {
                counts,
                voltage_mv: mv,
                samples: self.oversampling,
            };
        }

        #[cfg(not(feature = "std"))]
        {
            // On bare metal, this would call embedded-hal ADC:
            // let mut sum: u32 = 0;
            // for _ in 0..self.oversampling {
            //     sum += adc.read(&mut vstor_pin).unwrap() as u32;
            // }
            // let avg = (sum / self.oversampling as u32) as u16;
            // ... convert to mV ...
            // For now, return last known value
        }

        self.vstor
    }

    /// Read harvester input current in µA.
    pub fn read_harvest_current(&mut self) -> u32 {
        #[cfg(feature = "std")]
        {
            self.harvest_current_ua = self.sim_current_ua;
        }

        self.harvest_current_ua
    }

    /// Get the most recent VSTOR reading without re-sampling.
    pub fn last_vstor(&self) -> AdcReading {
        self.vstor
    }

    /// Get the most recent harvest current without re-sampling.
    pub fn last_harvest_current(&self) -> u32 {
        self.harvest_current_ua
    }

    /// Convert raw ADC counts to millivolts.
    pub fn counts_to_mv(&self, counts: u16) -> u16 {
        let max_counts = (1u32 << self.adc_resolution_bits) - 1;
        ((counts as u32 * self.adc_vref_mv as u32) / max_counts) as u16
    }

    // --- Simulation helpers (host only) ---

    /// Set simulated VSTOR voltage for host testing.
    #[cfg(feature = "std")]
    pub fn set_sim_vstor_mv(&mut self, mv: u16) {
        self.sim_vstor_mv = mv;
    }

    /// Set simulated harvest current for host testing.
    #[cfg(feature = "std")]
    pub fn set_sim_current_ua(&mut self, ua: u32) {
        self.sim_current_ua = ua;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adc_reader_defaults() {
        let cfg = HarvesterConfig::default();
        let mut adc = AdcReader::new(&cfg);
        let reading = adc.read_vstor();
        assert_eq!(reading.voltage_mv, cfg.th_wake_mv);
        assert_eq!(reading.samples, cfg.adc_oversampling);
    }

    #[test]
    fn sim_vstor_injection() {
        let cfg = HarvesterConfig::default();
        let mut adc = AdcReader::new(&cfg);

        adc.set_sim_vstor_mv(2500);
        let reading = adc.read_vstor();
        assert_eq!(reading.voltage_mv, 2500);
    }

    #[test]
    fn counts_to_mv_roundtrip() {
        let cfg = HarvesterConfig::default();
        let adc = AdcReader::new(&cfg);

        // 3300 mV should map to max counts and back
        let max_counts = (1u32 << cfg.adc_resolution_bits) - 1;
        let mv = adc.counts_to_mv(max_counts as u16);
        assert_eq!(mv, 3300);
    }

    #[test]
    fn harvest_current_simulation() {
        let cfg = HarvesterConfig::default();
        let mut adc = AdcReader::new(&cfg);

        adc.set_sim_current_ua(250);
        assert_eq!(adc.read_harvest_current(), 250);
    }
}
