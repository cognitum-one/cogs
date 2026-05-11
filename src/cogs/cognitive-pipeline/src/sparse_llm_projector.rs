// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE, multi-layer loading, and mesh delta-sync land as next-layer
// commits per ADR-095. Suppress the corresponding lints until then.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Q/K/V projection and FFN layer weights for sparse-LLM forward pass.
//!
//! Two representations are provided:
//!  - `LayerWeights`: f32 dequantized (legacy, high memory — ~13.5 MB/layer)
//!  - `LayerWeightsRaw`: raw Q4_K_M bytes (~1.9 MB/layer); dequant on demand
//!
//! On Pi Zero 2 W (512 MB), always use `LayerWeightsRaw` — 30 layers ≈ 57 MB
//! vs 405 MB for f32, enabling full attention through all layers.

use std::collections::HashMap;
use std::io::Seek;

use crate::sparse_llm_weights::{load_tensor_raw, matvec, matvec_raw, dequant_raw, GgufTensor, GgufTensorInfo};

// ---------------------------------------------------------------------------
// Layer weight names (SmolLM2 / LLaMA-style GGUF naming)
// ---------------------------------------------------------------------------

/// Return the expected GGUF tensor name for a transformer layer projection.
/// SmolLM2 uses the pattern: `blk.{layer}.{component}.weight`
fn layer_tensor(layer: usize, component: &str) -> String {
    format!("blk.{}.{}.weight", layer, component)
}

// ---------------------------------------------------------------------------
// Single transformer layer weights
// ---------------------------------------------------------------------------

/// Dequantized weights for one transformer layer.
pub struct LayerWeights {
    /// Q projection: [hidden_size × (num_heads × head_dim)]
    pub wq: Vec<f32>,
    /// K projection: [hidden_size × (num_kv_heads × head_dim)]
    pub wk: Vec<f32>,
    /// V projection: [hidden_size × (num_kv_heads × head_dim)]
    pub wv: Vec<f32>,
    /// O projection (attention output): [(num_heads × head_dim) × hidden_size]
    pub wo: Vec<f32>,
    /// FFN gate projection (SwiGLU): [hidden_size × ffn_dim]
    pub wg: Vec<f32>,
    /// FFN up projection: [hidden_size × ffn_dim]
    pub wu: Vec<f32>,
    /// FFN down projection: [ffn_dim × hidden_size]
    pub wd: Vec<f32>,
    /// Attention norm weight: [hidden_size]
    pub attn_norm: Vec<f32>,
    /// FFN norm weight: [hidden_size]
    pub ffn_norm: Vec<f32>,
}

impl LayerWeights {
    /// Compute Q, K, V tensors from a hidden state vector.
    /// Returns (q, k, v) as flat vectors, each [dim] long.
    pub fn qkv(&self, x: &[f32], hidden: usize, kv_dim: usize, q_dim: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let q = matvec(&self.wq, x, q_dim, hidden);
        let k = matvec(&self.wk, x, kv_dim, hidden);
        let v = matvec(&self.wv, x, kv_dim, hidden);
        (q, k, v)
    }

    /// Apply output projection to attention result.
    pub fn project_attn_out(&self, attn: &[f32], hidden: usize, q_dim: usize) -> Vec<f32> {
        matvec(&self.wo, attn, hidden, q_dim)
    }

    /// SwiGLU FFN: gate ⊗ up, then down projection.
    pub fn ffn_swiglu(&self, x: &[f32], hidden: usize, ffn_dim: usize) -> Vec<f32> {
        let gate_pre = matvec(&self.wg, x, ffn_dim, hidden);
        let up       = matvec(&self.wu, x, ffn_dim, hidden);
        // SiLU(gate) * up
        let gated: Vec<f32> = gate_pre.iter().zip(up.iter())
            .map(|(&g, &u)| (g / (1.0 + (-g).exp())) * u)
            .collect();
        matvec(&self.wd, &gated, hidden, ffn_dim)
    }
}

// ---------------------------------------------------------------------------
// Norm-only layer weights (for output norm before lm_head)
// ---------------------------------------------------------------------------

pub struct OutputNorm {
    pub weight: Vec<f32>,
}

// ---------------------------------------------------------------------------
// Loader: read layer weights for a single layer
// ---------------------------------------------------------------------------

/// Load one transformer layer's weights from an open GGUF file.
///
/// `by_name` is a pre-built name → GgufTensorInfo map.
/// `data_block_start` is the aligned byte offset for the tensor data section.
pub fn load_layer<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    layer: usize,
) -> Result<LayerWeights, String> {
    let mut load = |name: &str| -> Result<Vec<f32>, String> {
        let info = by_name.get(name)
            .ok_or_else(|| format!("missing tensor: {}", name))?;
        let raw = load_tensor_raw(reader, info, data_block_start)
            .map_err(|e| format!("{}: {}", name, e))?;
        Ok(GgufTensor { info: (*info).clone(), raw }.dequant_f32())
    };

    // Norm weights (always F32 in SmolLM2).
    let attn_norm = load(&layer_tensor(layer, "attn_norm"))?;
    let ffn_norm  = load(&layer_tensor(layer, "ffn_norm"))?;

    // Attention projections.
    let wq = load(&layer_tensor(layer, "attn_q"))?;
    let wk = load(&layer_tensor(layer, "attn_k"))?;
    let wv = load(&layer_tensor(layer, "attn_v"))?;
    let wo = load(&layer_tensor(layer, "attn_output"))?;

    // FFN projections.
    let wg = load(&layer_tensor(layer, "ffn_gate"))?;
    let wu = load(&layer_tensor(layer, "ffn_up"))?;
    let wd = load(&layer_tensor(layer, "ffn_down"))?;

    Ok(LayerWeights { wq, wk, wv, wo, wg, wu, wd, attn_norm, ffn_norm })
}

/// Load all transformer layer weights for a model.
/// On error for any individual layer, returns a partial result up to that layer.
pub fn load_all_layers<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    num_layers: usize,
) -> Vec<LayerWeights> {
    let mut layers = Vec::with_capacity(num_layers);
    for i in 0..num_layers {
        match load_layer(reader, by_name, data_block_start, i) {
            Ok(lw) => layers.push(lw),
            Err(_) => break,  // stop on first missing layer
        }
    }
    layers
}

/// Load the output (final) norm weight.
pub fn load_output_norm<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
) -> Option<OutputNorm> {
    let candidates = ["output_norm.weight", "model.norm.weight", "norm.weight"];
    for name in &candidates {
        if let Some(info) = by_name.get(name) {
            if let Ok(raw) = load_tensor_raw(reader, info, data_block_start) {
                let weight = GgufTensor { info: (*info).clone(), raw }.dequant_f32();
                return Some(OutputNorm { weight });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Compact (quantized) layer weights for memory-constrained devices
// ---------------------------------------------------------------------------

/// A single weight tensor stored in its original GGUF quantization (raw bytes).
/// Dequantization happens lazily at matvec time, keeping peak heap usage low.
pub struct RawWeight {
    pub data: Vec<u8>,
    pub ggml_type: u32,
    /// Number of output units (rows after matvec).
    pub rows: usize,
    /// Number of input units (columns / length of x).
    pub cols: usize,
}

impl RawWeight {
    /// Dequantize and multiply: y[rows] = W[rows,cols] @ x[cols].
    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        matvec_raw(&self.data, self.ggml_type, x, self.rows, self.cols)
    }
}

/// Compact representation of one transformer layer's weights.
/// All projection matrices stay in quantized form; norms are always f32.
/// Memory cost per layer for SmolLM2-135M Q4_K_M: ~1.9 MB (vs ~13.5 MB f32).
pub struct LayerWeightsRaw {
    pub wq: RawWeight,
    pub wk: RawWeight,
    pub wv: RawWeight,
    pub wo: RawWeight,
    pub wg: RawWeight,
    pub wu: RawWeight,
    pub wd: RawWeight,
    pub attn_norm: Vec<f32>,
    pub ffn_norm: Vec<f32>,
}

impl LayerWeightsRaw {
    pub fn qkv(&self, x: &[f32]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        (self.wq.matvec(x), self.wk.matvec(x), self.wv.matvec(x))
    }

    pub fn project_attn_out(&self, attn: &[f32]) -> Vec<f32> {
        self.wo.matvec(attn)
    }

    pub fn ffn_swiglu(&self, x: &[f32], _ffn_dim: usize) -> Vec<f32> {
        let gate_pre = self.wg.matvec(x);
        let up = self.wu.matvec(x);
        let gated: Vec<f32> = gate_pre.iter().zip(up.iter())
            .map(|(&g, &u)| (g / (1.0 + (-g).exp())) * u)
            .collect();
        self.wd.matvec(&gated)
    }
}

fn load_raw_weight<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    name: &str,
) -> Result<RawWeight, String> {
    let info = by_name.get(name).ok_or_else(|| format!("missing tensor: {}", name))?;
    let raw = load_tensor_raw(reader, info, data_block_start)
        .map_err(|e| format!("{}: {}", name, e))?;
    // GGUF stores shapes in reverse: shape[0]=cols(in_dim), shape[1]=rows(out_dim).
    let (rows, cols) = if info.n_dims >= 2 {
        (info.shape[1] as usize, info.shape[0] as usize)
    } else {
        (1, info.shape[0] as usize)
    };
    Ok(RawWeight { data: raw, ggml_type: info.ggml_type, rows, cols })
}

fn load_norm_weight<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    name: &str,
) -> Result<Vec<f32>, String> {
    let info = by_name.get(name).ok_or_else(|| format!("missing tensor: {}", name))?;
    let raw = load_tensor_raw(reader, info, data_block_start)
        .map_err(|e| format!("{}: {}", name, e))?;
    let n = info.num_elems() as usize;
    Ok(dequant_raw(&raw, info.ggml_type, n))
}

/// Load one transformer layer's weights as raw quantized bytes (no f32 expansion).
pub fn load_layer_raw<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    layer: usize,
) -> Result<LayerWeightsRaw, String> {
    let n = |s: &str| layer_tensor(layer, s);
    Ok(LayerWeightsRaw {
        attn_norm: load_norm_weight(reader, by_name, data_block_start, &n("attn_norm"))?,
        wq: load_raw_weight(reader, by_name, data_block_start, &n("attn_q"))?,
        wk: load_raw_weight(reader, by_name, data_block_start, &n("attn_k"))?,
        wv: load_raw_weight(reader, by_name, data_block_start, &n("attn_v"))?,
        wo: load_raw_weight(reader, by_name, data_block_start, &n("attn_output"))?,
        ffn_norm: load_norm_weight(reader, by_name, data_block_start, &n("ffn_norm"))?,
        wg: load_raw_weight(reader, by_name, data_block_start, &n("ffn_gate"))?,
        wu: load_raw_weight(reader, by_name, data_block_start, &n("ffn_up"))?,
        wd: load_raw_weight(reader, by_name, data_block_start, &n("ffn_down"))?,
    })
}

/// Load all transformer layers as compact raw weights.
/// On error for any layer, returns a partial result up to that layer.
pub fn load_all_layers_raw<R: std::io::Read + Seek>(
    reader: &mut R,
    by_name: &HashMap<&str, &GgufTensorInfo>,
    data_block_start: u64,
    num_layers: usize,
) -> Vec<LayerWeightsRaw> {
    let mut layers = Vec::with_capacity(num_layers);
    for i in 0..num_layers {
        match load_layer_raw(reader, by_name, data_block_start, i) {
            Ok(lw) => layers.push(lw),
            Err(_) => break,
        }
    }
    layers
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_layer() -> LayerWeights {
        let h = 4usize;
        let kv = 2usize;
        let q = 4usize;
        let ffn = 8usize;
        // Identity-like matrices (flat vectors, row-major).
        let eye_hq: Vec<f32> = (0..q * h)
            .map(|i| if i % (h + 1) == 0 { 1.0 } else { 0.0 })
            .collect();
        let eye_hkv: Vec<f32> = (0..kv * h)
            .map(|i| if i % (h + 1) == 0 { 1.0 } else { 0.0 })
            .collect();
        let eye_qh: Vec<f32> = (0..h * q)
            .map(|i| if i % (q + 1) == 0 { 1.0 } else { 0.0 })
            .collect();
        LayerWeights {
            wq: eye_hq.clone(),
            wk: eye_hkv.clone(),
            wv: eye_hkv,
            wo: eye_qh,
            wg: vec![0.0; ffn * h],
            wu: vec![0.0; ffn * h],
            wd: vec![0.0; h * ffn],
            attn_norm: vec![1.0; h],
            ffn_norm: vec![1.0; h],
        }
    }

    #[test]
    fn test_ffn_swiglu_zero_weights_gives_zeros() {
        let lw = mock_layer();
        let x = vec![1.0f32; 4];
        let out = lw.ffn_swiglu(&x, 4, 8);
        assert_eq!(out.len(), 4);
        // gate and up are zero → gated is zero → down gives zeros
        for v in &out { assert!(v.abs() < 1e-6, "expected 0, got {}", v); }
    }

    #[test]
    fn test_qkv_shape() {
        let lw = mock_layer();
        let x = vec![1.0f32, 0.0, 0.0, 0.0];
        let (q, k, v) = lw.qkv(&x, 4, 2, 4);
        assert_eq!(q.len(), 4);
        assert_eq!(k.len(), 2);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_layer_tensor_name() {
        assert_eq!(layer_tensor(0, "attn_q"), "blk.0.attn_q.weight");
        assert_eq!(layer_tensor(29, "ffn_down"), "blk.29.ffn_down.weight");
    }
}
