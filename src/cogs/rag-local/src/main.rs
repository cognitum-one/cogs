//! Cognitum Cog: RAG Local
//!
//! Local retrieval-augmented generation. Store text embeddings as vectors.
//! Query by similarity. Return relevant stored context. Simple TF-IDF-like
//! embedding (bag of character trigrams).
//!
//! Usage:
//!   cog-rag-local --once
//!   cog-rag-local --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Generate a simple character trigram embedding of dimension DIM
/// Uses hash-based feature hashing to map trigrams to fixed dimensions
fn trigram_embed(text: &str) -> [f64; DIM] {
    let mut vec = [0.0; DIM];
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();

    if chars.len() < 3 {
        // For very short text, use character codes
        for (i, &c) in chars.iter().enumerate() {
            vec[i % DIM] += c as u32 as f64 / 128.0;
        }
        normalize(&mut vec);
        return vec;
    }

    // Count trigrams and hash to DIM buckets
    let mut trigram_count = 0u32;
    for window in chars.windows(3) {
        let hash = simple_hash(window);
        let bucket = (hash as usize) % DIM;
        vec[bucket] += 1.0;
        trigram_count += 1;
    }

    // TF normalization
    if trigram_count > 0 {
        for v in &mut vec {
            *v /= trigram_count as f64;
        }
    }

    // Apply log(1+x) smoothing
    for v in &mut vec {
        *v = (1.0 + *v).ln();
    }

    normalize(&mut vec);
    vec
}

/// Simple hash function for character slices
fn simple_hash(chars: &[char]) -> u32 {
    let mut hash = 5381u32;
    for &c in chars {
        hash = hash.wrapping_mul(33).wrapping_add(c as u32);
    }
    hash
}

/// L2 normalize a vector
fn normalize(vec: &mut [f64; DIM]) {
    let mag: f64 = vec.iter().map(|v| v * v).sum::<f64>().sqrt();
    if mag > 1e-10 {
        for v in vec.iter_mut() {
            *v /= mag;
        }
    }
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f64; DIM], b: &[f64; DIM]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if mag_a < 1e-10 || mag_b < 1e-10 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

/// Build a text description from sensor data
fn sensor_to_text(channels: &std::collections::HashMap<String, Vec<f64>>) -> String {
    let mut parts = Vec::new();
    for (ch, vals) in channels {
        let mean = vals.iter().sum::<f64>() / vals.len().max(1) as f64;
        let level = if mean > 0.7 { "high" } else if mean > 0.3 { "medium" } else { "low" };
        parts.push(format!("{ch} {level} activity {mean:.2}"));
    }
    parts.join(" ")
}

#[derive(serde::Serialize)]
struct RagResult {
    query_text: String,
    query_embedding: [f64; DIM],
    retrieved_count: usize,
    top_similarity: f64,
    context_summary: String,
    stored: bool,
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn query_store(vector: &[f64; DIM]) -> Result<Vec<(Vec<f64>, f64)>, String> {
    let payload = serde_json::json!({ "vector": vector, "k": 5, "metric": "cosine" });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/query HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    let text = String::from_utf8_lossy(&resp);
    let json_start = text.find('{').or_else(|| text.find('[')).unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&text[json_start..]).unwrap_or(serde_json::json!({"results":[]}));
    let results = parsed.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    Ok(results.iter().filter_map(|r| {
        let vec: Vec<f64> = r.get("vector").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect())?;
        let score = r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
        if vec.len() == DIM { Some((vec, score)) } else { None }
    }).collect())
}

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once() -> Result<RagResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    let text = sensor_to_text(&channels);
    let embedding = trigram_embed(&text);

    // Retrieve similar past contexts
    let retrieved = query_store(&embedding).unwrap_or_default();
    let top_sim = retrieved.first().map(|(v, _)| {
        let mut arr = [0.0; DIM];
        for (i, &x) in v.iter().enumerate().take(DIM) { arr[i] = x; }
        cosine_similarity(&embedding, &arr)
    }).unwrap_or(0.0);

    let context = if retrieved.is_empty() {
        "no prior context available".into()
    } else {
        format!("{} similar contexts found (top similarity: {:.3})", retrieved.len(), top_sim)
    };

    let stored = store_vector(&embedding).is_ok();

    Ok(RagResult {
        query_text: text,
        query_embedding: embedding,
        retrieved_count: retrieved.len(),
        top_similarity: top_sim,
        context_summary: context,
        stored,
        vector: embedding,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-rag-local] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-rag-local] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
