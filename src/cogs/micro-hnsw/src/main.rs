//! Cognitum Cog: Micro-HNSW
//!
//! Hierarchical Navigable Small World graph for fast fingerprint matching.
//! Build graph index from stored vectors. Query nearest neighbors in
//! O(log N). On-device classification.
//!
//! Usage:
//!   cog-micro-hnsw --once
//!   cog-micro-hnsw --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;
const M: usize = 4; // Max connections per layer
const EF_CONSTRUCTION: usize = 8; // Search width during construction
const MAX_LEVEL: usize = 3;

/// A node in the HNSW graph
struct HnswNode {
    vector: [f64; DIM],
    /// Neighbors per layer: layer -> vec of node indices
    neighbors: Vec<Vec<usize>>,
    level: usize,
}

/// Micro HNSW index
struct HnswIndex {
    nodes: Vec<HnswNode>,
    entry_point: Option<usize>,
    max_level: usize,
}

impl HnswIndex {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            entry_point: None,
            max_level: 0,
        }
    }

    fn distance(a: &[f64; DIM], b: &[f64; DIM]) -> f64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Assign a random level to a new node (exponential distribution)
    fn random_level(node_idx: usize) -> usize {
        // Deterministic "random" based on node index for reproducibility
        let hash = node_idx.wrapping_mul(2654435761) >> 16;
        let mut level = 0;
        let mut h = hash;
        while h & 1 == 1 && level < MAX_LEVEL {
            level += 1;
            h >>= 1;
        }
        level
    }

    /// Search for ef nearest neighbors at a single layer
    fn search_layer(&self, query: &[f64; DIM], entry: usize, ef: usize, layer: usize) -> Vec<(usize, f64)> {
        let mut visited = vec![false; self.nodes.len()];
        let mut candidates: Vec<(usize, f64)> = vec![(entry, Self::distance(query, &self.nodes[entry].vector))];
        let mut results = candidates.clone();
        visited[entry] = true;

        while let Some(pos) = candidates.iter().enumerate()
            .min_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
        {
            let (c_idx, c_dist) = candidates.remove(pos);

            // If closest candidate is farther than worst result, stop
            if results.len() >= ef {
                let worst = results.iter().map(|r| r.1).fold(0.0f64, f64::max);
                if c_dist > worst {
                    break;
                }
            }

            // Explore neighbors
            if layer < self.nodes[c_idx].neighbors.len() {
                for &neighbor in &self.nodes[c_idx].neighbors[layer] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        let d = Self::distance(query, &self.nodes[neighbor].vector);
                        let should_add = results.len() < ef || {
                            let worst = results.iter().map(|r| r.1).fold(0.0f64, f64::max);
                            d < worst
                        };
                        if should_add {
                            candidates.push((neighbor, d));
                            results.push((neighbor, d));
                            if results.len() > ef {
                                // Remove worst
                                if let Some(worst_pos) = results.iter().enumerate()
                                    .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap_or(std::cmp::Ordering::Equal))
                                    .map(|(i, _)| i)
                                {
                                    results.remove(worst_pos);
                                }
                            }
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Insert a vector into the HNSW index
    fn insert(&mut self, vector: [f64; DIM]) {
        let node_idx = self.nodes.len();
        let level = Self::random_level(node_idx);

        let mut node = HnswNode {
            vector,
            neighbors: vec![Vec::new(); level + 1],
            level,
        };

        if self.nodes.is_empty() {
            self.nodes.push(node);
            self.entry_point = Some(0);
            self.max_level = level;
            return;
        }

        let mut ep = self.entry_point.unwrap();

        // Traverse from top to node's level, greedily descending
        for lc in (level + 1..=self.max_level).rev() {
            let results = self.search_layer(&vector, ep, 1, lc);
            if let Some(&(closest, _)) = results.first() {
                ep = closest;
            }
        }

        // Insert at each layer from node's level down to 0
        self.nodes.push(node);
        for lc in (0..=level.min(self.max_level)).rev() {
            let results = self.search_layer(&vector, ep, EF_CONSTRUCTION, lc);

            // Connect to M nearest
            let connect_to: Vec<usize> = results.iter().take(M).map(|r| r.0).collect();
            self.nodes[node_idx].neighbors[lc] = connect_to.clone();

            // Bidirectional links
            for &neighbor in &connect_to {
                if lc < self.nodes[neighbor].neighbors.len() {
                    self.nodes[neighbor].neighbors[lc].push(node_idx);
                    // Prune if too many connections
                    if self.nodes[neighbor].neighbors[lc].len() > M * 2 {
                        let nv = self.nodes[neighbor].vector;
                        let mut scored: Vec<(usize, f64)> = self.nodes[neighbor].neighbors[lc]
                            .iter()
                            .map(|&idx| (idx, Self::distance(&nv, &self.nodes[idx].vector)))
                            .collect();
                        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                        self.nodes[neighbor].neighbors[lc] = scored.iter().take(M).map(|s| s.0).collect();
                    }
                }
            }

            if let Some(&(closest, _)) = results.first() {
                ep = closest;
            }
        }

        if level > self.max_level {
            self.max_level = level;
            self.entry_point = Some(node_idx);
        }
    }

    /// Query k nearest neighbors
    fn query(&self, vector: &[f64; DIM], k: usize) -> Vec<(usize, f64)> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let mut ep = self.entry_point.unwrap();

        // Greedy descent from top
        for lc in (1..=self.max_level).rev() {
            let results = self.search_layer(vector, ep, 1, lc);
            if let Some(&(closest, _)) = results.first() {
                ep = closest;
            }
        }

        // Search at layer 0 with ef = max(k, EF_CONSTRUCTION)
        let mut results = self.search_layer(vector, ep, k.max(EF_CONSTRUCTION), 0);
        results.truncate(k);
        results
    }
}

#[derive(serde::Serialize)]
struct HnswResult {
    index_size: usize,
    query_results: Vec<HnswMatch>,
    build_time_us: u64,
    query_time_us: u64,
    max_level: usize,
    classification: String,
    vector: [f64; DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct HnswMatch {
    rank: usize,
    distance: f64,
    similarity: f64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); if buf.len() > 262144 { break; } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

fn query_store(vector: &[f64; DIM]) -> Result<Vec<Vec<f64>>, String> {
    let payload = serde_json::json!({ "vector": vector, "k": 20, "metric": "cosine" });
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
    conn.read_to_end(&mut resp).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&resp);
    let json_start = text.find('{').or_else(|| text.find('[')).unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&text[json_start..]).unwrap_or(serde_json::json!({"results":[]}));
    let results = parsed.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    Ok(results.iter().filter_map(|r| {
        r.get("vector").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().filter_map(|x| x.as_f64()).collect::<Vec<f64>>()
        })
    }).filter(|v| v.len() == DIM).collect())
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

fn extract_features(values: &[f64]) -> [f64; DIM] {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let energy = values.iter().map(|v| v * v).sum::<f64>() / n;
    let max_v = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_v = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut zc = 0;
    for i in 1..values.len() { if (values[i-1] >= mean) != (values[i] >= mean) { zc += 1; } }
    let max_d = values.windows(2).map(|w| (w[1]-w[0]).abs()).fold(0.0f64, f64::max);
    let kurtosis = if var > 1e-10 {
        let sd = var.sqrt();
        values.iter().map(|v| ((v - mean)/sd).powi(4)).sum::<f64>() / n - 3.0
    } else { 0.0 };
    [mean, var.sqrt(), energy, max_v - min_v, zc as f64 / n, max_d, kurtosis, n / 100.0]
}

fn run_once() -> Result<HnswResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let query_vec = extract_features(&values);

    // Fetch stored vectors from the seed store
    let stored = query_store(&query_vec).unwrap_or_default();

    // Build HNSW index
    let build_start = Instant::now();
    let mut index = HnswIndex::new();
    for v in &stored {
        let mut arr = [0.0; DIM];
        for (i, &x) in v.iter().enumerate().take(DIM) { arr[i] = x; }
        index.insert(arr);
    }
    let build_us = build_start.elapsed().as_micros() as u64;

    // Query
    let q_start = Instant::now();
    let results = index.query(&query_vec, 5);
    let query_us = q_start.elapsed().as_micros() as u64;

    let matches: Vec<HnswMatch> = results.iter().enumerate().map(|(rank, (_, dist))| {
        HnswMatch {
            rank: rank + 1,
            distance: *dist,
            similarity: 1.0 / (1.0 + dist),
        }
    }).collect();

    let classification = if matches.is_empty() {
        "unknown".into()
    } else if matches[0].similarity > 0.9 {
        "exact_match".into()
    } else if matches[0].similarity > 0.7 {
        "close_match".into()
    } else {
        "novel".into()
    };

    let _ = store_vector(&query_vec);

    Ok(HnswResult {
        index_size: index.nodes.len(),
        query_results: matches,
        build_time_us: build_us,
        query_time_us: query_us,
        max_level: index.max_level,
        classification,
        vector: query_vec,
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

    eprintln!("[cog-micro-hnsw] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.classification == "novel" {
                    eprintln!("[cog-micro-hnsw] ALERT: novel fingerprint detected");
                }
            }
            Err(e) => eprintln!("[cog-micro-hnsw] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
