//! cog-cognitive-pipeline — scaffold (ADR-095)
//!
//! This is the SCAFFOLD entry point. It proves the cog framework end-to-end
//! before PR #133's sparse-LLM modules are moved in:
//!   1. Reads `COGNITUM_COG_TOKEN` from the environment (set by the agent at /start).
//!   2. Binds an Axum HTTP server to `127.0.0.1:<bind_port>` (loopback only,
//!      per ADR-095 §1 — the agent's proxy is the only legitimate caller).
//!   3. Exposes `/info` (open) and `/generate` (paired, requires bearer token).
//!   4. Logs startup + each request to stdout — the agent captures into
//!      /var/lib/cognitum/apps/cognitive-pipeline/output.log.
//!
//! The next milestone moves PR #133's sparse_*.rs + sparse_pipeline.rs in here
//! and wires them to /generate, /pipeline/events, etc.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

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

/// Validate `Authorization: Bearer <token>` for `paired` endpoints.
/// Open endpoints (e.g. /info, /models) skip this check.
fn require_token(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = state.expected_token.as_ref() else {
        // Standalone-dev mode (no token configured) — accept everything; warned at boot.
        return Ok(());
    };
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Ok(s) = auth.to_str() else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Some(provided) = s.strip_prefix("Bearer ") else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    use subtle::ConstantTimeEq;
    if provided.as_bytes().ct_eq(expected.as_bytes()).into() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Serialize)]
struct InfoResponse {
    cog_id: &'static str,
    version: &'static str,
    status: &'static str,
    model: String,
    deadline_secs: u64,
    gate_threshold: f32,
    ring_cap: usize,
    uptime_secs: u64,
    weight_mode: &'static str, // "stub" until PR #133 modules land
    note: &'static str,
}

async fn handle_info(State(state): State<AppState>) -> Json<InfoResponse> {
    Json(InfoResponse {
        cog_id: COG_ID,
        version: COG_VERSION,
        status: "scaffold",
        model: state.args.model.clone(),
        deadline_secs: state.args.deadline_secs,
        gate_threshold: state.args.gate_threshold,
        ring_cap: state.args.ring_cap,
        uptime_secs: state.started_at.elapsed().as_secs(),
        weight_mode: "stub",
        note: "ADR-095 scaffold — sparse-LLM modules from PR #133 not yet moved in",
    })
}

#[derive(Serialize)]
struct ModelsResponse {
    base_dir: &'static str,
    models: Vec<ModelEntry>,
}

#[derive(Serialize)]
struct ModelEntry {
    id: &'static str,
    ready: bool,
    note: &'static str,
}

async fn handle_models() -> Json<ModelsResponse> {
    Json(ModelsResponse {
        base_dir: "/var/lib/cognitum/apps/cognitive-pipeline",
        models: vec![
            ModelEntry {
                id: "smollm2-135m",
                ready: false,
                note: "asset download wired in ADR-095 install flow (not yet implemented)",
            },
            ModelEntry {
                id: "qwen2.5-0.5b-q4",
                ready: false,
                note: "asset download wired in ADR-095 install flow (not yet implemented)",
            },
        ],
    })
}

#[derive(Deserialize)]
struct GenerateRequest {
    #[serde(default)]
    prompt: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
}
fn default_max_tokens() -> u32 { 1 }

#[derive(Serialize)]
struct GenerateResponse {
    model: String,
    text: String,
    token_ids: Vec<u32>,
    tokens_generated: u32,
    weight_mode: &'static str,
    time_ms: u128,
    note: &'static str,
}

async fn handle_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_req): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, StatusCode> {
    require_token(&state, &headers)?;
    let t0 = Instant::now();
    // Stub response — same shape as PR #133's /generate to keep clients stable
    // when the real inference moves in.
    Ok(Json(GenerateResponse {
        model: state.args.model.clone(),
        text: String::new(),
        token_ids: vec![392], // placeholder so smoke tests can assert shape
        tokens_generated: 1,
        weight_mode: "stub",
        time_ms: t0.elapsed().as_millis(),
        note: "scaffold — sparse-LLM inference not yet wired",
    }))
}

#[derive(Serialize)]
struct EventsResponse {
    events: Vec<()>,                // CognitiveEvent type lands when PR #133 modules move in
    next_since: u64,
    note: &'static str,
}

async fn handle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EventsResponse>, StatusCode> {
    require_token(&state, &headers)?;
    Ok(Json(EventsResponse {
        events: Vec::new(),
        next_since: 0,
        note: "scaffold — cognitive event ring not yet wired",
    }))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let args = Args::parse();

    if args.info {
        println!(
            "{{\"cog_id\":\"{}\",\"version\":\"{}\",\"status\":\"scaffold\"}}",
            COG_ID, COG_VERSION
        );
        return;
    }
    if args.once {
        log::info!("--once flag set: scaffold has nothing to do; exiting clean");
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
        log::info!("per-cog bearer token loaded ({} bytes)", expected_token.as_ref().unwrap().len());
    }

    let state = AppState {
        started_at: Instant::now(),
        args: Arc::new(args),
        expected_token,
    };

    let app = Router::new()
        .route("/info", get(handle_info))
        .route("/models", get(handle_models))
        .route("/generate", post(handle_generate))
        .route("/pipeline/events", get(handle_events))
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

    // Run until SIGTERM/SIGINT (agent /stop sends SIGTERM via kill_app_processes).
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            log::info!("SIGINT/SIGTERM received — graceful shutdown");
        })
        .await
    {
        log::error!("server error: {e}");
        std::process::exit(1);
    }
}
