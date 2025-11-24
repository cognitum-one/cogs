//! SIMD/AI Coprocessor - 524 GOPS Aggregate Performance
//!
//! This module implements high-performance SIMD operations for the Cognitum ASIC,
//! targeting 524 GOPS (Giga Operations Per Second) across 256 tiles.
//!
//! ## Features
//! - **Vector Arithmetic**: 256-bit vectors with 8/16/32-bit lanes
//! - **Matrix Operations**: Optimized 4×4 matrix multiply with accumulation
//! - **Neural Network Primitives**: ReLU, Sigmoid, Softmax, Pooling, Conv2D
//! - **Work RAM**: 64KB dedicated SIMD work memory
//! - **Accumulators**: Eight 32-bit accumulators for dot products

use crate::types::{CryptoError, Result};
use std::fmt;

/// 256-bit SIMD vector with 16×16-bit lanes
#[repr(align(32))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimdVector {
    /// Vector data: 16 lanes of 16-bit values
    pub data: [i16; 16],
}

impl SimdVector {
    /// Create a new zero-initialized SIMD vector
    pub fn new() -> Self {
        Self { data: [0; 16] }
    }

    /// Create a SIMD vector from a slice
    pub fn from_slice(slice: &[i16]) -> Self {
        let mut data = [0i16; 16];
        let len = slice.len().min(16);
        data[..len].copy_from_slice(&slice[..len]);
        Self { data }
    }

    /// Create a SIMD vector with all lanes set to the same value
    pub fn splat(value: i16) -> Self {
        Self { data: [value; 16] }
    }

    /// Get a reference to the underlying data
    pub fn as_slice(&self) -> &[i16] {
        &self.data
    }
}

impl Default for SimdVector {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SimdVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, &val) in self.data.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", val)?;
        }
        write!(f, "]")
    }
}

/// 4×4 Matrix for matrix operations
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4x4 {
    /// Matrix data: 4 rows × 4 columns of 16-bit values
    pub data: [[i16; 4]; 4],
}

impl Matrix4x4 {
    /// Create a new zero-initialized matrix
    pub fn new() -> Self {
        Self { data: [[0; 4]; 4] }
    }

    /// Create an identity matrix
    pub fn identity() -> Self {
        Self {
            data: [
                [1, 0, 0, 0],
                [0, 1, 0, 0],
                [0, 0, 1, 0],
                [0, 0, 0, 1],
            ],
        }
    }

    /// Create a matrix from flat array (row-major)
    pub fn from_array(arr: &[[i16; 4]; 4]) -> Self {
        Self { data: *arr }
    }
}

impl Default for Matrix4x4 {
    fn default() -> Self {
        Self::new()
    }
}

/// Pooling operation type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PoolingType {
    /// Maximum pooling
    Max,
    /// Average pooling
    Average,
}

/// Convolution kernel size
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KernelSize {
    /// 3×3 kernel
    K3x3,
    /// 5×5 kernel
    K5x5,
}

/// SIMD Coprocessor with 524 GOPS target performance
pub struct SimdCoprocessor {
    /// 64KB work RAM for SIMD operations (4096 vectors)
    work_ram: Vec<SimdVector>,
    /// Eight 32-bit accumulators for dot products
    accumulator: [i32; 8],
    /// Operation counter for performance tracking
    op_count: u64,
    /// Cycle counter for throughput measurement
    cycle_count: u64,
}

impl SimdCoprocessor {
    /// Create a new SIMD coprocessor
    pub fn new() -> Self {
        Self {
            work_ram: vec![SimdVector::new(); 4096], // 64KB / 16 bytes per vector
            accumulator: [0; 8],
            op_count: 0,
            cycle_count: 0,
        }
    }

    /// Reset performance counters
    pub fn reset_counters(&mut self) {
        self.op_count = 0;
        self.cycle_count = 0;
    }

    /// Get current GOPS (Giga Operations Per Second) estimate
    /// Assumes 1GHz clock for calculation
    pub fn get_gops(&self) -> f64 {
        if self.cycle_count == 0 {
            return 0.0;
        }
        // Operations per second at 1GHz
        (self.op_count as f64) / (self.cycle_count as f64)
    }

    /// Get operation count
    pub fn get_op_count(&self) -> u64 {
        self.op_count
    }

    /// Get cycle count
    pub fn get_cycle_count(&self) -> u64 {
        self.cycle_count
    }

    // ==================== Vector Arithmetic (8/16/32-bit) ====================

    /// VADD16 - Vector add (16-bit lanes)
    /// Cycles: 1, Operations: 16
    pub fn vadd16(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            result.data[i] = a.data[i].wrapping_add(b.data[i]);
        }
        self.op_count += 16;
        self.cycle_count += 1;
        result
    }

    /// VADD8 - Vector add (8-bit lanes, treating i16 as two i8)
    /// Cycles: 1, Operations: 32
    pub fn vadd8(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            let a_low = (a.data[i] & 0xFF) as i8;
            let a_high = ((a.data[i] >> 8) & 0xFF) as i8;
            let b_low = (b.data[i] & 0xFF) as i8;
            let b_high = ((b.data[i] >> 8) & 0xFF) as i8;

            let r_low = a_low.wrapping_add(b_low) as u8;
            let r_high = a_high.wrapping_add(b_high) as u8;

            result.data[i] = ((r_high as i16) << 8) | (r_low as i16);
        }
        self.op_count += 32;
        self.cycle_count += 1;
        result
    }

    /// VADD32 - Vector add (32-bit lanes, pairs of i16)
    /// Cycles: 2, Operations: 8
    pub fn vadd32(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in (0..16).step_by(2) {
            let a_val = ((a.data[i] as i32) & 0xFFFF) | ((a.data[i + 1] as i32) << 16);
            let b_val = ((b.data[i] as i32) & 0xFFFF) | ((b.data[i + 1] as i32) << 16);
            let r_val = a_val.wrapping_add(b_val);

            result.data[i] = (r_val & 0xFFFF) as i16;
            result.data[i + 1] = ((r_val >> 16) & 0xFFFF) as i16;
        }
        self.op_count += 8;
        self.cycle_count += 2;
        result
    }

    /// VSUB16 - Vector subtract (16-bit lanes)
    /// Cycles: 1, Operations: 16
    pub fn vsub16(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            result.data[i] = a.data[i].wrapping_sub(b.data[i]);
        }
        self.op_count += 16;
        self.cycle_count += 1;
        result
    }

    /// VSUB8 - Vector subtract (8-bit lanes)
    /// Cycles: 1, Operations: 32
    pub fn vsub8(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            let a_low = (a.data[i] & 0xFF) as i8;
            let a_high = ((a.data[i] >> 8) & 0xFF) as i8;
            let b_low = (b.data[i] & 0xFF) as i8;
            let b_high = ((b.data[i] >> 8) & 0xFF) as i8;

            let r_low = a_low.wrapping_sub(b_low) as u8;
            let r_high = a_high.wrapping_sub(b_high) as u8;

            result.data[i] = ((r_high as i16) << 8) | (r_low as i16);
        }
        self.op_count += 32;
        self.cycle_count += 1;
        result
    }

    /// VSUB32 - Vector subtract (32-bit lanes)
    /// Cycles: 2, Operations: 8
    pub fn vsub32(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in (0..16).step_by(2) {
            let a_val = ((a.data[i] as i32) & 0xFFFF) | ((a.data[i + 1] as i32) << 16);
            let b_val = ((b.data[i] as i32) & 0xFFFF) | ((b.data[i + 1] as i32) << 16);
            let r_val = a_val.wrapping_sub(b_val);

            result.data[i] = (r_val & 0xFFFF) as i16;
            result.data[i + 1] = ((r_val >> 16) & 0xFFFF) as i16;
        }
        self.op_count += 8;
        self.cycle_count += 2;
        result
    }

    /// VMUL16 - Vector multiply (16-bit lanes)
    /// Cycles: 2, Operations: 16
    pub fn vmul16(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            result.data[i] = a.data[i].wrapping_mul(b.data[i]);
        }
        self.op_count += 16;
        self.cycle_count += 2;
        result
    }

    /// VMUL8 - Vector multiply (8-bit lanes)
    /// Cycles: 2, Operations: 32
    pub fn vmul8(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            let a_low = (a.data[i] & 0xFF) as i8;
            let a_high = ((a.data[i] >> 8) & 0xFF) as i8;
            let b_low = (b.data[i] & 0xFF) as i8;
            let b_high = ((b.data[i] >> 8) & 0xFF) as i8;

            let r_low = a_low.wrapping_mul(b_low) as u8;
            let r_high = a_high.wrapping_mul(b_high) as u8;

            result.data[i] = ((r_high as i16) << 8) | (r_low as i16);
        }
        self.op_count += 32;
        self.cycle_count += 2;
        result
    }

    /// VMUL32 - Vector multiply (32-bit lanes)
    /// Cycles: 4, Operations: 8
    pub fn vmul32(&mut self, a: &SimdVector, b: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in (0..16).step_by(2) {
            let a_val = ((a.data[i] as i32) & 0xFFFF) | ((a.data[i + 1] as i32) << 16);
            let b_val = ((b.data[i] as i32) & 0xFFFF) | ((b.data[i + 1] as i32) << 16);
            let r_val = a_val.wrapping_mul(b_val);

            result.data[i] = (r_val & 0xFFFF) as i16;
            result.data[i + 1] = ((r_val >> 16) & 0xFFFF) as i16;
        }
        self.op_count += 8;
        self.cycle_count += 4;
        result
    }

    /// VDOT16 - Dot product (16-bit, returns 32-bit result)
    /// Cycles: 8, Operations: 32 (16 multiply + 16 add)
    pub fn vdot16(&mut self, a: &SimdVector, b: &SimdVector) -> i32 {
        let mut acc: i32 = 0;
        for i in 0..16 {
            acc = acc.wrapping_add((a.data[i] as i32).wrapping_mul(b.data[i] as i32));
        }
        self.op_count += 32; // 16 muls + 16 adds
        self.cycle_count += 8;
        acc
    }

    /// VDOT32 - Dot product (32-bit lanes)
    /// Cycles: 16, Operations: 16 (8 multiply + 8 add)
    pub fn vdot32(&mut self, a: &SimdVector, b: &SimdVector) -> i64 {
        let mut acc: i64 = 0;
        for i in (0..16).step_by(2) {
            let a_val = ((a.data[i] as i32) & 0xFFFF) | ((a.data[i + 1] as i32) << 16);
            let b_val = ((b.data[i] as i32) & 0xFFFF) | ((b.data[i + 1] as i32) << 16);
            acc = acc.wrapping_add((a_val as i64).wrapping_mul(b_val as i64));
        }
        self.op_count += 16; // 8 muls + 8 adds
        self.cycle_count += 16;
        acc
    }

    /// VMADD - Vector multiply-accumulate (16-bit)
    /// result = a * b + c
    /// Cycles: 3, Operations: 32 (16 multiply + 16 add)
    pub fn vmadd(&mut self, a: &SimdVector, b: &SimdVector, c: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            result.data[i] = a.data[i]
                .wrapping_mul(b.data[i])
                .wrapping_add(c.data[i]);
        }
        self.op_count += 32; // 16 muls + 16 adds
        self.cycle_count += 3;
        result
    }

    // ==================== Matrix Operations ====================

    /// MMUL - Matrix multiply (4×4, 16-bit)
    /// Cycles: 64, Operations: 128 (64 multiply + 64 add)
    pub fn mmul(&mut self, a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
        let mut result = Matrix4x4::new();
        for i in 0..4 {
            for j in 0..4 {
                let mut sum = 0i32;
                for k in 0..4 {
                    sum = sum.wrapping_add((a.data[i][k] as i32).wrapping_mul(b.data[k][j] as i32));
                }
                result.data[i][j] = sum.clamp(-32768, 32767) as i16;
            }
        }
        self.op_count += 128; // 64 muls + 64 adds
        self.cycle_count += 64;
        result
    }

    /// MMADD - Matrix multiply-accumulate (4×4, 16-bit)
    /// result = a * b + c
    /// Cycles: 80, Operations: 192 (64 multiply + 64 add + 64 add)
    pub fn mmadd(&mut self, a: &Matrix4x4, b: &Matrix4x4, c: &Matrix4x4) -> Matrix4x4 {
        let mut result = Matrix4x4::new();
        for i in 0..4 {
            for j in 0..4 {
                let mut sum = c.data[i][j] as i32;
                for k in 0..4 {
                    sum = sum.wrapping_add((a.data[i][k] as i32).wrapping_mul(b.data[k][j] as i32));
                }
                result.data[i][j] = sum.clamp(-32768, 32767) as i16;
            }
        }
        self.op_count += 192; // 64 muls + 128 adds
        self.cycle_count += 80;
        result
    }

    // ==================== Neural Network Primitives ====================

    /// RELU - ReLU activation function
    /// Cycles: 1, Operations: 16
    pub fn relu(&mut self, v: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            result.data[i] = v.data[i].max(0);
        }
        self.op_count += 16;
        self.cycle_count += 1;
        result
    }

    /// SIGMOID - Sigmoid activation (approximation)
    /// Uses piecewise linear approximation for hardware efficiency
    /// Cycles: 32, Operations: 64
    pub fn sigmoid(&mut self, v: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();
        for i in 0..16 {
            let x = v.data[i];
            // Piecewise linear approximation:
            // sigmoid(x) ≈ 0 for x < -4
            // sigmoid(x) ≈ 1 for x > 4
            // sigmoid(x) ≈ 0.5 + 0.125*x for -4 <= x <= 4
            result.data[i] = if x < -4 * 1024 {
                0
            } else if x > 4 * 1024 {
                32767 // ~1.0 in Q15 fixed point
            } else {
                // Q15 fixed point: 0.5 = 16384, 0.125 = 4096
                (16384 + (x / 8)).clamp(0, 32767)
            };
        }
        self.op_count += 64; // comparisons + arithmetic
        self.cycle_count += 32;
        result
    }

    /// SOFTMAX - Softmax activation (row-wise for 16 elements)
    /// Uses approximation for hardware efficiency
    /// Cycles: 128, Operations: 256
    pub fn softmax(&mut self, v: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();

        // Find max for numerical stability
        let max_val = *v.data.iter().max().unwrap_or(&0);

        // Compute exp(x - max) approximation and sum
        let mut exp_vals = [0i32; 16];
        let mut sum = 0i32;

        for i in 0..16 {
            let x = v.data[i] - max_val;
            // Simple exp approximation: exp(x) ≈ 1 + x + x²/2 (for small x)
            // Scale to avoid overflow
            let scaled = x / 256; // Scale down
            let exp_approx = 256 + scaled + (scaled * scaled) / 512;
            exp_vals[i] = (exp_approx as i32).max(1);
            sum = sum.wrapping_add(exp_vals[i]);
        }

        // Normalize
        for i in 0..16 {
            result.data[i] = ((exp_vals[i] as i64 * 32767) / sum as i64).clamp(0, 32767) as i16;
        }

        self.op_count += 256; // exp approximations + divisions
        self.cycle_count += 128;
        result
    }

    /// POOL_MAX - Max pooling (2×2)
    /// Input: 4×4 matrix (as 16-element vector in row-major order)
    /// Output: 2×2 matrix (as 4-element vector)
    /// Cycles: 8, Operations: 12
    pub fn pool_max_2x2(&mut self, input: &SimdVector) -> SimdVector {
        let mut result = SimdVector::new();

        // Process 2×2 pools
        // Pool 0: max(0,1,4,5)
        result.data[0] = input.data[0]
            .max(input.data[1])
            .max(input.data[4])
            .max(input.data[5]);

        // Pool 1: max(2,3,6,7)
        result.data[1] = input.data[2]
            .max(input.data[3])
            .max(input.data[6])
            .max(input.data[7]);

        // Pool 2: max(8,9,12,13)
        result.data[2] = input.data[8]
            .max(input.data[9])
            .max(input.data[12])
            .max(input.data[13]);

        // Pool 3: max(10,11,14,15)
        result.data[3] = input.data[10]
            .max(input.data[11])
            .max(input.data[14])
            .max(input.data[15]);

        self.op_count += 12; // 3 comparisons per pool × 4 pools
        self.cycle_count += 8;
        result
    }

    /// CONV2D - 2D convolution (3×3 kernel)
    /// Input: 5×5 matrix (25 elements)
    /// Kernel: 3×3 matrix (9 elements)
    /// Output: 3×3 matrix (9 elements)
    /// Cycles: 162, Operations: 243 (81 multiply + 162 add)
    pub fn conv2d_3x3(&mut self, input: &[i16; 25], kernel: &[i16; 9]) -> [i16; 9] {
        let mut output = [0i16; 9];

        // Perform 3×3 convolution
        for out_y in 0..3 {
            for out_x in 0..3 {
                let mut sum = 0i32;

                for ky in 0..3 {
                    for kx in 0..3 {
                        let in_y = out_y + ky;
                        let in_x = out_x + kx;
                        let in_idx = in_y * 5 + in_x;
                        let k_idx = ky * 3 + kx;

                        sum = sum.wrapping_add((input[in_idx] as i32).wrapping_mul(kernel[k_idx] as i32));
                    }
                }

                output[out_y * 3 + out_x] = sum.clamp(-32768, 32767) as i16;
            }
        }

        self.op_count += 243; // 9 outputs × (9 muls + 8 adds + 1 add)
        self.cycle_count += 162;
        output
    }

    /// CONV2D - 2D convolution (5×5 kernel)
    /// Input: 9×9 matrix (81 elements)
    /// Kernel: 5×5 matrix (25 elements)
    /// Output: 5×5 matrix (25 elements)
    /// Cycles: 1250, Operations: 1875 (625 multiply + 1250 add)
    pub fn conv2d_5x5(&mut self, input: &[i16; 81], kernel: &[i16; 25]) -> [i16; 25] {
        let mut output = [0i16; 25];

        // Perform 5×5 convolution
        for out_y in 0..5 {
            for out_x in 0..5 {
                let mut sum = 0i32;

                for ky in 0..5 {
                    for kx in 0..5 {
                        let in_y = out_y + ky;
                        let in_x = out_x + kx;
                        let in_idx = in_y * 9 + in_x;
                        let k_idx = ky * 5 + kx;

                        sum = sum.wrapping_add((input[in_idx] as i32).wrapping_mul(kernel[k_idx] as i32));
                    }
                }

                output[out_y * 5 + out_x] = sum.clamp(-32768, 32767) as i16;
            }
        }

        self.op_count += 1875; // 25 outputs × (25 muls + 24 adds + 1 add)
        self.cycle_count += 1250;
        output
    }

    // ==================== Work RAM Operations ====================

    /// Load vector from work RAM
    pub fn load_vector(&self, addr: usize) -> Result<SimdVector> {
        if addr >= self.work_ram.len() {
            return Err(CryptoError::OperationFailed);
        }
        Ok(self.work_ram[addr])
    }

    /// Store vector to work RAM
    pub fn store_vector(&mut self, addr: usize, vector: SimdVector) -> Result<()> {
        if addr >= self.work_ram.len() {
            return Err(CryptoError::OperationFailed);
        }
        self.work_ram[addr] = vector;
        Ok(())
    }

    /// Load accumulator value
    pub fn load_accumulator(&self, idx: usize) -> Result<i32> {
        if idx >= 8 {
            return Err(CryptoError::OperationFailed);
        }
        Ok(self.accumulator[idx])
    }

    /// Store accumulator value
    pub fn store_accumulator(&mut self, idx: usize, value: i32) -> Result<()> {
        if idx >= 8 {
            return Err(CryptoError::OperationFailed);
        }
        self.accumulator[idx] = value;
        Ok(())
    }

    /// Clear all accumulators
    pub fn clear_accumulators(&mut self) {
        self.accumulator = [0; 8];
    }
}

impl Default for SimdCoprocessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_vector_creation() {
        let v = SimdVector::new();
        assert_eq!(v.data, [0; 16]);

        let v2 = SimdVector::splat(42);
        assert_eq!(v2.data, [42; 16]);

        let v3 = SimdVector::from_slice(&[1, 2, 3, 4]);
        assert_eq!(v3.data[0], 1);
        assert_eq!(v3.data[3], 4);
        assert_eq!(v3.data[4], 0);
    }

    #[test]
    fn test_vadd16() {
        let mut cop = SimdCoprocessor::new();
        let a = SimdVector::splat(100);
        let b = SimdVector::splat(50);
        let result = cop.vadd16(&a, &b);

        assert_eq!(result.data, [150; 16]);
        assert_eq!(cop.get_op_count(), 16);
        assert_eq!(cop.get_cycle_count(), 1);
    }

    #[test]
    fn test_vdot16() {
        let mut cop = SimdCoprocessor::new();
        let a = SimdVector::splat(10);
        let b = SimdVector::splat(5);
        let result = cop.vdot16(&a, &b);

        assert_eq!(result, 16 * 10 * 5);
        assert_eq!(cop.get_op_count(), 32);
    }

    #[test]
    fn test_matrix_multiply() {
        let mut cop = SimdCoprocessor::new();
        let a = Matrix4x4::identity();
        let b = Matrix4x4::from_array(&[
            [1, 2, 3, 4],
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ]);

        let result = cop.mmul(&a, &b);
        assert_eq!(result.data, b.data);
    }

    #[test]
    fn test_relu() {
        let mut cop = SimdCoprocessor::new();
        let v = SimdVector::from_slice(&[-5, -1, 0, 1, 5]);
        let result = cop.relu(&v);

        assert_eq!(result.data[0], 0);
        assert_eq!(result.data[1], 0);
        assert_eq!(result.data[2], 0);
        assert_eq!(result.data[3], 1);
        assert_eq!(result.data[4], 5);
    }

    #[test]
    fn test_performance_tracking() {
        let mut cop = SimdCoprocessor::new();
        cop.reset_counters();

        let a = SimdVector::splat(1);
        let b = SimdVector::splat(2);

        cop.vadd16(&a, &b); // 16 ops, 1 cycle
        cop.vmul16(&a, &b); // 16 ops, 2 cycles

        assert_eq!(cop.get_op_count(), 32);
        assert_eq!(cop.get_cycle_count(), 3);

        let gops = cop.get_gops();
        assert!(gops > 10.0); // Should be 32/3 ≈ 10.67 GOPS at 1GHz
    }
}
