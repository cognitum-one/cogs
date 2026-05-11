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
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response as AxumResponse},
    routing::any,
    Router,
};
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
        log::info!("--once: scaffold path; full pipeline tick lands when the agent supplies sensor data");
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

    let app = Router::new()
        .fallback(any(handle_any))
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
    use super::rewrite_path;

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
