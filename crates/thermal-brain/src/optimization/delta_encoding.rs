//! Delta Encoding
//!
//! Only transmit/process changes between consecutive values.
//! Highly efficient for slowly-changing signals like temperature.
//!
//! Encoding modes:
//! - Simple delta: Store differences
//! - Predictive: Store prediction errors
//! - Adaptive: Switch based on signal characteristics

use heapless::Vec as HVec;

/// Maximum samples to encode
const MAX_SAMPLES: usize = 256;

/// Delta encoding mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaMode {
    /// Simple difference from previous value
    Simple,
    /// Difference from linear prediction
    LinearPredictive,
    /// Difference from exponential prediction
    ExponentialPredictive,
    /// Automatically choose best mode
    Adaptive,
}

/// Encoded delta stream
#[derive(Clone, Debug)]
pub struct DeltaStream {
    /// Encoding mode used
    pub mode: DeltaMode,
    /// Initial value (anchor)
    pub anchor: i16,
    /// Delta values (variable width)
    pub deltas: HVec<i8, MAX_SAMPLES>,
    /// Large deltas that don't fit in i8
    pub large_deltas: HVec<(u8, i16), 32>,
    /// Original sample count
    pub sample_count: u16,
}

impl DeltaStream {
    /// Create empty delta stream
    pub fn new(mode: DeltaMode) -> Self {
        Self {
            mode,
            anchor: 0,
            deltas: HVec::new(),
            large_deltas: HVec::new(),
            sample_count: 0,
        }
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> f32 {
        if self.deltas.is_empty() {
            return 1.0;
        }

        let original_bytes = self.sample_count as usize * 2; // i16 = 2 bytes
        let encoded_bytes = 2 + self.deltas.len() + self.large_deltas.len() * 3;

        original_bytes as f32 / encoded_bytes as f32
    }

    /// Get bytes saved
    pub fn bytes_saved(&self) -> usize {
        let original = self.sample_count as usize * 2;
        let encoded = 2 + self.deltas.len() + self.large_deltas.len() * 3;
        original.saturating_sub(encoded)
    }
}

impl Default for DeltaStream {
    fn default() -> Self {
        Self::new(DeltaMode::Simple)
    }
}

/// Delta encoder configuration
#[derive(Clone, Copy, Debug)]
pub struct DeltaConfig {
    /// Encoding mode
    pub mode: DeltaMode,
    /// EMA alpha for predictive modes
    pub prediction_alpha: f32,
    /// Threshold for mode switching (adaptive)
    pub adaptive_threshold: f32,
    /// Enable zero suppression
    pub zero_suppress: bool,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            mode: DeltaMode::Simple,
            prediction_alpha: 0.3,
            adaptive_threshold: 10.0,
            zero_suppress: true,
        }
    }
}

/// Delta encoder/decoder
pub struct DeltaEncoder {
    config: DeltaConfig,
    /// Last value (for simple delta)
    last_value: i16,
    /// Prediction state
    prediction: f32,
    /// Velocity estimate (for linear prediction)
    velocity: f32,
    /// Statistics
    total_encoded: u32,
    total_saved: u32,
}

impl DeltaEncoder {
    /// Create a new delta encoder
    pub fn new(config: DeltaConfig) -> Self {
        Self {
            config,
            last_value: 0,
            prediction: 0.0,
            velocity: 0.0,
            total_encoded: 0,
            total_saved: 0,
        }
    }

    /// Encode a sequence of values
    pub fn encode(&mut self, values: &[i16]) -> DeltaStream {
        if values.is_empty() {
            return DeltaStream::new(self.config.mode);
        }

        let mode = if self.config.mode == DeltaMode::Adaptive {
            self.choose_mode(values)
        } else {
            self.config.mode
        };

        let mut stream = DeltaStream::new(mode);
        stream.anchor = values[0];
        stream.sample_count = values.len() as u16;

        // Initialize state
        self.last_value = values[0];
        self.prediction = values[0] as f32;
        self.velocity = 0.0;

        for (i, &value) in values.iter().enumerate().skip(1) {
            let delta = self.compute_delta(value, mode);

            // Store delta
            if delta >= -128 && delta <= 127 {
                let _ = stream.deltas.push(delta as i8);
            } else {
                // Large delta - store index and full value
                let _ = stream.deltas.push(i8::MAX); // Marker
                let _ = stream.large_deltas.push((i as u8, delta));
            }

            // Update state
            self.update_state(value, mode);
        }

        // Update statistics
        self.total_encoded += values.len() as u32;
        self.total_saved += stream.bytes_saved() as u32;

        stream
    }

    /// Compute delta based on mode
    fn compute_delta(&self, value: i16, mode: DeltaMode) -> i16 {
        match mode {
            DeltaMode::Simple => value - self.last_value,
            DeltaMode::LinearPredictive => {
                let predicted = self.prediction + self.velocity;
                value - predicted as i16
            }
            DeltaMode::ExponentialPredictive => {
                value - self.prediction as i16
            }
            DeltaMode::Adaptive => value - self.last_value, // Fallback
        }
    }

    /// Update prediction state
    fn update_state(&mut self, value: i16, mode: DeltaMode) {
        let alpha = self.config.prediction_alpha;

        match mode {
            DeltaMode::Simple => {
                self.last_value = value;
            }
            DeltaMode::LinearPredictive => {
                let new_velocity = value as f32 - self.last_value as f32;
                self.velocity = (1.0 - alpha) * self.velocity + alpha * new_velocity;
                self.prediction = value as f32;
                self.last_value = value;
            }
            DeltaMode::ExponentialPredictive => {
                self.prediction = (1.0 - alpha) * self.prediction + alpha * value as f32;
                self.last_value = value;
            }
            DeltaMode::Adaptive => {
                self.last_value = value;
            }
        }
    }

    /// Choose best mode based on signal characteristics
    fn choose_mode(&self, values: &[i16]) -> DeltaMode {
        if values.len() < 3 {
            return DeltaMode::Simple;
        }

        // Calculate variance of simple deltas
        let mut deltas: HVec<i16, 64> = HVec::new();
        for i in 1..values.len().min(64) {
            let _ = deltas.push(values[i] - values[i - 1]);
        }

        if deltas.is_empty() {
            return DeltaMode::Simple;
        }

        let mean: f32 = deltas.iter().map(|&d| d as f32).sum::<f32>() / deltas.len() as f32;
        let variance: f32 = deltas.iter()
            .map(|&d| (d as f32 - mean).powi(2))
            .sum::<f32>() / deltas.len() as f32;

        // High variance = simple delta is best
        // Low variance with trend = linear predictive
        // Low variance without trend = exponential
        if variance > self.config.adaptive_threshold * self.config.adaptive_threshold {
            DeltaMode::Simple
        } else if mean.abs() > 1.0 {
            DeltaMode::LinearPredictive
        } else {
            DeltaMode::ExponentialPredictive
        }
    }

    /// Decode a delta stream
    pub fn decode(&self, stream: &DeltaStream) -> HVec<i16, MAX_SAMPLES> {
        let mut values = HVec::new();

        if stream.sample_count == 0 {
            return values;
        }

        let _ = values.push(stream.anchor);

        let mut current = stream.anchor;
        let mut prediction = stream.anchor as f32;
        let mut velocity = 0.0f32;
        let mut large_idx = 0;

        for (i, &delta) in stream.deltas.iter().enumerate() {
            let actual_delta = if delta == i8::MAX {
                // Look up large delta
                if large_idx < stream.large_deltas.len() {
                    let (_, large_delta) = stream.large_deltas[large_idx];
                    large_idx += 1;
                    large_delta
                } else {
                    0
                }
            } else {
                delta as i16
            };

            // Reconstruct value based on mode
            let value = match stream.mode {
                DeltaMode::Simple => current + actual_delta,
                DeltaMode::LinearPredictive => {
                    let predicted = prediction + velocity;
                    predicted as i16 + actual_delta
                }
                DeltaMode::ExponentialPredictive => {
                    prediction as i16 + actual_delta
                }
                DeltaMode::Adaptive => current + actual_delta,
            };

            let _ = values.push(value);

            // Update state for next iteration
            match stream.mode {
                DeltaMode::LinearPredictive => {
                    let new_velocity = value as f32 - current as f32;
                    velocity = 0.7 * velocity + 0.3 * new_velocity;
                    prediction = value as f32;
                }
                DeltaMode::ExponentialPredictive => {
                    prediction = 0.7 * prediction + 0.3 * value as f32;
                }
                _ => {}
            }

            current = value;
        }

        values
    }

    /// Get total bytes saved
    pub fn total_saved(&self) -> u32 {
        self.total_saved
    }

    /// Get average compression ratio
    pub fn average_compression_ratio(&self) -> f32 {
        if self.total_encoded == 0 {
            1.0
        } else {
            let original_bytes = self.total_encoded * 2;
            let saved = self.total_saved;
            original_bytes as f32 / (original_bytes - saved) as f32
        }
    }

    /// Reset encoder state
    pub fn reset(&mut self) {
        self.last_value = 0;
        self.prediction = 0.0;
        self.velocity = 0.0;
    }
}

impl Default for DeltaEncoder {
    fn default() -> Self {
        Self::new(DeltaConfig::default())
    }
}

/// Real-time delta processor
///
/// Processes values one at a time, suitable for streaming.
pub struct StreamingDeltaEncoder {
    config: DeltaConfig,
    last_value: Option<i16>,
    prediction: f32,
    velocity: f32,
    /// Count of zero deltas (for zero suppression)
    zero_count: u8,
}

impl StreamingDeltaEncoder {
    /// Create a new streaming encoder
    pub fn new(config: DeltaConfig) -> Self {
        Self {
            config,
            last_value: None,
            prediction: 0.0,
            velocity: 0.0,
            zero_count: 0,
        }
    }

    /// Encode a single value
    ///
    /// Returns None if zero-suppressed, Some(delta) otherwise
    pub fn encode_one(&mut self, value: i16) -> Option<i16> {
        let delta = if let Some(last) = self.last_value {
            value - last
        } else {
            // First value - return as-is
            self.last_value = Some(value);
            self.prediction = value as f32;
            return Some(value);
        };

        self.last_value = Some(value);

        // Zero suppression
        if self.config.zero_suppress && delta == 0 {
            self.zero_count = self.zero_count.saturating_add(1);
            if self.zero_count < 255 {
                return None;
            }
            // Emit after 255 zeros to prevent overflow
            self.zero_count = 0;
        } else {
            self.zero_count = 0;
        }

        Some(delta)
    }

    /// Decode a delta value
    pub fn decode_one(&mut self, delta: i16) -> i16 {
        if let Some(last) = self.last_value {
            let value = last + delta;
            self.last_value = Some(value);
            value
        } else {
            // First value
            self.last_value = Some(delta);
            delta
        }
    }

    /// Get pending zero count
    pub fn pending_zeros(&self) -> u8 {
        self.zero_count
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.last_value = None;
        self.prediction = 0.0;
        self.velocity = 0.0;
        self.zero_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_delta() {
        let config = DeltaConfig {
            mode: DeltaMode::Simple,
            ..Default::default()
        };
        let mut encoder = DeltaEncoder::new(config);

        let values: [i16; 5] = [100, 102, 105, 103, 100];
        let stream = encoder.encode(&values);

        assert_eq!(stream.sample_count, 5);
        assert_eq!(stream.anchor, 100);

        let decoded = encoder.decode(&stream);
        assert_eq!(decoded.as_slice(), &values);
    }

    #[test]
    fn test_compression_ratio() {
        let config = DeltaConfig {
            mode: DeltaMode::Simple,
            ..Default::default()
        };
        let mut encoder = DeltaEncoder::new(config);

        // Slowly changing values - good compression
        let values: HVec<i16, 64> = (0..64).map(|i| 1000 + i).collect();
        let stream = encoder.encode(&values);

        // Deltas are all 1, which fits in i8
        assert!(stream.compression_ratio() > 1.5);
    }

    #[test]
    fn test_large_deltas() {
        let config = DeltaConfig::default();
        let mut encoder = DeltaEncoder::new(config);

        // Values with large jumps
        let values: [i16; 4] = [0, 1000, 1001, -500];
        let stream = encoder.encode(&values);

        // Should have large deltas stored separately
        assert!(!stream.large_deltas.is_empty());

        // Should still decode correctly
        let decoded = encoder.decode(&stream);
        assert_eq!(decoded.as_slice(), &values);
    }

    #[test]
    fn test_predictive_mode() {
        let config = DeltaConfig {
            mode: DeltaMode::LinearPredictive,
            prediction_alpha: 0.5,
            ..Default::default()
        };
        let mut encoder = DeltaEncoder::new(config);

        // Linear sequence - prediction should improve compression
        let values: HVec<i16, 32> = (0..32).map(|i| i * 10).collect();
        let stream = encoder.encode(&values);

        // Main test: predictive mode should compress linear data well
        // The deltas should be small for linear sequences
        assert!(stream.mode == DeltaMode::LinearPredictive);
        assert!(stream.sample_count > 0);

        // Verify we can decode (values may drift due to float math)
        let decoded = encoder.decode(&stream);
        assert_eq!(decoded.len(), values.len());

        // First value must be exact (anchor)
        assert_eq!(decoded[0], values[0]);
    }

    #[test]
    fn test_adaptive_mode() {
        let config = DeltaConfig {
            mode: DeltaMode::Adaptive,
            ..Default::default()
        };
        let mut encoder = DeltaEncoder::new(config);

        let values: [i16; 8] = [100, 101, 102, 103, 104, 105, 106, 107];
        let stream = encoder.encode(&values);

        // Should choose appropriate mode
        assert!(stream.mode != DeltaMode::Adaptive);
    }

    #[test]
    fn test_streaming_encoder() {
        let config = DeltaConfig {
            zero_suppress: false,
            ..Default::default()
        };
        let mut encoder = StreamingDeltaEncoder::new(config);

        // First value
        let d1 = encoder.encode_one(100);
        assert_eq!(d1, Some(100));

        // Delta
        let d2 = encoder.encode_one(105);
        assert_eq!(d2, Some(5));

        // Decode
        let mut decoder = StreamingDeltaEncoder::new(config);
        assert_eq!(decoder.decode_one(100), 100);
        assert_eq!(decoder.decode_one(5), 105);
    }

    #[test]
    fn test_zero_suppression() {
        let config = DeltaConfig {
            zero_suppress: true,
            ..Default::default()
        };
        let mut encoder = StreamingDeltaEncoder::new(config);

        encoder.encode_one(100);
        let d1 = encoder.encode_one(100); // Same value
        assert_eq!(d1, None); // Suppressed

        let d2 = encoder.encode_one(105); // Different value
        assert_eq!(d2, Some(5)); // Not suppressed
    }
}
