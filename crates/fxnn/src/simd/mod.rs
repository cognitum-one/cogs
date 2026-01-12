//! SIMD-optimized kernels for molecular dynamics
//!
//! Provides vectorized implementations of common operations:
//! - Distance calculations with periodic boundaries
//! - Force accumulation
//! - Velocity/position updates
//!
//! # Architecture Support
//!
//! This module automatically detects and uses the best available SIMD:
//! - **AVX-512** (512-bit): 16 f32 elements per operation (Zen4, Ice Lake+)
//! - **AVX2** (256-bit): 8 f32 elements per operation (Haswell+, Zen+)
//! - **SSE4.1** (128-bit): 4 f32 elements per operation (Core 2+)
//! - **NEON** (128-bit): 4 f32 elements per operation (ARM64)
//!
//! # Performance
//!
//! Expected throughput improvements:
//! - AVX-512: Up to 16x over scalar
//! - AVX2: Up to 8x over scalar
//! - SSE4.1/NEON: Up to 4x over scalar

mod distance;

pub use distance::*;

use std::sync::OnceLock;

/// Cached SIMD capability detection result
static DETECTED_CAPABILITY: OnceLock<SimdCapability> = OnceLock::new();

/// Get the runtime SIMD width (cached)
#[inline]
pub fn runtime_simd_width() -> usize {
    get_simd_capability().width()
}

/// Get the detected SIMD capability (cached)
#[inline]
pub fn get_simd_capability() -> SimdCapability {
    *DETECTED_CAPABILITY.get_or_init(SimdCapability::detect)
}

/// Number of f32 values in a SIMD lane (compile-time default)
/// Runtime code should use `runtime_simd_width()` for optimal width
#[cfg(target_feature = "avx512f")]
pub const SIMD_WIDTH: usize = 16;

#[cfg(all(target_feature = "avx2", not(target_feature = "avx512f")))]
pub const SIMD_WIDTH: usize = 8;

#[cfg(all(not(target_feature = "avx2"), not(target_feature = "avx512f")))]
pub const SIMD_WIDTH: usize = 8; // Use 8 as default for wide crate compatibility

/// Align a slice length to SIMD width
#[inline]
pub const fn align_to_simd(n: usize) -> usize {
    (n + SIMD_WIDTH - 1) / SIMD_WIDTH * SIMD_WIDTH
}

/// Align a slice length to a specific width
#[inline]
pub const fn align_to_width(n: usize, width: usize) -> usize {
    (n + width - 1) / width * width
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

/// Check if the current CPU supports FMA (Fused Multiply-Add)
#[inline]
pub fn has_fma() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("fma")
    }
    #[cfg(target_arch = "aarch64")]
    {
        true // NEON always has FMA
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
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

    /// Get the SIMD width in bytes
    pub fn width_bytes(&self) -> usize {
        self.width() * std::mem::size_of::<f32>()
    }

    /// Check if FMA is available
    pub fn has_fma(&self) -> bool {
        match self {
            Self::Avx2 | Self::Avx512 | Self::Neon => has_fma(),
            _ => false,
        }
    }

    /// Get a descriptive name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scalar => "Scalar",
            Self::Sse4 => "SSE4.1",
            Self::Avx2 => "AVX2",
            Self::Avx512 => "AVX-512",
            Self::Neon => "NEON",
        }
    }
}

impl std::fmt::Display for SimdCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}x f32)", self.name(), self.width())
    }
}

/// Prefetch data for improved cache performance
/// This is a hint to the CPU and may be ignored
#[inline]
pub fn prefetch_read<T>(ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(ptr as *const i8, std::arch::x86_64::_MM_HINT_T0);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = ptr; // Suppress unused warning
    }
}

/// Prefetch data for write (exclusive access)
#[inline]
pub fn prefetch_write<T>(ptr: *mut T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(ptr as *const i8, std::arch::x86_64::_MM_HINT_ET0);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = ptr; // Suppress unused warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let cap = SimdCapability::detect();
        println!("Detected SIMD capability: {}", cap);
        assert!(cap.width() >= 1);
    }

    #[test]
    fn test_cached_detection() {
        let cap1 = get_simd_capability();
        let cap2 = get_simd_capability();
        assert_eq!(cap1, cap2);
    }

    #[test]
    fn test_align_to_width() {
        assert_eq!(align_to_width(10, 8), 16);
        assert_eq!(align_to_width(16, 8), 16);
        assert_eq!(align_to_width(17, 8), 24);
    }
}
