//! Platform abstraction layer
//!
//! Provides a unified interface across:
//! - ESP32-S3 (32-bit, SIMD, full features)
//! - Cognitum V1 (8-bit, minimal)
//! - Cognitum V2 (8-bit + pipeline)
//! - WASM (browser/Node.js)

/// Platform capabilities trait
pub trait Platform {
    /// Get platform name
    fn name() -> &'static str;

    /// Get maximum patterns supported
    fn max_patterns() -> usize;

    /// Get HNSW M parameter
    fn hnsw_m() -> usize;

    /// Get maximum HNSW vectors
    fn hnsw_max_vectors() -> usize;

    /// Check if SIMD is available
    fn has_simd() -> bool;

    /// Check if FFT is available
    fn has_fft() -> bool;

    /// Get feature vector dimensions
    fn feature_dims() -> usize;
}

/// ESP32-S3 platform
#[cfg(feature = "esp32s3")]
pub struct Esp32S3Platform;

#[cfg(feature = "esp32s3")]
impl Platform for Esp32S3Platform {
    fn name() -> &'static str { "ESP32-S3" }
    fn max_patterns() -> usize { 2000 }
    fn hnsw_m() -> usize { 8 }
    fn hnsw_max_vectors() -> usize { 2000 }
    fn has_simd() -> bool { true }
    fn has_fft() -> bool { true }
    fn feature_dims() -> usize { 16 }
}

/// Cognitum V1 platform
#[cfg(feature = "cognitum-v1")]
pub struct CognitumV1Platform;

#[cfg(feature = "cognitum-v1")]
impl Platform for CognitumV1Platform {
    fn name() -> &'static str { "Cognitum V1" }
    fn max_patterns() -> usize { 64 }
    fn hnsw_m() -> usize { 4 }
    fn hnsw_max_vectors() -> usize { 64 }
    fn has_simd() -> bool { false }
    fn has_fft() -> bool { false }
    fn feature_dims() -> usize { 8 }
}

/// Cognitum V2 platform
#[cfg(feature = "cognitum-v2")]
pub struct CognitumV2Platform;

#[cfg(feature = "cognitum-v2")]
impl Platform for CognitumV2Platform {
    fn name() -> &'static str { "Cognitum V2" }
    fn max_patterns() -> usize { 128 }
    fn hnsw_m() -> usize { 4 }
    fn hnsw_max_vectors() -> usize { 128 }
    fn has_simd() -> bool { true }
    fn has_fft() -> bool { false }
    fn feature_dims() -> usize { 16 }
}

/// WASM platform (default for web)
#[cfg(feature = "wasm")]
pub struct WasmPlatform;

#[cfg(feature = "wasm")]
impl Platform for WasmPlatform {
    fn name() -> &'static str { "WASM" }
    fn max_patterns() -> usize { 2000 }
    fn hnsw_m() -> usize { 8 }
    fn hnsw_max_vectors() -> usize { 2000 }
    fn has_simd() -> bool { false } // Could enable with SIMD proposal
    fn has_fft() -> bool { true }
    fn feature_dims() -> usize { 16 }
}

/// Pi Zero 2W platform (BCM2710A1, 4x Cortex-A53 @ 1.0GHz, 512MB PoP DRAM)
#[cfg(feature = "pi-zero-2w")]
pub struct PiZero2WPlatform;

#[cfg(feature = "pi-zero-2w")]
impl Platform for PiZero2WPlatform {
    fn name() -> &'static str { "Pi Zero 2W" }
    fn max_patterns() -> usize { 2000 }
    fn hnsw_m() -> usize { 8 }
    fn hnsw_max_vectors() -> usize { 2000 }
    fn has_simd() -> bool { true } // NEON on Cortex-A53
    fn has_fft() -> bool { true }
    fn feature_dims() -> usize { 16 }
}

/// Standard platform (default)
#[cfg(all(feature = "std", not(any(feature = "esp32s3", feature = "cognitum-v1", feature = "cognitum-v2", feature = "wasm", feature = "pi-zero-2w"))))]
pub struct StdPlatform;

#[cfg(all(feature = "std", not(any(feature = "esp32s3", feature = "cognitum-v1", feature = "cognitum-v2", feature = "wasm", feature = "pi-zero-2w"))))]
impl Platform for StdPlatform {
    fn name() -> &'static str { "Standard" }
    fn max_patterns() -> usize { 2000 }
    fn hnsw_m() -> usize { 8 }
    fn hnsw_max_vectors() -> usize { 2000 }
    fn has_simd() -> bool { true }
    fn has_fft() -> bool { true }
    fn feature_dims() -> usize { 16 }
}

/// Get current platform info
pub fn platform_info() -> PlatformInfo {
    PlatformInfo {
        #[cfg(feature = "esp32s3")]
        name: "ESP32-S3",
        #[cfg(feature = "cognitum-v1")]
        name: "Cognitum V1",
        #[cfg(feature = "cognitum-v2")]
        name: "Cognitum V2",
        #[cfg(feature = "wasm")]
        name: "WASM",
        #[cfg(feature = "pi-zero-2w")]
        name: "Pi Zero 2W",
        #[cfg(all(feature = "std", not(any(feature = "esp32s3", feature = "cognitum-v1", feature = "cognitum-v2", feature = "wasm", feature = "pi-zero-2w"))))]
        name: "Standard",
        #[cfg(not(any(feature = "std", feature = "esp32s3", feature = "cognitum-v1", feature = "cognitum-v2", feature = "wasm", feature = "pi-zero-2w")))]
        name: "no_std",

        max_patterns: get_max_patterns(),
        hnsw_m: get_hnsw_m(),
        has_simd: has_simd(),
    }
}

/// Platform information
#[derive(Clone, Debug)]
pub struct PlatformInfo {
    pub name: &'static str,
    pub max_patterns: usize,
    pub hnsw_m: usize,
    pub has_simd: bool,
}

/// Get maximum patterns for current platform
pub const fn get_max_patterns() -> usize {
    #[cfg(feature = "cognitum-v1")]
    { 64 }
    #[cfg(feature = "cognitum-v2")]
    { 128 }
    #[cfg(not(any(feature = "cognitum-v1", feature = "cognitum-v2")))]
    { 2000 }
}

/// Get HNSW M parameter for current platform
pub const fn get_hnsw_m() -> usize {
    #[cfg(any(feature = "cognitum-v1", feature = "cognitum-v2"))]
    { 4 }
    #[cfg(not(any(feature = "cognitum-v1", feature = "cognitum-v2")))]
    { 8 }
}

/// Check if SIMD is available
pub const fn has_simd() -> bool {
    #[cfg(feature = "cognitum-v1")]
    { false }
    #[cfg(not(feature = "cognitum-v1"))]
    { true }
}
