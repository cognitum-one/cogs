//! Adaptive Precision
//!
//! Dynamic bit-width selection based on layer importance and runtime conditions.
//! Achieves optimal accuracy-efficiency tradeoff by using higher precision
//! where it matters most.
//!
//! Precision levels:
//! - INT4: 4-bit weights, highest efficiency
//! - INT8: 8-bit weights, balanced
//! - INT16: 16-bit weights, high accuracy
//! - FP32: Full precision, maximum accuracy

use heapless::Vec as HVec;

/// Maximum layers to manage
const MAX_LAYERS: usize = 16;

/// Precision level for computation
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precision {
    /// 4-bit integer (packed, 2 values per byte)
    Int4 = 4,
    /// 8-bit integer
    Int8 = 8,
    /// 16-bit integer
    Int16 = 16,
    /// 32-bit floating point
    Fp32 = 32,
}

impl Precision {
    /// Get bits per value
    pub fn bits(&self) -> u8 {
        *self as u8
    }

    /// Get memory factor relative to FP32
    pub fn memory_factor(&self) -> f32 {
        match self {
            Precision::Int4 => 0.125,  // 4/32
            Precision::Int8 => 0.25,   // 8/32
            Precision::Int16 => 0.5,   // 16/32
            Precision::Fp32 => 1.0,    // 32/32
        }
    }

    /// Get compute factor (relative speed, higher = faster)
    pub fn compute_factor(&self) -> f32 {
        match self {
            Precision::Int4 => 4.0,   // SIMD packs more
            Precision::Int8 => 2.0,
            Precision::Int16 => 1.5,
            Precision::Fp32 => 1.0,
        }
    }

    /// Get typical accuracy factor (0.0 to 1.0)
    pub fn accuracy_factor(&self) -> f32 {
        match self {
            Precision::Int4 => 0.92,
            Precision::Int8 => 0.98,
            Precision::Int16 => 0.995,
            Precision::Fp32 => 1.0,
        }
    }

    /// Promote to higher precision
    pub fn promote(&self) -> Precision {
        match self {
            Precision::Int4 => Precision::Int8,
            Precision::Int8 => Precision::Int16,
            Precision::Int16 => Precision::Fp32,
            Precision::Fp32 => Precision::Fp32,
        }
    }

    /// Demote to lower precision
    pub fn demote(&self) -> Precision {
        match self {
            Precision::Int4 => Precision::Int4,
            Precision::Int8 => Precision::Int4,
            Precision::Int16 => Precision::Int8,
            Precision::Fp32 => Precision::Int16,
        }
    }
}

/// Layer sensitivity metrics
#[derive(Clone, Copy, Debug)]
pub struct LayerSensitivity {
    /// Layer ID
    pub layer_id: u8,
    /// Gradient magnitude (indicates importance)
    pub gradient_magnitude: f32,
    /// Output variance (indicates dynamic range needs)
    pub output_variance: f32,
    /// Error sensitivity (accuracy loss per precision drop)
    pub error_sensitivity: f32,
    /// Current precision
    pub precision: Precision,
    /// Recommended precision
    pub recommended: Precision,
}

impl Default for LayerSensitivity {
    fn default() -> Self {
        Self {
            layer_id: 0,
            gradient_magnitude: 0.5,
            output_variance: 1.0,
            error_sensitivity: 0.1,
            precision: Precision::Int8,
            recommended: Precision::Int8,
        }
    }
}

/// Adaptive precision configuration
#[derive(Clone, Copy, Debug)]
pub struct AdaptivePrecisionConfig {
    /// Minimum allowed precision
    pub min_precision: Precision,
    /// Maximum allowed precision
    pub max_precision: Precision,
    /// Error threshold for promotion
    pub promote_threshold: f32,
    /// Error threshold for demotion
    pub demote_threshold: f32,
    /// Gradient threshold for high precision
    pub gradient_threshold: f32,
    /// Enable automatic adaptation
    pub auto_adapt: bool,
    /// Adaptation interval (updates between adaptations)
    pub adapt_interval: u32,
}

impl Default for AdaptivePrecisionConfig {
    fn default() -> Self {
        Self {
            min_precision: Precision::Int4,
            max_precision: Precision::Int16,
            promote_threshold: 0.05,   // 5% accuracy loss triggers promotion
            demote_threshold: 0.01,    // <1% accuracy loss allows demotion
            gradient_threshold: 0.5,   // High gradient = high precision
            auto_adapt: true,
            adapt_interval: 100,
        }
    }
}

/// Adaptive precision controller
///
/// Manages per-layer precision to optimize the accuracy-efficiency tradeoff.
pub struct AdaptivePrecisionController {
    config: AdaptivePrecisionConfig,
    /// Layer sensitivities
    layers: HVec<LayerSensitivity, MAX_LAYERS>,
    /// Global error estimate
    global_error: f32,
    /// Update counter
    update_count: u32,
    /// Total memory usage (normalized)
    memory_usage: f32,
    /// Total compute usage (normalized)
    compute_usage: f32,
}

impl AdaptivePrecisionController {
    /// Create a new adaptive precision controller
    pub fn new(config: AdaptivePrecisionConfig, num_layers: usize) -> Self {
        let mut layers = HVec::new();
        for i in 0..num_layers.min(MAX_LAYERS) {
            let mut sensitivity = LayerSensitivity::default();
            sensitivity.layer_id = i as u8;
            let _ = layers.push(sensitivity);
        }

        Self {
            config,
            layers,
            global_error: 0.0,
            update_count: 0,
            memory_usage: 1.0,
            compute_usage: 1.0,
        }
    }

    /// Update layer sensitivity metrics
    pub fn update_sensitivity(
        &mut self,
        layer_id: usize,
        gradient_magnitude: f32,
        output_variance: f32,
        error: f32,
    ) {
        if let Some(layer) = self.layers.get_mut(layer_id) {
            // EMA update of metrics
            let alpha = 0.1;
            layer.gradient_magnitude = (1.0 - alpha) * layer.gradient_magnitude
                + alpha * gradient_magnitude;
            layer.output_variance = (1.0 - alpha) * layer.output_variance
                + alpha * output_variance;
            layer.error_sensitivity = (1.0 - alpha) * layer.error_sensitivity
                + alpha * error;
        }

        // Update recommendation separately to avoid borrow conflict
        self.update_layer_recommendation(layer_id);

        self.update_count += 1;

        // Auto-adapt if enabled
        if self.config.auto_adapt && self.update_count % self.config.adapt_interval == 0 {
            self.adapt_all();
        }
    }

    /// Update recommended precision for a specific layer
    fn update_layer_recommendation(&mut self, layer_id: usize) {
        // Read layer data
        let layer_data = if let Some(layer) = self.layers.get(layer_id) {
            Some((layer.gradient_magnitude, layer.output_variance, layer.error_sensitivity, layer.precision))
        } else {
            None
        };

        // Compute and update if found
        if let Some((grad_mag, out_var, err_sens, precision)) = layer_data {
            let recommended = self.compute_recommended_precision_values(grad_mag, out_var, err_sens, precision);
            if let Some(layer) = self.layers.get_mut(layer_id) {
                layer.recommended = recommended;
            }
        }
    }

    /// Compute recommended precision for a layer
    fn compute_recommended_precision(&self, layer: &LayerSensitivity) -> Precision {
        self.compute_recommended_precision_values(
            layer.gradient_magnitude,
            layer.output_variance,
            layer.error_sensitivity,
            layer.precision,
        )
    }

    /// Compute recommended precision from individual values
    fn compute_recommended_precision_values(
        &self,
        gradient_magnitude: f32,
        output_variance: f32,
        error_sensitivity: f32,
        current_precision: Precision,
    ) -> Precision {
        // High gradient magnitude -> high precision
        if gradient_magnitude > self.config.gradient_threshold {
            return self.config.max_precision;
        }

        // High error sensitivity -> higher precision
        if error_sensitivity > self.config.promote_threshold {
            return current_precision.promote();
        }

        // Low error sensitivity -> can use lower precision
        if error_sensitivity < self.config.demote_threshold {
            let demoted = current_precision.demote();
            if demoted >= self.config.min_precision {
                return demoted;
            }
        }

        // High output variance -> higher precision for dynamic range
        if output_variance > 2.0 {
            return current_precision.promote().min(self.config.max_precision);
        }

        current_precision
    }

    /// Adapt all layers to recommended precision
    pub fn adapt_all(&mut self) {
        for layer in self.layers.iter_mut() {
            // Clamp to config bounds
            let new_precision = layer.recommended
                .max(self.config.min_precision)
                .min(self.config.max_precision);
            layer.precision = new_precision;
        }

        self.update_usage_stats();
    }

    /// Update memory and compute usage statistics
    fn update_usage_stats(&mut self) {
        if self.layers.is_empty() {
            return;
        }

        let total_memory: f32 = self.layers.iter()
            .map(|l| l.precision.memory_factor())
            .sum();
        let total_compute: f32 = self.layers.iter()
            .map(|l| 1.0 / l.precision.compute_factor())
            .sum();

        self.memory_usage = total_memory / self.layers.len() as f32;
        self.compute_usage = total_compute / self.layers.len() as f32;
    }

    /// Get precision for a layer
    pub fn get_precision(&self, layer_id: usize) -> Precision {
        self.layers.get(layer_id)
            .map(|l| l.precision)
            .unwrap_or(Precision::Int8)
    }

    /// Set precision for a layer (manual override)
    pub fn set_precision(&mut self, layer_id: usize, precision: Precision) {
        if let Some(layer) = self.layers.get_mut(layer_id) {
            layer.precision = precision
                .max(self.config.min_precision)
                .min(self.config.max_precision);
        }
        self.update_usage_stats();
    }

    /// Force all layers to specific precision
    pub fn force_all_precision(&mut self, precision: Precision) {
        let clamped = precision
            .max(self.config.min_precision)
            .min(self.config.max_precision);

        for layer in self.layers.iter_mut() {
            layer.precision = clamped;
        }
        self.update_usage_stats();
    }

    /// Get memory usage factor (0.0 to 1.0)
    pub fn memory_usage(&self) -> f32 {
        self.memory_usage
    }

    /// Get compute usage factor (0.0 to 1.0)
    pub fn compute_usage(&self) -> f32 {
        self.compute_usage
    }

    /// Get estimated memory savings vs FP32
    pub fn memory_savings(&self) -> f32 {
        1.0 - self.memory_usage
    }

    /// Get estimated speedup vs FP32
    pub fn speedup(&self) -> f32 {
        if self.compute_usage > 0.0 {
            1.0 / self.compute_usage
        } else {
            1.0
        }
    }

    /// Get layer sensitivity info
    pub fn get_layer_info(&self, layer_id: usize) -> Option<&LayerSensitivity> {
        self.layers.get(layer_id)
    }

    /// Get number of layers
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Get distribution of precisions
    pub fn precision_distribution(&self) -> (usize, usize, usize, usize) {
        let mut int4 = 0;
        let mut int8 = 0;
        let mut int16 = 0;
        let mut fp32 = 0;

        for layer in self.layers.iter() {
            match layer.precision {
                Precision::Int4 => int4 += 1,
                Precision::Int8 => int8 += 1,
                Precision::Int16 => int16 += 1,
                Precision::Fp32 => fp32 += 1,
            }
        }

        (int4, int8, int16, fp32)
    }

    /// Set global error estimate (from validation)
    pub fn set_global_error(&mut self, error: f32) {
        self.global_error = error;

        // If global error is high, promote precision on sensitive layers
        if error > self.config.promote_threshold {
            for layer in self.layers.iter_mut() {
                if layer.error_sensitivity > self.config.demote_threshold {
                    layer.precision = layer.precision.promote()
                        .min(self.config.max_precision);
                }
            }
            self.update_usage_stats();
        }
    }

    /// Get global error estimate
    pub fn global_error(&self) -> f32 {
        self.global_error
    }

    /// Reset all layers to default precision
    pub fn reset(&mut self) {
        for layer in self.layers.iter_mut() {
            layer.precision = Precision::Int8;
            layer.gradient_magnitude = 0.5;
            layer.output_variance = 1.0;
            layer.error_sensitivity = 0.1;
        }
        self.global_error = 0.0;
        self.update_count = 0;
        self.update_usage_stats();
    }
}

/// Mixed precision matrix multiply
///
/// Performs matrix multiplication with adaptive precision per row/column.
pub fn mixed_precision_matmul(
    a: &[i8],
    b: &[i8],
    a_rows: usize,
    a_cols: usize,
    b_cols: usize,
    row_precision: &[Precision],
) -> HVec<i32, 256> {
    let mut result = HVec::new();

    for i in 0..a_rows {
        let precision = row_precision.get(i).copied().unwrap_or(Precision::Int8);

        for j in 0..b_cols {
            let mut sum: i32 = 0;

            for k in 0..a_cols {
                let a_val = a[i * a_cols + k] as i32;
                let b_val = b[k * b_cols + j] as i32;

                // Apply precision-based scaling
                let product = match precision {
                    Precision::Int4 => (a_val >> 4) * (b_val >> 4),
                    Precision::Int8 => a_val * b_val,
                    Precision::Int16 => a_val * b_val,
                    Precision::Fp32 => a_val * b_val,
                };

                sum += product;
            }

            let _ = result.push(sum);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precision_properties() {
        assert_eq!(Precision::Int4.bits(), 4);
        assert_eq!(Precision::Int8.bits(), 8);
        assert!(Precision::Int4.memory_factor() < Precision::Int8.memory_factor());
        assert!(Precision::Int4.compute_factor() > Precision::Int8.compute_factor());
    }

    #[test]
    fn test_precision_promotion() {
        assert_eq!(Precision::Int4.promote(), Precision::Int8);
        assert_eq!(Precision::Int8.promote(), Precision::Int16);
        assert_eq!(Precision::Fp32.promote(), Precision::Fp32);
    }

    #[test]
    fn test_precision_demotion() {
        assert_eq!(Precision::Fp32.demote(), Precision::Int16);
        assert_eq!(Precision::Int8.demote(), Precision::Int4);
        assert_eq!(Precision::Int4.demote(), Precision::Int4);
    }

    #[test]
    fn test_adaptive_controller() {
        let config = AdaptivePrecisionConfig::default();
        let controller = AdaptivePrecisionController::new(config, 4);

        assert_eq!(controller.num_layers(), 4);
        assert_eq!(controller.get_precision(0), Precision::Int8);
    }

    #[test]
    fn test_sensitivity_update() {
        let config = AdaptivePrecisionConfig {
            auto_adapt: false, // Manual control
            ..Default::default()
        };
        let mut controller = AdaptivePrecisionController::new(config, 4);

        // High gradient should recommend high precision
        controller.update_sensitivity(0, 0.8, 1.0, 0.01);

        let info = controller.get_layer_info(0).unwrap();
        assert!(info.gradient_magnitude > 0.5);
    }

    #[test]
    fn test_memory_savings() {
        let config = AdaptivePrecisionConfig::default();
        let mut controller = AdaptivePrecisionController::new(config, 4);

        // Force all to INT4
        controller.force_all_precision(Precision::Int4);

        // Should have significant memory savings
        assert!(controller.memory_savings() > 0.8);
    }

    #[test]
    fn test_precision_distribution() {
        let config = AdaptivePrecisionConfig::default();
        let mut controller = AdaptivePrecisionController::new(config, 4);

        controller.set_precision(0, Precision::Int4);
        controller.set_precision(1, Precision::Int8);
        controller.set_precision(2, Precision::Int8);
        controller.set_precision(3, Precision::Int16);

        let (int4, int8, int16, _fp32) = controller.precision_distribution();
        assert_eq!(int4, 1);
        assert_eq!(int8, 2);
        assert_eq!(int16, 1);
    }
}
