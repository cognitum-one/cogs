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
    /// Max model size in bytes (320 MB; leaves 192 MB for OS + agent).
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

    /// Validate a model file fits within the Pi Zero RAM budget.
    pub fn validate_model_size(&self, size_bytes: u64) -> Result<(), SeedLlmError> {
        if size_bytes > PiZeroProfile::MAX_MODEL_BYTES {
            return Err(SeedLlmError::ModelTooLarge {
                size_bytes,
                max_bytes: PiZeroProfile::MAX_MODEL_BYTES,
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
        let engine = make_engine(64);
        let result = engine.validate_model_size(1024 * 1024 * 1024);
        assert!(matches!(result, Err(SeedLlmError::ModelTooLarge { .. })));
        assert!(engine.validate_model_size(100 * 1024 * 1024).is_ok());
    }
}
