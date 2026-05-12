// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Real GGUF weight loading for sparse-LLM COG.
//!
//! Loads embedding table plus all 30 transformer layers from a GGUF file.
//! Layer projections are kept in quantized form (raw bytes) and dequantized
//! on demand per matvec — this costs ~57 MB for 30 layers vs ~405 MB f32,
//! enabling full GQA attention through all layers on Pi Zero 2 W (512 MB).
//!
//! On load failure, callers MUST fall back to `generate_stub`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

use crate::sparse_llm_weights::{
    load_tensor_raw, matvec, read_tensor_infos, GgufTensor, GgufTensorInfo,
};

// ---------------------------------------------------------------------------
// GGUF header offset calculator
// ---------------------------------------------------------------------------

/// Alignment used by GGUF v3 for the tensor data block.
const GGUF_DATA_ALIGNMENT: u64 = 32;

/// Calculate where the tensor data block starts in a GGUF file, given the
/// file offset *immediately after* the last KV metadata value was read.
/// GGUF v3 aligns the data block to 32-byte boundaries.
pub fn gguf_data_block_start(post_header_offset: u64) -> u64 {
    let r = post_header_offset % GGUF_DATA_ALIGNMENT;
    if r == 0 {
        post_header_offset
    } else {
        post_header_offset + (GGUF_DATA_ALIGNMENT - r)
    }
}

// ---------------------------------------------------------------------------
// Loaded weight set
// ---------------------------------------------------------------------------

/// Weights for full autoregressive inference.
/// Embedding table is f32 (lookup-intensive); layer projections stay quantized.
pub struct LoadedWeights {
    /// Token embedding table: [vocab_size × hidden_size] row-major f32.
    pub embed_table: Vec<f32>,
    /// Output projection (lm_head): [vocab_size × hidden_size] row-major f32.
    pub lm_head: Vec<f32>,
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub weight_tied: bool,
    /// Compact (raw-quantized) transformer layer weights — all 30 layers, ~57 MB total.
    pub layers_raw: Vec<crate::sparse_llm_projector::LayerWeightsRaw>,
    /// Final output normalisation applied before lm_head.
    pub output_norm: Option<crate::sparse_llm_projector::OutputNorm>,
}

impl LoadedWeights {
    /// Look up a token embedding by ID. Returns zeros on out-of-bounds.
    pub fn embed(&self, token_id: u32) -> Vec<f32> {
        let id = token_id as usize;
        if id >= self.vocab_size {
            return vec![0.0f32; self.hidden_size];
        }
        let start = id * self.hidden_size;
        self.embed_table[start..start + self.hidden_size].to_vec()
    }

    /// Project hidden state to vocabulary logits: [vocab_size] = lm_head @ hidden.
    /// When weight_tied=true, lm_head is empty — route through embed_table instead.
    pub fn project(&self, hidden: &[f32]) -> Vec<f32> {
        let weights = if self.weight_tied { &self.embed_table } else { &self.lm_head };
        matvec(weights, hidden, self.vocab_size, self.hidden_size)
    }
}

// ---------------------------------------------------------------------------
// Loader error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    MissingTensor(String),
    Parse(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "I/O: {}", e),
            LoadError::MissingTensor(n) => write!(f, "missing tensor: {}", n),
            LoadError::Parse(m) => write!(f, "parse: {}", m),
        }
    }
}

impl std::error::Error for LoadError {}
impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self { LoadError::Io(e) }
}

// ---------------------------------------------------------------------------
// Memory guard
// ---------------------------------------------------------------------------

/// Read MemAvailable from /proc/meminfo (Linux). Returns 0 on failure.
fn available_mem_kb() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/meminfo") else { return 0 };
    for line in s.lines() {
        if line.starts_with("MemAvailable:") {
            if let Some(kb) = line.split_whitespace().nth(1) {
                return kb.parse().unwrap_or(0);
            }
        }
    }
    0
}

// ---------------------------------------------------------------------------
// load_weights: open GGUF, find tensors, dequant to f32
// ---------------------------------------------------------------------------

/// Load embedding + lm_head weights from a GGUF model file.
///
/// On Pi Zero 2 W (512 MB RAM), only embedding + output-norm are loaded.
/// Transformer layer weights are skipped to stay within the memory budget —
/// generate_with_fallback uses stub sparse-attention for the middle layers.
///
/// Returns `Err` if the file is not found, not a GGUF, the required
/// tensors are absent, or < 160 MB is available. Callers fall back to stub.
pub fn load_weights(
    model_path: &Path,
    tensor_count: u64,
    post_kv_offset: u64,
) -> Result<LoadedWeights, LoadError> {
    // Guard: refuse to load if memory is too tight.
    let avail_kb = available_mem_kb();
    if avail_kb > 0 && avail_kb < 160 * 1024 {
        return Err(LoadError::Parse(format!(
            "insufficient memory: {} MB available, need ≥160 MB",
            avail_kb / 1024,
        )));
    }

    let file = File::open(model_path).map_err(LoadError::Io)?;
    let mut reader = BufReader::new(file);

    // Seek to start of tensor_info section (right after KV block).
    reader.seek(SeekFrom::Start(post_kv_offset)).map_err(LoadError::Io)?;

    // Parse all tensor descriptors.
    let infos: Vec<GgufTensorInfo> = read_tensor_infos(&mut reader, tensor_count)
        .map_err(LoadError::Parse)?;

    // Build name → info map.
    let by_name: HashMap<&str, &GgufTensorInfo> =
        infos.iter().map(|i| (i.name.as_str(), i)).collect();

    // Locate embedding tensor. SmolLM2 uses "token_embd.weight".
    let embed_name = find_tensor_name(&by_name, &[
        "token_embd.weight",
        "model.embed_tokens.weight",
        "transformer.wte.weight",
    ])
    .ok_or_else(|| LoadError::MissingTensor("token_embd.weight".into()))?;
    let embed_info = by_name[embed_name];

    // Data block start (aligned).
    let post_tensor_info_offset = reader.stream_position().map_err(LoadError::Io)?;
    let data_block_start = gguf_data_block_start(post_tensor_info_offset);

    // Load and dequant embedding table.
    let embed_raw = load_tensor_raw(&mut reader, embed_info, data_block_start)
        .map_err(LoadError::Parse)?;
    let embed_tensor = GgufTensor { info: embed_info.clone(), raw: embed_raw };
    let embed_table = embed_tensor.dequant_f32();

    // Shape: [vocab_size, hidden_size] (GGUF stores dims in reverse: [hidden, vocab]).
    let (vocab_size, hidden_size) = extract_embed_shape(embed_info)?;

    // Look for a separate lm_head. SmolLM2-135M ties weights → use embed.
    // When weight_tied=true, lm_head is left empty and project() uses embed_table.
    let (lm_head, weight_tied) = if let Some(lm_name) = find_tensor_name(&by_name, &[
        "output.weight",
        "lm_head.weight",
        "model.lm_head.weight",
    ]) {
        let lm_info = by_name[lm_name];
        let lm_raw = load_tensor_raw(&mut reader, lm_info, data_block_start)
            .map_err(LoadError::Parse)?;
        let lm_tensor = GgufTensor { info: lm_info.clone(), raw: lm_raw };
        (lm_tensor.dequant_f32(), false)
    } else {
        // Weight tying — avoid cloning 113 MB; project() routes through embed_table.
        (Vec::new(), true)
    };

    // Load all transformer layers as compact raw (quantized) bytes.
    // 30 layers × ~1.9 MB = ~57 MB — well within Pi Zero 2 W's budget.
    // On-demand dequantization during matvec keeps peak f32 usage to ~3.4 MB.
    let layers_raw = crate::sparse_llm_projector::load_all_layers_raw(
        &mut reader, &by_name, data_block_start, 30,
    );
    let output_norm = crate::sparse_llm_projector::load_output_norm(
        &mut reader, &by_name, data_block_start,
    );

    Ok(LoadedWeights { embed_table, lm_head, vocab_size, hidden_size, weight_tied, layers_raw, output_norm })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_tensor_name<'a>(
    by_name: &HashMap<&'a str, &GgufTensorInfo>,
    candidates: &[&'a str],
) -> Option<&'a str> {
    for &name in candidates {
        if by_name.contains_key(name) {
            return Some(name);
        }
    }
    None
}

/// Extract (vocab_size, hidden_size) from a 2-D embedding tensor info.
/// GGUF stores dims in reverse order: shape=[hidden_size, vocab_size].
fn extract_embed_shape(info: &GgufTensorInfo) -> Result<(usize, usize), LoadError> {
    if info.shape.len() < 2 {
        return Err(LoadError::Parse(format!(
            "{}: expected 2 dims, got {}",
            info.name,
            info.shape.len()
        )));
    }
    // GGUF dim[0] = innermost (hidden), dim[1] = outermost (vocab).
    let hidden = info.shape[0] as usize;
    let vocab  = info.shape[1] as usize;
    Ok((vocab, hidden))
}

// ---------------------------------------------------------------------------
// Integration: generate with real weights, fallback to stub
// ---------------------------------------------------------------------------

/// High-level generate that uses real weights when available.
/// Falls back to `SparseLlmRunner::generate_stub` transparently.
///
/// All 30 transformer layers execute full GQA attention with a per-layer KV cache.
/// Layer weights stay quantized in RAM and are dequantized on demand per matvec
/// (peak overhead: ~3.4 MB per call — the largest FFN gate/up/down projection).
/// Ensure the sampled token always appears in the logprob list.
/// If `tok` is already present, returns `lp` unchanged.
/// Otherwise computes its softmax log-probability from `logits` and appends it.
fn ensure_sampled_in_lp(logits: &[f32], mut lp: Vec<(u32, f32)>, tok: u32) -> Vec<(u32, f32)> {
    if lp.is_empty() || lp.iter().any(|&(id, _)| id == tok) {
        return lp;
    }
    let max_l = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let sum: f32 = logits.iter().map(|&v| (v - max_l).exp()).sum::<f32>().max(1e-30);
    if let Some(&tok_logit) = logits.get(tok as usize) {
        let tok_lp = ((tok_logit - max_l).exp() / sum).ln();
        lp.push((tok, tok_lp));
    }
    lp
}

pub fn generate_with_fallback(
    runner: &mut crate::sparse_llm_runner::SparseLlmRunner,
    weights: Option<&LoadedWeights>,
    token_ids: &[u32],
    max_tokens: usize,
    eos_id: u32,
    temperature: f32,
    top_k: usize,
    top_p: f32,
    min_p: f32,
    repetition_penalty: f32,
    presence_penalty: f32,
    frequency_penalty: f32,
    logit_bias: &std::collections::HashMap<u32, f32>,
    seed: Option<u64>,
    logprobs_k: usize,
    // Called with each token ID immediately after sampling (before EOS check).
    // Pass `&mut |_| {}` for non-streaming callers.
    on_token: &mut dyn FnMut(u32, &[(u32, f32)]),
) -> Result<(Vec<u32>, Vec<Vec<(u32, f32)>>), crate::sparse_llm_runner::RunnerError> {
    use crate::sparse_llm_runner::{temperature_sample, compute_top_logprobs, rms_norm};

    let Some(w) = weights else {
        let stub_ids = runner.generate_stub(token_ids, max_tokens)?;
        let empty_lp = vec![Vec::new(); stub_ids.len()];
        return Ok((stub_ids, empty_lp));
    };

    let hidden    = w.hidden_size;
    let head_dim  = runner.config.head_dim;
    let kv_heads  = runner.config.num_kv_heads;
    let num_heads = runner.config.num_heads;
    let ffn_dim   = runner.config.ffn_dim;
    let theta     = runner.config.rope_theta;
    let q_dim     = num_heads * head_dim;
    let kv_dim    = kv_heads * head_dim;
    let seq_len   = token_ids.len().max(1);

    // Prefill: embed all prompt tokens through all layers; last_x is the
    // hidden state of the final prompt token after all transformer layers.
    let last_x = prefill_all_layers(
        w, token_ids, runner, hidden, head_dim, num_heads, kv_heads,
        q_dim, kv_dim, ffn_dim, theta,
    ).map_err(|e| crate::sparse_llm_runner::RunnerError::InferenceError(e))?;

    // Seed kv_quant with all prefill tokens so decode attention sees the full
    // prompt context. prefill_all_layers stores KV in runner.kv_cache_k/v but
    // decode_token_real uses kv_quant exclusively. Without this seeding step,
    // kv_quant is empty and decode produces garbage output.
    {
        let n_prefill = token_ids.len();
        let n_layers  = runner.kv_cache_k.len();
        for layer_idx in 0..n_layers {
            for t in 0..n_prefill {
                let k = runner.kv_cache_k[layer_idx][t * kv_dim..(t + 1) * kv_dim].to_vec();
                let v = runner.kv_cache_v[layer_idx][t * kv_dim..(t + 1) * kv_dim].to_vec();
                runner.kv_quant.push(layer_idx, &k, &v);
            }
        }
    }

    // Track seen tokens for repetition penalty (prompt first, then generated).
    let mut seen: Vec<u32> = token_ids.to_vec();

    // Frequency map for presence/frequency penalty: token_id → occurrence count.
    // Initialized with prompt tokens; updated after each generated token.
    let use_additive = presence_penalty != 0.0 || frequency_penalty != 0.0;
    let mut freq_map: std::collections::HashMap<u32, usize> = if use_additive {
        let mut m = std::collections::HashMap::new();
        for &t in token_ids { *m.entry(t).or_insert(0) += 1; }
        m
    } else {
        std::collections::HashMap::new()
    };

    // Apply additive OpenAI-style penalties to a logit vector:
    // presence_penalty subtracts from every token seen ≥ once;
    // frequency_penalty subtracts proportionally to occurrence count.
    // Both are applied before temperature_sample (before repetition_penalty).
    let apply_additive = |logits: &[f32], freq: &std::collections::HashMap<u32, usize>| -> Vec<f32> {
        let mut adj = logits.to_vec();
        for (&id, &cnt) in freq {
            if let Some(l) = adj.get_mut(id as usize) {
                *l -= presence_penalty + frequency_penalty * cnt as f32;
            }
        }
        adj
    };

    // Sample the first generated token directly from the prefill hidden state.
    // This avoids re-embedding the last prompt token through decode step 0,
    // which would produce the wrong positional encoding and ignore the
    // already-computed KV cache for the full prompt.
    let (first_token, first_lp) = {
        let mut x_out = last_x.clone();
        if let Some(on) = &w.output_norm {
            rms_norm(&mut x_out, &on.weight, 1e-5);
        }
        let logits_raw = w.project(&x_out);
        let mut logits = if use_additive { apply_additive(&logits_raw, &freq_map) } else { logits_raw };
        for (&id, &bias) in logit_bias {
            if let Some(l) = logits.get_mut(id as usize) { *l += bias; }
        }
        let lp = compute_top_logprobs(&logits, logprobs_k);
        let tok = temperature_sample(&logits, temperature, top_k, top_p, min_p, repetition_penalty, &seen, seed);
        let lp = ensure_sampled_in_lp(&logits, lp, tok);
        (tok, lp)
    };

    let mut output = Vec::with_capacity(max_tokens);
    let mut logprobs_out: Vec<Vec<(u32, f32)>> = Vec::with_capacity(max_tokens);
    // EOS as first token: return empty (nothing to say after EOS).
    if first_token == eos_id {
        return Ok((output, logprobs_out));
    }
    on_token(first_token, &first_lp);
    output.push(first_token);
    logprobs_out.push(first_lp);
    seen.push(first_token);
    if use_additive { *freq_map.entry(first_token).or_insert(0) += 1; }
    let mut last_token = first_token;
    let mut last_x = last_x;
    let decode_deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);

    for step in 1..max_tokens {
        if std::time::Instant::now() > decode_deadline { break; }
        let pos = seq_len + step - 1;
        last_x = decode_all_layers(
            w, last_token, runner, pos, hidden, head_dim,
            num_heads, kv_heads, q_dim, kv_dim, ffn_dim, theta,
        ).map_err(|e| crate::sparse_llm_runner::RunnerError::InferenceError(e))?;

        // Apply output norm → lm_head logits.
        let mut x_out = last_x.clone();
        if let Some(on) = &w.output_norm {
            rms_norm(&mut x_out, &on.weight, 1e-5);
        }
        let logits_raw = w.project(&x_out);
        let mut logits = if use_additive { apply_additive(&logits_raw, &freq_map) } else { logits_raw };
        for (&id, &bias) in logit_bias {
            if let Some(l) = logits.get_mut(id as usize) { *l += bias; }
        }
        let step_lp = compute_top_logprobs(&logits, logprobs_k);
        // Derive a per-step seed so each decode step is independently seeded
        // but the sequence is still fully deterministic given the same base seed.
        let step_seed = seed.map(|s| s.wrapping_add(step as u64 * 0x9e3779b9u64));
        let next_token = temperature_sample(&logits, temperature, top_k, top_p, min_p, repetition_penalty, &seen, step_seed);
        if next_token == eos_id {
            break;
        }
        let step_lp = ensure_sampled_in_lp(&logits, step_lp, next_token);
        on_token(next_token, &step_lp);
        output.push(next_token);
        logprobs_out.push(step_lp);
        seen.push(next_token);
        if use_additive { *freq_map.entry(next_token).or_insert(0) += 1; }
        last_token = next_token;
    }

    Ok((output, logprobs_out))
}

/// Prefill: run all tokens through all layers; return last token's hidden state.
fn prefill_all_layers(
    w: &LoadedWeights,
    token_ids: &[u32],
    runner: &mut crate::sparse_llm_runner::SparseLlmRunner,
    hidden: usize, head_dim: usize, num_heads: usize, kv_heads: usize,
    q_dim: usize, kv_dim: usize, ffn_dim: usize, theta: f32,
) -> Result<Vec<f32>, String> {
    use crate::sparse_llm_runner::rms_norm;
    let seq_len = token_ids.len().max(1);

    // Reset per-layer KV caches for a new sequence.
    runner.kv_cache_k.clear();
    runner.kv_cache_v.clear();
    runner.kv_quant.clear();

    // Initialise per-layer KV caches.
    for _ in 0..w.layers_raw.len() {
        runner.kv_cache_k.push(Vec::new());
        runner.kv_cache_v.push(Vec::new());
    }

    // Embed all prompt tokens.
    let mut xs: Vec<Vec<f32>> = token_ids.iter().map(|&tid| w.embed(tid)).collect();

    // Run all prefill tokens through every layer with full GQA attention.
    for (layer_idx, lw) in w.layers_raw.iter().enumerate() {
        // Collect Q/K/V for all positions in this layer.
        let mut acc_q = vec![0.0f32; seq_len * q_dim];
        let mut acc_k = vec![0.0f32; seq_len * kv_dim];
        let mut acc_v = vec![0.0f32; seq_len * kv_dim];
        for (pos, x) in xs.iter().enumerate() {
            let mut xn = x.clone();
            rms_norm(&mut xn, &lw.attn_norm, 1e-5);
            let (mut q, mut k, v) = lw.qkv(&xn);
            rope_all_heads(&mut q, pos, num_heads, head_dim, theta);
            rope_all_heads(&mut k, pos, kv_heads, head_dim, theta);
            acc_q[pos * q_dim..(pos + 1) * q_dim].copy_from_slice(&q);
            acc_k[pos * kv_dim..(pos + 1) * kv_dim].copy_from_slice(&k);
            acc_v[pos * kv_dim..(pos + 1) * kv_dim].copy_from_slice(&v);
        }
        // Store full prefill KV for decode-time incremental attention.
        runner.kv_cache_k[layer_idx] = acc_k.clone();
        runner.kv_cache_v[layer_idx] = acc_v.clone();

        // Compute each position's output (causal attention: pos attends to [0..=pos]).
        let mut new_xs = vec![vec![0.0f32; hidden]; seq_len];
        for pos in 0..seq_len {
            let q_pos = &acc_q[pos * q_dim..(pos + 1) * q_dim];
            // Causal: only attend to positions 0..=pos.
            let k_causal = &acc_k[..((pos + 1) * kv_dim)];
            let v_causal = &acc_v[..((pos + 1) * kv_dim)];
            let attn_out = gqa_attn_single_query(
                q_pos, k_causal, v_causal,
                pos + 1, num_heads, kv_heads, head_dim,
            );
            let o_proj = lw.project_attn_out(&attn_out);
            let mut x_new = xs[pos].clone();
            for (xi, oi) in x_new.iter_mut().zip(o_proj.iter()) { *xi += oi; }
            // FFN residual.
            let mut xn = x_new.clone();
            rms_norm(&mut xn, &lw.ffn_norm, 1e-5);
            let ffn = lw.ffn_swiglu(&xn, ffn_dim);
            for (xi, fi) in x_new.iter_mut().zip(ffn.iter()) { *xi += fi; }
            new_xs[pos] = x_new;
        }
        xs = new_xs;
    }

    // No transformer layers → pass-through (stub mode: embed → lm_head directly).
    Ok(xs.into_iter().last().unwrap_or_else(|| vec![0.0f32; hidden]))
}

/// Decode one new token through all layers; returns the new hidden state.
fn decode_all_layers(
    w: &LoadedWeights,
    token_id: u32,
    runner: &mut crate::sparse_llm_runner::SparseLlmRunner,
    pos: usize,
    hidden: usize, head_dim: usize, num_heads: usize, kv_heads: usize,
    q_dim: usize, kv_dim: usize, ffn_dim: usize, theta: f32,
) -> Result<Vec<f32>, String> {
    use crate::sparse_llm_runner::rms_norm;
    let mut x = w.embed(token_id);

    if w.layers_raw.is_empty() {
        // Stub: no transformer layers — apply a no-op FFN so x stays meaningful.
        let ffn = crate::sparse_llm_runner::ffn_swiglu_stub(&x, hidden, ffn_dim);
        for (xi, fi) in x.iter_mut().zip(ffn.iter()) { *xi += fi; }
        return Ok(x);
    }

    // Reusable materialization buffers (avoids per-layer allocation in the loop).
    let mut mat_k: Vec<f32> = Vec::new();
    let mut mat_v: Vec<f32> = Vec::new();

    for (layer_idx, lw) in w.layers_raw.iter().enumerate() {
        // Ensure per-layer FP32 KV cache exists (still used for prefill output).
        while runner.kv_cache_k.len() <= layer_idx {
            runner.kv_cache_k.push(Vec::new());
            runner.kv_cache_v.push(Vec::new());
        }

        let residual = x.clone();
        let mut xn = x.clone();
        rms_norm(&mut xn, &lw.attn_norm, 1e-5);
        let (mut q, mut k, v) = lw.qkv(&xn);
        rope_all_heads(&mut q, pos, num_heads, head_dim, theta);
        rope_all_heads(&mut k, pos, kv_heads, head_dim, theta);

        // Push new token into the 3-tier quantized cache and materialize for attention.
        // This keeps old tokens compressed (INT8 / 4-bit) while the hot window stays FP32.
        runner.kv_quant.push(layer_idx, &k, &v);
        runner.kv_quant.materialize(layer_idx, &mut mat_k, &mut mat_v);
        let cached_len = runner.kv_quant.total_len(layer_idx);

        let attn_out = gqa_attn_single_query(
            &q, &mat_k, &mat_v,
            cached_len, num_heads, kv_heads, head_dim,
        );

        x = residual;
        let o_proj = lw.project_attn_out(&attn_out);
        for (xi, oi) in x.iter_mut().zip(o_proj.iter()) { *xi += oi; }
        let mut xn = x.clone();
        rms_norm(&mut xn, &lw.ffn_norm, 1e-5);
        let ffn = lw.ffn_swiglu(&xn, ffn_dim);
        for (xi, fi) in x.iter_mut().zip(ffn.iter()) { *xi += fi; }
    }
    Ok(x)
}

/// Apply RoPE to all heads in a flat [n_heads × head_dim] buffer (Q or K).
fn rope_all_heads(q: &mut [f32], pos: usize, num_heads: usize, head_dim: usize, theta: f32) {
    use crate::sparse_llm_runner::apply_rope;
    for h in 0..num_heads {
        let start = h * head_dim;
        apply_rope(&mut q[start..start + head_dim], pos, head_dim, theta);
    }
}

/// GQA attention for a single query token against a history of K/V.
///
/// `q` is flat [num_heads × head_dim] for the current token.
/// `k_hist` / `v_hist` are flat [cached_len × kv_dim] (kv_dim = kv_heads × head_dim).
/// Returns flat [num_heads × head_dim] output for the current token.
fn gqa_attn_single_query(
    q: &[f32],
    k_hist: &[f32],
    v_hist: &[f32],
    cached_len: usize,
    num_heads: usize,
    kv_heads: usize,
    head_dim: usize,
) -> Vec<f32> {
    let scale = 1.0 / (head_dim as f32).sqrt();
    let repeat = num_heads / kv_heads;
    let kv_dim = kv_heads * head_dim;
    let mut out = vec![0.0f32; num_heads * head_dim];

    for qh in 0..num_heads {
        let kvh = qh / repeat;
        let q_row = &q[qh * head_dim..(qh + 1) * head_dim];

        // One-pass online softmax over K history.
        let mut running_max = f32::NEG_INFINITY;
        let mut denom = 0.0f32;
        let mut acc = vec![0.0f32; head_dim];

        for t in 0..cached_len {
            let k_row = &k_hist[t * kv_dim + kvh * head_dim..t * kv_dim + (kvh + 1) * head_dim];
            let score = q_row.iter().zip(k_row.iter()).map(|(a, b)| a * b).sum::<f32>() * scale;
            if score > running_max {
                let corr = (running_max - score).exp();
                for a in acc.iter_mut() { *a *= corr; }
                denom *= corr;
                running_max = score;
            }
            let w = (score - running_max).exp();
            denom += w;
            let v_row = &v_hist[t * kv_dim + kvh * head_dim..t * kv_dim + (kvh + 1) * head_dim];
            for (a, v) in acc.iter_mut().zip(v_row.iter()) { *a += w * v; }
        }

        if denom > 1e-9 {
            for (a, o) in acc.iter().zip(out[qh * head_dim..(qh + 1) * head_dim].iter_mut()) {
                *o = a / denom;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_block_alignment_already_aligned() {
        assert_eq!(gguf_data_block_start(64), 64);
        assert_eq!(gguf_data_block_start(0), 0);
    }

    #[test]
    fn test_data_block_alignment_partial() {
        // 33 → next 32-byte boundary = 64
        assert_eq!(gguf_data_block_start(33), 64);
        assert_eq!(gguf_data_block_start(1), 32);
    }

    #[test]
    fn test_embed_shape_extraction() {
        let info = GgufTensorInfo {
            name: "token_embd.weight".into(),
            n_dims: 2,
            shape: vec![576, 49152], // [hidden, vocab] in GGUF order
            ggml_type: 0,
            data_offset: 0,
        };
        let (vocab, hidden) = extract_embed_shape(&info).unwrap();
        assert_eq!(vocab, 49152);
        assert_eq!(hidden, 576);
    }

    #[test]
    fn test_loaded_weights_embed_oob() {
        let w = LoadedWeights {
            embed_table: vec![1.0f32; 4],
            lm_head: vec![1.0f32; 4],
            vocab_size: 2,
            hidden_size: 2,
            weight_tied: true,
            layers_raw: Vec::new(),
            output_norm: None,
        };
        let v = w.embed(99);
        assert_eq!(v, vec![0.0f32; 2]);
    }

    #[test]
    fn test_loaded_weights_project() {
        // lm_head = eye(2) → project([a,b]) = [a, b]
        let w = LoadedWeights {
            embed_table: vec![0.0f32; 4],
            lm_head: vec![1.0, 0.0, 0.0, 1.0], // 2×2 identity
            vocab_size: 2,
            hidden_size: 2,
            weight_tied: false,
            layers_raw: Vec::new(),
            output_norm: None,
        };
        let logits = w.project(&[3.0, 7.0]);
        assert!((logits[0] - 3.0).abs() < 1e-5);
        assert!((logits[1] - 7.0).abs() < 1e-5);
    }
}
