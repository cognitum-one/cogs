//! Feature extraction from sensor data

use crate::config::EncodingConfig;
use crate::types::{FeatureVector, FEATURE_DIMS};
use libm::{fabsf, sqrtf};

/// Ring buffer size (compile-time constant)
const BUFFER_SIZE: usize = 500;

/// Ring buffer for sample storage
#[derive(Clone)]
pub struct RingBuffer {
    data: [f32; BUFFER_SIZE],
    write_idx: usize,
    count: usize,
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RingBuffer {
    /// Create a new empty ring buffer
    pub const fn new() -> Self {
        Self {
            data: [0.0; BUFFER_SIZE],
            write_idx: 0,
            count: 0,
        }
    }

    /// Push a value into the buffer
    pub fn push(&mut self, value: f32) {
        self.data[self.write_idx] = value;
        self.write_idx = (self.write_idx + 1) % BUFFER_SIZE;
        if self.count < BUFFER_SIZE {
            self.count += 1;
        }
    }

    /// Get the number of samples in the buffer
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.count = 0;
        self.write_idx = 0;
    }

    /// Get the last N samples (oldest to newest)
    pub fn last_n(&self, n: usize) -> impl Iterator<Item = f32> + '_ {
        let n = n.min(self.count);
        let start = if self.count >= BUFFER_SIZE {
            (self.write_idx + BUFFER_SIZE - n) % BUFFER_SIZE
        } else {
            self.count.saturating_sub(n)
        };
        (0..n).map(move |i| self.data[(start + i) % BUFFER_SIZE])
    }

    /// Get the most recent sample
    pub fn last(&self) -> Option<f32> {
        if self.count == 0 {
            None
        } else {
            let idx = (self.write_idx + BUFFER_SIZE - 1) % BUFFER_SIZE;
            Some(self.data[idx])
        }
    }

    /// Get sample at index (0 = oldest available)
    pub fn get(&self, index: usize) -> Option<f32> {
        if index >= self.count {
            return None;
        }
        let start = if self.count >= BUFFER_SIZE {
            self.write_idx
        } else {
            0
        };
        Some(self.data[(start + index) % BUFFER_SIZE])
    }
}

/// Feature extractor
pub struct FeatureExtractor {
    buffer: RingBuffer,
    config: EncodingConfig,
    last_features: FeatureVector,
}

impl FeatureExtractor {
    /// Create a new feature extractor
    pub fn new(config: EncodingConfig) -> Self {
        Self {
            buffer: RingBuffer::new(),
            config,
            last_features: [0.0; FEATURE_DIMS],
        }
    }

    /// Push a sample into the buffer
    pub fn push(&mut self, value: f32) {
        self.buffer.push(value);
    }

    /// Get buffer length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Compute feature vector from current buffer state
    pub fn compute(&mut self) -> FeatureVector {
        let window_size = self.config.short_window.min(self.buffer.len());
        if window_size < 2 {
            return [0.0; FEATURE_DIMS];
        }

        // Collect window samples
        let samples: heapless::Vec<f32, 500> = self.buffer.last_n(window_size).collect();

        // Compute features
        let mean = self.calc_mean(&samples);
        let std_dev = self.calc_std(&samples, mean);
        let (min, max) = self.calc_minmax(&samples);
        let range = max - min;
        let slope = self.calc_slope(&samples);
        let delta = samples.last().unwrap_or(&0.0) - samples.first().unwrap_or(&0.0);
        let zero_crossings = self.count_zero_crossings(&samples, mean) as f32;
        let peak_count = self.count_peaks(&samples) as f32;
        let energy = self.calc_energy(&samples);
        let rms = self.calc_rms(&samples);
        let skewness = self.calc_skewness(&samples, mean, std_dev);
        let kurtosis = self.calc_kurtosis(&samples, mean, std_dev);

        // Placeholder for FFT features (would need more complex implementation)
        let dominant_freq = 0.0;
        let spectral_centroid = 0.0;
        let spectral_energy = 0.0;

        let mut features = [
            mean,              // [0]
            std_dev,           // [1]
            min,               // [2]
            max,               // [3]
            range,             // [4]
            slope,             // [5]
            delta,             // [6]
            zero_crossings,    // [7]
            peak_count,        // [8]
            energy,            // [9]
            rms,               // [10]
            dominant_freq,     // [11]
            spectral_centroid, // [12]
            spectral_energy,   // [13]
            skewness,          // [14]
            kurtosis,          // [15]
        ];

        // Normalize features to [-1, 1] range
        self.normalize_features(&mut features);
        self.last_features = features;

        self.last_features
    }

    /// Get the last computed features
    pub fn last_features(&self) -> &FeatureVector {
        &self.last_features
    }

    /// Calculate mean of samples
    fn calc_mean(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = samples.iter().sum();
        sum / samples.len() as f32
    }

    /// Calculate standard deviation
    fn calc_std(&self, samples: &[f32], mean: f32) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }
        let variance: f32 = samples.iter().map(|&x| (x - mean) * (x - mean)).sum();
        sqrtf(variance / (samples.len() - 1) as f32)
    }

    /// Calculate min and max
    fn calc_minmax(&self, samples: &[f32]) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &s in samples {
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }
        if min == f32::MAX {
            min = 0.0;
        }
        if max == f32::MIN {
            max = 0.0;
        }
        (min, max)
    }

    /// Calculate linear regression slope
    fn calc_slope(&self, samples: &[f32]) -> f32 {
        let n = samples.len() as f32;
        if n < 2.0 {
            return 0.0;
        }

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;

        for (i, &y) in samples.iter().enumerate() {
            let x = i as f32;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        let denom = n * sum_xx - sum_x * sum_x;
        if fabsf(denom) < 1e-10 {
            return 0.0;
        }

        (n * sum_xy - sum_x * sum_y) / denom
    }

    /// Count zero crossings (mean crossings)
    fn count_zero_crossings(&self, samples: &[f32], mean: f32) -> usize {
        let mut count = 0;
        let centered: heapless::Vec<f32, 500> = samples.iter().map(|&x| x - mean).collect();

        for i in 1..centered.len() {
            if (centered[i - 1] > 0.0 && centered[i] <= 0.0)
                || (centered[i - 1] < 0.0 && centered[i] >= 0.0)
            {
                count += 1;
            }
        }
        count
    }

    /// Count local peaks
    fn count_peaks(&self, samples: &[f32]) -> usize {
        if samples.len() < 3 {
            return 0;
        }
        let mut count = 0;
        for i in 1..samples.len() - 1 {
            if samples[i] > samples[i - 1] && samples[i] > samples[i + 1] {
                count += 1;
            }
        }
        count
    }

    /// Calculate energy (sum of squared values)
    fn calc_energy(&self, samples: &[f32]) -> f32 {
        samples.iter().map(|&x| x * x).sum()
    }

    /// Calculate RMS
    fn calc_rms(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        sqrtf(self.calc_energy(samples) / samples.len() as f32)
    }

    /// Calculate skewness
    fn calc_skewness(&self, samples: &[f32], mean: f32, std: f32) -> f32 {
        if samples.len() < 3 || fabsf(std) < 1e-10 {
            return 0.0;
        }
        let n = samples.len() as f32;
        let m3: f32 = samples.iter().map(|&x| {
            let z = (x - mean) / std;
            z * z * z
        }).sum();
        m3 / n
    }

    /// Calculate kurtosis
    fn calc_kurtosis(&self, samples: &[f32], mean: f32, std: f32) -> f32 {
        if samples.len() < 4 || fabsf(std) < 1e-10 {
            return 0.0;
        }
        let n = samples.len() as f32;
        let m4: f32 = samples.iter().map(|&x| {
            let z = (x - mean) / std;
            z * z * z * z
        }).sum();
        (m4 / n) - 3.0 // Excess kurtosis
    }

    /// Normalize features to [-1, 1] range using tanh-like scaling
    fn normalize_features(&self, features: &mut FeatureVector) {
        // Normalization factors for each feature (empirically determined)
        let scales = [
            100.0,  // mean (temp in celsius)
            20.0,   // std
            100.0,  // min
            100.0,  // max
            50.0,   // range
            1.0,    // slope
            20.0,   // delta
            50.0,   // zero_crossings
            20.0,   // peak_count
            10000.0, // energy
            100.0,  // rms
            1.0,    // dominant_freq
            1.0,    // spectral_centroid
            1.0,    // spectral_energy
            3.0,    // skewness
            10.0,   // kurtosis
        ];

        for (f, s) in features.iter_mut().zip(scales.iter()) {
            // Soft normalization using tanh-like function
            let x = *f / s;
            *f = x / (1.0 + fabsf(x)); // Approximation of tanh
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer() {
        let mut buf = RingBuffer::new();
        assert!(buf.is_empty());

        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.last(), Some(3.0));

        let last_2: Vec<f32> = buf.last_n(2).collect();
        assert_eq!(last_2, vec![2.0, 3.0]);
    }

    #[test]
    fn test_ring_buffer_wrap() {
        let mut buf = RingBuffer::new();

        // Fill buffer to capacity + some
        for i in 0..600 {
            buf.push(i as f32);
        }

        assert_eq!(buf.len(), BUFFER_SIZE);
        assert_eq!(buf.last(), Some(599.0));
    }

    #[test]
    fn test_feature_extractor() {
        let config = EncodingConfig::default();
        let mut extractor = FeatureExtractor::new(config);

        // Push some temperature samples
        for i in 0..100 {
            extractor.push(25.0 + (i as f32 * 0.1));
        }

        let features = extractor.compute();

        // Mean should be around 25 + 4.95 = ~29.95
        // All features should be normalized to roughly [-1, 1]
        for f in features.iter() {
            assert!(*f >= -1.0 && *f <= 1.0, "Feature {} out of range", f);
        }
    }

    #[test]
    fn test_calc_slope() {
        let config = EncodingConfig::default();
        let extractor = FeatureExtractor::new(config);

        // Linear data: 0, 1, 2, 3, 4 -> slope = 1
        let samples = [0.0, 1.0, 2.0, 3.0, 4.0];
        let slope = extractor.calc_slope(&samples);
        assert!((slope - 1.0).abs() < 0.01);

        // Constant data -> slope = 0
        let samples = [5.0, 5.0, 5.0, 5.0];
        let slope = extractor.calc_slope(&samples);
        assert!(slope.abs() < 0.01);
    }
}
