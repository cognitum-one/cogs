//! Page storage format: binary serialization, checksums, encoding/decoding.
//!
//! Pages are the atomic unit of I/O. This module handles packing vectors into
//! pages, computing checksums, and reading/writing pages to byte buffers.

use super::types::*;
use crate::ruvector::types::EmbeddingId;

// ============================================================================
// CRC32 checksum (simple implementation for deterministic audit)
// ============================================================================

/// Compute CRC32 checksum over a byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ============================================================================
// Quantization encoding / decoding
// ============================================================================

/// Encode a single f32 vector to quantized bytes for the given tier.
pub fn encode_vector(vector: &[f32], tier: QuantTier, params: &QuantScaleParams) -> Vec<u8> {
    match tier {
        QuantTier::Hot => encode_8bit(vector, params),
        QuantTier::Warm => encode_5bit(vector, params),
        QuantTier::Cold => encode_3bit(vector, params),
    }
}

/// Decode quantized bytes back to f32 vector.
pub fn decode_vector(
    data: &[u8],
    dimension: usize,
    tier: QuantTier,
    params: &QuantScaleParams,
) -> Vec<f32> {
    match tier {
        QuantTier::Hot => decode_8bit(data, dimension, params),
        QuantTier::Warm => decode_5bit(data, dimension, params),
        QuantTier::Cold => decode_3bit(data, dimension, params),
    }
}

/// Bytes required for one vector at a given tier and dimension.
pub fn encoded_vector_size(dimension: usize, tier: QuantTier) -> usize {
    match tier {
        QuantTier::Hot => dimension,
        QuantTier::Warm => (dimension * 5 + 7) / 8,
        QuantTier::Cold => (dimension * 3 + 7) / 8,
    }
}

// -- 8-bit (hot) --

fn encode_8bit(vector: &[f32], params: &QuantScaleParams) -> Vec<u8> {
    let range = (params.max_val - params.min_val).max(1e-8);
    vector
        .iter()
        .map(|&v| {
            let normalized = ((v - params.min_val) / range * 255.0).clamp(0.0, 255.0);
            normalized as u8
        })
        .collect()
}

fn decode_8bit(data: &[u8], dimension: usize, params: &QuantScaleParams) -> Vec<f32> {
    let range = params.max_val - params.min_val;
    data.iter()
        .take(dimension)
        .map(|&b| (b as f32 / 255.0) * range + params.min_val)
        .collect()
}

// -- 5-bit (warm) --

fn encode_5bit(vector: &[f32], params: &QuantScaleParams) -> Vec<u8> {
    let range = (params.max_val - params.min_val).max(1e-8);
    let max_val = 31u8; // 2^5 - 1
    let mut bits: u64 = 0;
    let mut bit_count = 0;
    let mut output = Vec::with_capacity((vector.len() * 5 + 7) / 8);

    for &v in vector {
        let normalized = ((v - params.min_val) / range * max_val as f32).clamp(0.0, max_val as f32);
        let code = normalized as u8;
        bits |= (code as u64) << bit_count;
        bit_count += 5;

        while bit_count >= 8 {
            output.push((bits & 0xFF) as u8);
            bits >>= 8;
            bit_count -= 8;
        }
    }

    if bit_count > 0 {
        output.push((bits & 0xFF) as u8);
    }

    output
}

fn decode_5bit(data: &[u8], dimension: usize, params: &QuantScaleParams) -> Vec<f32> {
    let range = params.max_val - params.min_val;
    let max_val = 31.0f32;
    let mut result = Vec::with_capacity(dimension);
    let mut bits: u64 = 0;
    let mut bit_count = 0;
    let mut byte_idx = 0;

    for _ in 0..dimension {
        while bit_count < 5 && byte_idx < data.len() {
            bits |= (data[byte_idx] as u64) << bit_count;
            bit_count += 8;
            byte_idx += 1;
        }
        let code = (bits & 0x1F) as f32;
        bits >>= 5;
        bit_count -= 5;
        result.push((code / max_val) * range + params.min_val);
    }

    result
}

// -- 3-bit (cold) --

fn encode_3bit(vector: &[f32], params: &QuantScaleParams) -> Vec<u8> {
    let range = (params.max_val - params.min_val).max(1e-8);
    let max_val = 7u8; // 2^3 - 1
    let mut bits: u64 = 0;
    let mut bit_count = 0;
    let mut output = Vec::with_capacity((vector.len() * 3 + 7) / 8);

    for &v in vector {
        let normalized = ((v - params.min_val) / range * max_val as f32).clamp(0.0, max_val as f32);
        let code = normalized as u8;
        bits |= (code as u64) << bit_count;
        bit_count += 3;

        while bit_count >= 8 {
            output.push((bits & 0xFF) as u8);
            bits >>= 8;
            bit_count -= 8;
        }
    }

    if bit_count > 0 {
        output.push((bits & 0xFF) as u8);
    }

    output
}

fn decode_3bit(data: &[u8], dimension: usize, params: &QuantScaleParams) -> Vec<f32> {
    let range = params.max_val - params.min_val;
    let max_val = 7.0f32;
    let mut result = Vec::with_capacity(dimension);
    let mut bits: u64 = 0;
    let mut bit_count = 0;
    let mut byte_idx = 0;

    for _ in 0..dimension {
        while bit_count < 3 && byte_idx < data.len() {
            bits |= (data[byte_idx] as u64) << bit_count;
            bit_count += 8;
            byte_idx += 1;
        }
        let code = (bits & 0x07) as f32;
        bits >>= 3;
        bit_count -= 3;
        result.push((code / max_val) * range + params.min_val);
    }

    result
}

// ============================================================================
// Page serialization / deserialization
// ============================================================================

/// Serialize a PageNode to a byte vector.
///
/// Format:
///   [magic: 4 bytes] [header_len: 4 bytes] [header_json: N bytes]
///   [centroid: dim*4 bytes] [sub_centroid_count: 4] [sub_centroids...]
///   [neighbor_count: 4] [neighbor_ids: N*8] [neighbor_weights: N*4]
///   [vector_count: 4] [vector_ids: N*8] [encoded_vectors: M bytes]
///   [timestamps: N*8] [bloom_len: 4] [bloom_filter: K bytes]
///   [checksum: 4 bytes]
pub fn serialize_page(page: &PageNode) -> Vec<u8> {
    let mut buf = Vec::with_capacity(page.byte_size() + 64);

    // Magic bytes
    buf.extend_from_slice(b"PANN");

    // Serialize header as JSON (for flexibility; binary in production)
    let header_json = serde_json::to_vec(&page.header).unwrap_or_default();
    buf.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
    buf.extend_from_slice(&header_json);

    // Centroid
    let dim = page.centroid.len();
    buf.extend_from_slice(&(dim as u32).to_le_bytes());
    for &v in &page.centroid {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    // Sub-centroids
    buf.extend_from_slice(&(page.sub_centroids.len() as u32).to_le_bytes());
    for sc in &page.sub_centroids {
        buf.extend_from_slice(&(sc.len() as u32).to_le_bytes());
        for &v in sc {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }

    // Neighbors
    buf.extend_from_slice(&(page.neighbor_ids.len() as u32).to_le_bytes());
    for &nid in &page.neighbor_ids {
        buf.extend_from_slice(&nid.0.to_le_bytes());
    }
    for &w in &page.neighbor_weights {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Vector IDs
    let vec_count = page.vector_ids.len();
    buf.extend_from_slice(&(vec_count as u32).to_le_bytes());
    for &vid in &page.vector_ids {
        buf.extend_from_slice(&vid.0.to_le_bytes());
    }

    // Encoded vectors
    buf.extend_from_slice(&(page.encoded_vectors.len() as u32).to_le_bytes());
    buf.extend_from_slice(&page.encoded_vectors);

    // Timestamps
    for &ts in &page.timestamps {
        buf.extend_from_slice(&ts.to_le_bytes());
    }

    // Bloom filter
    buf.extend_from_slice(&(page.bloom_filter.len() as u32).to_le_bytes());
    buf.extend_from_slice(&page.bloom_filter);

    // Compute and append checksum over everything so far
    let checksum = crc32(&buf);
    buf.extend_from_slice(&checksum.to_le_bytes());

    buf
}

/// Deserialize a PageNode from bytes, verifying the checksum.
pub fn deserialize_page(data: &[u8]) -> Result<PageNode, PageStorageError> {
    if data.len() < 12 {
        return Err(PageStorageError::CorruptPage("too short".into()));
    }

    // Verify magic
    if &data[0..4] != b"PANN" {
        return Err(PageStorageError::CorruptPage("bad magic".into()));
    }

    // Verify checksum (last 4 bytes are the checksum of everything before)
    if data.len() < 4 {
        return Err(PageStorageError::CorruptPage("no checksum".into()));
    }
    let payload = &data[..data.len() - 4];
    let stored_checksum = u32::from_le_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    let computed_checksum = crc32(payload);
    if stored_checksum != computed_checksum {
        return Err(PageStorageError::ChecksumMismatch {
            expected: stored_checksum,
            actual: computed_checksum,
        });
    }

    let mut pos = 4; // skip magic

    // Header JSON
    let header_len = read_u32(data, &mut pos)? as usize;
    if pos + header_len > payload.len() {
        return Err(PageStorageError::CorruptPage("header truncated".into()));
    }
    let header: PageHeader = serde_json::from_slice(&data[pos..pos + header_len])
        .map_err(|e| PageStorageError::CorruptPage(format!("header parse: {}", e)))?;
    pos += header_len;

    // Centroid
    let dim = read_u32(data, &mut pos)? as usize;
    let centroid = read_f32_vec(data, &mut pos, dim)?;

    // Sub-centroids
    let sc_count = read_u32(data, &mut pos)? as usize;
    let mut sub_centroids = Vec::with_capacity(sc_count);
    for _ in 0..sc_count {
        let sc_dim = read_u32(data, &mut pos)? as usize;
        sub_centroids.push(read_f32_vec(data, &mut pos, sc_dim)?);
    }

    // Neighbors
    let neighbor_count = read_u32(data, &mut pos)? as usize;
    let mut neighbor_ids = Vec::with_capacity(neighbor_count);
    for _ in 0..neighbor_count {
        neighbor_ids.push(PageId(read_u64(data, &mut pos)?));
    }
    let mut neighbor_weights = Vec::with_capacity(neighbor_count);
    for _ in 0..neighbor_count {
        neighbor_weights.push(read_f32(data, &mut pos)?);
    }

    // Vector IDs
    let vec_count = read_u32(data, &mut pos)? as usize;
    let mut vector_ids = Vec::with_capacity(vec_count);
    for _ in 0..vec_count {
        vector_ids.push(EmbeddingId(read_u64(data, &mut pos)?));
    }

    // Encoded vectors
    let enc_len = read_u32(data, &mut pos)? as usize;
    if pos + enc_len > payload.len() {
        return Err(PageStorageError::CorruptPage("vectors truncated".into()));
    }
    let encoded_vectors = data[pos..pos + enc_len].to_vec();
    pos += enc_len;

    // Timestamps
    let mut timestamps = Vec::with_capacity(vec_count);
    for _ in 0..vec_count {
        timestamps.push(read_u64(data, &mut pos)?);
    }

    // Tenant IDs (same count as vectors; stored as 0 if not multi-tenant)
    let vector_tenant_ids = vec![header.tenant_id; vec_count];

    // Bloom filter
    let bloom_len = read_u32(data, &mut pos)? as usize;
    if pos + bloom_len > payload.len() {
        return Err(PageStorageError::CorruptPage("bloom truncated".into()));
    }
    let bloom_filter = data[pos..pos + bloom_len].to_vec();

    Ok(PageNode {
        header,
        centroid,
        sub_centroids,
        neighbor_ids,
        neighbor_weights,
        encoded_vectors,
        vector_ids,
        residuals: None,
        timestamps,
        vector_tenant_ids,
        bloom_filter,
    })
}

// ============================================================================
// Helpers
// ============================================================================

fn read_u32(data: &[u8], pos: &mut usize) -> Result<u32, PageStorageError> {
    if *pos + 4 > data.len() {
        return Err(PageStorageError::CorruptPage("u32 truncated".into()));
    }
    let val = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(val)
}

fn read_u64(data: &[u8], pos: &mut usize) -> Result<u64, PageStorageError> {
    if *pos + 8 > data.len() {
        return Err(PageStorageError::CorruptPage("u64 truncated".into()));
    }
    let val = u64::from_le_bytes([
        data[*pos],
        data[*pos + 1],
        data[*pos + 2],
        data[*pos + 3],
        data[*pos + 4],
        data[*pos + 5],
        data[*pos + 6],
        data[*pos + 7],
    ]);
    *pos += 8;
    Ok(val)
}

fn read_f32(data: &[u8], pos: &mut usize) -> Result<f32, PageStorageError> {
    if *pos + 4 > data.len() {
        return Err(PageStorageError::CorruptPage("f32 truncated".into()));
    }
    let val = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(val)
}

fn read_f32_vec(
    data: &[u8],
    pos: &mut usize,
    count: usize,
) -> Result<Vec<f32>, PageStorageError> {
    let mut v = Vec::with_capacity(count);
    for _ in 0..count {
        v.push(read_f32(data, pos)?);
    }
    Ok(v)
}

// ============================================================================
// Errors
// ============================================================================

/// Errors from page storage operations.
#[derive(Debug, thiserror::Error)]
pub enum PageStorageError {
    #[error("Corrupt page: {0}")]
    CorruptPage(String),

    #[error("Checksum mismatch: expected {expected:#010X}, got {actual:#010X}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Page not found: {0:?}")]
    PageNotFound(PageId),
}

// ============================================================================
// Fit quantization params from training vectors
// ============================================================================

/// Compute quantization scale params from a set of training vectors.
pub fn fit_quant_params(vectors: &[Vec<f32>]) -> QuantScaleParams {
    if vectors.is_empty() {
        return QuantScaleParams::default();
    }

    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    for v in vectors {
        for &val in v {
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }
    }

    let range = (max_val - min_val).max(1e-8);

    QuantScaleParams {
        min_val,
        max_val,
        scale: range / 255.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_page(vec_count: usize, dim: usize) -> PageNode {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let centroid: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>()).collect();
        let params = QuantScaleParams {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 255.0,
        };

        let mut vector_ids = Vec::with_capacity(vec_count);
        let mut encoded = Vec::new();
        let mut timestamps = Vec::with_capacity(vec_count);

        for i in 0..vec_count {
            let v: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>()).collect();
            encoded.extend_from_slice(&encode_vector(&v, QuantTier::Hot, &params));
            vector_ids.push(EmbeddingId(i as u64));
            timestamps.push(1000 + i as u64);
        }

        PageNode {
            header: PageHeader {
                page_id: PageId(42),
                version: PageVersion(1),
                checksum: 0,
                vector_count: vec_count as u32,
                dimension: dim as u16,
                quant_tier: QuantTier::Hot,
                quant_params: params,
                is_delta: false,
                collection_id: CollectionId(1),
                tenant_id: TenantId(1),
                created_at: 1000,
                modified_at: 1000,
            },
            centroid,
            sub_centroids: vec![],
            neighbor_ids: vec![PageId(10), PageId(20), PageId(30)],
            neighbor_weights: vec![0.9, 0.8, 0.7],
            encoded_vectors: encoded,
            vector_ids,
            residuals: None,
            timestamps,
            vector_tenant_ids: vec![TenantId(1); vec_count],
            bloom_filter: vec![0xFF; 8],
        }
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let page = make_test_page(10, 64);
        let bytes = serialize_page(&page);
        let recovered = deserialize_page(&bytes).unwrap();

        assert_eq!(recovered.header.page_id, page.header.page_id);
        assert_eq!(recovered.header.version, page.header.version);
        assert_eq!(recovered.vector_ids.len(), 10);
        assert_eq!(recovered.neighbor_ids.len(), 3);
        assert_eq!(recovered.centroid.len(), 64);
        assert_eq!(recovered.bloom_filter.len(), 8);
    }

    #[test]
    fn test_checksum_detects_corruption() {
        let page = make_test_page(5, 32);
        let mut bytes = serialize_page(&page);

        // Corrupt a byte in the middle
        if bytes.len() > 50 {
            bytes[50] ^= 0xFF;
        }

        let result = deserialize_page(&bytes);
        assert!(matches!(result, Err(PageStorageError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_encode_decode_8bit_roundtrip() {
        let params = QuantScaleParams {
            min_val: -1.0,
            max_val: 1.0,
            scale: 2.0 / 255.0,
        };
        let vector = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let encoded = encode_8bit(&vector, &params);
        let decoded = decode_8bit(&encoded, 5, &params);

        for (orig, dec) in vector.iter().zip(&decoded) {
            assert!((orig - dec).abs() < 0.02, "8bit: {} vs {}", orig, dec);
        }
    }

    #[test]
    fn test_encode_decode_5bit_roundtrip() {
        let params = QuantScaleParams {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 31.0,
        };
        let vector = vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.1, 0.9];
        let encoded = encode_5bit(&vector, &params);
        let decoded = decode_5bit(&encoded, 7, &params);

        for (orig, dec) in vector.iter().zip(&decoded) {
            assert!((orig - dec).abs() < 0.1, "5bit: {} vs {}", orig, dec);
        }
    }

    #[test]
    fn test_encode_decode_3bit_roundtrip() {
        let params = QuantScaleParams {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 7.0,
        };
        let vector = vec![0.0, 0.5, 1.0, 0.25, 0.75];
        let encoded = encode_3bit(&vector, &params);
        let decoded = decode_3bit(&encoded, 5, &params);

        for (orig, dec) in vector.iter().zip(&decoded) {
            assert!((orig - dec).abs() < 0.2, "3bit: {} vs {}", orig, dec);
        }
    }

    #[test]
    fn test_encoded_vector_sizes() {
        assert_eq!(encoded_vector_size(256, QuantTier::Hot), 256);
        assert_eq!(encoded_vector_size(256, QuantTier::Warm), 160); // ceil(256*5/8)
        assert_eq!(encoded_vector_size(256, QuantTier::Cold), 96); // ceil(256*3/8)
    }

    #[test]
    fn test_fit_quant_params() {
        let vectors = vec![
            vec![-1.0, 0.5, 2.0],
            vec![0.0, 1.0, 3.0],
        ];
        let params = fit_quant_params(&vectors);
        assert_eq!(params.min_val, -1.0);
        assert_eq!(params.max_val, 3.0);
    }

    #[test]
    fn test_deterministic_serialization() {
        let page = make_test_page(3, 16);
        let bytes1 = serialize_page(&page);
        let bytes2 = serialize_page(&page);
        assert_eq!(bytes1, bytes2, "Serialization must be deterministic");
    }

    #[test]
    fn test_crc32_basic() {
        let data = b"hello world";
        let checksum = crc32(data);
        assert_ne!(checksum, 0);
        // Same input should produce same checksum
        assert_eq!(checksum, crc32(data));
        // Different input should produce different checksum
        assert_ne!(checksum, crc32(b"hello worlD"));
    }
}
