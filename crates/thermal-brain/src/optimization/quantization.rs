//! INT4/INT8 Mixed-Precision Quantization
//!
//! Based on:
//! - NeurIPS 2025: INT4/INT8 edge deployment
//! - SIMD-based MKMP multiplier (50% hardware savings)
//! - 94% PDP reduction, 3100% LUT efficiency improvement
//!
//! Reference: Post-training quantization for FPGA (Aug 2025)

use crate::types::{PatternVector, FeatureVector, FEATURE_DIMS};

/// Quantization precision levels
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantPrecision {
    /// 4-bit integer (-8 to +7)
    Int4,
    /// 8-bit integer (-128 to +127)
    Int8,
    /// 16-bit integer
    Int16,
    /// 32-bit floating point
    Float32,
    /// Mixed precision (INT4 weights, INT8 activations)
    MixedInt4Int8,
}

/// INT4 packed representation (2 values per byte)
#[derive(Clone, Copy, Debug)]
pub struct Int4Packed {
    /// Packed data (high nibble = first value, low nibble = second)
    pub data: [u8; 8], // 16 INT4 values
}

impl Int4Packed {
    pub fn new() -> Self {
        Self { data: [0; 8] }
    }

    /// Pack a value at index
    #[inline(always)]
    pub fn set(&mut self, idx: usize, value: i8) {
        if idx >= 16 {
            return;
        }
        let byte_idx = idx / 2;
        let clamped = value.clamp(-8, 7) as u8 & 0x0F;

        if idx % 2 == 0 {
            // High nibble
            self.data[byte_idx] = (self.data[byte_idx] & 0x0F) | (clamped << 4);
        } else {
            // Low nibble
            self.data[byte_idx] = (self.data[byte_idx] & 0xF0) | clamped;
        }
    }

    /// Get value at index
    #[inline(always)]
    pub fn get(&self, idx: usize) -> i8 {
        if idx >= 16 {
            return 0;
        }
        let byte_idx = idx / 2;
        let nibble = if idx % 2 == 0 {
            (self.data[byte_idx] >> 4) & 0x0F
        } else {
            self.data[byte_idx] & 0x0F
        };

        // Sign extend from 4-bit
        if nibble & 0x08 != 0 {
            (nibble | 0xF0) as i8
        } else {
            nibble as i8
        }
    }

    /// Dot product with INT8 vector (mixed precision)
    #[inline(always)]
    pub fn dot_int8(&self, other: &PatternVector) -> i32 {
        let mut sum = 0i32;
        for i in 0..16 {
            sum += (self.get(i) as i32) * (other[i] as i32);
        }
        sum
    }

    /// Memory size in bytes
    pub const fn size_bytes() -> usize {
        8
    }
}

impl Default for Int4Packed {
    fn default() -> Self {
        Self::new()
    }
}

/// Quantization configuration
#[derive(Clone, Copy, Debug)]
pub struct QuantConfig {
    /// Weight precision
    pub weight_precision: QuantPrecision,
    /// Activation precision
    pub activation_precision: QuantPrecision,
    /// Scale factor for dequantization
    pub scale: f32,
    /// Zero point offset
    pub zero_point: i32,
    /// Enable dynamic quantization
    pub dynamic: bool,
}

impl Default for QuantConfig {
    fn default() -> Self {
        Self {
            weight_precision: QuantPrecision::Int8,
            activation_precision: QuantPrecision::Int8,
            scale: 1.0 / 127.0,
            zero_point: 0,
            dynamic: false,
        }
    }
}

/// Quantizer for converting between precisions
pub struct Quantizer {
    config: QuantConfig,
    /// Running min for dynamic quantization
    running_min: f32,
    /// Running max for dynamic quantization
    running_max: f32,
    /// EMA alpha for dynamic tracking
    ema_alpha: f32,
}

impl Quantizer {
    pub fn new(config: QuantConfig) -> Self {
        Self {
            config,
            running_min: -1.0,
            running_max: 1.0,
            ema_alpha: 0.1,
        }
    }

    /// Quantize f32 to INT8
    #[inline(always)]
    pub fn quantize_int8(&self, value: f32) -> i8 {
        let scaled = value / self.config.scale;
        let shifted = scaled + self.config.zero_point as f32;
        shifted.round().clamp(-128.0, 127.0) as i8
    }

    /// Dequantize INT8 to f32
    #[inline(always)]
    pub fn dequantize_int8(&self, value: i8) -> f32 {
        ((value as i32 - self.config.zero_point) as f32) * self.config.scale
    }

    /// Quantize f32 to INT4
    #[inline(always)]
    pub fn quantize_int4(&self, value: f32) -> i8 {
        let scaled = value / (self.config.scale * 16.0); // Scale for 4-bit
        scaled.round().clamp(-8.0, 7.0) as i8
    }

    /// Dequantize INT4 to f32
    #[inline(always)]
    pub fn dequantize_int4(&self, value: i8) -> f32 {
        (value as f32) * self.config.scale * 16.0
    }

    /// Quantize feature vector to pattern vector (INT8)
    pub fn quantize_features(&self, features: &FeatureVector) -> PatternVector {
        let mut pattern = [0i8; FEATURE_DIMS];
        for (i, &f) in features.iter().enumerate() {
            pattern[i] = self.quantize_int8(f);
        }
        pattern
    }

    /// Quantize feature vector to INT4 packed
    pub fn quantize_features_int4(&self, features: &FeatureVector) -> Int4Packed {
        let mut packed = Int4Packed::new();
        for (i, &f) in features.iter().enumerate() {
            packed.set(i, self.quantize_int4(f));
        }
        packed
    }

    /// Dequantize pattern vector to features
    pub fn dequantize_pattern(&self, pattern: &PatternVector) -> FeatureVector {
        let mut features = [0.0f32; FEATURE_DIMS];
        for (i, &p) in pattern.iter().enumerate() {
            features[i] = self.dequantize_int8(p);
        }
        features
    }

    /// Update dynamic range (for dynamic quantization)
    pub fn update_range(&mut self, features: &FeatureVector) {
        if !self.config.dynamic {
            return;
        }

        let min = features.iter().fold(f32::MAX, |a, &b| a.min(b));
        let max = features.iter().fold(f32::MIN, |a, &b| a.max(b));

        self.running_min = self.running_min * (1.0 - self.ema_alpha) + min * self.ema_alpha;
        self.running_max = self.running_max * (1.0 - self.ema_alpha) + max * self.ema_alpha;

        // Update scale
        let range = self.running_max - self.running_min;
        if range > 1e-10 {
            self.config.scale = range / 254.0; // Full INT8 range
            self.config.zero_point = (-self.running_min / self.config.scale).round() as i32;
        }
    }

    /// Get current quantization error estimate
    pub fn estimate_error(&self, features: &FeatureVector) -> f32 {
        let quantized = self.quantize_features(features);
        let reconstructed = self.dequantize_pattern(&quantized);

        let mut mse = 0.0f32;
        for i in 0..FEATURE_DIMS {
            let diff = features[i] - reconstructed[i];
            mse += diff * diff;
        }
        libm::sqrtf(mse / FEATURE_DIMS as f32)
    }

    /// Memory savings ratio
    pub fn memory_ratio(&self) -> f32 {
        match self.config.weight_precision {
            QuantPrecision::Int4 => 8.0,      // 32-bit -> 4-bit = 8x
            QuantPrecision::Int8 => 4.0,      // 32-bit -> 8-bit = 4x
            QuantPrecision::Int16 => 2.0,     // 32-bit -> 16-bit = 2x
            QuantPrecision::Float32 => 1.0,   // No savings
            QuantPrecision::MixedInt4Int8 => 6.0, // Average
        }
    }
}

/// Mixed precision compute kernel
pub struct MixedPrecisionKernel {
    /// INT4 weights (packed)
    weights_int4: heapless::Vec<Int4Packed, 64>,
    /// INT8 biases
    biases_int8: heapless::Vec<i8, 64>,
    /// Scale factors per weight
    scales: heapless::Vec<f32, 64>,
}

impl MixedPrecisionKernel {
    pub fn new() -> Self {
        Self {
            weights_int4: heapless::Vec::new(),
            biases_int8: heapless::Vec::new(),
            scales: heapless::Vec::new(),
        }
    }

    /// Add a weight pattern (quantizes to INT4)
    pub fn add_weight(&mut self, pattern: &PatternVector, scale: f32) -> bool {
        if self.weights_int4.is_full() {
            return false;
        }

        let mut packed = Int4Packed::new();
        for (i, &v) in pattern.iter().enumerate() {
            // Scale and quantize to 4-bit
            let scaled = (v as f32 / 16.0).round().clamp(-8.0, 7.0) as i8;
            packed.set(i, scaled);
        }

        let _ = self.weights_int4.push(packed);
        let _ = self.biases_int8.push(0);
        let _ = self.scales.push(scale);
        true
    }

    /// Forward pass with mixed precision
    pub fn forward(&self, input: &PatternVector) -> heapless::Vec<i32, 64> {
        let mut output = heapless::Vec::new();

        for (i, weights) in self.weights_int4.iter().enumerate() {
            let dot = weights.dot_int8(input);
            let bias = self.biases_int8.get(i).copied().unwrap_or(0) as i32;
            let _ = output.push(dot + bias);
        }

        output
    }

    /// Get memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.weights_int4.len() * Int4Packed::size_bytes()
            + self.biases_int8.len()
            + self.scales.len() * 4
    }

    /// Pattern count
    pub fn len(&self) -> usize {
        self.weights_int4.len()
    }

    pub fn is_empty(&self) -> bool {
        self.weights_int4.is_empty()
    }
}

impl Default for MixedPrecisionKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int4_packed() {
        let mut packed = Int4Packed::new();

        packed.set(0, 7);
        packed.set(1, -8);
        packed.set(2, 3);
        packed.set(15, -1);

        assert_eq!(packed.get(0), 7);
        assert_eq!(packed.get(1), -8);
        assert_eq!(packed.get(2), 3);
        assert_eq!(packed.get(15), -1);
    }

    #[test]
    fn test_quantize_dequantize() {
        let config = QuantConfig::default();
        let quantizer = Quantizer::new(config);

        let value = 0.5f32;
        let quantized = quantizer.quantize_int8(value);
        let recovered = quantizer.dequantize_int8(quantized);

        assert!((value - recovered).abs() < 0.02);
    }

    #[test]
    fn test_mixed_precision_kernel() {
        let mut kernel = MixedPrecisionKernel::new();

        let pattern = [64, 32, 16, 8, 4, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        kernel.add_weight(&pattern, 1.0);

        let input = [127, 64, 32, 16, 8, 4, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        let output = kernel.forward(&input);

        assert_eq!(output.len(), 1);
    }

    #[test]
    fn test_memory_savings() {
        let config = QuantConfig {
            weight_precision: QuantPrecision::Int4,
            ..Default::default()
        };
        let quantizer = Quantizer::new(config);

        assert_eq!(quantizer.memory_ratio(), 8.0);
    }
}
