// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Autoregressive LLM inference runner for SmolLM2-135M on Pi Zero 2 W.
//!
//! Provides `SparseLlmRunner`: GGUF header parsing, SmolLM2Config defaults,
//! `PiZeroInferenceEngine` wrapper, and a stub generate path used when real
//! GGUF weights have not been downloaded. Real weights are loaded and used
//! by `sparse_llm_loader::generate_with_fallback` (see `sparse_llm_loader.rs`).

use std::collections::HashMap;
use std::io::{Read as IoRead, Seek};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the sparse LLM runner.
#[derive(Debug)]
pub enum RunnerError {
    Io(std::io::Error),
    InvalidGguf(String),
    TokenizerError(String),
    InferenceError(String),
    ModelTooLarge { size_bytes: u64, budget_bytes: u64 },
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::Io(e) => write!(f, "I/O error: {}", e),
            RunnerError::InvalidGguf(m) => write!(f, "invalid GGUF: {}", m),
            RunnerError::TokenizerError(m) => write!(f, "tokenizer error: {}", m),
            RunnerError::InferenceError(m) => write!(f, "inference error: {}", m),
            RunnerError::ModelTooLarge { size_bytes, budget_bytes } => {
                write!(f, "model too large: {} bytes > budget {} bytes", size_bytes, budget_bytes)
            }
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<std::io::Error> for RunnerError {
    fn from(e: std::io::Error) -> Self {
        RunnerError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// GGUF value enum
// ---------------------------------------------------------------------------

/// A typed value read from a GGUF metadata key-value pair.
#[derive(Clone)]
pub enum GgufValue {
    Str(String),
    U32(u32),
    F32(f32),
    Bool(bool),
    Array(Vec<GgufValue>),
}

// ---------------------------------------------------------------------------
// GGUF header
// ---------------------------------------------------------------------------

/// Minimal GGUF v3 header reader. Reads magic, version, tensor count, and
/// key-value metadata. Tensor data is loaded separately via `sparse_llm_loader`.
pub struct GgufHeader {
    pub version: u32,
    pub tensor_count: u64,
    pub metadata: HashMap<String, GgufValue>,
    /// File offset immediately after the last KV pair — where tensor_info section begins.
    pub post_kv_file_offset: u64,
}

// Helper: read exact bytes from a reader.
fn read_bytes<R: IoRead>(r: &mut R, n: usize) -> Result<Vec<u8>, RunnerError> {
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).map_err(RunnerError::Io)?;
    Ok(buf)
}

fn read_u32_le<R: IoRead>(r: &mut R) -> Result<u32, RunnerError> {
    let b = read_bytes(r, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u64_le<R: IoRead>(r: &mut R) -> Result<u64, RunnerError> {
    let b = read_bytes(r, 8)?;
    Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

fn read_f32_le<R: IoRead>(r: &mut R) -> Result<f32, RunnerError> {
    let b = read_bytes(r, 4)?;
    Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_gguf_string<R: IoRead>(r: &mut R) -> Result<String, RunnerError> {
    let len = read_u64_le(r)? as usize;
    let bytes = read_bytes(r, len)?;
    String::from_utf8(bytes)
        .map_err(|e| RunnerError::InvalidGguf(format!("invalid UTF-8 in string: {}", e)))
}

// GGUF value type constants (from spec):
// 0=UINT8 1=INT8 2=UINT16 3=INT16 4=UINT32 5=INT32 6=FLOAT32
// 7=BOOL 8=STRING 9=ARRAY 10=UINT64 11=INT64 12=FLOAT64
fn read_gguf_value<R: IoRead>(r: &mut R, value_type: u32) -> Result<GgufValue, RunnerError> {
    match value_type {
        0 | 1 => { read_bytes(r, 1)?; Ok(GgufValue::U32(0)) }   // UINT8 / INT8
        2 | 3 => { read_bytes(r, 2)?; Ok(GgufValue::U32(0)) }   // UINT16 / INT16
        4 | 5 => Ok(GgufValue::U32(read_u32_le(r)?)),            // UINT32 / INT32
        6     => Ok(GgufValue::F32(read_f32_le(r)?)),            // FLOAT32
        7     => { let b = read_bytes(r, 1)?; Ok(GgufValue::Bool(b[0] != 0)) } // BOOL
        8     => Ok(GgufValue::Str(read_gguf_string(r)?)),       // STRING
        9 => {
            // ARRAY: u32 elem_type, u64 count, then count elements
            let elem_type = read_u32_le(r)?;
            let count = read_u64_le(r)?;
            // Cap in-memory storage at 256 but always consume all bytes.
            let cap = count.min(256) as usize;
            let mut arr = Vec::with_capacity(cap);
            for i in 0..count {
                let v = read_gguf_value(r, elem_type)?;
                if i < 256 { arr.push(v); }
            }
            Ok(GgufValue::Array(arr))
        }
        10 | 11 => { read_bytes(r, 8)?; Ok(GgufValue::U32(0)) } // UINT64 / INT64
        12      => { read_bytes(r, 8)?; Ok(GgufValue::F32(0.0)) }// FLOAT64
        _ => {
            // Unknown type — cannot skip safely; abort parsing.
            Err(RunnerError::InvalidGguf(format!(
                "unknown GGUF value type: {}", value_type
            )))
        }
    }
}

impl GgufHeader {
    /// Read and validate the GGUF header from a file path.
    /// Returns `Err` if magic != `b"GGUF"` or version < 2.
    pub fn from_file(path: &std::path::Path) -> Result<Self, RunnerError> {
        let mut f = std::fs::File::open(path).map_err(RunnerError::Io)?;

        // Magic
        let magic = read_bytes(&mut f, 4)?;
        if &magic != b"GGUF" {
            return Err(RunnerError::InvalidGguf(format!(
                "bad magic: {:?}", magic
            )));
        }

        // Version
        let version = read_u32_le(&mut f)?;
        if version < 2 {
            return Err(RunnerError::InvalidGguf(format!(
                "unsupported GGUF version: {} (need >= 2)", version
            )));
        }

        // Tensor count
        let tensor_count = read_u64_le(&mut f)?;

        // Metadata KV count
        let kv_count = read_u64_le(&mut f)?;

        let mut metadata = HashMap::new();
        for _ in 0..kv_count {
            let key = read_gguf_string(&mut f)?;
            let value_type = read_u32_le(&mut f)?;
            let value = read_gguf_value(&mut f, value_type)?;
            metadata.insert(key, value);
        }

        let post_kv_file_offset = f.stream_position().map_err(RunnerError::Io)?;
        Ok(Self { version, tensor_count, metadata, post_kv_file_offset })
    }
}

// ---------------------------------------------------------------------------
// SmolLM2-135M architecture constants
// ---------------------------------------------------------------------------

/// SmolLM2-135M architecture constants (verified from HuggingFace config.json).
#[derive(Clone)]
pub struct SmolLm2Config {
    pub hidden_size: usize,   // 576
    pub num_heads: usize,     // 9
    pub num_kv_heads: usize,  // 3
    pub head_dim: usize,      // 64
    pub num_layers: usize,    // 30
    pub ffn_dim: usize,       // 1536
    pub vocab_size: usize,    // 49152
    pub max_seq_len: usize,   // 2048
    pub rope_theta: f32,      // 100000.0 (SmolLM2: llama.rope.freq_base = 1e5)
}

impl Default for SmolLm2Config {
    fn default() -> Self {
        Self {
            hidden_size: 576,
            num_heads: 9,
            num_kv_heads: 3,
            head_dim: 64,
            num_layers: 30,
            ffn_dim: 1536,
            vocab_size: 49152,
            max_seq_len: 2048,
            rope_theta: 100000.0, // SmolLM2: llama.rope.freq_base = 1e5
        }
    }
}

impl SmolLm2Config {
    /// Read architecture parameters from a GGUF header's metadata.
    ///
    /// Falls back to SmolLM2-135M defaults for any key that is absent or has
    /// the wrong type — so the function always returns a usable config.
    ///
    /// Standard GGUF keys (llama-arch):
    ///   llama.embedding_length, llama.attention.head_count,
    ///   llama.attention.head_count_kv, llama.block_count,
    ///   llama.feed_forward_length, llama.context_length,
    ///   llama.rope.freq_base, tokenizer.ggml.tokens (vocab size)
    pub fn from_gguf(header: &GgufHeader) -> Self {
        let def = Self::default();
        let get_u32 = |key: &str| -> Option<usize> {
            match header.metadata.get(key)? {
                GgufValue::U32(v) => Some(*v as usize),
                _ => None,
            }
        };
        let get_f32 = |key: &str| -> Option<f32> {
            match header.metadata.get(key)? {
                GgufValue::F32(v) => Some(*v),
                GgufValue::U32(v) => Some(*v as f32), // some models store as uint
                _ => None,
            }
        };
        let get_arr_len = |key: &str| -> Option<usize> {
            match header.metadata.get(key)? {
                GgufValue::Array(a) => Some(a.len()),
                _ => None,
            }
        };

        let hidden_size   = get_u32("llama.embedding_length").unwrap_or(def.hidden_size);
        let num_heads     = get_u32("llama.attention.head_count").unwrap_or(def.num_heads);
        let num_kv_heads  = get_u32("llama.attention.head_count_kv").unwrap_or(def.num_kv_heads);
        let num_layers    = get_u32("llama.block_count").unwrap_or(def.num_layers);
        let ffn_dim       = get_u32("llama.feed_forward_length").unwrap_or(def.ffn_dim);
        let max_seq_len   = get_u32("llama.context_length").unwrap_or(def.max_seq_len);
        let rope_theta    = get_f32("llama.rope.freq_base").unwrap_or(def.rope_theta);
        let vocab_size    = get_arr_len("tokenizer.ggml.tokens")
            .or_else(|| get_u32("llama.vocab_size"))
            .unwrap_or(def.vocab_size);
        // head_dim = hidden_size / num_heads (standard transformer convention).
        let head_dim = if num_heads > 0 { hidden_size / num_heads } else { def.head_dim };

        Self { hidden_size, num_heads, num_kv_heads, head_dim, num_layers, ffn_dim, vocab_size, max_seq_len, rope_theta }
    }
}

// ---------------------------------------------------------------------------
// RoPE encoding
// ---------------------------------------------------------------------------

/// Compute RoPE rotation for a single token at position `pos`.
/// Modifies `x` in-place: (x[2i], x[2i+1]) → rotated pair.
/// `dim` is head_dim, `theta` is rope_theta.
pub fn apply_rope(x: &mut [f32], pos: usize, dim: usize, theta: f32) {
    let half = dim / 2;
    for i in 0..half {
        let freq = 1.0_f32 / theta.powf(2.0 * i as f32 / dim as f32);
        let angle = pos as f32 * freq;
        let (sin_a, cos_a) = angle.sin_cos();
        let x0 = x[2 * i];
        let x1 = x[2 * i + 1];
        x[2 * i]     = x0 * cos_a - x1 * sin_a;
        x[2 * i + 1] = x0 * sin_a + x1 * cos_a;
    }
}

// ---------------------------------------------------------------------------
// RMS norm
// ---------------------------------------------------------------------------

/// RMS normalization: x = x / rms(x) * weight, with `eps` for stability.
pub fn rms_norm(x: &mut [f32], weight: &[f32], eps: f32) {
    let n = x.len();
    let rms = (x.iter().map(|v| v * v).sum::<f32>() / n as f32 + eps).sqrt();
    for (xi, wi) in x.iter_mut().zip(weight.iter()) {
        *xi = (*xi / rms) * wi;
    }
}

// ---------------------------------------------------------------------------
// SwiGLU FFN (stub — all-ones weights)
// ---------------------------------------------------------------------------

/// Single FFN step: gate = silu(W_gate @ x), out = gate * (W_up @ x), return W_down @ out.
/// Stub: W_gate, W_up, W_down are identity (all-ones) matrices scaled by 1/hidden.
/// Will be replaced with real weight loading in a future iteration.
pub fn ffn_swiglu_stub(x: &[f32], hidden: usize, ffn_dim: usize) -> Vec<f32> {
    let scale = 1.0 / hidden as f32;

    // gate = sum(x) * scale for each ffn neuron — stub: same value broadcast
    let sum_x: f32 = x.iter().sum::<f32>() * scale;

    // SiLU: gate_act = gate * sigmoid(gate)
    let silu = |v: f32| -> f32 { v / (1.0 + (-v).exp()) };
    let gate_act = silu(sum_x);

    // up = same stub sum
    let up = sum_x;
    let gated = gate_act * up;

    // W_down @ (gate * up): same value broadcast to hidden dims
    vec![gated * scale; hidden.min(ffn_dim)]
        .into_iter()
        .chain(std::iter::repeat(0.0))
        .take(hidden)
        .collect()
}

// ---------------------------------------------------------------------------
// Embedding lookup (stub)
// ---------------------------------------------------------------------------

/// Token embedding lookup. Stub: returns a deterministic pseudo-embedding
/// based on token_id (sin of token_id * i * 0.01) until real weights load.
pub fn embed_token(token_id: u32, hidden_size: usize) -> Vec<f32> {
    (0..hidden_size)
        .map(|i| (token_id as f32 * (i + 1) as f32 * 0.01).sin() * 0.1)
        .collect()
}

// ---------------------------------------------------------------------------
// Greedy sampler
// ---------------------------------------------------------------------------

/// Greedy argmax over logits. Returns the token_id with the highest logit.
pub fn greedy_sample(logits: &[f32]) -> u32 {
    logits
        .iter()
        .enumerate()
        .fold(
            (0usize, f32::NEG_INFINITY),
            |(best_i, best_v), (i, &v)| {
                if v > best_v { (i, v) } else { (best_i, best_v) }
            },
        )
        .0 as u32
}

// ---------------------------------------------------------------------------
// Temperature + top-k sampling
// ---------------------------------------------------------------------------

/// Minimal xorshift64 PRNG — no external dependency needed.
struct Xorshift64(u64);

impl Xorshift64 {
    /// Create an RNG. `seed = None` → time-based (non-deterministic).
    /// `seed = Some(s)` → fully deterministic: same seed → same sequence.
    fn new(seed: Option<u64>) -> Self {
        let s = seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| {
                    let ns = d.subsec_nanos() as u64;
                    let s  = d.as_secs().wrapping_mul(6364136223846793005);
                    ns ^ s
                })
                .unwrap_or(0xdeadbeef_cafebabe_u64)
        });
        Self(s | 1) // ensure non-zero
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }
}

/// Sample a token from `logits` with temperature, top-k, top-p, min-p, and repetition penalty.
///
/// Parameters:
/// - `temperature ≤ 0` → greedy argmax (repetition penalty still applies)
/// - `top_k = 0` → no top-k filter (consider all tokens)
/// - `top_p = 0` → no nucleus filter; `top_p ∈ (0,1)` → keep smallest cumsum set with prob ≥ top_p
/// - `min_p = 0` → disabled; `min_p ∈ (0,1)` → remove tokens whose prob < min_p × max_token_prob
/// - `repetition_penalty = 1.0` → no penalty; `> 1.0` → discourages repeating seen tokens
/// - `seen_tokens` → prompt + previously generated tokens used for repetition penalty
/// - `seed` → `None` = time-seeded (non-deterministic); `Some(s)` = fully reproducible
///
/// Filter order: top_k → softmax → min_p → top_p → CDF sample.
/// Uses a xorshift64 PRNG — no external crate needed.
pub fn temperature_sample(
    logits: &[f32],
    temperature: f32,
    top_k: usize,
    top_p: f32,
    min_p: f32,
    repetition_penalty: f32,
    seen_tokens: &[u32],
    seed: Option<u64>,
) -> u32 {
    // 1. Apply repetition penalty: push logits of seen tokens away from zero.
    let apply_penalty = repetition_penalty != 1.0 && !seen_tokens.is_empty();
    let penalized: Option<Vec<f32>> = if apply_penalty {
        let mut adj = logits.to_vec();
        for &tok_id in seen_tokens {
            if let Some(l) = adj.get_mut(tok_id as usize) {
                if *l > 0.0 { *l /= repetition_penalty; } else { *l *= repetition_penalty; }
            }
        }
        Some(adj)
    } else {
        None
    };
    let logits: &[f32] = penalized.as_deref().unwrap_or(logits);

    // 2. Greedy short-circuit (after penalty).
    if temperature < 1e-6 {
        return greedy_sample(logits);
    }

    // 3. Sort (index, logit) pairs descending.
    let mut pairs: Vec<(usize, f32)> = logits.iter().copied().enumerate().collect();
    pairs.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // 4. Top-k filter.
    let k = if top_k == 0 || top_k >= pairs.len() { pairs.len() } else { top_k };
    pairs.truncate(k);

    // 5. Softmax with temperature.
    let max_l = pairs[0].1;
    let mut probs: Vec<f32> = pairs.iter()
        .map(|&(_, v)| ((v - max_l) / temperature).exp())
        .collect();
    let sum: f32 = probs.iter().sum();
    for p in &mut probs { *p /= sum; }

    // 5.5. Min-p filter: keep only tokens whose prob ≥ min_p × max_token_prob.
    //      Pairs are sorted descending, so probs[0] is the maximum.
    //      Always keep at least 1 token.
    if min_p > 0.0 && min_p < 1.0 {
        let threshold = min_p * probs[0];
        let cutoff = probs.partition_point(|&p| p >= threshold).max(1);
        pairs.truncate(cutoff);
        probs.truncate(cutoff);
        let s: f32 = probs.iter().sum();
        for p in &mut probs { *p /= s; }
    }

    // 6. Top-p (nucleus) filter: keep fewest tokens whose cumulative prob ≥ top_p.
    if top_p > 0.0 && top_p < 1.0 {
        let mut cumsum = 0.0f32;
        let mut cutoff = probs.len();
        for (i, &p) in probs.iter().enumerate() {
            cumsum += p;
            if cumsum >= top_p {
                cutoff = i + 1;
                break;
            }
        }
        pairs.truncate(cutoff);
        probs.truncate(cutoff);
        let s: f32 = probs.iter().sum();
        for p in &mut probs { *p /= s; }
    }

    // 7. CDF sample.
    let mut rng = Xorshift64::new(seed);
    let r = rng.next_f32();
    let mut cdf = 0.0f32;
    for (idx, &p) in probs.iter().enumerate() {
        cdf += p;
        if r < cdf {
            return pairs[idx].0 as u32;
        }
    }
    pairs.last().map(|&(i, _)| i as u32).unwrap_or(0)
}

/// Return the top-`k` (token_id, ln_prob) pairs from a logit vector.
///
/// Applies softmax over the full vocabulary (no temperature, no penalty),
/// sorts descending by log-probability, and truncates to `k`. Returns an
/// empty Vec when `k == 0` or `logits` is empty.
pub fn compute_top_logprobs(logits: &[f32], k: usize) -> Vec<(u32, f32)> {
    if k == 0 || logits.is_empty() {
        return Vec::new();
    }
    let max_l = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|&v| (v - max_l).exp()).collect();
    let sum: f32 = exps.iter().sum::<f32>().max(1e-30);
    let mut pairs: Vec<(u32, f32)> = exps.iter().enumerate()
        .map(|(i, &e)| (i as u32, (e / sum).ln()))
        .collect();
    pairs.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    pairs.truncate(k);
    pairs
}

// ---------------------------------------------------------------------------
// Runner struct
// ---------------------------------------------------------------------------

/// Full autoregressive inference runner for SmolLM2-135M.
pub struct SparseLlmRunner {
    pub(crate) config: SmolLm2Config,
    pub(crate) attention: crate::sparse_llm::PiZeroInferenceEngine,
    pub model_path: PathBuf,
    pub gguf_header: Option<GgufHeader>,
    /// Per-layer KV cache — keys, indexed [layer][pos × kv_dim].
    pub kv_cache_k: Vec<Vec<f32>>,
    /// Per-layer KV cache — values, indexed [layer][pos × kv_dim].
    pub kv_cache_v: Vec<Vec<f32>>,
    /// 3-tier quantized KV cache (Hot FP32 / Warm INT8 / Cold 4-bit).
    /// Populated during autoregressive decode; cleared at request start.
    pub kv_quant: crate::sparse_llm_kv_quant::QuantKvCache,
}

impl SparseLlmRunner {
    /// Create runner. Does NOT open the model file — call `load_header()` to validate.
    pub fn new(model_path: PathBuf) -> Result<Self, RunnerError> {
        let cfg = SmolLm2Config::default();
        let kv_dim = cfg.num_kv_heads * cfg.head_dim;
        let n_layers = cfg.num_layers;
        let attn_cfg = crate::sparse_llm::PiZeroInferenceConfig {
            model_id: "smollm2-135m".into(),
            max_seq: 256,
            kv_budget_bytes: 32 * 1024 * 1024,
        };
        let attention = crate::sparse_llm::PiZeroInferenceEngine::new(
            attn_cfg,
            cfg.num_kv_heads,
            cfg.head_dim,
        )
        .map_err(|e| RunnerError::InferenceError(e.to_string()))?;
        Ok(Self {
            config: cfg,
            attention,
            model_path,
            gguf_header: None,
            kv_cache_k: Vec::new(),
            kv_cache_v: Vec::new(),
            kv_quant: crate::sparse_llm_kv_quant::QuantKvCache::new(n_layers, kv_dim),
        })
    }

    /// Read and cache the GGUF header. Validates magic + version.
    /// Also updates `self.config` from the GGUF metadata so all downstream
    /// inference uses the actual model architecture, not hardcoded defaults.
    pub fn load_header(&mut self) -> Result<(), RunnerError> {
        let header = GgufHeader::from_file(&self.model_path)?;
        self.config = SmolLm2Config::from_gguf(&header);
        self.gguf_header = Some(header);
        Ok(())
    }

    /// Generate `max_tokens` tokens from `token_ids` prompt using stub weights.
    /// Returns generated token IDs (not including prompt).
    pub fn generate_stub(
        &mut self,
        token_ids: &[u32],
        max_tokens: usize,
    ) -> Result<Vec<u32>, RunnerError> {
        let hidden = self.config.hidden_size;
        let head_dim = self.config.head_dim;
        let kv_heads = self.config.num_kv_heads;
        let ffn_dim = self.config.ffn_dim;
        let vocab = self.config.vocab_size;
        let theta = self.config.rope_theta;

        // Prefill: accumulate a running-mean hidden state from all prompt tokens.
        let ones = vec![1.0f32; hidden];
        let mut hidden_state = vec![0.0f32; hidden];
        let seq_len = token_ids.len().max(1);

        for (pos, &tid) in token_ids.iter().enumerate() {
            let mut emb = embed_token(tid, hidden);
            apply_rope(&mut emb, pos, head_dim.min(hidden), theta);
            rms_norm(&mut emb, &ones, 1e-5);
            let ffn_out = ffn_swiglu_stub(&emb, hidden, ffn_dim);
            for (h, f) in hidden_state.iter_mut().zip(ffn_out.iter()) {
                *h += f;
            }
        }
        for h in hidden_state.iter_mut() {
            *h /= seq_len as f32;
        }

        use ruvllm_sparse_attention::Tensor3;
        let q_prefill = Tensor3::zeros(seq_len, kv_heads, head_dim);
        let k_prefill = Tensor3::zeros(seq_len, kv_heads, head_dim);
        let v_prefill = Tensor3::zeros(seq_len, kv_heads, head_dim);
        self.attention
            .prefill(&q_prefill, &k_prefill, &v_prefill)
            .map_err(|e| RunnerError::InferenceError(e.to_string()))?;

        // Decode loop.
        let mut output = Vec::with_capacity(max_tokens);
        let mut last_token = token_ids.last().copied().unwrap_or(1);

        for step in 0..max_tokens {
            let pos = seq_len + step;
            let mut emb = embed_token(last_token, hidden);
            apply_rope(&mut emb, pos, head_dim.min(hidden), theta);
            rms_norm(&mut emb, &ones, 1e-5);
            let ffn_out = ffn_swiglu_stub(&emb, hidden, ffn_dim);

            for (h, f) in hidden_state.iter_mut().zip(ffn_out.iter()) {
                *h = (*h + f) * 0.5;
            }

            let q_dec = Tensor3::zeros(1, kv_heads, head_dim);
            let k_dec = Tensor3::zeros(1, kv_heads, head_dim);
            let v_dec = Tensor3::zeros(1, kv_heads, head_dim);
            if !self.attention.is_cache_full() {
                self.attention
                    .decode_step(&q_dec, &k_dec, &v_dec)
                    .map_err(|e| RunnerError::InferenceError(e.to_string()))?;
            }

            let logit_len = vocab.min(hidden);
            let mut logits = vec![0.0f32; logit_len];
            for (i, l) in logits.iter_mut().enumerate() {
                let h = hidden_state[i % hidden];
                let e = emb[i % hidden];
                *l = h * e;
            }

            let next_token = greedy_sample(&logits);
            output.push(next_token);
            last_token = next_token;
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smollm2_config_defaults() {
        let cfg = SmolLm2Config::default();
        assert_eq!(cfg.hidden_size, 576);
        assert_eq!(cfg.num_heads, 9);
        assert_eq!(cfg.num_kv_heads, 3);
        assert_eq!(cfg.head_dim, 64);
        assert_eq!(cfg.num_layers, 30);
        assert_eq!(cfg.ffn_dim, 1536);
        assert_eq!(cfg.vocab_size, 49152);
        assert_eq!(cfg.max_seq_len, 2048);
        assert!((cfg.rope_theta - 100000.0).abs() < 1e-2);
    }

    #[test]
    fn test_config_from_gguf_reads_metadata() {
        let mut meta = HashMap::new();
        meta.insert("llama.embedding_length".into(),      GgufValue::U32(1024));
        meta.insert("llama.attention.head_count".into(),  GgufValue::U32(16));
        meta.insert("llama.attention.head_count_kv".into(), GgufValue::U32(4));
        meta.insert("llama.block_count".into(),           GgufValue::U32(24));
        meta.insert("llama.feed_forward_length".into(),   GgufValue::U32(4096));
        meta.insert("llama.context_length".into(),        GgufValue::U32(4096));
        meta.insert("llama.rope.freq_base".into(),        GgufValue::F32(500000.0));
        meta.insert("tokenizer.ggml.tokens".into(),       GgufValue::Array(vec![GgufValue::U32(0); 32000]));
        let hdr = GgufHeader { version: 3, tensor_count: 0, metadata: meta, post_kv_file_offset: 0 };
        let cfg = SmolLm2Config::from_gguf(&hdr);
        assert_eq!(cfg.hidden_size, 1024);
        assert_eq!(cfg.num_heads, 16);
        assert_eq!(cfg.num_kv_heads, 4);
        assert_eq!(cfg.head_dim, 64); // 1024 / 16
        assert_eq!(cfg.num_layers, 24);
        assert_eq!(cfg.ffn_dim, 4096);
        assert_eq!(cfg.max_seq_len, 4096);
        assert!((cfg.rope_theta - 500000.0).abs() < 1.0);
        assert_eq!(cfg.vocab_size, 32000);
    }

    #[test]
    fn test_config_from_gguf_falls_back_to_defaults() {
        let hdr = GgufHeader { version: 3, tensor_count: 0, metadata: HashMap::new(), post_kv_file_offset: 0 };
        let cfg = SmolLm2Config::from_gguf(&hdr);
        let def = SmolLm2Config::default();
        assert_eq!(cfg.hidden_size, def.hidden_size);
        assert_eq!(cfg.rope_theta.to_bits(), def.rope_theta.to_bits());
    }

    #[test]
    fn test_rope_identity_at_pos_zero() {
        let original = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mut x = original.clone();
        apply_rope(&mut x, 0, 8, 10000.0);
        // At pos=0 all angles are 0 → cos=1, sin=0 → no change.
        for (a, b) in original.iter().zip(x.iter()) {
            assert!((a - b).abs() < 1e-6, "pos=0 RoPE changed value: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_rms_norm_unit_weight() {
        let mut x = vec![1.0f32, 2.0, 3.0, 4.0];
        let weight = vec![1.0f32; 4];
        rms_norm(&mut x, &weight, 1e-5);
        // After normalization, RMS of x should be ≈ 1.0.
        let n = x.len() as f32;
        let rms_after = (x.iter().map(|v| v * v).sum::<f32>() / n).sqrt();
        assert!((rms_after - 1.0).abs() < 1e-4, "RMS after norm: {}", rms_after);
    }

    #[test]
    fn test_greedy_sample_returns_argmax() {
        let logits = vec![0.1f32, 0.9, 0.2];
        assert_eq!(greedy_sample(&logits), 1);
        let logits2 = vec![-1.0f32, -2.0, 0.5, 0.1];
        assert_eq!(greedy_sample(&logits2), 2);
    }

    #[test]
    fn test_temperature_sample_zero_is_greedy() {
        let logits = vec![0.1f32, 5.0, 0.2, 0.3];
        assert_eq!(temperature_sample(&logits, 0.0, 0, 0.0, 0.0, 1.0, &[], None), 1);
    }

    #[test]
    fn test_temperature_sample_top1_is_greedy() {
        let logits = vec![0.1f32, 5.0, 0.2, 0.3];
        assert_eq!(temperature_sample(&logits, 1.0, 1, 0.0, 0.0, 1.0, &[], None), 1);
    }

    #[test]
    fn test_temperature_sample_returns_valid_index() {
        let logits = vec![1.0f32; 100];
        let tok = temperature_sample(&logits, 1.0, 0, 0.0, 0.0, 1.0, &[], None);
        assert!((tok as usize) < 100, "expected index <100, got {}", tok);
    }

    #[test]
    fn test_temperature_sample_top_k_limits_range() {
        let mut logits = vec![-100.0f32; 50];
        logits[3] = 10.0;
        logits[7] = 9.0;
        for _ in 0..20 {
            let tok = temperature_sample(&logits, 1.0, 2, 0.0, 0.0, 1.0, &[], None);
            assert!(tok == 3 || tok == 7, "got {}", tok);
        }
    }

    #[test]
    fn test_top_p_restricts_to_nucleus() {
        // Index 0 has logit 10.0, index 1 has 9.0, rest are -100.
        // With top_p=0.99, after softmax only tokens 0+1 will have enough mass.
        let mut logits = vec![-100.0f32; 20];
        logits[0] = 10.0;
        logits[1] = 9.0;
        for _ in 0..20 {
            let tok = temperature_sample(&logits, 0.5, 0, 0.99, 0.0, 1.0, &[], None);
            assert!(tok == 0 || tok == 1, "top_p=0.99 should only pick tok 0 or 1, got {}", tok);
        }
    }

    #[test]
    fn test_repetition_penalty_suppresses_seen_token() {
        // Logits: index 0=5.0 (would win), index 1=4.9. Penalise index 0 so index 1 wins.
        let logits = vec![5.0f32, 4.9, -10.0, -10.0];
        // penalty=10 divides logit[0]=5.0 by 10 → 0.5, below logit[1]=4.9.
        let tok = temperature_sample(&logits, 0.0, 0, 0.0, 0.0, 10.0, &[0u32], None);
        assert_eq!(tok, 1, "penalty should suppress token 0 so token 1 wins, got {}", tok);
    }

    #[test]
    fn test_temperature_sample_seeded_is_deterministic() {
        let logits = vec![1.0f32; 100];
        let t1 = temperature_sample(&logits, 1.0, 0, 0.0, 0.0, 1.0, &[], Some(42));
        let t2 = temperature_sample(&logits, 1.0, 0, 0.0, 0.0, 1.0, &[], Some(42));
        assert_eq!(t1, t2, "same seed must produce same token");
        // Different seeds should (almost certainly) differ on a 100-token uniform.
        let t3 = temperature_sample(&logits, 1.0, 0, 0.0, 0.0, 1.0, &[], Some(1337));
        // Not a hard assertion (could collide), but log when they do.
        let _ = t3; // used
    }

    #[test]
    fn test_generate_stub_returns_tokens() {
        // new() does NOT open the file — nonexistent path is fine until load_header().
        let mut runner = SparseLlmRunner::new(PathBuf::from("/nonexistent/model.gguf"))
            .expect("SparseLlmRunner::new should succeed without opening the file");
        let tokens = runner
            .generate_stub(&[1u32, 2, 3], 5)
            .expect("generate_stub should succeed with stub weights");
        assert_eq!(tokens.len(), 5, "expected 5 generated tokens, got {}", tokens.len());
    }
}
