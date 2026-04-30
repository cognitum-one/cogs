//! Cognitum Cog: Psycho-Symbolic
//!
//! Hybrid reasoning over signal patterns. Build a simple knowledge graph
//! (entity-relation triples stored as vectors). Reason over graph to
//! detect complex multi-step events.
//!
//! Usage:
//!   cog-psycho-symbolic --once
//!   cog-psycho-symbolic --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// A triple in the knowledge graph: (subject, relation, object)
#[derive(Clone)]
struct Triple {
    subject: String,
    relation: String,
    object: String,
    confidence: f64,
    timestamp: u64,
}

/// Simple knowledge graph for event reasoning
struct KnowledgeGraph {
    triples: Vec<Triple>,
    max_triples: usize,
}

impl KnowledgeGraph {
    fn new(max_triples: usize) -> Self {
        Self { triples: Vec::new(), max_triples }
    }

    fn add(&mut self, subject: &str, relation: &str, object: &str, confidence: f64, timestamp: u64) {
        self.triples.push(Triple {
            subject: subject.into(),
            relation: relation.into(),
            object: object.into(),
            confidence,
            timestamp,
        });
        // Evict oldest if full
        while self.triples.len() > self.max_triples {
            self.triples.remove(0);
        }
    }

    /// Query triples matching a pattern (None = wildcard)
    fn query(&self, subject: Option<&str>, relation: Option<&str>, object: Option<&str>) -> Vec<&Triple> {
        self.triples.iter().filter(|t| {
            subject.map(|s| t.subject == s).unwrap_or(true)
                && relation.map(|r| t.relation == r).unwrap_or(true)
                && object.map(|o| t.object == o).unwrap_or(true)
        }).collect()
    }

    /// Multi-hop reasoning: find chains A->B->C
    fn reason_chain(&self, start: &str, max_hops: usize) -> Vec<Vec<&Triple>> {
        let mut chains: Vec<Vec<&Triple>> = Vec::new();
        let first_hop = self.query(Some(start), None, None);

        for t1 in &first_hop {
            let chain = vec![*t1];
            if max_hops == 1 {
                chains.push(chain);
                continue;
            }
            let second_hop = self.query(Some(&t1.object), None, None);
            for t2 in &second_hop {
                let mut chain2 = chain.clone();
                chain2.push(*t2);
                if max_hops == 2 {
                    chains.push(chain2);
                    continue;
                }
                let third_hop = self.query(Some(&t2.object), None, None);
                for t3 in &third_hop {
                    let mut chain3 = chain2.clone();
                    chain3.push(*t3);
                    chains.push(chain3);
                }
            }
        }
        chains
    }

    /// Encode graph state as feature vector
    fn to_vector(&self) -> [f64; DIM] {
        let n = self.triples.len() as f64;
        let unique_entities: std::collections::HashSet<&str> = self.triples.iter()
            .flat_map(|t| vec![t.subject.as_str(), t.object.as_str()])
            .collect();
        let unique_relations: std::collections::HashSet<&str> = self.triples.iter()
            .map(|t| t.relation.as_str())
            .collect();
        let avg_conf = self.triples.iter().map(|t| t.confidence).sum::<f64>() / n.max(1.0);

        [
            n / 100.0,
            unique_entities.len() as f64 / 20.0,
            unique_relations.len() as f64 / 10.0,
            avg_conf,
            0.0, 0.0, 0.0, 0.0,
        ]
    }
}

/// Detect complex events from sensor patterns
fn detect_events(channels: &std::collections::HashMap<String, Vec<f64>>) -> Vec<(String, String, String, f64)> {
    let mut events = Vec::new();

    for (ch, vals) in channels {
        if vals.is_empty() { continue; }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64;
        let last = *vals.last().unwrap();

        // Generate triples from observed patterns
        if mean > 0.5 {
            events.push((ch.clone(), "has_state".into(), "active".into(), mean));
        } else {
            events.push((ch.clone(), "has_state".into(), "idle".into(), 1.0 - mean));
        }

        if var > 0.1 {
            events.push((ch.clone(), "exhibits".into(), "high_variance".into(), var.min(1.0)));
        }

        if last > mean + 2.0 * var.sqrt() {
            events.push((ch.clone(), "detected".into(), "spike".into(), 0.9));
        }

        // Cross-channel relationships
        for (ch2, vals2) in channels {
            if ch == ch2 || vals2.is_empty() { continue; }
            let mean2 = vals2.iter().sum::<f64>() / vals2.len() as f64;
            if (mean - mean2).abs() < 0.1 {
                events.push((ch.clone(), "correlates_with".into(), ch2.clone(), 0.8));
            }
        }
    }
    events
}

/// Reason about complex events using the knowledge graph
fn reason_complex_events(kg: &KnowledgeGraph) -> Vec<String> {
    let mut events = Vec::new();

    // Rule: if A is active AND A correlates_with B AND B has spike -> "propagation event"
    let active = kg.query(None, Some("has_state"), Some("active"));
    for t in &active {
        let correlations = kg.query(Some(&t.subject), Some("correlates_with"), None);
        for corr in &correlations {
            let spikes = kg.query(Some(&corr.object), Some("detected"), Some("spike"));
            if !spikes.is_empty() {
                events.push(format!("PROPAGATION: {} -> {} via correlation", t.subject, corr.object));
            }
        }
    }

    // Rule: if 3+ channels are active -> "distributed event"
    if active.len() >= 3 {
        events.push(format!("DISTRIBUTED_EVENT: {} channels simultaneously active", active.len()));
    }

    // Rule: if channel has high_variance AND spike -> "anomalous burst"
    let high_var = kg.query(None, Some("exhibits"), Some("high_variance"));
    for t in &high_var {
        let spikes = kg.query(Some(&t.subject), Some("detected"), Some("spike"));
        if !spikes.is_empty() {
            events.push(format!("ANOMALOUS_BURST: {} (high variance + spike)", t.subject));
        }
    }

    events
}

#[derive(serde::Serialize)]
struct PsychoResult {
    graph_size: usize,
    entities: usize,
    relations_added: usize,
    complex_events: Vec<String>,
    reasoning_chains: usize,
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
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

fn run_once(kg: &mut KnowledgeGraph) -> Result<PsychoResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    // Detect events and add to knowledge graph
    let events = detect_events(&channels);
    for (s, r, o, conf) in &events {
        kg.add(s, r, o, *conf, now);
    }

    // Reason over the graph
    let complex_events = reason_complex_events(kg);

    // Count reasoning chains
    let entities: std::collections::HashSet<&str> = kg.triples.iter()
        .flat_map(|t| vec![t.subject.as_str(), t.object.as_str()])
        .collect();
    let mut chain_count = 0;
    for e in &entities {
        chain_count += kg.reason_chain(e, 2).len();
    }

    let mut vector = kg.to_vector();
    vector[4] = complex_events.len() as f64 / 10.0;
    vector[5] = chain_count as f64 / 50.0;
    vector[6] = events.len() as f64 / 20.0;
    vector[7] = channels.len() as f64 / 10.0;

    let _ = store_vector(&vector);

    Ok(PsychoResult {
        graph_size: kg.triples.len(),
        entities: entities.len(),
        relations_added: events.len(),
        complex_events,
        reasoning_chains: chain_count,
        vector,
        timestamp: now,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-psycho-symbolic] starting (interval={interval}s, once={once})");

    let mut kg = KnowledgeGraph::new(200);

    loop {
        let start = Instant::now();
        match run_once(&mut kg) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.complex_events.is_empty() {
                    eprintln!("[cog-psycho-symbolic] ALERT: {:?}", r.complex_events);
                }
            }
            Err(e) => eprintln!("[cog-psycho-symbolic] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
