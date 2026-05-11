// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE, multi-layer loading, and mesh delta-sync land as next-layer
// commits per ADR-095. Suppress the corresponding lints until then.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! 3-tier KV cache quantization for Pi Zero 2W sparse LLM inference.
//!
//! Tier layout per layer (TurboQuant/PyramidKV inspired):
//!   Hot  (last HOT_WINDOW tokens): FP32  — exact, no overhead
//!   Warm (next WARM_WINDOW tokens): INT8  — ~7-bit effective, 4× smaller
//!   Cold (all older tokens):        4-bit — ~3-bit effective, 8× smaller
//!
//! On a 512-token decode at 30 layers / kv_dim=192:
//!   FP32:   23.6 MB
//!   Tiered: ≈5.2 MB  (4.5× compression)
//!
//! Dequantization is lazy: only done when attention materializes the cache.
//! Quantization is lossless within each tier's precision budget.

pub const HOT_WINDOW: usize = 32;
pub const WARM_WINDOW: usize = 96;
pub const QUANT_GROUP: usize = 32; // elements per scale group

// ── Quantization helpers ─────────────────────────────────────────────

/// Quantize f32 slice to INT8 (u8) per group. Returns (quant_bytes, scales, zeros).
fn quantize_int8(src: &[f32]) -> (Vec<u8>, Vec<f32>, Vec<f32>) {
    let n = src.len();
    let n_groups = n.div_ceil(QUANT_GROUP);
    let mut data = vec![0u8; n];
    let mut scales = vec![0.0f32; n_groups];
    let mut zeros = vec![0.0f32; n_groups];
    for g in 0..n_groups {
        let start = g * QUANT_GROUP;
        let end = (start + QUANT_GROUP).min(n);
        let slice = &src[start..end];
        let min = slice.iter().copied().fold(f32::INFINITY, f32::min);
        let max = slice.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(1e-8);
        let scale = range / 255.0;
        scales[g] = scale;
        zeros[g] = min;
        for (i, &v) in slice.iter().enumerate() {
            data[start + i] = ((v - min) / scale).round().clamp(0.0, 255.0) as u8;
        }
    }
    (data, scales, zeros)
}

fn dequantize_int8(data: &[u8], scales: &[f32], zeros: &[f32], dst: &mut Vec<f32>) {
    let n = data.len();
    dst.resize(n, 0.0);
    for g in 0..scales.len() {
        let start = g * QUANT_GROUP;
        let end = (start + QUANT_GROUP).min(n);
        let s = scales[g];
        let z = zeros[g];
        for i in start..end {
            dst[i] = data[i] as f32 * s + z;
        }
    }
}

/// Quantize f32 slice to 4-bit packed (2 values per byte). Returns (packed, scales, zeros).
fn quantize_4bit(src: &[f32]) -> (Vec<u8>, Vec<f32>, Vec<f32>) {
    let n = src.len();
    let packed_len = n.div_ceil(2);
    let n_groups = n.div_ceil(QUANT_GROUP);
    let mut data = vec![0u8; packed_len];
    let mut scales = vec![0.0f32; n_groups];
    let mut zeros = vec![0.0f32; n_groups];
    for g in 0..n_groups {
        let start = g * QUANT_GROUP;
        let end = (start + QUANT_GROUP).min(n);
        let slice = &src[start..end];
        let min = slice.iter().copied().fold(f32::INFINITY, f32::min);
        let max = slice.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(1e-8);
        let scale = range / 15.0;
        scales[g] = scale;
        zeros[g] = min;
        for (i, &v) in slice.iter().enumerate() {
            let qi = ((v - min) / scale).round().clamp(0.0, 15.0) as u8;
            let idx = start + i;
            if idx % 2 == 0 {
                data[idx / 2] = qi;
            } else {
                data[idx / 2] |= qi << 4;
            }
        }
    }
    (data, scales, zeros)
}

fn dequantize_4bit(data: &[u8], scales: &[f32], zeros: &[f32], n: usize, dst: &mut Vec<f32>) {
    dst.resize(n, 0.0);
    for g in 0..scales.len() {
        let start = g * QUANT_GROUP;
        let end = (start + QUANT_GROUP).min(n);
        let s = scales[g];
        let z = zeros[g];
        for i in start..end {
            let byte = data[i / 2];
            let nibble = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };
            dst[i] = nibble as f32 * s + z;
        }
    }
}

/// Compute per-layer hot window (PyramidKV): early layers keep more FP32 tokens,
/// late layers compress aggressively. Linear interpolation over [HOT_MAX, HOT_MIN].
pub fn pyramid_hot_window(layer_idx: usize, n_layers: usize) -> usize {
    const HOT_MAX: usize = 64; // layer 0: keep 64 tokens FP32
    const HOT_MIN: usize = 16; // last layer: keep 16 tokens FP32
    if n_layers <= 1 { return HOT_MAX; }
    let frac = layer_idx as f32 / (n_layers - 1) as f32;
    (HOT_MAX as f32 - frac * (HOT_MAX - HOT_MIN) as f32).round() as usize
}

// ── Per-layer cache ───────────────────────────────────────────────────

struct LayerKvCache {
    /// Per-layer hot window size (set from PyramidKV budget at construction).
    hot_window: usize,
    warm_window: usize,

    // Hot tier: FP32 ring of last `hot_window` token KV pairs.
    hot_k: Vec<f32>,
    hot_v: Vec<f32>,

    // Warm tier: INT8.
    warm_k: Vec<u8>,
    warm_v: Vec<u8>,
    warm_scale_k: Vec<f32>,
    warm_zero_k: Vec<f32>,
    warm_scale_v: Vec<f32>,
    warm_zero_v: Vec<f32>,
    warm_len: usize,

    // Cold tier: 4-bit packed.
    cold_k: Vec<u8>,
    cold_v: Vec<u8>,
    cold_scale_k: Vec<f32>,
    cold_zero_k: Vec<f32>,
    cold_scale_v: Vec<f32>,
    cold_zero_v: Vec<f32>,
    cold_len: usize,
}

impl LayerKvCache {
    fn new_with_budget(hot_window: usize, warm_window: usize) -> Self {
        Self {
            hot_window,
            warm_window,
            hot_k: Vec::new(), hot_v: Vec::new(),
            warm_k: Vec::new(), warm_v: Vec::new(),
            warm_scale_k: Vec::new(), warm_zero_k: Vec::new(),
            warm_scale_v: Vec::new(), warm_zero_v: Vec::new(),
            warm_len: 0,
            cold_k: Vec::new(), cold_v: Vec::new(),
            cold_scale_k: Vec::new(), cold_zero_k: Vec::new(),
            cold_scale_v: Vec::new(), cold_zero_v: Vec::new(),
            cold_len: 0,
        }
    }

    fn hot_len(&self, kv_dim: usize) -> usize {
        self.hot_k.len() / kv_dim
    }

    fn total_len(&self, kv_dim: usize) -> usize {
        self.cold_len + self.warm_len + self.hot_len(kv_dim)
    }

    fn push(&mut self, k: &[f32], v: &[f32]) {
        let kv_dim = k.len();
        // If hot is full (per-layer adaptive budget), promote oldest to warm.
        if self.hot_k.len() / kv_dim >= self.hot_window {
            let oldest_k = self.hot_k[..kv_dim].to_vec();
            let oldest_v = self.hot_v[..kv_dim].to_vec();
            self.hot_k.drain(..kv_dim);
            self.hot_v.drain(..kv_dim);
            self.push_to_warm(&oldest_k, &oldest_v, kv_dim);
        }
        self.hot_k.extend_from_slice(k);
        self.hot_v.extend_from_slice(v);
    }

    fn push_to_warm(&mut self, k: &[f32], v: &[f32], _kv_dim: usize) {
        // If warm is full (per-layer budget), promote oldest warm token to cold.
        if self.warm_len >= self.warm_window {
            // Oldest warm token = first kv_dim elements of warm after decompressing.
            let kd = k.len();
            let n_groups_per_tok = kd.div_ceil(QUANT_GROUP);
            // Dequant oldest warm K.
            let mut tmp_k = Vec::new();
            dequantize_int8(
                &self.warm_k[..kd],
                &self.warm_scale_k[..n_groups_per_tok],
                &self.warm_zero_k[..n_groups_per_tok],
                &mut tmp_k,
            );
            let mut tmp_v = Vec::new();
            dequantize_int8(
                &self.warm_v[..kd],
                &self.warm_scale_v[..n_groups_per_tok],
                &self.warm_zero_v[..n_groups_per_tok],
                &mut tmp_v,
            );
            // Remove oldest warm entry.
            self.warm_k.drain(..kd);
            self.warm_v.drain(..kd);
            self.warm_scale_k.drain(..n_groups_per_tok);
            self.warm_zero_k.drain(..n_groups_per_tok);
            self.warm_scale_v.drain(..n_groups_per_tok);
            self.warm_zero_v.drain(..n_groups_per_tok);
            self.warm_len -= 1;
            // Push to cold.
            self.push_to_cold(&tmp_k, &tmp_v);
        }
        let (qk, sk, zk) = quantize_int8(k);
        let (qv, sv, zv) = quantize_int8(v);
        self.warm_k.extend_from_slice(&qk);
        self.warm_v.extend_from_slice(&qv);
        self.warm_scale_k.extend_from_slice(&sk);
        self.warm_zero_k.extend_from_slice(&zk);
        self.warm_scale_v.extend_from_slice(&sv);
        self.warm_zero_v.extend_from_slice(&zv);
        self.warm_len += 1;
    }

    fn push_to_cold(&mut self, k: &[f32], v: &[f32]) {
        let (qk, sk, zk) = quantize_4bit(k);
        let (qv, sv, zv) = quantize_4bit(v);
        self.cold_k.extend_from_slice(&qk);
        self.cold_v.extend_from_slice(&qv);
        self.cold_scale_k.extend_from_slice(&sk);
        self.cold_zero_k.extend_from_slice(&zk);
        self.cold_scale_v.extend_from_slice(&sv);
        self.cold_zero_v.extend_from_slice(&zv);
        self.cold_len += 1;
    }

    /// Materialize the full FP32 K and V cache in temporal order (cold→warm→hot).
    fn materialize(&self, kv_dim: usize, k_out: &mut Vec<f32>, v_out: &mut Vec<f32>) {
        let n_groups_per_tok = kv_dim.div_ceil(QUANT_GROUP);
        let packed_per_tok = kv_dim.div_ceil(2);

        k_out.clear();
        v_out.clear();
        let total = self.cold_len + self.warm_len + (self.hot_k.len() / kv_dim);
        k_out.reserve(total * kv_dim);
        v_out.reserve(total * kv_dim);

        // Cold.
        let mut tmp = Vec::with_capacity(kv_dim);
        for t in 0..self.cold_len {
            let byte_start = t * packed_per_tok;
            let g_start = t * n_groups_per_tok;
            dequantize_4bit(
                &self.cold_k[byte_start..byte_start + packed_per_tok],
                &self.cold_scale_k[g_start..g_start + n_groups_per_tok],
                &self.cold_zero_k[g_start..g_start + n_groups_per_tok],
                kv_dim, &mut tmp,
            );
            k_out.extend_from_slice(&tmp);
            dequantize_4bit(
                &self.cold_v[byte_start..byte_start + packed_per_tok],
                &self.cold_scale_v[g_start..g_start + n_groups_per_tok],
                &self.cold_zero_v[g_start..g_start + n_groups_per_tok],
                kv_dim, &mut tmp,
            );
            v_out.extend_from_slice(&tmp);
        }

        // Warm.
        for t in 0..self.warm_len {
            let byte_start = t * kv_dim;
            let g_start = t * n_groups_per_tok;
            dequantize_int8(
                &self.warm_k[byte_start..byte_start + kv_dim],
                &self.warm_scale_k[g_start..g_start + n_groups_per_tok],
                &self.warm_zero_k[g_start..g_start + n_groups_per_tok],
                &mut tmp,
            );
            k_out.extend_from_slice(&tmp);
            dequantize_int8(
                &self.warm_v[byte_start..byte_start + kv_dim],
                &self.warm_scale_v[g_start..g_start + n_groups_per_tok],
                &self.warm_zero_v[g_start..g_start + n_groups_per_tok],
                &mut tmp,
            );
            v_out.extend_from_slice(&tmp);
        }

        // Hot (FP32, exact).
        k_out.extend_from_slice(&self.hot_k);
        v_out.extend_from_slice(&self.hot_v);
    }

    fn ram_bytes(&self, kv_dim: usize) -> usize {
        let hot = self.hot_k.len() * 4 + self.hot_v.len() * 4;
        let warm = self.warm_k.len() + self.warm_v.len()
            + (self.warm_scale_k.len() + self.warm_zero_k.len()
               + self.warm_scale_v.len() + self.warm_zero_v.len()) * 4;
        let cold = self.cold_k.len() + self.cold_v.len()
            + (self.cold_scale_k.len() + self.cold_zero_k.len()
               + self.cold_scale_v.len() + self.cold_zero_v.len()) * 4;
        let _ = kv_dim;
        hot + warm + cold
    }
}

// ── Public interface ──────────────────────────────────────────────────

/// Per-request KV cache tier statistics for telemetry.
#[derive(Default, Clone)]
pub struct KvTierStats {
    pub hot_tokens: usize,
    pub warm_tokens: usize,
    pub cold_tokens: usize,
    pub ram_bytes: usize,
    pub fp32_equiv_bytes: usize,
}

impl KvTierStats {
    pub fn compression_ratio(&self) -> f32 {
        if self.ram_bytes == 0 { return 1.0; }
        self.fp32_equiv_bytes as f32 / self.ram_bytes as f32
    }

    pub fn total_tokens(&self) -> usize {
        self.hot_tokens + self.warm_tokens + self.cold_tokens
    }
}

/// 3-tier quantized KV cache for all transformer layers.
/// Layer budgets use PyramidKV: early layers keep more FP32 (larger hot window),
/// later layers compress more aggressively (smaller hot window).
pub struct QuantKvCache {
    layers: Vec<LayerKvCache>,
    pub kv_dim: usize,
    n_layers: usize,
}

impl QuantKvCache {
    pub fn new(n_layers: usize, kv_dim: usize) -> Self {
        let layers = (0..n_layers)
            .map(|l| LayerKvCache::new_with_budget(
                pyramid_hot_window(l, n_layers),
                WARM_WINDOW,
            ))
            .collect();
        Self { layers, kv_dim, n_layers }
    }

    /// Push new K/V vectors for a given layer (one token).
    pub fn push(&mut self, layer: usize, k: &[f32], v: &[f32]) {
        while self.layers.len() <= layer {
            let idx = self.layers.len();
            self.layers.push(LayerKvCache::new_with_budget(
                pyramid_hot_window(idx, self.n_layers.max(idx + 1)),
                WARM_WINDOW,
            ));
        }
        self.layers[layer].push(k, v);
    }

    /// Materialize the full FP32 K and V for a layer into the provided buffers.
    pub fn materialize(&self, layer: usize, k_out: &mut Vec<f32>, v_out: &mut Vec<f32>) {
        if let Some(l) = self.layers.get(layer) {
            l.materialize(self.kv_dim, k_out, v_out);
        }
    }

    /// Total cached token count for a layer.
    pub fn total_len(&self, layer: usize) -> usize {
        self.layers.get(layer).map(|l| l.total_len(self.kv_dim)).unwrap_or(0)
    }

    /// Total RAM in bytes across all layers.
    pub fn ram_bytes(&self) -> usize {
        self.layers.iter().map(|l| l.ram_bytes(self.kv_dim)).sum()
    }

    /// Aggregate KV tier statistics across all layers.
    pub fn tier_stats(&self) -> KvTierStats {
        let mut s = KvTierStats::default();
        for l in &self.layers {
            s.hot_tokens  += l.hot_len(self.kv_dim);
            s.warm_tokens += l.warm_len;
            s.cold_tokens += l.cold_len;
            s.ram_bytes   += l.ram_bytes(self.kv_dim);
        }
        let total_toks = s.hot_tokens + s.warm_tokens + s.cold_tokens;
        s.fp32_equiv_bytes = total_toks * self.kv_dim * 2 * 4; // K+V × f32
        s
    }

    /// Clear all cached data (start of a new request).
    pub fn clear(&mut self) {
        for (i, l) in self.layers.iter_mut().enumerate() {
            *l = LayerKvCache::new_with_budget(
                pyramid_hot_window(i, self.n_layers.max(1)),
                WARM_WINDOW,
            );
        }
    }

    pub fn n_layers(&self) -> usize { self.layers.len() }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ramp(start: f32, len: usize) -> Vec<f32> {
        (0..len).map(|i| start + i as f32 * 0.01).collect()
    }

    #[test]
    fn test_int8_roundtrip_accuracy() {
        let src: Vec<f32> = (0..64).map(|i| i as f32 * 0.1 - 3.0).collect();
        let (data, scales, zeros) = quantize_int8(&src);
        let mut dst = Vec::new();
        dequantize_int8(&data, &scales, &zeros, &mut dst);
        for (a, b) in src.iter().zip(dst.iter()) {
            let err = (a - b).abs();
            assert!(err < 0.025, "INT8 roundtrip error {:.6} exceeds 0.025", err);
        }
    }

    #[test]
    fn test_4bit_roundtrip_accuracy() {
        let src: Vec<f32> = (0..32).map(|i| i as f32 * 0.2 - 3.0).collect();
        let (data, scales, zeros) = quantize_4bit(&src);
        let mut dst = Vec::new();
        dequantize_4bit(&data, &scales, &zeros, src.len(), &mut dst);
        for (a, b) in src.iter().zip(dst.iter()) {
            let err = (a - b).abs();
            assert!(err < 0.42, "4-bit roundtrip error {:.6} exceeds expected", err);
        }
    }

    #[test]
    fn test_push_single_token_stays_hot() {
        let kv_dim = 8;
        let mut cache = QuantKvCache::new(1, kv_dim);
        let k = make_ramp(1.0, kv_dim);
        let v = make_ramp(2.0, kv_dim);
        cache.push(0, &k, &v);
        assert_eq!(cache.total_len(0), 1);
        let mut k_out = Vec::new();
        let mut v_out = Vec::new();
        cache.materialize(0, &mut k_out, &mut v_out);
        assert_eq!(k_out.len(), kv_dim);
        // Hot tokens are FP32-exact.
        for (a, b) in k.iter().zip(k_out.iter()) {
            assert!((a - b).abs() < 1e-6, "hot token K mismatch");
        }
    }

    #[test]
    fn test_hot_overflow_promotes_to_warm() {
        let kv_dim = 8;
        let mut cache = QuantKvCache::new(1, kv_dim);
        // Layer 0 of 1: pyramid_hot_window(0, 1) = HOT_MAX = 64
        let hw = pyramid_hot_window(0, 1);
        for i in 0..=hw {
            let k = make_ramp(i as f32, kv_dim);
            let v = make_ramp(i as f32 + 100.0, kv_dim);
            cache.push(0, &k, &v);
        }
        assert_eq!(cache.total_len(0), hw + 1);
        assert_eq!(cache.layers[0].warm_len, 1, "one token should be in warm tier");
        let mut k_out = Vec::new();
        let mut v_out = Vec::new();
        cache.materialize(0, &mut k_out, &mut v_out);
        assert_eq!(k_out.len(), (hw + 1) * kv_dim);
    }

    #[test]
    fn test_warm_overflow_promotes_to_cold() {
        let kv_dim = 8;
        let mut cache = QuantKvCache::new(1, kv_dim);
        let hw = pyramid_hot_window(0, 1);
        let n = hw + WARM_WINDOW + 1;
        for i in 0..n {
            let k = make_ramp(i as f32 * 0.1, kv_dim);
            let v = make_ramp(i as f32 * 0.2 + 50.0, kv_dim);
            cache.push(0, &k, &v);
        }
        assert_eq!(cache.total_len(0), n);
        assert!(cache.layers[0].cold_len >= 1, "cold tier should have at least 1 token");
    }

    #[test]
    fn test_ram_bytes_less_than_fp32() {
        let kv_dim = 64;
        let n_layers = 4;
        let hw = pyramid_hot_window(0, n_layers);
        let n_tokens = hw + WARM_WINDOW + 32;
        let mut cache = QuantKvCache::new(n_layers, kv_dim);
        // Only test a single layer.
        for i in 0..n_tokens {
            let k = make_ramp(i as f32 * 0.01, kv_dim);
            let v = make_ramp(i as f32 * 0.02, kv_dim);
            cache.push(0, &k, &v);
        }
        let quant_bytes = cache.ram_bytes();
        let fp32_bytes = n_tokens * kv_dim * 2 * 4; // K+V, f32
        assert!(quant_bytes < fp32_bytes, "quant ({} B) must be smaller than fp32 ({} B)", quant_bytes, fp32_bytes);
    }

    #[test]
    fn test_pyramid_hot_window_decreases_with_depth() {
        let n = 30;
        let h0 = pyramid_hot_window(0, n);
        let h14 = pyramid_hot_window(14, n);
        let h29 = pyramid_hot_window(29, n);
        assert!(h0 > h14, "early layer should have larger hot window than mid");
        assert!(h14 > h29, "mid layer should have larger hot window than last");
        assert!(h0 >= 64, "first layer must have maximum hot window (64)");
        assert!(h29 >= 16, "last layer must have minimum hot window (16)");
    }

    #[test]
    fn test_tier_stats_sum_to_total_tokens() {
        let kv_dim = 8;
        let n_layers = 2;
        let mut cache = QuantKvCache::new(n_layers, kv_dim);
        let hw = pyramid_hot_window(0, n_layers);
        // Push enough tokens so layer 0 has hot+warm tokens.
        for i in 0..(hw + 10) {
            let k = make_ramp(i as f32, kv_dim);
            let v = make_ramp(i as f32 + 50.0, kv_dim);
            cache.push(0, &k, &v);
        }
        let stats = cache.tier_stats();
        assert_eq!(stats.total_tokens(), cache.total_len(0), "tier stats total must match total_len");
        assert!(stats.fp32_equiv_bytes > stats.ram_bytes, "fp32 equiv must exceed actual usage");
        assert!(stats.compression_ratio() > 1.0, "compression ratio must exceed 1");
    }

    #[test]
    fn test_pyramid_early_layers_retain_more_fp32() {
        let n_layers = 30;
        let kv_dim = 8;
        let mut cache = QuantKvCache::new(n_layers, kv_dim);
        // Push exactly HOT_MAX tokens so layer 0 stays fully hot,
        // layer 29 (smaller hot_window) starts spilling to warm.
        let hw0 = pyramid_hot_window(0, n_layers);
        let hw29 = pyramid_hot_window(29, n_layers);
        let n_push = hw0; // exactly fills layer-0 hot, overflows layer-29 hot
        for i in 0..n_push {
            let k = make_ramp(i as f32, kv_dim);
            let v = make_ramp(i as f32, kv_dim);
            cache.push(0, &k, &v);
            cache.push(29, &k, &v);
        }
        assert_eq!(cache.layers[0].warm_len, 0, "layer 0 should still be all-hot");
        let expected_warm29 = n_push.saturating_sub(hw29);
        assert_eq!(cache.layers[29].warm_len, expected_warm29,
            "layer 29 should have {} warm tokens", expected_warm29);
    }
}
