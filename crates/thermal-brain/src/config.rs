//! Configuration structures for ThermalBrain

/// Main system configuration
#[derive(Clone, Debug)]
pub struct ThermalBrainConfig {
    /// Thermal governor configuration
    pub thermal: ThermalConfig,
    /// Sparse encoding configuration
    pub encoding: EncodingConfig,
    /// Neural network configuration
    pub neural: NeuralConfig,
    /// Storage configuration
    pub storage: StorageConfig,
}

impl Default for ThermalBrainConfig {
    fn default() -> Self {
        Self {
            thermal: ThermalConfig::default(),
            encoding: EncodingConfig::default(),
            neural: NeuralConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

/// Thermal governor configuration
#[derive(Clone, Debug)]
pub struct ThermalConfig {
    /// Target temperature in Celsius
    pub target_temp_c: f32,
    /// EMA smoothing factor (0.0 - 1.0)
    pub ema_alpha: f32,
    /// Temperature hysteresis for zone transitions
    pub hysteresis_c: f32,
    /// Zone thresholds [cool->warm, warm->hot, hot->critical, critical->emergency]
    pub zone_thresholds_c: [f32; 4],
}

impl Default for ThermalConfig {
    fn default() -> Self {
        Self {
            target_temp_c: 50.0,
            ema_alpha: 0.1,
            hysteresis_c: 2.0,
            zone_thresholds_c: [40.0, 50.0, 60.0, 70.0],
        }
    }
}

/// Sparse encoding configuration
#[derive(Clone, Debug)]
pub struct EncodingConfig {
    /// Feature vector dimensions
    pub feature_dims: usize,
    /// Ring buffer size (samples)
    pub buffer_size: usize,
    /// Short window size (samples)
    pub short_window: usize,
    /// Medium window size (samples)
    pub medium_window: usize,
    /// Long window size (samples)
    pub long_window: usize,
    /// Enable FFT features
    pub fft_enabled: bool,
}

impl Default for EncodingConfig {
    fn default() -> Self {
        Self {
            feature_dims: 16,
            buffer_size: 500,
            short_window: 50,
            medium_window: 200,
            long_window: 500,
            fft_enabled: false, // Disabled by default for embedded
        }
    }
}

/// Neural network configuration
#[derive(Clone, Debug)]
pub struct NeuralConfig {
    /// Number of pattern neurons
    pub num_neurons: usize,
    /// Membrane time constant (ms)
    pub tau_ms: f32,
    /// Base firing threshold
    pub base_threshold: f32,
    /// Base refractory period (ms)
    pub base_refractory_ms: u32,
    /// HNSW M parameter (connections per node)
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter
    pub hnsw_ef_search: usize,
}

impl Default for NeuralConfig {
    fn default() -> Self {
        Self {
            num_neurons: 64,
            tau_ms: 20.0,
            base_threshold: 0.5,
            base_refractory_ms: 10,
            hnsw_m: 8,
            hnsw_ef_construction: 50,
            hnsw_ef_search: 20,
        }
    }
}

/// Storage configuration
#[derive(Clone, Debug)]
pub struct StorageConfig {
    /// Maximum stored patterns
    pub max_patterns: usize,
    /// Auto-save interval (seconds, 0 = disabled)
    pub autosave_interval_s: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            max_patterns: 2000,
            autosave_interval_s: 300,
        }
    }
}

/// Builder for ThermalBrainConfig
pub struct ConfigBuilder {
    config: ThermalBrainConfig,
}

impl ConfigBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: ThermalBrainConfig::default(),
        }
    }

    /// Set thermal target temperature
    pub fn target_temp(mut self, temp_c: f32) -> Self {
        self.config.thermal.target_temp_c = temp_c;
        self
    }

    /// Set EMA smoothing factor
    pub fn ema_alpha(mut self, alpha: f32) -> Self {
        self.config.thermal.ema_alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// Set zone thresholds
    pub fn zone_thresholds(mut self, thresholds: [f32; 4]) -> Self {
        self.config.thermal.zone_thresholds_c = thresholds;
        self
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.encoding.buffer_size = size;
        self
    }

    /// Set number of neurons
    pub fn num_neurons(mut self, count: usize) -> Self {
        self.config.neural.num_neurons = count;
        self
    }

    /// Set neuron time constant
    pub fn tau_ms(mut self, tau: f32) -> Self {
        self.config.neural.tau_ms = tau;
        self
    }

    /// Set HNSW M parameter
    pub fn hnsw_m(mut self, m: usize) -> Self {
        self.config.neural.hnsw_m = m;
        self
    }

    /// Set maximum patterns
    pub fn max_patterns(mut self, max: usize) -> Self {
        self.config.storage.max_patterns = max;
        self
    }

    /// Enable FFT features
    pub fn enable_fft(mut self, enabled: bool) -> Self {
        self.config.encoding.fft_enabled = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> ThermalBrainConfig {
        self.config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ThermalBrainConfig::default();
        assert_eq!(config.thermal.target_temp_c, 50.0);
        assert_eq!(config.neural.num_neurons, 64);
        assert_eq!(config.storage.max_patterns, 2000);
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .target_temp(45.0)
            .num_neurons(32)
            .hnsw_m(4)
            .build();

        assert_eq!(config.thermal.target_temp_c, 45.0);
        assert_eq!(config.neural.num_neurons, 32);
        assert_eq!(config.neural.hnsw_m, 4);
    }
}
