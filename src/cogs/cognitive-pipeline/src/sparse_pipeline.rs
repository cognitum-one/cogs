// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Cognitive microkernel pipeline: FastGRNN reflex → event gate → SmolLM2 summary.
//!
//! Architecture (per Seed):
//!   sensor frames → FastGRNN score → event gate → SmolLM2 summary → delta
//!
//! Endpoints:
//!   POST /api/v1/llm/sparse/pipeline          — process one sensor window
//!   PUT  /api/v1/llm/sparse/pipeline/weights  — upload pre-trained GRNN weights
//!   GET  /api/v1/llm/sparse/pipeline/weights  — download current weights
//!   GET  /api/v1/llm/sparse/pipeline/status   — counters + timing + memory

use crate::http::Request;
use crate::sparse_fastgrnn::{FastGrnnDetector, DEFAULT_HIDDEN_DIM, DEFAULT_INPUT_DIM};
use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ── Global pipeline state ───────────────────────────────────────────────────

static PIPELINE: Mutex<Option<CognitivePipeline>> = Mutex::new(None);

/// Separate pipeline driven by the sensor reflex loop.
/// Keeps distinct input_dim (feature vector size) and warmup counter from
/// the HTTP pipeline so sensor data and API demos don't interfere.
static SENSOR_PIPELINE: Mutex<Option<CognitivePipeline>> = Mutex::new(None);

/// Cognitive event ring buffer — stores the last EVENT_RING_CAP anomaly events
/// emitted by the sensor-loop FastGRNN gate. Survives across HTTP requests so
/// callers can poll /api/v1/llm/sparse/pipeline/events without losing events.
const EVENT_RING_CAP: usize = 100;
static SENSOR_EVENTS: Mutex<VecDeque<CognitiveEvent>> = Mutex::new(VecDeque::new());

/// Guard: set while a sensor-loop inference thread is running.
/// sensor_tick returns immediately (no new summary) if already set.
static INFERENCE_BUSY: AtomicBool = AtomicBool::new(false);

/// JSONL file that persists SENSOR_EVENTS across restarts.
/// Written append-only; compacted to the last EVENT_RING_CAP lines on startup load.
///
/// Resolves under `COGNITUM_COG_DATA_DIR` (set by the agent at /start), defaulting
/// to the canonical sandbox path per ADR-095 §4. Cog data lives under
/// `<dir>/cognitive-events.jsonl` — never the agent-global `/var/lib/cognitum/`.
fn event_store_path() -> &'static str {
    use std::sync::OnceLock;
    static ESP: OnceLock<String> = OnceLock::new();
    ESP.get_or_init(|| {
        let base = std::env::var("COGNITUM_COG_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/cognitum/apps/cognitive-pipeline".to_string());
        format!("{}/cognitive-events.jsonl", base)
    })
    .as_str()
}

// ── Pipeline state struct ───────────────────────────────────────────────────

/// Minimum windows before event gating activates. Gives EMA time to
/// converge so startup transients don't trigger spurious events.
const WARMUP_WINDOWS: u64 = 10;

pub struct CognitivePipeline {
    pub detector:    FastGrnnDetector,
    pub threshold:   f32,
    pub sensor_type: String,

    // Cumulative counters
    pub frames_seen:          u64,
    pub windows_seen:         u64,
    pub events_gated:         u64,
    pub summaries_generated:  u64,

    // Timing accumulators (milliseconds)
    pub total_fastgrnn_ms: f64,
    pub total_summary_ms:  f64,

    // Last-window state
    pub last_anomaly_score: f32,
    pub last_gate_decision: &'static str,   // "triggered" | "suppressed" | "warmup"
    pub last_summary:       Option<String>,
    pub last_tok_per_sec:   f32,
}

impl CognitivePipeline {
    fn new(sensor_type: &str, input_dim: usize, threshold: f32) -> Self {
        Self {
            detector:   FastGrnnDetector::new(input_dim, DEFAULT_HIDDEN_DIM),
            threshold,
            sensor_type: sensor_type.to_string(),
            frames_seen: 0, windows_seen: 0,
            events_gated: 0, summaries_generated: 0,
            total_fastgrnn_ms: 0.0, total_summary_ms: 0.0,
            last_anomaly_score: 0.0,
            last_gate_decision: "warmup",
            last_summary: None,
            last_tok_per_sec: 0.0,
        }
    }
}

// ── Cognitive event ─────────────────────────────────────────────────────────

/// A single anomaly event emitted by the cognitive pipeline.
/// Serialised as JSON for the /events endpoint and future delta-sync payloads.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CognitiveEvent {
    /// Unix seconds at event time.
    pub timestamp_s:    u64,
    /// Sensor-loop window counter at the moment of detection.
    pub windows_seen:   u64,
    /// FastGRNN hidden-state norm that crossed the threshold.
    pub anomaly_score:  f32,
    pub sensor_type:    String,
    /// SmolLM2 one-sentence summary (may be empty if model not loaded).
    pub summary:        String,
    pub rss_mb:         f32,
    /// Wall-clock time SmolLM2 spent generating this summary (ms).
    #[serde(default)]
    pub inference_ms:   f64,
}

// ── HTTP shapes ─────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct PipelineRequest {
    #[serde(default = "default_sensor")]
    sensor_type: String,
    /// Sensor frames — each inner vec is one time-step's feature vector.
    /// Accepts key "frames" (preferred) or "window" (compat alias).
    #[serde(alias = "window")]
    frames: Vec<Vec<f32>>,
    #[serde(default = "default_threshold")]
    threshold: f32,
    #[serde(default = "default_summary_tokens")]
    max_summary_tokens: usize,
    /// LLM prompt prefix (e.g. "Describe this sensor anomaly in one sentence:").
    #[serde(alias = "prompt")]
    prompt_prefix: Option<String>,
}

fn default_sensor()         -> String { "generic".to_string() }
fn default_threshold()      -> f32    { 0.5 }
fn default_summary_tokens() -> usize  { 32 }

#[derive(serde::Serialize)]
struct PipelineResponse {
    event_triggered:    bool,
    anomaly_score:      f32,
    threshold:          f32,
    frames:             usize,
    summary:            Option<String>,
    hidden_norm:        f32,
    // Timing
    fastgrnn_ms:        f64,
    summary_ms:         f64,
    total_ms:           u64,
    // Cumulative stats
    frames_seen:        u64,
    windows_seen:       u64,
    events_gated:       u64,
    summaries_generated: u64,
    avg_fastgrnn_ms:    f64,
    avg_summary_ms:     f64,
    last_gate_decision: &'static str,
    tok_per_sec:        f32,
    rss_mb:             f32,
}

// ── Handler: POST /api/v1/llm/sparse/pipeline ──────────────────────────────

pub fn handle_sparse_pipeline(
    req: &Request,
    state: &crate::api::DeviceState,
    authorized: bool,
) -> (usize, String) {
    if !authorized {
        return err(401, "UNAUTHORIZED", "authorization required");
    }
    let preq: PipelineRequest = match serde_json::from_slice(&req.body) {
        Ok(r) => r,
        Err(e) => return err(400, "BAD_REQUEST", &format!("invalid JSON: {e}")),
    };
    if preq.frames.is_empty() {
        return err(400, "BAD_REQUEST", "frames must not be empty");
    }
    if preq.max_summary_tokens > 200 {
        return err(400, "BAD_REQUEST", "max_summary_tokens exceeds 200");
    }

    let t0 = Instant::now();

    // ── FastGRNN scoring (locked) ─────────────────────────────────────────
    let (anomaly_score, final_hidden, grnn_ms, n_frames) = {
        let mut guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        let pipeline = guard.get_or_insert_with(|| {
            let input_dim = preq.frames.first().map(|v| v.len()).unwrap_or(DEFAULT_INPUT_DIM);
            CognitivePipeline::new(&preq.sensor_type, input_dim, preq.threshold)
        });
        pipeline.threshold   = preq.threshold;
        pipeline.sensor_type = preq.sensor_type.clone();

        let t_grnn = Instant::now();
        let (score, hidden) = pipeline.detector.score_window(&preq.frames);
        let grnn_ms = t_grnn.elapsed().as_secs_f64() * 1000.0;

        let n_frames = preq.frames.len() as u64;
        pipeline.frames_seen  += n_frames;
        pipeline.windows_seen += 1;
        pipeline.total_fastgrnn_ms += grnn_ms;
        pipeline.last_anomaly_score = score;

        (score, hidden, grnn_ms, n_frames)
    };

    // Check warmup: suppress event gate until EMA has converged.
    let in_warmup = {
        let guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|p| p.windows_seen <= WARMUP_WINDOWS).unwrap_or(true)
    };

    // ── Event gate ────────────────────────────────────────────────────────
    let (summary, summary_ms, tok_per_sec, gate_decision) =
        if !in_warmup && anomaly_score >= preq.threshold {
            let prompt = build_prompt(&preq, anomaly_score);
            let t_llm = Instant::now();
            let text = run_llm_summary(state, &prompt, preq.max_summary_tokens);
            let sm = t_llm.elapsed().as_secs_f64() * 1000.0;
            let tps = text.as_deref().map(|t| {
                let words = t.split_whitespace().count();
                if sm > 0.0 { words as f32 / (sm as f32 / 1000.0) } else { 0.0 }
            }).unwrap_or(0.0);

            // Update counters (re-lock after LLM call)
            {
                let mut guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(p) = guard.as_mut() {
                    p.events_gated += 1;
                    p.total_summary_ms += sm;
                    p.last_gate_decision = "triggered";
                    p.last_tok_per_sec   = tps;
                    if text.is_some() {
                        p.summaries_generated += 1;
                        p.last_summary = text.clone();
                    }
                }
            }
            (text, sm, tps, "triggered")
        } else {
            let decision: &'static str = if in_warmup { "warmup" } else { "suppressed" };
            {
                let mut guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(p) = guard.as_mut() {
                    p.last_gate_decision = decision;
                }
            }
            (None, 0.0, 0.0, decision)
        };

    // ── Collect final stats ───────────────────────────────────────────────
    let (ws, fs, eg, sg, af_ms, as_ms) = {
        let guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|p| (
            p.windows_seen, p.frames_seen,
            p.events_gated, p.summaries_generated,
            if p.windows_seen > 0 { p.total_fastgrnn_ms / p.windows_seen as f64 } else { 0.0 },
            if p.events_gated > 0 { p.total_summary_ms  / p.events_gated  as f64 } else { 0.0 },
        )).unwrap_or((0, 0, 0, 0, 0.0, 0.0))
    };

    ok(PipelineResponse {
        event_triggered:     gate_decision == "triggered",
        anomaly_score,
        threshold:           preq.threshold,
        frames:              n_frames as usize,
        summary,
        hidden_norm:         vec_norm(&final_hidden),
        fastgrnn_ms:         grnn_ms,
        summary_ms,
        total_ms:            t0.elapsed().as_millis() as u64,
        frames_seen:         fs,
        windows_seen:        ws,
        events_gated:        eg,
        summaries_generated: sg,
        avg_fastgrnn_ms:     af_ms,
        avg_summary_ms:      as_ms,
        last_gate_decision:  gate_decision,
        tok_per_sec,
        rss_mb:              read_rss_mb(),
    })
}

// ── Handler: GET /api/v1/llm/sparse/pipeline/status ────────────────────────

pub fn handle_pipeline_status() -> (usize, String) {
    let guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        None => ok_val(serde_json::json!({
            "initialized": false,
            "frames_seen": 0, "windows_seen": 0,
            "events_gated": 0, "summaries_generated": 0,
            "rss_mb": read_rss_mb(),
        })),
        Some(p) => ok_val(serde_json::json!({
            "initialized":         true,
            "sensor_type":         p.sensor_type,
            "threshold":           p.threshold,
            "input_dim":           p.detector.input_dim,
            "hidden_dim":          p.detector.hidden_dim,
            "frames_seen":         p.frames_seen,
            "windows_seen":        p.windows_seen,
            "events_gated":        p.events_gated,
            "summaries_generated": p.summaries_generated,
            "avg_fastgrnn_ms":
                if p.windows_seen > 0 { p.total_fastgrnn_ms / p.windows_seen as f64 } else { 0.0 },
            "avg_summary_ms":
                if p.events_gated > 0 { p.total_summary_ms / p.events_gated as f64 } else { 0.0 },
            "last_anomaly_score":  p.last_anomaly_score,
            "last_gate_decision":  p.last_gate_decision,
            "last_summary":        p.last_summary.as_deref().unwrap_or(""),
            "baseline_ema":        p.detector.baseline_ema,
            "last_tok_per_sec":    p.last_tok_per_sec,
            "rss_mb":              read_rss_mb(),
        })),
    }
}

// ── Handler: GET /api/v1/llm/sparse/pipeline/sensor-status ─────────────────

/// Expose SENSOR_PIPELINE counters for observability.
/// Endpoint: GET /api/v1/llm/sparse/pipeline/sensor-status
pub fn handle_sensor_pipeline_status() -> (usize, String) {
    let guard = SENSOR_PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        None => ok_val(serde_json::json!({
            "source":       "sensor_reflex_loop",
            "initialized":  false,
            "frames_seen":  0, "windows_seen": 0,
            "events_gated": 0, "summaries_generated": 0,
            "rss_mb":       read_rss_mb(),
        })),
        Some(p) => {
            let avg_infer_ms = if p.summaries_generated > 0 {
                p.total_summary_ms / p.summaries_generated as f64
            } else { 0.0 };
            ok_val(serde_json::json!({
                "source":              "sensor_reflex_loop",
                "initialized":         true,
                "sensor_type":         p.sensor_type,
                "threshold":           p.threshold,
                "input_dim":           p.detector.input_dim,
                "frames_seen":         p.frames_seen,
                "windows_seen":        p.windows_seen,
                "events_gated":        p.events_gated,
                "summaries_generated": p.summaries_generated,
                "total_summary_ms":    p.total_summary_ms,
                "avg_inference_ms":    avg_infer_ms,
                "inference_busy":      INFERENCE_BUSY.load(Ordering::Relaxed),
                "last_anomaly_score":  p.last_anomaly_score,
                "last_gate_decision":  p.last_gate_decision,
                "last_summary":        p.last_summary.as_deref().unwrap_or(""),
                "baseline_ema":        p.detector.baseline_ema,
                "warmup_windows":      WARMUP_WINDOWS,
                "rss_mb":              read_rss_mb(),
            }))
        },
    }
}

// ── Handler: GET /api/v1/llm/sparse/pipeline/events ────────────────────────

/// Return the most recent cognitive anomaly events from the sensor loop.
///
/// Query params:
///   `limit`  — max events to return (default 20, max 100)
///   `since`  — Unix-second timestamp; return only events newer than this value
///              (exclusive).  Clients can store the `next_since` field from the
///              last response and poll with it for zero-overhead delta sync.
///
/// Endpoint: GET /api/v1/llm/sparse/pipeline/events[?limit=N][&since=TS]
pub fn handle_sensor_events(req: &crate::http::Request) -> (usize, String) {
    let qs = req.path.split('?').nth(1).unwrap_or("");
    let mut limit: usize = 20;
    let mut since: Option<u64> = None;
    for kv in qs.split('&') {
        let mut it = kv.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("limit"), Some(v)) => { limit = v.parse().unwrap_or(20).min(EVENT_RING_CAP); }
            (Some("since"), Some(v)) => { since = v.parse().ok(); }
            _ => {}
        }
    }
    limit = limit.min(EVENT_RING_CAP);

    let guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    let events: Vec<&CognitiveEvent> = guard.iter().rev()
        .filter(|e| since.map_or(true, |s| e.timestamp_s > s))
        .take(limit)
        .collect();
    let next_since = events.first().map(|e| e.timestamp_s).unwrap_or(since.unwrap_or(0));
    ok_val(serde_json::json!({
        "total":      guard.len(),
        "returned":   events.len(),
        "limit":      limit,
        "since":      since,
        "next_since": next_since,
        "events":     events,
    }))
}

// ── Handler: DELETE /api/v1/llm/sparse/pipeline/events ─────────────────────

/// Flush the event ring buffer.  Useful for upstream sync agents that want to
/// acknowledge consumed events without reprocessing them on the next poll.
/// Endpoint: DELETE /api/v1/llm/sparse/pipeline/events
pub fn handle_sensor_events_clear(authorized: bool) -> (usize, String) {
    if !authorized {
        return err(401, "UNAUTHORIZED", "authorization required");
    }
    let cleared = {
        let mut guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
        let n = guard.len();
        guard.clear();
        n
    };
    compact_event_store();
    ok_val(serde_json::json!({ "cleared": cleared }))
}

// ── Handler: PUT /api/v1/llm/sparse/pipeline/weights ───────────────────────

pub fn handle_pipeline_weights_upload(req: &Request, authorized: bool) -> (usize, String) {
    if !authorized {
        return err(401, "UNAUTHORIZED", "authorization required");
    }
    let json: serde_json::Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => return err(400, "BAD_REQUEST", &format!("invalid JSON: {e}")),
    };
    let det = match FastGrnnDetector::from_json(&json) {
        Some(d) => d,
        None => return err(400, "BAD_REQUEST", "missing or invalid weight fields"),
    };
    let mut guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_mut() {
        Some(p) => p.detector = det,
        None => {
            *guard = Some(CognitivePipeline::new("generic", det.input_dim, 0.5));
            guard.as_mut().unwrap().detector = det;
        }
    }
    ok_val(serde_json::json!({ "status": "weights loaded" }))
}

// ── Handler: GET /api/v1/llm/sparse/pipeline/weights ───────────────────────

pub fn handle_pipeline_weights_download(authorized: bool) -> (usize, String) {
    if !authorized {
        return err(401, "UNAUTHORIZED", "authorization required");
    }
    let guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        None => err(404, "NOT_INITIALIZED", "pipeline not yet initialised"),
        Some(p) => ok_val(p.detector.to_json()),
    }
}

// ── Event store persistence ──────────────────────────────────────────────────

/// Load the last EVENT_RING_CAP events from the JSONL store into SENSOR_EVENTS.
/// Called once at startup; silently ignores a missing or corrupt store file.
pub fn load_events_from_disk() {
    let path = std::path::Path::new(event_store_path());
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return, // no store file yet — normal on first boot
    };
    let mut loaded: VecDeque<CognitiveEvent> = VecDeque::new();
    for line in std::io::BufReader::new(file).lines().flatten() {
        if let Ok(ev) = serde_json::from_str::<CognitiveEvent>(&line) {
            if loaded.len() >= EVENT_RING_CAP { loaded.pop_front(); }
            loaded.push_back(ev);
        }
    }
    if loaded.is_empty() { return; }
    let count = loaded.len();
    let mut guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    *guard = loaded;
    eprintln!("[cognitive] loaded {} events from {}", count, event_store_path());
}

/// Append a single event to the JSONL store (best-effort; failures are logged).
fn persist_event(event: &CognitiveEvent) {
    let line = match serde_json::to_string(event) {
        Ok(l) => l,
        Err(_) => return,
    };
    match std::fs::OpenOptions::new().append(true).create(true).open(event_store_path()) {
        Ok(mut f) => { let _ = writeln!(f, "{}", line); }
        Err(e) => eprintln!("[cognitive] event persist failed: {}", e),
    }
}

/// Compact the store to the last EVENT_RING_CAP lines.
/// Called after flushing the ring buffer (DELETE /events) to prevent unbounded growth.
fn compact_event_store() {
    let guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    let lines: Vec<String> = guard.iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .collect();
    drop(guard);
    let content = lines.join("\n") + if lines.is_empty() { "" } else { "\n" };
    let _ = std::fs::write(event_store_path(), content);
}

// ── Delta-sync helpers ───────────────────────────────────────────────────────

/// Return up to `limit` recent events from the ring buffer for inclusion in a
/// delta-sync payload.  Events are NOT removed — they remain available for HTTP
/// polling until the ring fills.  Callers that want acknowledgement-based drain
/// should call `handle_sensor_events_clear` after a successful upstream push.
pub fn drain_events_for_sync(limit: usize) -> Vec<CognitiveEvent> {
    let guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    guard.iter().rev().take(limit).cloned().collect()
}

/// Merge cognitive events received from a remote peer into the local ring buffer.
/// De-duplicates by (timestamp_s, sensor_type) — same-window events from the
/// same sensor are not stored twice.
pub fn merge_events_from_peer(incoming: &[CognitiveEvent]) {
    if incoming.is_empty() { return; }
    let mut guard = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    for event in incoming {
        let already = guard.iter().any(|e| {
            e.timestamp_s == event.timestamp_s && e.sensor_type == event.sensor_type
        });
        if !already {
            if guard.len() >= EVENT_RING_CAP { guard.pop_front(); }
            guard.push_back(event.clone());
        }
    }
}

// ── Sensor-loop integration ──────────────────────────────────────────────────

/// Called by `sensor_reflex_loop` once per window.
///
/// Feeds the normalised feature vector through the sensor-side FastGRNN detector,
/// applies the same warmup + threshold gate as the HTTP pipeline, and — if an
/// anomaly is detected — requests an LLM summary from the shared model cache.
///
/// Returns `Some(summary)` on a triggered event, `None` otherwise.
/// Returns `None` immediately if the model cache is not yet warm (no model loaded).
pub fn sensor_tick(features: &[f32], sensor_type: &str, threshold: f32) -> Option<String> {
    if features.is_empty() { return None; }

    let (anomaly_score, in_warmup) = {
        let mut guard = SENSOR_PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        let pipeline = guard.get_or_insert_with(|| {
            CognitivePipeline::new(sensor_type, features.len(), threshold)
        });
        pipeline.threshold   = threshold;
        pipeline.sensor_type = sensor_type.to_string();

        let (score, _hidden) = pipeline.detector.score_window(&[features.to_vec()]);
        pipeline.frames_seen  += 1;
        pipeline.windows_seen += 1;
        pipeline.last_anomaly_score = score;

        let warmup = pipeline.windows_seen <= WARMUP_WINDOWS;
        let decision = if warmup { "warmup" } else if score < threshold { "suppressed" } else { "triggered" };
        pipeline.last_gate_decision = decision;
        (score, warmup)
    };

    if in_warmup || anomaly_score < threshold { return None; }

    // Anomaly gate passed — spawn inference on a background thread so the
    // sensor loop keeps advancing windows_seen while SmolLM2 generates.
    // If an inference is already in-flight, skip this window to avoid
    // piling up 58-second threads on a Pi Zero 2W.
    if INFERENCE_BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_err() {
        return None;
    }

    let windows_seen = {
        let guard = SENSOR_PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|p| p.windows_seen).unwrap_or(0)
    };
    let prompt = format!(
        "<|im_start|>system\nYou are a concise sensor anomaly analyst.<|im_end|>\n\
         <|im_start|>user\nSensor: {}. Anomaly score: {:.2} (threshold 1.0). \
         Window: {}. Summarise in under 20 words.<|im_end|>\n\
         <|im_start|>assistant\n",
        sensor_type, anomaly_score, windows_seen,
    );
    let sensor_type_owned = sensor_type.to_string();

    std::thread::spawn(move || {
        let t_infer = Instant::now();
        let summary = crate::sparse_llm_api::generate_summary_from_cache(&prompt, 32);
        let inference_ms = t_infer.elapsed().as_secs_f64() * 1000.0;
        INFERENCE_BUSY.store(false, Ordering::Release);

        {
            let mut guard = SENSOR_PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(p) = guard.as_mut() {
                p.events_gated += 1;
                p.total_summary_ms += inference_ms;
                if let Some(ref s) = summary {
                    p.summaries_generated += 1;
                    p.last_summary = Some(s.clone());
                }
            }
        }

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let event = CognitiveEvent {
            timestamp_s:  ts,
            windows_seen,
            anomaly_score,
            sensor_type:  sensor_type_owned,
            summary:      summary.unwrap_or_default(),
            rss_mb:       read_rss_mb(),
            inference_ms,
        };
        persist_event(&event);
        let mut ev = SENSOR_EVENTS.lock().unwrap_or_else(|e| e.into_inner());
        if ev.len() >= EVENT_RING_CAP { ev.pop_front(); }
        ev.push_back(event);
    });

    // Return None immediately — the event appears in the ring when inference completes.
    None
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn build_prompt(preq: &PipelineRequest, score: f32) -> String {
    let user_msg = preq.prompt_prefix.as_deref().map(|p| format!(
        "Sensor: {}. Anomaly score: {:.2} (threshold 1.0). {}",
        preq.sensor_type, score, p,
    )).unwrap_or_else(|| format!(
        "Sensor: {}. Anomaly score: {:.2} (threshold 1.0). Summarise in under 20 words.",
        preq.sensor_type, score,
    ));
    format!(
        "<|im_start|>system\nYou are a concise sensor anomaly analyst.<|im_end|>\n\
         <|im_start|>user\n{user_msg}<|im_end|>\n\
         <|im_start|>assistant\n",
    )
}

fn run_llm_summary(_state: &crate::api::DeviceState, prompt: &str, max_tokens: usize) -> Option<String> {
    crate::sparse_llm_api::generate_summary_from_cache(prompt, max_tokens)
}

fn vec_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Read current process RSS from /proc/self/status (Linux only).
fn read_rss_mb() -> f32 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
            for line in s.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb) = line.split_whitespace().nth(1)
                        .and_then(|n| n.parse::<f32>().ok())
                    {
                        return kb / 1024.0;
                    }
                }
            }
        }
    }
    0.0
}

fn err(status: usize, code: &str, msg: &str) -> (usize, String) {
    (status, serde_json::json!({ "error": msg, "code": code }).to_string())
}

fn ok(resp: PipelineResponse) -> (usize, String) {
    (200, serde_json::to_string(&resp).unwrap_or_default())
}

fn ok_val(v: serde_json::Value) -> (usize, String) {
    (200, v.to_string())
}

// ── Streaming handler: POST /api/v1/llm/sparse/pipeline/stream ─────────────
//
// Wire format (Server-Sent Events):
//   data: {"type":"gate","score":X,"decision":"triggered|suppressed|warmup","frames":N}
//   data: {"type":"token","text":"..."}      — one per generated token
//   data: {"type":"final","summary":"...","tok_s":X,"time_ms":Y}
//   data: [DONE]
//
// Early stop: sentence-ending punctuation (. ! ?) or max 48 tokens.

/// Hard cap on tokens generated in streaming mode.
const STREAM_MAX_TOKENS: usize = 48;
/// Stop streaming after these punctuation chars (emit token, then stop).
const SENTENCE_END: &[char] = &['.', '!', '?'];

pub fn handle_sparse_pipeline_stream<W: std::io::Write>(
    req: &Request,
    _state: &crate::api::DeviceState,
    authorized: bool,
    writer: &mut W,
) {
    use std::io::Write as IoWrite;

    macro_rules! sse_err {
        ($status:expr, $msg:expr) => {{
            let body = format!("{{\"error\":{}}}", serde_json::Value::String($msg.to_string()));
            let _ = write!(writer,
                "HTTP/1.1 {} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                $status, body.len(), body);
            let _ = writer.flush();
            return;
        }};
    }

    if !authorized { sse_err!(401, "authorization required"); }

    let preq: PipelineRequest = match serde_json::from_slice(&req.body) {
        Ok(r) => r,
        Err(e) => { sse_err!(400, format!("invalid JSON: {e}")); }
    };
    if preq.frames.is_empty() { sse_err!(400, "frames must not be empty"); }

    // SSE headers
    let hdr = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
    if writer.write_all(hdr.as_bytes()).is_err() { return; }
    if writer.flush().is_err() { return; }

    macro_rules! emit {
        ($v:expr) => {{
            if write!(writer, "data: {}\n\n", $v).is_err() { return; }
            if writer.flush().is_err() { return; }
        }};
    }

    let t0 = Instant::now();

    // FastGRNN scoring
    let (anomaly_score, grnn_ms, in_warmup) = {
        let mut guard = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        let pipeline = guard.get_or_insert_with(|| {
            let input_dim = preq.frames.first().map(|v| v.len()).unwrap_or(DEFAULT_INPUT_DIM);
            CognitivePipeline::new(&preq.sensor_type, input_dim, preq.threshold)
        });
        pipeline.threshold   = preq.threshold;
        pipeline.sensor_type = preq.sensor_type.clone();
        let tg = Instant::now();
        let (score, _) = pipeline.detector.score_window(&preq.frames);
        let ms = tg.elapsed().as_secs_f64() * 1000.0;
        pipeline.frames_seen  += preq.frames.len() as u64;
        pipeline.windows_seen += 1;
        pipeline.total_fastgrnn_ms += ms;
        pipeline.last_anomaly_score = score;
        let warmup = pipeline.windows_seen <= WARMUP_WINDOWS;
        (score, ms, warmup)
    };

    // Gate frame
    let triggered = !in_warmup && anomaly_score >= preq.threshold;
    let decision: &str = if triggered { "triggered" } else if in_warmup { "warmup" } else { "suppressed" };
    emit!(serde_json::json!({
        "type": "gate", "score": anomaly_score, "decision": decision,
        "frames": preq.frames.len(), "fastgrnn_ms": grnn_ms,
    }));

    if !triggered {
        emit!(serde_json::json!({"type":"final","summary":"","tok_s":0.0,"time_ms":t0.elapsed().as_millis() as u64}));
        let _ = writer.write_all(b"data: [DONE]\n\n");
        let _ = writer.flush();
        return;
    }

    // SmolLM2 streaming
    let max_tokens = preq.max_summary_tokens.min(STREAM_MAX_TOKENS);
    let prompt = build_prompt(&preq, anomaly_score);
    let mut summary = String::new();
    let mut early_stop = false;

    {
        let mut g = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = g.as_mut() { p.events_gated += 1; p.last_gate_decision = "triggered"; }
    }

    let (n_tokens, llm_ms) = crate::sparse_llm_api::generate_summary_streaming(
        &prompt,
        max_tokens,
        &mut |text| {
            if early_stop { return false; }
            summary.push_str(text);
            let escaped = serde_json::to_string(text).unwrap_or_default();
            let _ = write!(writer, "data: {{\"type\":\"token\",\"text\":{}}}\n\n", escaped);
            let _ = writer.flush();
            if text.chars().any(|c| SENTENCE_END.contains(&c)) { early_stop = true; return false; }
            true
        },
    );

    // Update counters
    {
        let mut g = PIPELINE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = g.as_mut() {
            p.total_summary_ms += llm_ms;
            if !summary.is_empty() { p.summaries_generated += 1; p.last_summary = Some(summary.clone()); }
            p.last_tok_per_sec = if llm_ms > 0.0 { n_tokens as f32 / (llm_ms as f32 / 1000.0) } else { 0.0 };
        }
    }

    let tok_s = if llm_ms > 0.0 { n_tokens as f32 / (llm_ms as f32 / 1000.0) } else { 0.0 };
    emit!(serde_json::json!({"type":"final","summary":summary,"tok_s":tok_s,"time_ms":t0.elapsed().as_millis() as u64,"n_tokens":n_tokens,"rss_mb":read_rss_mb()}));
    let _ = writer.write_all(b"data: [DONE]\n\n");
    let _ = writer.flush();
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Request;
    use std::collections::HashMap;

    fn make_req(method: &str, path: &str, body: &[u8]) -> Request {
        Request {
            method: method.to_string(),
            path: path.to_string(),
            body: body.to_vec(),
            headers: HashMap::new(),
            peer_addr: Some("127.0.0.1:0".to_string()),
            client_cn: None,
        }
    }

    #[test]
    fn test_pipeline_normal_no_anomaly() {
        *PIPELINE.lock().unwrap() = None;
        let mut det = FastGrnnDetector::new(16, DEFAULT_HIDDEN_DIM);
        let frames: Vec<Vec<f32>> = (0..10).map(|_| vec![0.01f32; 16]).collect();
        let (score, _) = det.score_window(&frames);
        assert!(score < 10.0, "normal frames scored {score}");
    }

    #[test]
    fn test_pipeline_spike_triggers_high_score() {
        let mut det = FastGrnnDetector::new(4, 8);
        // Prime EMA with normal frames
        for _ in 0..20 {
            det.push_frame(&[0.1, 0.1, 0.1, 0.1]);
        }
        let spike_window = vec![vec![10.0f32, -10.0, 10.0, -10.0]];
        let (score, _) = det.score_window(&spike_window);
        assert!(score > 0.3, "spike should score above baseline, got {score}");
    }

    #[test]
    fn test_status_uninitialised() {
        *PIPELINE.lock().unwrap() = None;
        let (status, body) = handle_pipeline_status();
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["initialized"], false);
    }

    #[test]
    fn test_weights_upload_download_roundtrip() {
        *PIPELINE.lock().unwrap() = None;
        let det = FastGrnnDetector::new(4, 8);
        let weights_json = det.to_json().to_string();
        let req = make_req("PUT", "/api/v1/llm/sparse/pipeline/weights", weights_json.as_bytes());
        let (status, _) = handle_pipeline_weights_upload(&req, true);
        assert_eq!(status, 200);
        let (status2, body2) = handle_pipeline_weights_download(true);
        assert_eq!(status2, 200);
        let v: serde_json::Value = serde_json::from_str(&body2).unwrap();
        assert_eq!(v["input_dim"], 4);
        assert_eq!(v["hidden_dim"], 8);
    }

    #[test]
    fn test_weights_upload_requires_auth() {
        let req = make_req("PUT", "/api/v1/llm/sparse/pipeline/weights", b"{}");
        let (status, _) = handle_pipeline_weights_upload(&req, false);
        assert_eq!(status, 401);
    }

    #[test]
    fn test_request_accepts_frames_alias() {
        let json = serde_json::json!({
            "frames": [[0.1f32, 0.2], [0.3, 0.4]],
            "threshold": 0.5
        }).to_string();
        let r: PipelineRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(r.frames.len(), 2);
    }

    #[test]
    fn test_request_accepts_window_compat_alias() {
        let json = serde_json::json!({
            "window": [[0.1f32, 0.2]],
            "threshold": 0.5
        }).to_string();
        let r: PipelineRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(r.frames.len(), 1);
    }

    #[test]
    fn test_rss_read() {
        // Just verify it doesn't panic and returns a non-negative value.
        let rss = read_rss_mb();
        assert!(rss >= 0.0);
    }
}
