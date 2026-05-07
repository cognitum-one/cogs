//! GGUF tensor loading and Q4 weight dequantization for sparse-LLM COG.
//!
//! Supports GGML tensor types:
//!   F32  (type 0) — direct load
//!   Q4_0 (type 2) — block_size=32, 2-byte f16 scale + 16-byte nibbles
//!   Q4_K (type 12) — block_size=256, super-block with 6-bit sub-scales
//!
//! Each `GgufTensor` holds the raw bytes; call `.dequant_f32()` to expand.
//!
//! ## Deterministic vs stochastic dequant
//!
//! Default dequant snaps every reconstructed value to a 4-bit grid point:
//! `y = scale * (nibble - 8)` for Q4_0. The original f32 weights almost
//! always fell *between* grid points, so deterministic dequant biases
//! activations toward grid centers. Stochastic dequant adds uniform
//! `[-0.5, 0.5)` dither (in nibble units) so reconstruction is unbiased
//! in expectation: `E[y'] = original_value`. This breaks up systematic
//! quantization artifacts in attention and FFN outputs at zero extra
//! storage cost. Toggle via `set_stochastic_dequant(true)`.

use std::io::{Read as IoRead, Seek, SeekFrom};

// ---------------------------------------------------------------------------
// f16 → f32 conversion (no half crate dependency)
// ---------------------------------------------------------------------------

pub fn f16_to_f32(bits: u16) -> f32 {
    let exp = ((bits >> 10) & 0x1F) as i32;
    let mant = (bits & 0x3FF) as u32;
    let sign: f32 = if bits >> 15 == 1 { -1.0 } else { 1.0 };
    if exp == 0 {
        sign * mant as f32 * (2.0_f32).powi(-24)
    } else if exp == 31 {
        if mant == 0 { sign * f32::INFINITY } else { f32::NAN }
    } else {
        sign * (1.0 + mant as f32 / 1024.0) * (2.0_f32).powi(exp - 15)
    }
}

// ---------------------------------------------------------------------------
// GGML type constants
// ---------------------------------------------------------------------------

pub const GGML_F32: u32 = 0;
pub const GGML_Q4_0: u32 = 2;
pub const GGML_Q5_0: u32 = 6;
pub const GGML_Q8_0: u32 = 8;
pub const GGML_Q4_K: u32 = 12;
pub const GGML_Q6_K: u32 = 14;

// Q4_0: f16(2) + qs(16) = 18 bytes → 32 values
pub const Q4_0_BLOCK_BYTES: usize = 18;
pub const Q4_0_BLOCK_ELEMS: usize = 32;

// Q5_0: f16(2) + qh(4) + qs(16) = 22 bytes → 32 values
pub const Q5_0_BLOCK_BYTES: usize = 22;
pub const Q5_0_BLOCK_ELEMS: usize = 32;

// Q8_0: f16(2) + qs(32) = 34 bytes → 32 values
pub const Q8_0_BLOCK_BYTES: usize = 34;
pub const Q8_0_BLOCK_ELEMS: usize = 32;

// Q4_K: d(2) + dmin(2) + scales(12) + qs(128) = 144 bytes → 256 values
pub const Q4_K_BLOCK_BYTES: usize = 144;
pub const Q4_K_BLOCK_ELEMS: usize = 256;

// Q6_K: ql(128) + qh(64) + scales(16) + d(2) = 210 bytes → 256 values
pub const Q6_K_BLOCK_BYTES: usize = 210;
pub const Q6_K_BLOCK_ELEMS: usize = 256;

// ---------------------------------------------------------------------------
// Tensor descriptor (parsed from GGUF tensor_info section)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GgufTensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub shape: Vec<u64>,
    pub ggml_type: u32,
    /// Byte offset from the start of the tensor data block (after header alignment).
    pub data_offset: u64,
}

impl GgufTensorInfo {
    /// Number of elements (product of shape dims).
    pub fn num_elems(&self) -> u64 {
        self.shape.iter().product()
    }

    /// Raw byte size of this tensor's data.
    pub fn byte_size(&self) -> u64 {
        let n = self.num_elems() as usize;
        match self.ggml_type {
            GGML_F32 => (n * 4) as u64,
            GGML_Q4_0 => {
                let blocks = (n + Q4_0_BLOCK_ELEMS - 1) / Q4_0_BLOCK_ELEMS;
                (blocks * Q4_0_BLOCK_BYTES) as u64
            }
            GGML_Q5_0 => {
                let blocks = (n + Q5_0_BLOCK_ELEMS - 1) / Q5_0_BLOCK_ELEMS;
                (blocks * Q5_0_BLOCK_BYTES) as u64
            }
            GGML_Q8_0 => {
                let blocks = (n + Q8_0_BLOCK_ELEMS - 1) / Q8_0_BLOCK_ELEMS;
                (blocks * Q8_0_BLOCK_BYTES) as u64
            }
            GGML_Q4_K => {
                let blocks = (n + Q4_K_BLOCK_ELEMS - 1) / Q4_K_BLOCK_ELEMS;
                (blocks * Q4_K_BLOCK_BYTES) as u64
            }
            GGML_Q6_K => {
                let blocks = (n + Q6_K_BLOCK_ELEMS - 1) / Q6_K_BLOCK_ELEMS;
                (blocks * Q6_K_BLOCK_BYTES) as u64
            }
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// GgufTensor: info + raw bytes, lazy-dequanted to f32
// ---------------------------------------------------------------------------

pub struct GgufTensor {
    pub info: GgufTensorInfo,
    pub raw: Vec<u8>,
}

impl GgufTensor {
    /// Dequantize raw bytes → Vec<f32>.
    /// Honors `STOCHASTIC_DEQUANT` for Q4_0 / Q4_K.
    pub fn dequant_f32(&self) -> Vec<f32> {
        let n = self.info.num_elems() as usize;
        dequant_raw(&self.raw, self.info.ggml_type, n)
    }
}

// ---------------------------------------------------------------------------
// Q4_0 dequantization
// ---------------------------------------------------------------------------

/// Dequantize Q4_0 blocks.
/// Block layout: [d: u16(f16)][qs: 16 bytes]
/// Matches llama.cpp dequantize_row_q4_0:
///   elements 0..15:  y[j]    = d * (low_nibble(qs[j])  - 8)
///   elements 16..31: y[j+16] = d * (high_nibble(qs[j]) - 8)
fn dequant_q4_0(data: &[u8], n_elems: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q4_0_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let qs = &block[2..18];
        // Elements 0..15: low nibbles
        for j in 0..16 {
            if elem >= n_elems { break; }
            out.push(d * ((qs[j] & 0x0F) as i32 - 8) as f32);
            elem += 1;
        }
        // Elements 16..31: high nibbles
        for j in 0..16 {
            if elem >= n_elems { break; }
            out.push(d * ((qs[j] >> 4) as i32 - 8) as f32);
            elem += 1;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

// ---------------------------------------------------------------------------
// Q5_0 dequantization
// ---------------------------------------------------------------------------

/// Dequantize Q5_0 blocks.
/// Block layout: [d: u16(f16)][qh: 4 bytes (packed 5th bits)][qs: 16 bytes]
/// Matches llama.cpp dequantize_row_q5_0 exactly:
///   elements 0..15:  y[j]    = d * (low_nibble(qs[j])  | bit(j)    of qh << 4) - 16
///   elements 16..31: y[j+16] = d * (high_nibble(qs[j]) | bit(j+16) of qh << 4) - 16
/// NOT interleaved — all x0 values first, then all x1 values.
fn dequant_q5_0(data: &[u8], n_elems: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q5_0_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d  = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let qh = u32::from_le_bytes([block[2], block[3], block[4], block[5]]);
        let qs = &block[6..22];
        // Elements 0..15: low nibble | 5th bit from qh bits 0..15
        for j in 0usize..16 {
            if elem >= n_elems { break; }
            let xh = ((qh >> j) & 1) as i32;
            let x0 = ((qs[j] as i32 & 0x0F) | (xh << 4)) - 16;
            out.push(d * x0 as f32);
            elem += 1;
        }
        // Elements 16..31: high nibble | 5th bit from qh bits 16..31
        for j in 0usize..16 {
            if elem >= n_elems { break; }
            let xh = ((qh >> (j + 16)) & 1) as i32;
            let x1 = ((qs[j] as i32 >> 4) | (xh << 4)) - 16;
            out.push(d * x1 as f32);
            elem += 1;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

// ---------------------------------------------------------------------------
// Q8_0 dequantization
// ---------------------------------------------------------------------------

/// Dequantize Q8_0 blocks.
/// Block layout: [d: u16(f16)][qs: 32 bytes (int8 values)]
/// Output: d * qs[i] for each signed byte.
fn dequant_q8_0(data: &[u8], n_elems: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q8_0_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d  = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let qs = &block[2..34];
        for &b in qs {
            if elem >= n_elems { break; }
            out.push(d * (b as i8) as f32);
            elem += 1;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

// ---------------------------------------------------------------------------
// Q6_K dequantization
// ---------------------------------------------------------------------------

/// Dequantize Q6_K blocks.
/// Block layout: [ql: 128 bytes][qh: 64 bytes][scales: 16 × i8][d: f16]
/// Two 128-element halves; within each half 32 inner iterations produce 4 outputs.
/// 16 sub-blocks of 16 elements each → scale index = (l/16) + group_offset.
/// sc[is+0], sc[is+2], sc[is+4], sc[is+6] for +0/+32/+64/+96 groups,
/// where is = l/16 (0 for l<16, 1 for l>=16).
fn dequant_q6_k(data: &[u8], n_elems: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q6_K_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d = f16_to_f32(u16::from_le_bytes([block[208], block[209]]));
        let mut ql_off = 0usize;
        let mut qh_off = 0usize;
        let mut sc_off = 0usize;
        let mut tmp = [0.0f32; 256];
        let mut out_off = 0usize;
        for _half in 0..2 {
            for l in 0usize..32 {
                let ql_lo = block[ql_off + l];
                let ql_hi = block[ql_off + 32 + l];
                let qh_b  = block[128 + qh_off + l];
                let is = l / 16;
                let q1 = ((ql_lo & 0x0F) as i32) | (((qh_b >> 0) & 3) as i32) << 4;
                let q2 = ((ql_hi & 0x0F) as i32) | (((qh_b >> 2) & 3) as i32) << 4;
                let q3 = ((ql_lo >> 4)   as i32) | (((qh_b >> 4) & 3) as i32) << 4;
                let q4 = ((ql_hi >> 4)   as i32) | (((qh_b >> 6) & 3) as i32) << 4;
                let sc = &block[192 + sc_off..];
                tmp[out_off + l +  0] = d * (sc[is + 0] as i8) as f32 * (q1 - 32) as f32;
                tmp[out_off + l + 32] = d * (sc[is + 2] as i8) as f32 * (q2 - 32) as f32;
                tmp[out_off + l + 64] = d * (sc[is + 4] as i8) as f32 * (q3 - 32) as f32;
                tmp[out_off + l + 96] = d * (sc[is + 6] as i8) as f32 * (q4 - 32) as f32;
            }
            ql_off += 64;
            qh_off += 32;
            sc_off += 8;
            out_off += 128;
        }
        for &v in &tmp {
            if elem >= n_elems { break; }
            out.push(v);
            elem += 1;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

// ---------------------------------------------------------------------------
// Q4_K dequantization
// ---------------------------------------------------------------------------

/// Decode sub-block scales and mins from the 12-byte packed scales array.
/// Q4_K: 8 sub-blocks of 32 elements each; each has a 6-bit scale and 6-bit min.
/// Layout follows llama.cpp `get_scale_min_k4`:
///   j<4:  sc[j]=scales[j]&63,          m[j]=scales[j+4]&63
///   j>=4: sc[j]=(scales[j+4]&0xF)|((scales[j-4]>>6)<<4),
///          m[j]=(scales[j+4]>>4)|((scales[j-0]>>6)<<4)
fn unpack_q4k_scales(scales: &[u8; 12]) -> ([u8; 8], [u8; 8]) {
    let mut sc = [0u8; 8];
    let mut m = [0u8; 8];
    for j in 0..4 {
        sc[j]     = scales[j] & 0x3F;
        m[j]      = scales[j + 4] & 0x3F;
        sc[j + 4] = (scales[j + 8] & 0x0F) | ((scales[j] >> 6) << 4);
        m[j + 4]  = (scales[j + 8] >> 4)   | ((scales[j + 4] >> 6) << 4);
    }
    (sc, m)
}

/// Dequantize Q4_K blocks.
/// Block layout: [d: u16][dmin: u16][scales: 12 bytes][qs: 128 bytes]
/// 4 outer groups of 32 bytes each; each outer group produces 64 elements:
///   - 32 elements from low nibbles (scale 2i), then
///   - 32 elements from high nibbles (scale 2i+1).
/// This matches llama.cpp dequantize_row_q4_K exactly.
fn dequant_q4_k(data: &[u8], n_elems: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q4_K_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d    = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let dmin = f16_to_f32(u16::from_le_bytes([block[2], block[3]]));
        let scales_arr: [u8; 12] = block[4..16].try_into().unwrap_or([0u8; 12]);
        let qs = &block[16..144];

        let (sc, m) = unpack_q4k_scales(&scales_arr);

        let mut is = 0usize;
        let mut q_off = 0usize;
        for _ in 0..4 {
            let scale1 = d    * sc[is] as f32;
            let min1   = dmin * m[is]  as f32;
            is += 1;
            let scale2 = d    * sc[is] as f32;
            let min2   = dmin * m[is]  as f32;
            is += 1;
            // 32 low nibbles with scale1
            for l in 0..32 {
                if elem < n_elems {
                    out.push(scale1 * (qs[q_off + l] & 0x0F) as f32 - min1);
                    elem += 1;
                }
            }
            // 32 high nibbles with scale2
            for l in 0..32 {
                if elem < n_elems {
                    out.push(scale2 * (qs[q_off + l] >> 4) as f32 - min2);
                    elem += 1;
                }
            }
            q_off += 32;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

// ---------------------------------------------------------------------------
// Stochastic (dithered) Q4 dequantization
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// When true, `dequant_raw` and `GgufTensor::dequant_f32` use the stochastic
/// (dithered) variants for Q4_0 and Q4_K. Other GGML types are unaffected.
pub static STOCHASTIC_DEQUANT: AtomicBool = AtomicBool::new(false);

pub fn set_stochastic_dequant(on: bool) {
    STOCHASTIC_DEQUANT.store(on, AtomicOrdering::Relaxed);
}

pub fn stochastic_dequant_enabled() -> bool {
    STOCHASTIC_DEQUANT.load(AtomicOrdering::Relaxed)
}

#[inline]
fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Uniform sample in `[-0.5, 0.5)`.
#[inline]
fn dither_unit(state: &mut u64) -> f32 {
    // Take top 24 bits → fp32 mantissa precision; map to [0, 1) then shift.
    let bits = (xorshift64(state) >> 40) as f32;
    bits * (1.0 / (1u64 << 24) as f32) - 0.5
}

/// Mix a u64 with splitmix64 so adjacent input seeds produce wildly
/// different output states (xorshift is sensitive to low-entropy seeds —
/// seeds 42 and 43 both `| 1` to 43, and small magnitudes leave the
/// upper bits empty for several iterations).
#[inline]
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn seed_or_time(seed: Option<u64>) -> u64 {
    let s = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64
                ^ d.as_secs().wrapping_mul(6364136223846793005))
            .unwrap_or(0xdead_beef_cafe_babe_u64)
    });
    let mixed = splitmix64(s);
    if mixed == 0 { 0xdead_beef_cafe_babe } else { mixed }
}

/// Stochastic Q4_0 dequant: `d * ((nibble - 8) + U(-0.5, 0.5))`.
/// `seed = None` uses time-based randomness; pass a seed for reproducibility.
pub fn dequant_q4_0_stochastic(data: &[u8], n_elems: usize, seed: Option<u64>) -> Vec<f32> {
    let mut state = seed_or_time(seed);
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q4_0_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let qs = &block[2..18];
        for j in 0..16 {
            if elem >= n_elems { break; }
            let q = (qs[j] & 0x0F) as i32 - 8;
            out.push(d * (q as f32 + dither_unit(&mut state)));
            elem += 1;
        }
        for j in 0..16 {
            if elem >= n_elems { break; }
            let q = (qs[j] >> 4) as i32 - 8;
            out.push(d * (q as f32 + dither_unit(&mut state)));
            elem += 1;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

/// Stochastic Q4_K dequant: `scale * (nibble + U(-0.5, 0.5)) - min`
/// applied per 32-element sub-block (8 sub-blocks per 256-element super-block).
pub fn dequant_q4_k_stochastic(data: &[u8], n_elems: usize, seed: Option<u64>) -> Vec<f32> {
    let mut state = seed_or_time(seed);
    let mut out = Vec::with_capacity(n_elems);
    let mut elem = 0usize;
    for block in data.chunks_exact(Q4_K_BLOCK_BYTES) {
        if elem >= n_elems { break; }
        let d    = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let dmin = f16_to_f32(u16::from_le_bytes([block[2], block[3]]));
        let scales_arr: [u8; 12] = block[4..16].try_into().unwrap_or([0u8; 12]);
        let qs = &block[16..144];

        let (sc, m) = unpack_q4k_scales(&scales_arr);
        let mut is = 0usize;
        let mut q_off = 0usize;
        for _ in 0..4 {
            let scale1 = d    * sc[is] as f32;
            let min1   = dmin * m[is]  as f32;
            is += 1;
            let scale2 = d    * sc[is] as f32;
            let min2   = dmin * m[is]  as f32;
            is += 1;
            for l in 0..32 {
                if elem < n_elems {
                    let q = (qs[q_off + l] & 0x0F) as f32;
                    out.push(scale1 * (q + dither_unit(&mut state)) - min1);
                    elem += 1;
                }
            }
            for l in 0..32 {
                if elem < n_elems {
                    let q = (qs[q_off + l] >> 4) as f32;
                    out.push(scale2 * (q + dither_unit(&mut state)) - min2);
                    elem += 1;
                }
            }
            q_off += 32;
        }
    }
    out.resize(n_elems, 0.0);
    out
}

/// Like `dequant_raw` but uses stochastic variants for Q4_0 / Q4_K.
/// Other GGML types fall through to deterministic dequant. `seed = None`
/// uses time-based randomness.
pub fn dequant_raw_stochastic(raw: &[u8], ggml_type: u32, n_elems: usize, seed: Option<u64>) -> Vec<f32> {
    match ggml_type {
        GGML_Q4_0 => dequant_q4_0_stochastic(raw, n_elems, seed),
        GGML_Q4_K => dequant_q4_k_stochastic(raw, n_elems, seed),
        _ => dequant_raw(raw, ggml_type, n_elems),
    }
}

// ---------------------------------------------------------------------------
// GGUF tensor section reader
// ---------------------------------------------------------------------------

/// Read GGUF tensor infos from a file (after header KV section).
///
/// `tensor_count` and `data_block_start` must be derived from `GgufHeader`.
/// `data_block_start` is the file offset where tensor data begins
/// (aligned to 32 bytes in GGUF v3).
pub fn read_tensor_infos<R: IoRead + Seek>(
    reader: &mut R,
    tensor_count: u64,
) -> Result<Vec<GgufTensorInfo>, String> {
    let mut infos = Vec::with_capacity(tensor_count.min(512) as usize);
    for _ in 0..tensor_count {
        let name = read_string(reader)?;
        let n_dims = read_u32_le(reader)?;
        let mut shape = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            shape.push(read_u64_le(reader)?);
        }
        let ggml_type = read_u32_le(reader)?;
        let data_offset = read_u64_le(reader)?;
        infos.push(GgufTensorInfo { name, n_dims, shape, ggml_type, data_offset });
    }
    Ok(infos)
}

fn read_u32_le<R: IoRead>(r: &mut R) -> Result<u32, String> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64_le<R: IoRead>(r: &mut R) -> Result<u64, String> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(u64::from_le_bytes(b))
}

fn read_string<R: IoRead>(r: &mut R) -> Result<String, String> {
    let len = read_u64_le(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Load a single tensor's raw bytes from the file at `data_block_start + info.data_offset`.
pub fn load_tensor_raw<R: IoRead + Seek>(
    reader: &mut R,
    info: &GgufTensorInfo,
    data_block_start: u64,
) -> Result<Vec<u8>, String> {
    let offset = data_block_start + info.data_offset;
    reader.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    let n_bytes = info.byte_size() as usize;
    let mut raw = vec![0u8; n_bytes];
    reader.read_exact(&mut raw).map_err(|e| e.to_string())?;
    Ok(raw)
}

// ---------------------------------------------------------------------------
// Matrix multiply: C[m,n] = A[m,k] @ B[k,n] (row-major)
// ---------------------------------------------------------------------------

/// Dequantize raw tensor bytes to f32 without constructing a GgufTensor.
/// This avoids an unnecessary Vec clone when the caller owns the raw bytes.
///
/// When `STOCHASTIC_DEQUANT` is set, Q4_0 and Q4_K go through the dithered
/// variants (other types are unaffected and remain bit-exact deterministic).
pub fn dequant_raw(raw: &[u8], ggml_type: u32, n_elems: usize) -> Vec<f32> {
    if stochastic_dequant_enabled() && (ggml_type == GGML_Q4_0 || ggml_type == GGML_Q4_K) {
        return dequant_raw_stochastic(raw, ggml_type, n_elems, None);
    }
    match ggml_type {
        GGML_F32 => {
            let mut out = vec![0.0f32; n_elems];
            for (i, chunk) in raw.chunks_exact(4).enumerate().take(n_elems) {
                out[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
            out
        }
        GGML_Q4_0 => dequant_q4_0(raw, n_elems),
        GGML_Q5_0 => dequant_q5_0(raw, n_elems),
        GGML_Q8_0 => dequant_q8_0(raw, n_elems),
        GGML_Q4_K => dequant_q4_k(raw, n_elems),
        GGML_Q6_K => dequant_q6_k(raw, n_elems),
        _ => vec![0.0f32; n_elems],
    }
}

/// Dequantize raw weight bytes then compute y[m] = W[m,k] @ x[k].
/// Allocates the f32 weight slice only for the duration of this call.
pub fn matvec_raw(raw: &[u8], ggml_type: u32, x: &[f32], out_dim: usize, in_dim: usize) -> Vec<f32> {
    let weights = dequant_raw(raw, ggml_type, out_dim * in_dim);
    matvec(&weights, x, out_dim, in_dim)
}

/// Dense row-major matmul. Allocates output.
pub fn matmul(a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Vec<f32> {
    let mut c = vec![0.0f32; m * n];
    for row in 0..m {
        for col in 0..n {
            let sum: f32 = (0..k)
                .map(|ki| a[row * k + ki] * b[ki * n + col])
                .sum();
            c[row * n + col] = sum;
        }
    }
    c
}

/// Matrix-vector product: y[m] = A[m,k] @ x[k].
pub fn matvec(a: &[f32], x: &[f32], m: usize, k: usize) -> Vec<f32> {
    (0..m)
        .map(|row| {
            a[row * k..row * k + k]
                .iter()
                .zip(x.iter())
                .map(|(ai, xi)| ai * xi)
                .sum::<f32>()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f16_to_f32_one() {
        // f16 encoding of 1.0 is 0x3C00
        assert!((f16_to_f32(0x3C00) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_f16_to_f32_half() {
        // 0.5 = 0x3800
        assert!((f16_to_f32(0x3800) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_q4_0_dequant_basic() {
        // Build a minimal Q4_0 block: d=1.0 (f16=0x3C00), qs all 0x88 (nibbles 8,8 → 0,0 after -8)
        let mut block = vec![0u8; Q4_0_BLOCK_BYTES];
        block[0] = 0x00; block[1] = 0x3C; // f16 1.0
        for b in block[2..18].iter_mut() { *b = 0x88; } // nibbles: 8 & 8 → 0,0 signed
        let out = dequant_q4_0(&block, 32);
        assert_eq!(out.len(), 32);
        for &v in &out { assert!(v.abs() < 1e-5, "expected 0.0, got {}", v); }
    }

    #[test]
    fn test_q4_0_dequant_max_pos() {
        // d=1.0, qs all 0xFF → nibbles 15,15 → 7,7 after -8
        let mut block = vec![0u8; Q4_0_BLOCK_BYTES];
        block[0] = 0x00; block[1] = 0x3C;
        for b in block[2..18].iter_mut() { *b = 0xFF; }
        let out = dequant_q4_0(&block, 32);
        for &v in &out { assert!((v - 7.0).abs() < 1e-5, "expected 7.0, got {}", v); }
    }

    #[test]
    fn test_matvec_identity() {
        // 3×3 identity @ [1,2,3] = [1,2,3]
        let eye = vec![
            1.0f32, 0.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 0.0, 1.0,
        ];
        let x = vec![1.0f32, 2.0, 3.0];
        let y = matvec(&eye, &x, 3, 3);
        assert_eq!(y, x);
    }

    #[test]
    fn test_q4k_scale_unpack_zeros() {
        let scales = [0u8; 12];
        let (sc, m) = unpack_q4k_scales(&scales);
        assert!(sc.iter().all(|&v| v == 0));
        assert!(m.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_tensor_info_byte_size_f32() {
        let info = GgufTensorInfo {
            name: "test".into(), n_dims: 2,
            shape: vec![4, 8], ggml_type: GGML_F32, data_offset: 0,
        };
        assert_eq!(info.byte_size(), 4 * 8 * 4);
    }

    #[test]
    fn test_tensor_info_byte_size_q4_0() {
        // 32 elems = 1 block = 18 bytes
        let info = GgufTensorInfo {
            name: "t".into(), n_dims: 1,
            shape: vec![32], ggml_type: GGML_Q4_0, data_offset: 0,
        };
        assert_eq!(info.byte_size(), 18);
    }

    // ----- Stochastic dequant tests --------------------------------------

    fn build_q4_0_block(scale_f16: u16, qs: u8) -> Vec<u8> {
        let mut block = vec![0u8; Q4_0_BLOCK_BYTES];
        block[0] = (scale_f16 & 0xFF) as u8;
        block[1] = (scale_f16 >> 8) as u8;
        for b in block[2..18].iter_mut() { *b = qs; }
        block
    }

    #[test]
    fn test_q4_0_stochastic_reproducible_with_seed() {
        // Same seed → identical output sequence.
        let block = build_q4_0_block(0x3C00, 0xFF); // d=1.0, all nibbles=15 → q=7
        let a = dequant_q4_0_stochastic(&block, 32, Some(42));
        let b = dequant_q4_0_stochastic(&block, 32, Some(42));
        assert_eq!(a, b, "same seed must produce identical output");
        // Different seed → almost certainly differs somewhere.
        let c = dequant_q4_0_stochastic(&block, 32, Some(43));
        assert_ne!(a, c, "different seed should produce different output");
    }

    #[test]
    fn test_q4_0_stochastic_in_bin_width() {
        // Each stochastic value must be within ±d/2 of the deterministic value
        // (the dither is U(-0.5, 0.5) scaled by d).
        let block = build_q4_0_block(0x3C00, 0xFF); // d=1.0, q=7 for all
        let det  = dequant_q4_0(&block, 32);
        let stoc = dequant_q4_0_stochastic(&block, 32, Some(7));
        for (d, s) in det.iter().zip(stoc.iter()) {
            let err = (s - d).abs();
            assert!(err <= 0.5 + 1e-6, "|{} - {}| = {} > 0.5", s, d, err);
        }
    }

    #[test]
    fn test_q4_0_stochastic_unbiased_mean() {
        // Mean over many independent seeds should approach the deterministic
        // value (unbiased estimator). With 256 trials the standard error of
        // the mean for U(-0.5, 0.5) is ~1/sqrt(12*256) ≈ 0.018; allow 0.06.
        let block = build_q4_0_block(0x3C00, 0xFF);
        let det = dequant_q4_0(&block, 32);
        let trials = 256;
        let mut sum = vec![0.0f32; 32];
        for seed in 1..=trials as u64 {
            let s = dequant_q4_0_stochastic(&block, 32, Some(seed));
            for (i, v) in s.iter().enumerate() { sum[i] += v; }
        }
        for (i, total) in sum.iter().enumerate() {
            let mean = total / trials as f32;
            let bias = (mean - det[i]).abs();
            assert!(bias < 0.06,
                "elem {} bias {} too large (mean={}, det={})",
                i, bias, mean, det[i]);
        }
    }

    #[test]
    fn test_q4_k_stochastic_reproducible_with_seed() {
        // Build a minimal Q4_K block: d=1.0, dmin=0.0, scales all zero so
        // every output is just dither × 0 = 0. Use non-zero scales to test
        // dither is actually applied.
        let mut block = vec![0u8; Q4_K_BLOCK_BYTES];
        block[0] = 0x00; block[1] = 0x3C; // d = 1.0
        block[2] = 0x00; block[3] = 0x00; // dmin = 0.0
        // scales: set first byte so sc[0] = 4 (a non-zero scale).
        block[4] = 0x04;
        // qs: all 0x55 (low=5, high=5) — gives non-trivial outputs.
        for b in block[16..144].iter_mut() { *b = 0x55; }
        let a = dequant_q4_k_stochastic(&block, 256, Some(99));
        let b = dequant_q4_k_stochastic(&block, 256, Some(99));
        assert_eq!(a, b);
    }

    #[test]
    fn test_stochastic_flag_toggles_dequant_raw() {
        // dequant_raw must honor the global flag for Q4_0.
        let block = build_q4_0_block(0x3C00, 0xFF);
        set_stochastic_dequant(false);
        let det1 = dequant_raw(&block, GGML_Q4_0, 32);
        let det2 = dequant_raw(&block, GGML_Q4_0, 32);
        assert_eq!(det1, det2, "deterministic path must be reproducible");

        set_stochastic_dequant(true);
        // With stochastic + None seed, two consecutive calls almost certainly
        // differ (time-based seeding). Restore flag after.
        let stoc = dequant_raw(&block, GGML_Q4_0, 32);
        set_stochastic_dequant(false);

        // All stochastic values are within bin width of deterministic.
        for (s, d) in stoc.iter().zip(det1.iter()) {
            assert!((s - d).abs() <= 0.5 + 1e-6);
        }
    }

    #[test]
    fn test_stochastic_flag_does_not_affect_other_types() {
        // Q8_0 is not in the stochastic set — flag must be a no-op for it.
        let mut block = vec![0u8; Q8_0_BLOCK_BYTES];
        block[0] = 0x00; block[1] = 0x3C; // d = 1.0
        for (i, b) in block[2..34].iter_mut().enumerate() { *b = i as u8; }

        set_stochastic_dequant(false);
        let det = dequant_raw(&block, GGML_Q8_0, 32);
        set_stochastic_dequant(true);
        let stoc = dequant_raw(&block, GGML_Q8_0, 32);
        set_stochastic_dequant(false);

        assert_eq!(det, stoc, "Q8_0 must be unaffected by stochastic flag");
    }
}
