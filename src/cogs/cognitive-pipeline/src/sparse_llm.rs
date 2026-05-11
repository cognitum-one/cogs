// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Pi Zero sparse attention inference (Phase 2B-zero, feature = "sparse-llm").
//!
//! Wraps ruvllm_sparse_attention for the Pi Zero 2 W profile:
//! - window=64, block_size=32, tile_size=16 (fits Cortex-A53 32 KB L1)
//! - sort_candidates=true (helps 256 KB shared L2 locality)
//! - No rayon (thread overhead > benefit at 512 MB RAM + seq < 512)
//! - FP16 KV cache (halves the 64 MB KV budget to ~32 MB used)
//! - Q4-quantized sub-1B models only (SmolLM2-135M, Qwen2.5-0.5B)

use ruvllm_sparse_attention::{
    AttentionBackend, AttentionError as RawAttentionError, KvCacheF16, SparseAttentionConfig,
    SubquadraticSparseAttention, Tensor3,
};
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the Pi Zero sparse LLM engine.
#[derive(Debug)]
pub enum SeedLlmError {
    /// Underlying sparse attention kernel error.
    AttentionError(String),
    /// KV cache is full; call `reset()` to clear.
    CacheFull,
    /// Model file exceeds the Pi Zero RAM budget.
    ModelTooLarge { size_bytes: u64, max_bytes: u64 },
    /// Hardware is not a Pi Zero 2 W or compatible Cortex-A53 device.
    UnsupportedHardware(String),
}

impl fmt::Display for SeedLlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeedLlmError::AttentionError(m) => write!(f, "sparse attention error: {}", m),
            SeedLlmError::CacheFull => write!(f, "KV cache full — call reset()"),
            SeedLlmError::ModelTooLarge { size_bytes, max_bytes } => {
                write!(f, "model too large: {} > {} bytes", size_bytes, max_bytes)
            }
            SeedLlmError::UnsupportedHardware(m) => write!(f, "unsupported hardware: {}", m),
        }
    }
}

impl std::error::Error for SeedLlmError {}

impl From<RawAttentionError> for SeedLlmError {
    fn from(e: RawAttentionError) -> Self {
        SeedLlmError::AttentionError(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Pi Zero profile constants
// ---------------------------------------------------------------------------

/// Hardware-tuned constants for Pi Zero 2 W (Cortex-A53 @ 1 GHz, 512 MB RAM).
pub struct PiZeroProfile;

impl PiZeroProfile {
    /// Sliding attention window (tokens).
    pub const WINDOW: usize = 64;
    /// Block granularity for block-sparse attention.
    pub const BLOCK_SIZE: usize = 32;
    /// Tile size for inner matmul loops (documents intended tiling; not a kernel param).
    pub const TILE_SIZE: usize = 16;
    /// Sort candidates for L2 cache locality.
    pub const SORT_CANDIDATES: bool = true;
    /// Rayon disabled — spawn overhead exceeds benefit at seq < 512.
    pub const USE_RAYON: bool = false;
    /// Max model size in bytes — **device-class default for Pi Zero 2 W**
    /// (320 MB; leaves 192 MB for OS + agent on a 512 MB device).
    ///
    /// Other device classes get larger caps via
    /// `max_model_bytes_for_current_device()`:
    /// * `pi-zero-2w` (Cortex-A53, ≤600 MB RAM): 320 MB (this constant)
    /// * `v0-appliance` (Pi 5, 8 GB RAM): 4 GB
    /// * unknown / dev VMs: 1 GB conservative default
    ///
    /// The original 320 MB cap was named "Pi Zero profile" but applied
    /// universally — installing a 469 MB Qwen2.5-0.5B Q4_K_M failed
    /// on v0-appliance even though it had 8 GB of RAM. The device-class
    /// helper below restores the intent: gate by what the device can
    /// actually run, not by the lifted profile's hardcoded constant.
    pub const MAX_MODEL_BYTES: u64 = 320 * 1024 * 1024;
    /// Max KV cache budget in bytes.
    pub const MAX_KV_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
    /// Max sequence length in tokens.
    pub const MAX_SEQ: usize = 512;
}

// ---------------------------------------------------------------------------
// Pi Zero attention worker
// ---------------------------------------------------------------------------

/// Wraps `SubquadraticSparseAttention` + `KvCacheF16` with Pi Zero defaults.
pub struct PiZeroAttentionWorker {
    attention: SubquadraticSparseAttention,
    kv_cache: KvCacheF16,
    max_seq: usize,
}

impl PiZeroAttentionWorker {
    /// Build a worker. `kv_heads`/`head_dim` come from the model file.
    pub fn new(max_seq: usize, kv_heads: usize, head_dim: usize) -> Result<Self, SeedLlmError> {
        let cfg = SparseAttentionConfig {
            window: PiZeroProfile::WINDOW,
            block_size: PiZeroProfile::BLOCK_SIZE,
            global_tokens: vec![0],
            causal: true,
            use_log_stride: true,
            use_landmarks: true,
            sort_candidates: PiZeroProfile::SORT_CANDIDATES,
        };
        let attention = SubquadraticSparseAttention::new(cfg)
            .map_err(|e: RawAttentionError| SeedLlmError::AttentionError(e.to_string()))?;
        let kv_cache = KvCacheF16::new(max_seq, kv_heads, head_dim, PiZeroProfile::BLOCK_SIZE);
        Ok(Self { attention, kv_cache, max_seq })
    }

    /// Sparse attention over a full prompt, then populates the FP16 KV cache.
    pub fn prefill(
        &mut self,
        q: &Tensor3,
        k: &Tensor3,
        v: &Tensor3,
    ) -> Result<Tensor3, SeedLlmError> {
        let out = self.attention.forward(q, k, v).map_err(SeedLlmError::from)?;
        for t in 0..k.seq {
            if self.kv_cache.is_full() {
                return Err(SeedLlmError::CacheFull);
            }
            let k_t = Tensor3::from_vec(
                k.data[t * k.heads * k.dim..(t + 1) * k.heads * k.dim].to_vec(),
                1, k.heads, k.dim,
            )
            .map_err(SeedLlmError::AttentionError)?;
            let v_t = Tensor3::from_vec(
                v.data[t * v.heads * v.dim..(t + 1) * v.heads * v.dim].to_vec(),
                1, v.heads, v.dim,
            )
            .map_err(SeedLlmError::AttentionError)?;
            self.kv_cache.try_append(&k_t, &v_t).map_err(SeedLlmError::from)?;
        }
        Ok(out)
    }

    /// Single-token decode step. `k`/`v` must have `seq == 1`.
    pub fn decode_step(
        &mut self,
        q: &Tensor3,
        k: &Tensor3,
        v: &Tensor3,
    ) -> Result<Tensor3, SeedLlmError> {
        if self.kv_cache.is_full() {
            return Err(SeedLlmError::CacheFull);
        }
        self.kv_cache.try_append(k, v).map_err(SeedLlmError::from)?;
        self.kv_cache.decode_step_f16(&self.attention, q).map_err(SeedLlmError::from)
    }

    pub fn cache_len(&self) -> usize { self.kv_cache.len }

    pub fn is_cache_full(&self) -> bool { self.kv_cache.len >= self.max_seq }

    pub fn reset(&mut self) { self.kv_cache.reset(); }
}

// ---------------------------------------------------------------------------
// Inference config
// ---------------------------------------------------------------------------

/// Configuration for the Pi Zero sparse LLM engine.
#[derive(Debug, Clone)]
pub struct PiZeroInferenceConfig {
    /// Model identifier (e.g. `"smollm2-135m"` or `"qwen2.5-0.5b-q4"`).
    pub model_id: String,
    /// Maximum token context length (capped at `PiZeroProfile::MAX_SEQ`).
    pub max_seq: usize,
    /// KV cache budget in bytes (informational; capped at `MAX_KV_BUDGET_BYTES`).
    pub kv_budget_bytes: u64,
}

impl Default for PiZeroInferenceConfig {
    fn default() -> Self {
        Self {
            model_id: "smollm2-135m".to_string(),
            max_seq: 256,
            kv_budget_bytes: 32 * 1024 * 1024,
        }
    }
}

// ---------------------------------------------------------------------------
// Inference engine
// ---------------------------------------------------------------------------

/// High-level Pi Zero sparse LLM inference engine.
pub struct PiZeroInferenceEngine {
    config: PiZeroInferenceConfig,
    worker: PiZeroAttentionWorker,
}

impl PiZeroInferenceEngine {
    /// Construct an engine. `kv_heads` and `head_dim` come from the loaded model.
    ///
    /// # Errors
    ///
    /// Returns `SeedLlmError::AttentionError` if the kernel rejects the config.
    pub fn new(
        cfg: PiZeroInferenceConfig,
        kv_heads: usize,
        head_dim: usize,
    ) -> Result<Self, SeedLlmError> {
        let max_seq = cfg.max_seq.min(PiZeroProfile::MAX_SEQ);
        let kv_budget = cfg.kv_budget_bytes.min(PiZeroProfile::MAX_KV_BUDGET_BYTES);
        let worker = PiZeroAttentionWorker::new(max_seq, kv_heads, head_dim)?;
        Ok(Self {
            config: PiZeroInferenceConfig { max_seq, kv_budget_bytes: kv_budget, ..cfg },
            worker,
        })
    }

    /// Validate a model file fits within the current device's RAM budget.
    /// Uses `max_model_bytes_for_current_device()` so Pi Zero stays at
    /// 320 MB while v0-appliance can run multi-GB models.
    pub fn validate_model_size(&self, size_bytes: u64) -> Result<(), SeedLlmError> {
        let cap = max_model_bytes_for_current_device();
        if size_bytes > cap {
            return Err(SeedLlmError::ModelTooLarge {
                size_bytes,
                max_bytes: cap,
            });
        }
        Ok(())
    }

    /// Run sparse attention over a full prompt (prefill phase).
    pub fn prefill(
        &mut self, q: &Tensor3, k: &Tensor3, v: &Tensor3,
    ) -> Result<Tensor3, SeedLlmError> {
        self.worker.prefill(q, k, v)
    }

    /// Run sparse attention for one new token (decode phase).
    ///
    /// Returns `SeedLlmError::CacheFull` when `max_seq` is reached.
    pub fn decode_step(
        &mut self, q: &Tensor3, k: &Tensor3, v: &Tensor3,
    ) -> Result<Tensor3, SeedLlmError> {
        self.worker.decode_step(q, k, v)
    }

    /// Clear the KV cache to begin a new conversation without reallocating.
    pub fn reset(&mut self) { self.worker.reset(); }

    /// Number of tokens currently in the KV cache.
    pub fn cache_len(&self) -> usize { self.worker.cache_len() }

    /// Whether the KV cache has reached capacity.
    pub fn is_cache_full(&self) -> bool { self.worker.is_cache_full() }

    /// Return the resolved config (with capped values applied).
    pub fn config(&self) -> &PiZeroInferenceConfig { &self.config }
}

// ---------------------------------------------------------------------------
// Hardware detection
// ---------------------------------------------------------------------------

/// Returns `true` when running on Pi Zero 2 W or a low-RAM Cortex-A53 device.
///
/// Reads `/proc/cpuinfo` (primary: `"Pi Zero 2"`) and falls back to
/// `Cortex-A53` + `MemTotal < 600 000 kB` from `/proc/meminfo`.
/// Returns `false` on any read error (e.g., non-Linux platforms).
pub fn detect_pi_zero() -> bool {
    detect_pi_zero_impl().unwrap_or(false)
}

fn detect_pi_zero_impl() -> Result<bool, std::io::Error> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")?;
    if cpuinfo.contains("Pi Zero 2") {
        return Ok(true);
    }
    if cpuinfo.contains("Cortex-A53") {
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(u64::MAX);
                if kb < 600_000 {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Device classes the cog knows about. Aligned with ADR-095 §6's
/// `hardware_requirements` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// Pi Zero 2 W (Cortex-A53, 512 MB RAM). Tight memory budget.
    PiZero2W,
    /// v0-appliance (Pi 5, 8 GB RAM). Comfortable budget.
    V0Appliance,
    /// Anything else — dev VMs, Pi 4 boards, unknown ARM. Conservative.
    Other,
}

impl DeviceClass {
    /// Per-class hard cap on model file size. Used by
    /// `validate_model_size` to refuse loading a model the device can't
    /// fit. ADR-095 §6 + ADR-094 envelope.
    ///
    /// Pi Zero 2 W: 320 MB (model file + KV + activations ≈ 400 MB,
    /// leaves 100 MB for OS + agent + buffers).
    /// v0-appliance: 4 GB (Pi 5 has 8 GB; cap leaves >half for system).
    /// Other: 1 GB conservative — assumes dev VM with ≥2 GB RAM.
    pub fn max_model_bytes(self) -> u64 {
        match self {
            DeviceClass::PiZero2W => 320 * 1024 * 1024,
            DeviceClass::V0Appliance => 4 * 1024 * 1024 * 1024,
            DeviceClass::Other => 1024 * 1024 * 1024,
        }
    }
}

/// Detect the device class from `/proc/cpuinfo` + `/proc/firmware/devicetree`.
/// Returns `Other` for non-Linux or unparseable systems — that's a safe
/// default because `Other`'s cap is intermediate.
pub fn detect_device_class() -> DeviceClass {
    detect_device_class_impl().unwrap_or(DeviceClass::Other)
}

fn detect_device_class_impl() -> Result<DeviceClass, std::io::Error> {
    // ADR-095 §6 detection priority: /sys/firmware/devicetree/base/model is
    // canonical on Raspbian. Pi 5 models advertise "Raspberry Pi 5"; Pi Zero
    // 2 W advertises "Raspberry Pi Zero 2 W".
    if let Ok(model) = std::fs::read_to_string("/sys/firmware/devicetree/base/model") {
        if model.contains("Pi Zero 2") {
            return Ok(DeviceClass::PiZero2W);
        }
        if model.contains("Pi 5") {
            return Ok(DeviceClass::V0Appliance);
        }
    }
    // Fallback: /proc/cpuinfo + /proc/meminfo. Pi Zero 2 W has Cortex-A53
    // + < 600 MB total RAM; Pi 5 has Cortex-A76 + >= 4 GB.
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")?;
    if cpuinfo.contains("Pi Zero 2") {
        return Ok(DeviceClass::PiZero2W);
    }
    if cpuinfo.contains("Cortex-A76") {
        return Ok(DeviceClass::V0Appliance);
    }
    if cpuinfo.contains("Cortex-A53") {
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(u64::MAX);
                if kb < 600_000 {
                    return Ok(DeviceClass::PiZero2W);
                }
            }
        }
    }
    Ok(DeviceClass::Other)
}

/// Current device's hard cap on model file size, used by
/// `PiZeroInferenceEngine::validate_model_size`. Computed once per process
/// at first call.
pub fn max_model_bytes_for_current_device() -> u64 {
    use std::sync::OnceLock;
    static CAP: OnceLock<u64> = OnceLock::new();
    *CAP.get_or_init(|| detect_device_class().max_model_bytes())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const KV_HEADS: usize = 1;
    const HEAD_DIM: usize = 8;

    fn make_engine(max_seq: usize) -> PiZeroInferenceEngine {
        let cfg = PiZeroInferenceConfig {
            model_id: "smollm2-135m".to_string(),
            max_seq,
            kv_budget_bytes: 4 * 1024 * 1024,
        };
        PiZeroInferenceEngine::new(cfg, KV_HEADS, HEAD_DIM)
            .expect("engine construction should succeed")
    }

    fn zeros(seq: usize) -> Tensor3 { Tensor3::zeros(seq, KV_HEADS, HEAD_DIM) }

    #[test]
    fn test_pi_zero_profile_constants() {
        assert_eq!(PiZeroProfile::WINDOW, 64);
        assert_eq!(PiZeroProfile::BLOCK_SIZE, 32);
        assert_eq!(PiZeroProfile::TILE_SIZE, 16);
        assert!(PiZeroProfile::SORT_CANDIDATES);
        assert!(!PiZeroProfile::USE_RAYON);
        assert_eq!(PiZeroProfile::MAX_MODEL_BYTES, 320 * 1024 * 1024);
        assert_eq!(PiZeroProfile::MAX_KV_BUDGET_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn test_prefill_and_decode() {
        let mut engine = make_engine(128);
        let _out = engine.prefill(&zeros(4), &zeros(4), &zeros(4))
            .expect("prefill should succeed");
        let _out2 = engine.decode_step(&zeros(1), &zeros(1), &zeros(1))
            .expect("decode_step should succeed");
        assert!(engine.cache_len() > 0);
        assert!(!engine.is_cache_full());
    }

    #[test]
    fn test_cache_full_returns_error() {
        let mut engine = make_engine(4);
        // Prefill fills the cache to max_seq.
        let _ = engine.prefill(&zeros(4), &zeros(4), &zeros(4));
        assert!(engine.is_cache_full(), "cache should be full after prefill to max_seq");
        let result = engine.decode_step(&zeros(1), &zeros(1), &zeros(1));
        assert!(
            matches!(result, Err(SeedLlmError::CacheFull)),
            "expected CacheFull, got: {:?}", result
        );
        engine.reset();
        assert_eq!(engine.cache_len(), 0);
        assert!(!engine.is_cache_full());
    }

    #[test]
    fn test_model_too_large_error() {
        // Cap is device-aware now (`max_model_bytes_for_current_device`).
        // On the dev machine this resolves to `DeviceClass::Other` (1 GB);
        // on Pi Zero 2 W it's 320 MB; on v0-appliance it's 4 GB. Pick
        // values that are above the largest realistic cap and below the
        // smallest so the assertion holds across all classes.
        let engine = make_engine(64);
        let too_big = 5u64 * 1024 * 1024 * 1024; // 5 GB — over every class
        let fits    = 50u64 * 1024 * 1024;       // 50 MB — under every class
        assert!(matches!(engine.validate_model_size(too_big), Err(SeedLlmError::ModelTooLarge { .. })));
        assert!(engine.validate_model_size(fits).is_ok());
    }

    #[test]
    fn test_device_class_caps_are_ordered_correctly() {
        // Pi Zero ≤ Other ≤ v0-appliance — the install-time gate in the
        // agent relies on this ordering (smaller device → tighter cap).
        let pz = DeviceClass::PiZero2W.max_model_bytes();
        let oth = DeviceClass::Other.max_model_bytes();
        let v0 = DeviceClass::V0Appliance.max_model_bytes();
        assert!(pz < oth, "Pi Zero cap ({pz}) must be smaller than Other ({oth})");
        assert!(oth < v0, "Other cap ({oth}) must be smaller than v0-appliance ({v0})");
    }

    #[test]
    fn test_device_class_caps_match_documented_envelope() {
        // Documented values in ADR-094 / ADR-095 — guard against drift.
        assert_eq!(DeviceClass::PiZero2W.max_model_bytes(), 320 * 1024 * 1024);
        assert_eq!(DeviceClass::V0Appliance.max_model_bytes(), 4u64 * 1024 * 1024 * 1024);
        assert_eq!(DeviceClass::Other.max_model_bytes(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_qwen_q4_k_m_fits_v0_appliance_but_not_pi_zero() {
        // The whole reason device-aware caps exist. Qwen2.5-0.5B-Instruct
        // Q4_K_M is 469 MB — fits v0-appliance, must NOT fit Pi Zero.
        let qwen_bytes: u64 = 469 * 1024 * 1024;
        assert!(qwen_bytes > DeviceClass::PiZero2W.max_model_bytes(),
            "qwen must be rejected on Pi Zero");
        assert!(qwen_bytes < DeviceClass::V0Appliance.max_model_bytes(),
            "qwen must fit on v0-appliance");
    }
}
