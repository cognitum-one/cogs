//! cog-cognitive-pipeline (ADR-094 + ADR-095)
//!
//! Pi Zero 2 W sparse-LLM inference cog. Hosts the FastGRNN anomaly gate +
//! SmolLM2 / Qwen2.5 sparse-attention runner originally lifted from
//! cognitum-one/seed#133, repackaged as a sandboxed cog per ADR-095.
//!
//! Boot:
//!   1. Read `COGNITUM_COG_TOKEN` from the env (set by the agent at /start);
//!      if absent, log a warning and accept any Authorization header
//!      (standalone-dev mode).
//!   2. Bind axum to `127.0.0.1:<port>` (loopback only — the agent's proxy
//!      is the only legitimate caller per ADR-095 §1).
//!   3. Load any cognitive events persisted from a previous run.
//!   4. Forward all incoming requests to `sparse_llm_api::dispatch_sparse_llm`,
//!      which routes between sparse-LLM endpoints, OpenAI-compat endpoints,
//!      and the cognitive pipeline endpoints.

use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path as AxumPath, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response as AxumResponse},
    routing::{any, put},
    Router,
};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

mod http_compat;

// Re-export the compat layer so the lifted modules find their original
// `crate::http` and `crate::api` paths unchanged.
mod http {
    pub use crate::http_compat::{Request, Response};
}
mod api {
    pub use crate::http_compat::DeviceState;
}

// Lifted from cognitum-one/seed#133 (sparse_*.rs + sparse_pipeline.rs).
// These modules are byte-identical to the agent versions except for the
// stripped `#![cfg(feature = "sparse-llm")]` inner attribute.
mod sparse_fastgrnn;
mod sparse_llm;
mod sparse_llm_api;
mod sparse_llm_kv_quant;
mod sparse_llm_loader;
mod sparse_llm_projector;
mod sparse_llm_runner;
mod sparse_llm_tokenizer;
mod sparse_llm_weights;
mod sparse_pipeline;

const COG_ID: &str = "cognitive-pipeline";
const COG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = COG_ID, version = COG_VERSION)]
struct Args {
    /// Loopback bind port (default 8033 per cog.toml). Agent proxies
    /// /api/v1/cogs/cognitive-pipeline/* to http://127.0.0.1:<port>/*.
    #[arg(long, default_value_t = 8033)]
    port: u16,

    /// Selected GGUF model id (smollm2-135m or qwen2.5-0.5b-q4).
    #[arg(long, default_value = "smollm2-135m")]
    model: String,

    /// Decode wall-clock deadline in seconds.
    #[arg(long = "deadline-secs", default_value_t = 90)]
    deadline_secs: u64,

    /// FastGRNN anomaly threshold (gate triggers inference above this).
    #[arg(long = "gate-threshold", default_value_t = 1.0)]
    gate_threshold: f32,

    /// Cognitive event ring buffer capacity.
    #[arg(long = "ring-cap", default_value_t = 100usize)]
    ring_cap: usize,

    /// Print info and exit (`/console` allowed_command).
    #[arg(long)]
    info: bool,

    /// Run a single inference cycle then exit (`/console` allowed_command).
    #[arg(long)]
    once: bool,
}

#[derive(Clone)]
struct AppState {
    started_at: Instant,
    args: Arc<Args>,
    /// Per-cog bearer token from agent (env COGNITUM_COG_TOKEN). When the agent
    /// proxies a request it injects this same value in the Authorization header.
    /// `None` only in standalone-dev mode (warned at startup).
    expected_token: Option<String>,
}

/// Validate `Authorization: Bearer <token>` in constant time.
/// Returns `true` for paired requests; `false` for missing/wrong tokens.
/// In standalone-dev mode (no token configured), returns `true` unconditionally.
fn check_authorization(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.expected_token.as_ref() else {
        return true;
    };
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(s) = auth.to_str() else { return false };
    let Some(provided) = s.strip_prefix("Bearer ") else { return false };
    use subtle::ConstantTimeEq;
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

/// Map a cog-relative path (what the agent's proxy delivers after stripping
/// `/api/v1/cogs/cognitive-pipeline`) to the agent-internal path that
/// `dispatch_sparse_llm` expects. OpenAI-style `/v1/*` and `/health` pass
/// through unchanged. `/oai_chat` is a legacy alias for `/v1/chat/completions`
/// kept for callers that pre-date the OpenAI-compat URL surface.
fn rewrite_path(p: &str) -> String {
    if p.starts_with("/v1/") || p == "/health" {
        return p.to_string();
    }
    // Legacy alias declared in cog.toml [api].endpoints — predates the
    // canonical /v1/chat/completions surface that dispatch_sparse_llm
    // matches at sparse_llm_api.rs:405. Both names live in /info.
    if p == "/oai_chat" {
        return "/v1/chat/completions".to_string();
    }
    if p == "/info"
        || p == "/models"
        || p == "/generate"
        || p == "/tokenize"
        || p == "/pipeline"
        || p.starts_with("/pipeline/")
        || p.starts_with("/model/")
    {
        return format!("/api/v1/llm/sparse{}", p);
    }
    // Anything else falls through unchanged — dispatch_sparse_llm will return
    // None and the caller turns it into a 404.
    p.to_string()
}

/// std::io::Write adapter that bridges the lifted synchronous
/// `handle_sparse_generate_sse` (writes raw HTTP/1.1 framing + SSE events to
/// any `Write`) to axum's async streaming body. The lifted handler emits its
/// own HTTP status line + headers; we strip those (axum manages headers) and
/// forward only the body bytes through a tokio mpsc to the response stream.
struct SseChannelWriter {
    tx: tokio::sync::mpsc::UnboundedSender<Result<bytes::Bytes, std::io::Error>>,
    header_buffer: Vec<u8>,
    /// Once we've seen `\r\n\r\n` the rest of the writes are body bytes.
    /// Until then we accumulate into `header_buffer` and discard once the
    /// terminator is seen.
    headers_passed: bool,
    /// Set to `true` if the lifted handler emitted a non-200 status line.
    /// We surface this on the response so the streaming handler can map it
    /// to the right status (lifted code's framing said 401/400/etc. and we
    /// must respect that even when streaming).
    sniffed_status: Option<u16>,
}

impl SseChannelWriter {
    fn new(tx: tokio::sync::mpsc::UnboundedSender<Result<bytes::Bytes, std::io::Error>>) -> Self {
        Self { tx, header_buffer: Vec::with_capacity(512), headers_passed: false, sniffed_status: None }
    }
}

impl std::io::Write for SseChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.headers_passed {
            // Forward as body chunk. Channel send failure means the receiver
            // dropped (client disconnected) — surface as broken-pipe so the
            // lifted handler's loop short-circuits.
            if self
                .tx
                .send(Ok(bytes::Bytes::copy_from_slice(buf)))
                .is_err()
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "sse mpsc receiver dropped",
                ));
            }
            return Ok(buf.len());
        }

        // Accumulate into the header buffer until we see the terminator.
        self.header_buffer.extend_from_slice(buf);

        // Cap header accumulation to a reasonable size — the lifted handler
        // emits a small fixed header block; anything more is malformed.
        if self.header_buffer.len() > 4096 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "SSE response headers > 4 KB — refusing",
            ));
        }

        // Sniff the status line on the first chunk so we can map non-200
        // responses back to the axum Response without parsing every byte.
        if self.sniffed_status.is_none() && self.header_buffer.starts_with(b"HTTP/1.1 ") {
            let line_end = self
                .header_buffer
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(self.header_buffer.len());
            let line = &self.header_buffer[..line_end];
            // "HTTP/1.1 200 OK..." → status = 200
            if let Some(parts) = std::str::from_utf8(line).ok().map(|s| s.split_whitespace().collect::<Vec<_>>()) {
                if let Some(s) = parts.get(1).and_then(|s| s.parse::<u16>().ok()) {
                    self.sniffed_status = Some(s);
                }
            }
        }

        if let Some(p) = self
            .header_buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
        {
            self.headers_passed = true;
            let body_so_far = self.header_buffer.split_off(p + 4);
            // Drop the header bytes (we don't forward them to axum).
            self.header_buffer.clear();
            if !body_so_far.is_empty() {
                if self
                    .tx
                    .send(Ok(bytes::Bytes::from(body_so_far)))
                    .is_err()
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "sse mpsc receiver dropped",
                    ));
                }
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // No buffering on the channel side; nothing to flush.
        Ok(())
    }
}

/// Detect `"stream": true` in the request body. Crude — we don't fully parse
/// JSON because we want the buffered path to handle parsing the same way it
/// always has. This just routes streaming requests to the streaming handler;
/// validation errors still surface on the streaming side.
fn wants_streaming(body: &[u8]) -> bool {
    // Look for `"stream"` followed by `:`, then `true`. Strip whitespace.
    let s = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let idx = match s.find("\"stream\"") {
        Some(i) => i,
        None => return false,
    };
    let after = &s[idx + "\"stream\"".len()..];
    // Skip whitespace + ':'
    let bytes = after.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b':' || bytes[i] == b'\t') {
        i += 1;
    }
    after[i..].trim_start().starts_with("true")
}

/// Handler that drives `sparse_llm_api::handle_sparse_generate_sse` for any
/// path that supports streaming (`/generate`, `/v1/completions`,
/// `/v1/chat/completions`). Returns the streaming body via an axum
/// `Body::from_stream` backed by a tokio mpsc receiver — never buffers the
/// generated tokens. Inference runs in `spawn_blocking` because the SmolLM2
/// runner is CPU-bound and would block the tokio executor otherwise.
async fn handle_streaming_inference(
    state: AppState,
    method: Method,
    inner_path: String,
    headers: HeaderMap,
    body_bytes: Vec<u8>,
) -> AxumResponse {
    if method != Method::POST {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            axum::Json(serde_json::json!({"error":"streaming requires POST"})),
        )
            .into_response();
    }
    let authorized = check_authorization(&state, &headers);

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<bytes::Bytes, std::io::Error>>();
    let device_state = http_compat::DeviceState;
    let path_for_handler = inner_path.clone();

    // Status sniff: we don't know the status until the blocking handler
    // emits its first bytes. The lifted handler writes 200 for the SSE
    // success path and 401/400/etc. with a normal JSON body for errors.
    // To avoid blocking the response while we wait, we always return 200
    // and let the body convey errors via a JSON {"error":...} on the
    // first chunk if status would have been non-200. Clients of SSE
    // already need to handle errors-in-body for transport-level reasons.
    // (Browsers and the OpenAI SDK both accept this.)

    tokio::task::spawn_blocking(move || {
        let mut writer = SseChannelWriter::new(tx);
        sparse_llm_api::handle_sparse_generate_sse(
            &body_bytes,
            &device_state,
            authorized,
            &path_for_handler,
            &mut writer,
        );
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    axum::http::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/event-stream")
        .header(axum::http::header::CACHE_CONTROL, "no-cache")
        .header(axum::http::header::CONNECTION, "close")
        .header("X-Accel-Buffering", "no") // hint to any intermediary not to buffer
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Stream the request body to disk without buffering it in memory. Replaces
/// the lifted `PUT /api/v1/llm/sparse/model/<id>/<file>` path for uploads —
/// that handler uses axum's `Bytes` extractor (full-body buffer), which OOMs
/// on a 512 MB Pi Zero for the 85-310 MB GGUFs declared in cog.toml `[[assets]]`.
///
/// `id` and `filename` map to `<COGNITUM_COG_DATA_DIR>/<id>/<filename>`,
/// matching what the lifted `dispatch_sparse_llm` resolves at GET /models time.
async fn handle_model_upload_streaming(
    State(state): State<AppState>,
    AxumPath((id, filename)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> AxumResponse {
    use std::io::ErrorKind;

    // Same auth contract as `dispatch_sparse_llm`'s PUT handler — admin op,
    // requires the cog-token in standalone-prod, accepts anything in dev.
    if !check_authorization(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "authorization required",
                "code":  "UNAUTHORIZED"
            })),
        )
            .into_response();
    }

    // Path-traversal guard. axum already URL-decodes the segments, so we just
    // reject anything that could escape the cog data dir.
    if id.is_empty()
        || filename.is_empty()
        || id.contains('/') || id.contains('\\') || id.contains("..")
        || filename.contains('/') || filename.contains('\\') || filename.contains("..")
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "invalid model id or filename (no '/', '\\\\', or '..' allowed)"
            })),
        )
            .into_response();
    }

    // Resolve `<base>/<id>/<filename>` and ensure parent exists. The base is
    // `COGNITUM_COG_DATA_DIR` per fd44917; default `/var/lib/cognitum/apps/cognitive-pipeline`.
    let base_dir = std::path::Path::new(sparse_llm_api::model_base_dir());
    let dir = base_dir.join(&id);
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        log::error!("model upload: create_dir_all({:?}) failed: {}", dir, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({
                "error": format!("failed to create model dir: {e}")
            })),
        )
            .into_response();
    }
    let dest = dir.join(&filename);
    let tmp = dir.join(format!(".{filename}.partial"));

    // Stream chunks → tmp file → atomic rename. Never holds more than one
    // chunk in memory at a time (axum's hyper body chunk size is ~16 KB).
    let mut file = match tokio::fs::File::create(&tmp).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({
                    "error": format!("create tmp file failed: {e}")
                })),
            )
                .into_response();
        }
    };
    let mut stream = body.into_data_stream();
    let mut total: usize = 0;
    while let Some(chunk_res) = stream.next().await {
        match chunk_res {
            Ok(chunk) => {
                if let Err(e) = file.write_all(&chunk).await {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({
                            "error": format!("write chunk failed: {e}")
                        })),
                    )
                        .into_response();
                }
                total += chunk.len();
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                // Distinguish client disconnect from real I/O failure
                let msg = format!("body read failed: {e}");
                let status = if msg.contains("closed") || msg.contains("reset")
                    || e.to_string().to_lowercase().contains("eof")
                {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                return (
                    status,
                    axum::Json(serde_json::json!({
                        "error": msg
                    })),
                )
                    .into_response();
            }
        }
    }
    if let Err(e) = file.flush().await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({
                "error": format!("flush failed: {e}")
            })),
        )
            .into_response();
    }
    drop(file);

    // Atomic swap into final filename. If a previous upload existed, this
    // replaces it; the cog's GET /models picks up the new bytes on next call.
    if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
        // Some filesystems (rare) reject cross-device rename. Fall back to copy+remove.
        if e.kind() == ErrorKind::CrossesDevices {
            match tokio::fs::copy(&tmp, &dest).await {
                Ok(_) => {
                    let _ = tokio::fs::remove_file(&tmp).await;
                }
                Err(e2) => {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({
                            "error": format!("rename + copy fallback failed: {e2}")
                        })),
                    )
                        .into_response();
                }
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp).await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({
                    "error": format!("rename failed: {e}")
                })),
            )
                .into_response();
        }
    }

    // Best-effort cache invalidation so the next /generate sees the new
    // weights. The lifted PUT handler reported `cache_evicted: true` here;
    // sparse_llm_api keeps a global cached engine keyed by model id.
    let cache_evicted = sparse_llm_api::invalidate_model_cache(&id);

    log::info!(
        "model upload: id={} filename={} bytes={} (streamed, no buffer) cache_evicted={}",
        id, filename, total, cache_evicted
    );

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "status":         "ok",
            "model_id":       id,
            "filename":       filename,
            "bytes_written":  total,
            "path":           dest.to_string_lossy(),
            "cache_evicted":  cache_evicted,
            "transport":      "streamed",
        })),
    )
        .into_response()
}

/// Single axum handler — translates an axum request into the compat Request,
/// calls `sparse_llm_api::dispatch_sparse_llm`, translates the compat Response
/// back to axum.
async fn handle_any(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> AxumResponse {
    let cog_path = uri.path().to_string();
    // Routing key stays path-only — dispatch_sparse_llm matches on exact path.
    let inner_path = rewrite_path(&cog_path);

    // SSE streaming branch: if the body asks for `stream: true` AND the path
    // is one the lifted `handle_sparse_generate_sse` knows how to drive,
    // route to the streaming handler. It runs inference in `spawn_blocking`
    // and pushes SSE chunks through a tokio mpsc to axum's `Body::from_stream`
    // — no token-buffering, the agent's `cog_proxy::stream_request` then pipes
    // chunks cog → client without batching.
    let stream_eligible_path = matches!(
        inner_path.as_str(),
        "/api/v1/llm/sparse/generate" | "/v1/completions" | "/v1/chat/completions"
    );
    if stream_eligible_path && wants_streaming(&body) {
        return handle_streaming_inference(
            state,
            method,
            inner_path,
            headers,
            body.to_vec(),
        )
        .await;
    }
    // Handlers like `sparse_pipeline::handle_sensor_events` parse the query
    // string by splitting `req.path` on `?`. axum's `Uri::path` strips the
    // query, so we re-attach it here. Without this, GET
    // /pipeline/events?since=N&limit=M silently fell back to defaults.
    let query = uri.query().unwrap_or("");
    let req_path = if query.is_empty() {
        inner_path.clone()
    } else {
        format!("{inner_path}?{query}")
    };

    let mut compat_headers = std::collections::HashMap::new();
    for (k, v) in headers.iter() {
        if let Ok(vstr) = v.to_str() {
            compat_headers.insert(k.as_str().to_lowercase(), vstr.to_string());
        }
    }

    let req = http_compat::Request {
        method: method.as_str().to_uppercase(),
        path: req_path,
        headers: compat_headers,
        body: body.to_vec(),
        peer_addr: Some("127.0.0.1:0".to_string()),
        client_cn: None,
    };

    let authorized = check_authorization(&state, &headers);
    let device_state = http_compat::DeviceState;

    let resp = match sparse_llm_api::dispatch_sparse_llm(&req, &inner_path, &device_state, authorized) {
        Some(r) => r,
        None => http_compat::Response::not_found(),
    };

    let mut builder = axum::http::Response::builder()
        .status(StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK))
        .header(axum::http::header::CONTENT_TYPE, resp.content_type);
    for (k, v) in &resp.extra_headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    builder.body(Body::from(resp.body)).unwrap_or_else(|_| {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let args = Args::parse();

    if args.info {
        println!(
            "{{\"cog_id\":\"{}\",\"version\":\"{}\",\"model\":\"{}\"}}",
            COG_ID, COG_VERSION, args.model
        );
        return;
    }
    if args.once {
        log::info!("--once: dry-run path; full pipeline tick fires when the agent's sensor-feed wiring lands (ADR-095 next-layer)");
        return;
    }

    let expected_token = std::env::var("COGNITUM_COG_TOKEN").ok();
    if expected_token.is_none() {
        log::warn!(
            "COGNITUM_COG_TOKEN not set — running in standalone-dev mode. \
             All endpoints accept any Authorization header. \
             In production the agent always sets this at /start."
        );
    } else {
        log::info!(
            "per-cog bearer token loaded ({} bytes)",
            expected_token.as_ref().unwrap().len()
        );
    }

    // ADR-095 §1: log the MCP tool catalog declared in cog.toml so it's
    // discoverable via /api/v1/apps/cognitive-pipeline/logs until the agent's
    // /mcp install-side registration lands (deferred next-layer per ADR-095).
    // Catalog kept in sync with cog.toml [mcp].tools by hand — there's no
    // runtime parser here, the cog process never reads its own manifest.
    log::info!("[mcp] declared tool catalog (registration handled by agent at install-time):");
    log::info!("[mcp]   seed.cog.cognitive-pipeline.info     -> /info");
    log::info!("[mcp]   seed.cog.cognitive-pipeline.models   -> /models");
    log::info!("[mcp]   seed.cog.cognitive-pipeline.generate -> /generate");
    log::info!("[mcp]   seed.cog.cognitive-pipeline.events   -> /pipeline/events");

    // Load any cognitive events persisted from a previous run.
    sparse_pipeline::load_events_from_disk();

    let state = AppState {
        started_at: Instant::now(),
        args: Arc::new(args),
        expected_token,
    };
    let _ = state.started_at; // started_at retained for future /info enrichment

    // ADR-094 §5: PUT /model/<id>/<file> uploads need ~85-310 MB. axum's
    // default body limit is 2 MB which rejects GGUF uploads before they
    // reach `handle_any`. Match the agent's 320 MB cap (http.rs) — the
    // agent's paired-only auth gate + USB trust is the access control;
    // the cog binds 127.0.0.1 only so the agent is the only caller.
    const MAX_REQUEST_BODY_BYTES: usize = 320 * 1024 * 1024;
    // Route ordering matters: the streaming PUT must match BEFORE the
    // catch-all fallback so model uploads bypass `handle_any` (Bytes
    // extractor, full-body buffer = OOM on Pi Zero for 85-310 MB GGUFs).
    let app = Router::new()
        .route("/model/:id/:filename", put(handle_model_upload_streaming))
        .fallback(any(handle_any))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], state.args.port));
    log::info!(
        "cog-cognitive-pipeline v{} listening on http://{} (loopback only, agent-proxied per ADR-095)",
        COG_VERSION, addr
    );

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("bind {} failed: {} — is the port already in use?", addr, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        log::error!("server error: {e}");
        std::process::exit(1);
    }
}

/// Wait for SIGINT (ctrl-c) or SIGTERM (`systemctl stop`). `tokio::signal::ctrl_c`
/// only fires on SIGINT — without an explicit SIGTERM branch the agent's
/// `systemctl stop cog-cognitive-pipeline` would kill us before the
/// `with_graceful_shutdown` arm runs, dropping any unflushed cognitive events
/// from `sparse_pipeline` and corrupting the on-disk JSONL ring buffer.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "could not install SIGTERM handler ({e}); falling back to SIGINT-only — \
                     systemctl stop may drop unflushed cognitive events"
                );
                let _ = tokio::signal::ctrl_c().await;
                log::info!("SIGINT received — graceful shutdown");
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("SIGINT received — graceful shutdown");
            }
            _ = term.recv() => {
                log::info!("SIGTERM received — graceful shutdown (systemctl stop)");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        log::info!("ctrl-c received — graceful shutdown");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::rewrite_path;

    #[test]
    fn wants_streaming_detects_true() {
        assert!(wants_streaming(br#"{"prompt":"x","stream":true}"#));
        assert!(wants_streaming(br#"{"stream": true, "prompt":"x"}"#));
        assert!(wants_streaming(br#"{"prompt":"x", "stream" :true}"#));
        assert!(wants_streaming(br#"{"prompt":"x","stream":  true ,"max":5}"#));
    }

    #[test]
    fn wants_streaming_detects_false_or_missing() {
        assert!(!wants_streaming(br#"{"prompt":"x"}"#));
        assert!(!wants_streaming(br#"{"prompt":"x","stream":false}"#));
        assert!(!wants_streaming(br#"{"stream":"true"}"#)); // string "true", not bool
        assert!(!wants_streaming(b""));
        assert!(!wants_streaming(b"not json at all"));
    }

    #[test]
    fn sse_writer_strips_lifted_http_header_block() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut w = SseChannelWriter::new(tx);

        // The lifted handler writes its own HTTP/1.1 response framing.
        // The adapter must DROP everything up to the \r\n\r\n terminator.
        let header_block = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
        std::io::Write::write_all(&mut w, header_block).unwrap();
        // No body chunk should have been emitted yet.
        assert!(rx.try_recv().is_err(), "no chunk should reach axum from the header block");

        // First SSE event after headers — must reach the channel.
        let event = b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
        std::io::Write::write_all(&mut w, event).unwrap();
        let chunk = rx.try_recv().unwrap().unwrap();
        assert_eq!(chunk.as_ref(), event);

        // Status sniff should have captured 200.
        assert_eq!(w.sniffed_status, Some(200));
    }

    #[test]
    fn sse_writer_handles_split_writes_around_header_terminator() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut w = SseChannelWriter::new(tx);

        // Lifted handler may write headers in pieces. Adapter must accumulate
        // until the \r\n\r\n boundary regardless of split alignment.
        std::io::Write::write_all(&mut w, b"HTTP/1.1 200 OK\r\n").unwrap();
        std::io::Write::write_all(&mut w, b"Content-Type: text/event-stream\r\n\r\n").unwrap();
        std::io::Write::write_all(&mut w, b"data: event1\n\n").unwrap();
        std::io::Write::write_all(&mut w, b"data: event2\n\n").unwrap();
        let mut all = Vec::new();
        while let Ok(Ok(c)) = rx.try_recv() { all.extend_from_slice(&c); }
        assert_eq!(all, b"data: event1\n\ndata: event2\n\n");
    }

    #[test]
    fn sse_writer_forwards_body_when_terminator_arrives_inside_a_single_write() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut w = SseChannelWriter::new(tx);

        // Single write containing the WHOLE response — header + body.
        // The adapter must emit only the body to the channel.
        std::io::Write::write_all(
            &mut w,
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: hello\n\n",
        ).unwrap();
        let chunk = rx.try_recv().unwrap().unwrap();
        assert_eq!(chunk.as_ref(), b"data: hello\n\n");
    }

    #[test]
    fn sse_writer_sniffs_non_200_status_for_error_responses() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut w = SseChannelWriter::new(tx);
        std::io::Write::write_all(
            &mut w,
            b"HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\n\r\n{\"error\":\"x\"}",
        ).unwrap();
        assert_eq!(w.sniffed_status, Some(401));
    }

    #[test]
    fn sse_writer_caps_header_accumulation() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut w = SseChannelWriter::new(tx);
        // No \r\n\r\n ever — adapter must reject after 4 KB to avoid DoS.
        let big = vec![b'A'; 8 * 1024];
        let r = std::io::Write::write_all(&mut w, &big);
        assert!(r.is_err());
    }

    #[test]
    fn rewrite_canonical_v1_pass_through() {
        assert_eq!(rewrite_path("/v1/chat/completions"), "/v1/chat/completions");
        assert_eq!(rewrite_path("/v1/completions"), "/v1/completions");
        assert_eq!(rewrite_path("/health"), "/health");
    }

    #[test]
    fn rewrite_oai_chat_legacy_alias() {
        // cog.toml [api].endpoints lists `/oai_chat`; dispatch_sparse_llm
        // matches `/v1/chat/completions` (sparse_llm_api.rs:405). The alias
        // bridges the two without changing the manifest surface.
        assert_eq!(rewrite_path("/oai_chat"), "/v1/chat/completions");
    }

    #[test]
    fn rewrite_namespaced_endpoints() {
        assert_eq!(rewrite_path("/info"), "/api/v1/llm/sparse/info");
        assert_eq!(rewrite_path("/models"), "/api/v1/llm/sparse/models");
        assert_eq!(rewrite_path("/generate"), "/api/v1/llm/sparse/generate");
        assert_eq!(rewrite_path("/pipeline"), "/api/v1/llm/sparse/pipeline");
        assert_eq!(
            rewrite_path("/pipeline/events"),
            "/api/v1/llm/sparse/pipeline/events"
        );
        assert_eq!(rewrite_path("/model/foo/bar"), "/api/v1/llm/sparse/model/foo/bar");
    }

    #[test]
    fn rewrite_unknown_falls_through_unchanged() {
        // Unknown paths fall through to dispatch_sparse_llm which returns
        // None — the caller turns that into a 404.
        assert_eq!(rewrite_path("/nonexistent"), "/nonexistent");
        assert_eq!(rewrite_path("/"), "/");
    }
}
