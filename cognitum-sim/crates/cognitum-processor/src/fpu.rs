// IEEE 754 Floating-Point Unit Implementation
// Supports single (f32) and double (f64) precision operations
// Matches A2S v2r3 ISA specification

use crate::error::Result;

/// IEEE 754 exception flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FpuFlags {
    /// Invalid operation (NaN operand, invalid conversion, etc.)
    pub invalid: bool,
    /// Division by zero
    pub division_by_zero: bool,
    /// Result too large for destination format
    pub overflow: bool,
    /// Result too small for normalized representation
    pub underflow: bool,
    /// Result not exact (rounded)
    pub inexact: bool,
}

impl FpuFlags {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn any(&self) -> bool {
        self.invalid || self.division_by_zero || self.overflow || self.underflow || self.inexact
    }
}

/// IEEE 754 rounding modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingMode {
    /// Round to nearest, ties to even (default)
    NearestTiesToEven,
    /// Round toward +infinity
    TowardPositive,
    /// Round toward -infinity
    TowardNegative,
    /// Round toward zero (truncate)
    TowardZero,
}

impl Default for RoundingMode {
    fn default() -> Self {
        RoundingMode::NearestTiesToEven
    }
}

/// Floating-Point Unit
pub struct Fpu {
    /// IEEE 754 exception flags
    pub flags: FpuFlags,
    /// Current rounding mode
    pub rounding_mode: RoundingMode,
}

impl Default for Fpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Fpu {
    /// Create new FPU with default state
    pub fn new() -> Self {
        Self {
            flags: FpuFlags::new(),
            rounding_mode: RoundingMode::default(),
        }
    }

    /// Clear all exception flags
    pub fn clear_flags(&mut self) {
        self.flags.clear();
    }

    /// Update flags for f32 result
    fn update_flags_f32(&mut self, result: f32) {
        if result.is_nan() {
            self.flags.invalid = true;
        }
        if result.is_infinite() {
            self.flags.overflow = true;
        }
        // Check for subnormal (denormalized) numbers
        if result != 0.0 && result.abs() < f32::MIN_POSITIVE {
            self.flags.underflow = true;
        }
    }

    /// Update flags for f64 result
    fn update_flags_f64(&mut self, result: f64) {
        if result.is_nan() {
            self.flags.invalid = true;
        }
        if result.is_infinite() {
            self.flags.overflow = true;
        }
        // Check for subnormal (denormalized) numbers
        if result != 0.0 && result.abs() < f64::MIN_POSITIVE {
            self.flags.underflow = true;
        }
    }

    // ========================================
    // Single Precision Operations (f32)
    // ========================================

    /// FADD: Single-precision addition
    pub fn fadd(&mut self, a: f32, b: f32) -> f32 {
        let result = a + b;
        self.update_flags_f32(result);
        result
    }

    /// FSUB: Single-precision subtraction
    pub fn fsub(&mut self, a: f32, b: f32) -> f32 {
        let result = a - b;
        self.update_flags_f32(result);
        result
    }

    /// FMUL: Single-precision multiplication
    pub fn fmul(&mut self, a: f32, b: f32) -> f32 {
        let result = a * b;
        self.update_flags_f32(result);
        result
    }

    /// FDIV: Single-precision division
    pub fn fdiv(&mut self, a: f32, b: f32) -> Result<f32> {
        if b == 0.0 {
            self.flags.division_by_zero = true;
            if a == 0.0 {
                // 0/0 = NaN
                self.flags.invalid = true;
                return Ok(f32::NAN);
            } else if a > 0.0 {
                return Ok(f32::INFINITY);
            } else {
                return Ok(f32::NEG_INFINITY);
            }
        }
        let result = a / b;
        self.update_flags_f32(result);
        Ok(result)
    }

    /// FSQRT: Single-precision square root
    pub fn fsqrt(&mut self, a: f32) -> Result<f32> {
        if a < 0.0 {
            self.flags.invalid = true;
            return Ok(f32::NAN);
        }
        let result = a.sqrt();
        self.update_flags_f32(result);
        Ok(result)
    }

    /// FCMP: Single-precision compare
    /// Returns: -1 if a < b, 0 if a == b, 1 if a > b, NaN behavior per IEEE 754
    pub fn fcmp(&mut self, a: f32, b: f32) -> i32 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return 0; // Unordered comparison
        }

        if a < b {
            -1
        } else if a > b {
            1
        } else {
            0
        }
    }

    /// FCLT: Single-precision less than
    pub fn fclt(&mut self, a: f32, b: f32) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a < b
    }

    /// FCEQ: Single-precision equal
    pub fn fceq(&mut self, a: f32, b: f32) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a == b
    }

    /// FCLE: Single-precision less than or equal
    pub fn fcle(&mut self, a: f32, b: f32) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a <= b
    }

    /// F2I: Single-precision float to signed integer
    pub fn f2i(&mut self, f: f32) -> Result<i32> {
        if f.is_nan() {
            self.flags.invalid = true;
            return Ok(0);
        }
        if f.is_infinite() {
            self.flags.invalid = true;
            return Ok(if f > 0.0 { i32::MAX } else { i32::MIN });
        }

        // Check for overflow
        if f > i32::MAX as f32 {
            self.flags.overflow = true;
            return Ok(i32::MAX);
        }
        if f < i32::MIN as f32 {
            self.flags.overflow = true;
            return Ok(i32::MIN);
        }

        let result = f as i32;
        // Check if conversion was exact
        if (result as f32) != f {
            self.flags.inexact = true;
        }
        Ok(result)
    }

    /// I2F: Signed integer to single-precision float
    pub fn i2f(&mut self, n: i32) -> f32 {
        let result = n as f32;
        // Check if conversion was exact
        if (result as i32) != n {
            self.flags.inexact = true;
        }
        result
    }

    /// FABS: Single-precision absolute value
    pub fn fabs(&mut self, f: f32) -> f32 {
        f.abs()
    }

    /// FCHS: Single-precision negate (change sign)
    pub fn fchs(&mut self, f: f32) -> f32 {
        -f
    }

    /// FMAX: Single-precision maximum
    pub fn fmax(&mut self, a: f32, b: f32) -> f32 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return f32::NAN;
        }
        a.max(b)
    }

    /// FMIN: Single-precision minimum
    pub fn fmin(&mut self, a: f32, b: f32) -> f32 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return f32::NAN;
        }
        a.min(b)
    }

    // ========================================
    // Double Precision Operations (f64)
    // ========================================

    /// DADD: Double-precision addition
    pub fn dadd(&mut self, a: f64, b: f64) -> f64 {
        let result = a + b;
        self.update_flags_f64(result);
        result
    }

    /// DSUB: Double-precision subtraction
    pub fn dsub(&mut self, a: f64, b: f64) -> f64 {
        let result = a - b;
        self.update_flags_f64(result);
        result
    }

    /// DMUL: Double-precision multiplication
    pub fn dmul(&mut self, a: f64, b: f64) -> f64 {
        let result = a * b;
        self.update_flags_f64(result);
        result
    }

    /// DDIV: Double-precision division
    pub fn ddiv(&mut self, a: f64, b: f64) -> Result<f64> {
        if b == 0.0 {
            self.flags.division_by_zero = true;
            if a == 0.0 {
                self.flags.invalid = true;
                return Ok(f64::NAN);
            } else if a > 0.0 {
                return Ok(f64::INFINITY);
            } else {
                return Ok(f64::NEG_INFINITY);
            }
        }
        let result = a / b;
        self.update_flags_f64(result);
        Ok(result)
    }

    /// DSQRT: Double-precision square root
    pub fn dsqrt(&mut self, a: f64) -> Result<f64> {
        if a < 0.0 {
            self.flags.invalid = true;
            return Ok(f64::NAN);
        }
        let result = a.sqrt();
        self.update_flags_f64(result);
        Ok(result)
    }

    /// DCMP: Double-precision compare
    pub fn dcmp(&mut self, a: f64, b: f64) -> i32 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return 0;
        }

        if a < b {
            -1
        } else if a > b {
            1
        } else {
            0
        }
    }

    /// DCLT: Double-precision less than
    pub fn dclt(&mut self, a: f64, b: f64) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a < b
    }

    /// DCEQ: Double-precision equal
    pub fn dceq(&mut self, a: f64, b: f64) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a == b
    }

    /// DCLE: Double-precision less than or equal
    pub fn dcle(&mut self, a: f64, b: f64) -> bool {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return false;
        }
        a <= b
    }

    /// D2I: Double-precision float to signed integer
    pub fn d2i(&mut self, d: f64) -> Result<i32> {
        if d.is_nan() {
            self.flags.invalid = true;
            return Ok(0);
        }
        if d.is_infinite() {
            self.flags.invalid = true;
            return Ok(if d > 0.0 { i32::MAX } else { i32::MIN });
        }

        if d > i32::MAX as f64 {
            self.flags.overflow = true;
            return Ok(i32::MAX);
        }
        if d < i32::MIN as f64 {
            self.flags.overflow = true;
            return Ok(i32::MIN);
        }

        let result = d as i32;
        if (result as f64) != d {
            self.flags.inexact = true;
        }
        Ok(result)
    }

    /// I2D: Signed integer to double-precision float
    pub fn i2d(&mut self, n: i32) -> f64 {
        let result = n as f64;
        // i32 to f64 is always exact
        result
    }

    /// F2D: Single to double precision conversion
    pub fn f2d(&mut self, f: f32) -> f64 {
        // f32 to f64 is always exact
        f as f64
    }

    /// D2F: Double to single precision conversion
    pub fn d2f(&mut self, d: f64) -> f32 {
        let result = d as f32;
        // Check for overflow
        if result.is_infinite() && !d.is_infinite() {
            self.flags.overflow = true;
        }
        // Check for inexact conversion
        if (result as f64) != d && !d.is_nan() {
            self.flags.inexact = true;
        }
        result
    }

    /// DABS: Double-precision absolute value
    pub fn dabs(&mut self, d: f64) -> f64 {
        d.abs()
    }

    /// DCHS: Double-precision negate
    pub fn dchs(&mut self, d: f64) -> f64 {
        -d
    }

    /// DMAX: Double-precision maximum
    pub fn dmax(&mut self, a: f64, b: f64) -> f64 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return f64::NAN;
        }
        a.max(b)
    }

    /// DMIN: Double-precision minimum
    pub fn dmin(&mut self, a: f64, b: f64) -> f64 {
        if a.is_nan() || b.is_nan() {
            self.flags.invalid = true;
            return f64::NAN;
        }
        a.min(b)
    }

    // ========================================
    // Utility Functions
    // ========================================

    /// Check if a float is NaN and return 0.0 if so
    pub fn fnan(&mut self, f: f32) -> f32 {
        if f.is_nan() {
            0.0
        } else {
            f
        }
    }

    /// Check if a double is NaN and return 0.0 if so
    pub fn dnan(&mut self, d: f64) -> f64 {
        if d.is_nan() {
            0.0
        } else {
            d
        }
    }

    /// Get exception flags as a bitfield
    pub fn get_flags_bits(&self) -> u8 {
        let mut bits = 0u8;
        if self.flags.invalid {
            bits |= 0b00001;
        }
        if self.flags.division_by_zero {
            bits |= 0b00010;
        }
        if self.flags.overflow {
            bits |= 0b00100;
        }
        if self.flags.underflow {
            bits |= 0b01000;
        }
        if self.flags.inexact {
            bits |= 0b10000;
        }
        bits
    }

    /// Set exception flags from a bitfield
    pub fn set_flags_bits(&mut self, bits: u8) {
        self.flags.invalid = (bits & 0b00001) != 0;
        self.flags.division_by_zero = (bits & 0b00010) != 0;
        self.flags.overflow = (bits & 0b00100) != 0;
        self.flags.underflow = (bits & 0b01000) != 0;
        self.flags.inexact = (bits & 0b10000) != 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fadd_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fadd(1.0, 2.0), 3.0);
        assert_eq!(fpu.fadd(-1.0, 1.0), 0.0);
    }

    #[test]
    fn test_fsub_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fsub(5.0, 3.0), 2.0);
        assert_eq!(fpu.fsub(1.0, 1.0), 0.0);
    }

    #[test]
    fn test_fmul_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fmul(3.0, 4.0), 12.0);
        assert_eq!(fpu.fmul(-2.0, 3.0), -6.0);
    }

    #[test]
    fn test_fdiv_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fdiv(10.0, 2.0).unwrap(), 5.0);
        assert_eq!(fpu.fdiv(7.0, 2.0).unwrap(), 3.5);
    }

    #[test]
    fn test_fdiv_by_zero() {
        let mut fpu = Fpu::new();
        let result = fpu.fdiv(5.0, 0.0).unwrap();
        assert!(result.is_infinite());
        assert!(fpu.flags.division_by_zero);
    }

    #[test]
    fn test_fsqrt_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fsqrt(4.0).unwrap(), 2.0);
        assert_eq!(fpu.fsqrt(9.0).unwrap(), 3.0);
    }

    #[test]
    fn test_fsqrt_negative() {
        let mut fpu = Fpu::new();
        let result = fpu.fsqrt(-1.0).unwrap();
        assert!(result.is_nan());
        assert!(fpu.flags.invalid);
    }

    #[test]
    fn test_fcmp() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.fcmp(1.0, 2.0), -1);
        assert_eq!(fpu.fcmp(2.0, 1.0), 1);
        assert_eq!(fpu.fcmp(1.0, 1.0), 0);
    }

    #[test]
    fn test_f2i_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.f2i(3.14).unwrap(), 3);
        assert_eq!(fpu.f2i(-2.7).unwrap(), -2);
    }

    #[test]
    fn test_i2f_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.i2f(42), 42.0);
        assert_eq!(fpu.i2f(-10), -10.0);
    }

    #[test]
    fn test_dadd_basic() {
        let mut fpu = Fpu::new();
        assert_eq!(fpu.dadd(1.0, 2.0), 3.0);
    }

    #[test]
    fn test_f2d_d2f_roundtrip() {
        let mut fpu = Fpu::new();
        let f = 3.14f32;
        let d = fpu.f2d(f);
        let f2 = fpu.d2f(d);
        assert_eq!(f, f2);
    }

    #[test]
    fn test_nan_handling() {
        let mut fpu = Fpu::new();
        let result = fpu.fadd(f32::NAN, 1.0);
        assert!(result.is_nan());
        assert!(fpu.flags.invalid);
    }
}
