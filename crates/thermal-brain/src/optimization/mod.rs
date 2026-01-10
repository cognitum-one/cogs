//! Extreme Optimization Module for ThermalBrain
//!
//! Implements state-of-the-art neuromorphic optimization techniques for
//! maximum performance, minimal power consumption, and efficient memory usage.
//!
//! ## Power Management
//! - **DVFS**: Dynamic Voltage and Frequency Scaling (SpiNNaker-2 style, 75% power reduction)
//! - **Power Gating**: Per-bank power control with retention states (95% idle savings)
//! - **Burst Mode**: Time-limited turbo operation (2x performance)
//!
//! ## Computation Efficiency
//! - **SIMD Operations**: Vectorized sparse operations (Polaris 23 / SpikeStream)
//! - **Quantization**: INT4/INT8 mixed-precision (50% memory, 4x compute)
//! - **Adaptive Precision**: Dynamic bit-width per layer
//! - **Network Pruning**: Remove low-weight connections
//!
//! ## Memory Efficiency
//! - **Spike Compression**: RLE/Delta/Bitmap encoding (4-8x reduction)
//! - **Delta Encoding**: Store only changes (2x reduction)
//! - **Memory Arena**: Pool-based allocation for no_std
//!
//! ## Advanced Processing
//! - **Event-Driven**: Process only on spikes (10x power reduction)
//! - **Temporal Coding**: Use spike timing for information
//! - **Predictive Thermal**: ML-based temperature forecasting
//! - **Meta-Plasticity**: BCM learning with homeostatic adaptation

// Power Management
mod dvfs;
mod power_gating;
mod burst_mode;

// Computation Efficiency
mod simd_ops;
mod quantization;
mod adaptive_precision;
mod network_pruning;

// Memory Efficiency
mod spike_compression;
mod delta_encoding;
mod memory_arena;

// Advanced Processing
mod event_driven;
mod temporal_coding;
mod predictive_thermal;
mod meta_plasticity;

// Re-exports - Power Management
pub use dvfs::*;
pub use power_gating::*;
pub use burst_mode::*;

// Re-exports - Computation Efficiency
pub use simd_ops::*;
pub use quantization::*;
pub use adaptive_precision::*;
pub use network_pruning::*;

// Re-exports - Memory Efficiency
pub use spike_compression::*;
pub use delta_encoding::*;
pub use memory_arena::*;

// Re-exports - Advanced Processing
pub use event_driven::*;
pub use temporal_coding::*;
pub use predictive_thermal::*;
pub use meta_plasticity::*;
