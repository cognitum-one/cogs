//! SIMD-optimized kernels for molecular dynamics
//!
//! Provides vectorized implementations of common operations:
//! - Distance calculations with periodic boundaries
//! - Force accumulation
//! - Velocity/position updates

mod distance;

pub use distance::*;

/// Number of f32 values in a SIMD lane (AVX2 = 8, AVX-512 = 16)
#[cfg(target_feature = "avx2")]
pub const SIMD_WIDTH: usize = 8;

/// Number of f32 values in a SIMD lane (SSE/NEON fallback = 4)
#[cfg(not(target_feature = "avx2"))]
pub const SIMD_WIDTH: usize = 4;

/// Align a slice length to SIMD width
#[inline]
pub const fn align_to_simd(n: usize) -> usize {
    (n + SIMD_WIDTH - 1) / SIMD_WIDTH * SIMD_WIDTH
}

/// Check if the current CPU supports AVX2
#[inline]
pub fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Check if the current CPU supports AVX-512
#[inline]
pub fn has_avx512() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx512f")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Runtime SIMD capability detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    /// No SIMD available
    Scalar,
    /// SSE4.1 (128-bit)
    Sse4,
    /// AVX2 (256-bit)
    Avx2,
    /// AVX-512 (512-bit)
    Avx512,
    /// ARM NEON (128-bit)
    Neon,
}

impl SimdCapability {
    /// Detect the best available SIMD capability
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return Self::Avx512;
            }
            if is_x86_feature_detected!("avx2") {
                return Self::Avx2;
            }
            if is_x86_feature_detected!("sse4.1") {
                return Self::Sse4;
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on aarch64
            return Self::Neon;
        }
        Self::Scalar
    }

    /// Get the SIMD width in f32 elements
    pub fn width(&self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Sse4 | Self::Neon => 4,
            Self::Avx2 => 8,
            Self::Avx512 => 16,
        }
    }
}
