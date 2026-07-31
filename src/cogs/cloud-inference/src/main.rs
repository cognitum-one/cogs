//! cog-cloud-inference (ADR-090 §e + ADR-095)
//!
//! Standalone "provider" cog: it owns the cloud-inference endpoint config and
//! forwards OpenAI-compatible `/v1/chat/completions` to the metered meta-llm
//! gateway with its own `cog_` key. The seed reaches Tier-3 cloud inference
//! *through* this cog — the agent's `neural_router` "Powerful" arm and the
//! `agent_runtime` tool-call loop dispatch here (they already speak the
//! OpenAI-compat contract on the cog's loopback socket, ADR-095 §1).
//!
//! Boot:
//!   1. Read `COGNITUM_COG_TOKEN` (per-cog bearer the agent injects) — absent
//!      ⇒ standalone-dev mode (any Authorization accepted; warned).
//!   2. Read the gateway bearer from `COG_CLOUD_INFERENCE_KEY_FILE` on every
//!      request (preferred, so short-lived OAuth credentials rotate without a
//!      cog restart), or from the legacy `COG_CLOUD_INFERENCE_KEY` env value.
//!      Both are SECRET inputs, never cli-args/registry fields. Absent ⇒ every
//!      completion returns 503 so the agent falls back to the local sparse-LLM.
//!   3. Bind axum to `127.0.0.1:<port>` (loopback only — the agent proxy is the
//!      only legitimate caller, ADR-095 §1).
//!
//! Design choices mirror the v0 hub's Tier-3 handler (ADR-090 Phase 1) so both
//! egress points behave identically: `model: "cognitum-auto"` (the gateway
//! routes, ADR-090 §4), non-streaming in v1 (a dropped stream is still billed),
//! `max_tokens` defaulted+capped, `402/429 + Retry-After` propagated verbatim,
//! and a response-shape guard so a mis-pointed URL fails safe as 502 instead of
//! passing a non-completion 2xx through as success.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use clap::Parser;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const COG_ID: &str = "cloud-inference";
const COG_VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTO_MODEL: &str = "cognitum-auto";
const DEFAULT_MAX_TOKENS: u64 = 512;
const KEY_ENV: &str = "COG_CLOUD_INFERENCE_KEY";
const KEY_FILE_ENV: &str = "COG_CLOUD_INFERENCE_KEY_FILE";
const MAX_CREDENTIAL_BYTES: u64 = 64 * 1024;
/// ADR-106 §2 / ADR-258 §5 R-b — cap down to the key's held tier instead of
/// 403-ing. Rationale at [`build_forward_body`].
const FALLBACK_POLICY: &str = "best_effort";
/// The completions plane. **Updated 2026-07-17**: `api.cognitum.one` now routes
/// `/v1/*` to the meta-llm `apicompletions` service (verified: bad key → `401
/// invalid_api_key`, identical to the raw URL; unknown paths → `404`, no longer
/// a catalog `200`). The raw `apicompletions-…run.app` URL was a stopgap while
/// that mapping was pending. v0's hub flipped to this same value in ADR-258
/// §1/§2 (hardware-verified on dev4) — both egress points now agree.
const DEFAULT_INFERENCE_BASE_URL: &str = "https://api.cognitum.one";

#[derive(Parser, Debug)]
#[command(name = COG_ID, version = COG_VERSION)]
struct Args {
    /// Loopback bind port (default 8040 per cog.toml).
    #[arg(long, default_value_t = 8040)]
    port: u16,

    /// Inference gateway base URL — the completions plane, or a paired v0 hub
    /// for hub-mediated mode. See [`DEFAULT_INFERENCE_BASE_URL`].
    #[arg(long = "inference-base-url", default_value = DEFAULT_INFERENCE_BASE_URL)]
    inference_base_url: String,

    /// Upstream request timeout (seconds).
    #[arg(long = "timeout-secs", default_value_t = 60u64)]
    timeout_secs: u64,

    /// Print info and exit (`/console` allowed_command).
    #[arg(long)]
    info: bool,
}

#[derive(Clone)]
struct AppState {
    started_at: Instant,
    /// Per-cog bearer the agent injects (`COGNITUM_COG_TOKEN`). `None` = dev mode.
    expected_token: Option<String>,
    /// The gateway bearer. A file is re-read for every request so an OAuth
    /// access token can rotate atomically without restarting the cog.
    cloud_credential: CloudCredentialSource,
    base_url: String,
    client: reqwest::Client,
}

// ─────────────────────────── pure helpers ──────────────────────────

#[derive(Clone)]
enum CloudCredentialSource {
    File(Arc<PathBuf>),
    Static(Arc<String>),
    Missing,
}

impl CloudCredentialSource {
    fn from_env() -> Self {
        if let Some(path) = std::env::var_os(KEY_FILE_ENV).filter(|p| !p.is_empty()) {
            return Self::File(Arc::new(PathBuf::from(path)));
        }
        std::env::var(KEY_ENV)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|s| Self::Static(Arc::new(s)))
            .unwrap_or(Self::Missing)
    }

    async fn configured(&self) -> bool {
        self.load().await.is_ok()
    }

    async fn load(&self) -> Result<Arc<String>, CredentialError> {
        match self {
            Self::Static(key) => Ok(key.clone()),
            Self::Missing => Err(CredentialError::Missing),
            Self::File(path) => load_credential_file(path).await,
        }
    }
}

#[derive(Debug)]
enum CredentialError {
    Missing,
    UnsafeFile,
    Unreadable,
}

async fn load_credential_file(path: &Path) -> Result<Arc<String>, CredentialError> {
    if !path.is_absolute() {
        return Err(CredentialError::UnsafeFile);
    }
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|_| CredentialError::Unreadable)?;
    if !metadata.file_type().is_file() {
        return Err(CredentialError::UnsafeFile);
    }
    if metadata.len() > MAX_CREDENTIAL_BYTES {
        return Err(CredentialError::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(CredentialError::UnsafeFile);
        }
    }
    let key = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| CredentialError::Unreadable)?
        .trim()
        .to_string();
    if key.is_empty() {
        return Err(CredentialError::Missing);
    }
    Ok(Arc::new(key))
}

/// Validate `Authorization: Bearer <token>` in constant time. Dev mode (no
/// token configured) accepts anything. Mirrors the cognitive-pipeline cog.
fn check_authorization(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.expected_token.as_ref() else {
        return true;
    };
    let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(provided) = auth.strip_prefix("Bearer ") else {
        return false;
    };
    use subtle::ConstantTimeEq;
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

/// Force `model: cognitum-auto` (the gateway routes, ADR-090 §4), bound
/// `max_tokens`, disable streaming (v1), and default the tier-overflow policy
/// to `best_effort` (ADR-106 §2 / ADR-258 §5 R-b). Returns the body to forward.
///
/// On `fallback_policy`: meta-llm's `resolveTier` clamps `cognitum-auto` to the
/// key's `highestHeldTier`; when the router wants a tier the key lacks, the
/// policy decides — `fail_fast` (meta-llm's DEFAULT) returns
/// `403 tier_scope_insufficient`, while `best_effort` serves the held tier and
/// stamps `cap_degraded`. ~97% of fleet owner keys are `completions:low`-only,
/// so inheriting the default would 403 exactly the prompts the router judged
/// hard — a cohort-wide failure on the requests users care most about. The v0
/// hub makes the identical choice in `llm::prepare_cloud_body`, so both egress
/// points stay byte-identical. A caller-supplied policy still wins (`entry`).
fn build_forward_body(mut body: Value) -> Value {
    if let Some(obj) = body.as_object_mut() {
        obj.insert("model".to_string(), Value::String(AUTO_MODEL.to_string()));
        obj.entry("max_tokens")
            .or_insert_with(|| json!(DEFAULT_MAX_TOKENS));
        obj.insert("stream".to_string(), Value::Bool(false));
        obj.entry("fallback_policy")
            .or_insert_with(|| json!(FALLBACK_POLICY));
    }
    body
}

/// Does an upstream 2xx body actually look like an OpenAI chat completion? A
/// mis-pointed `inference_base_url` must fail safe as 502, not pass through as
/// success. (The original motivating case — `api.cognitum.one` 200-ing a
/// storefront catalog for any path — was fixed upstream on 2026-07-17 and that
/// domain is now the default; the guard stays as defense-in-depth against any
/// future mis-mapping, which is exactly the class of bug it exists for.)
fn is_completion_body(v: &Value) -> bool {
    v.get("choices").map(|c| c.is_array()).unwrap_or(false)
}

fn error_json(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "type": code, "message": message } })),
    )
        .into_response()
}

// ─────────────────────────── handlers ──────────────────────────────

async fn get_info(State(state): State<AppState>) -> Response {
    Json(json!({
        "id": COG_ID,
        "version": COG_VERSION,
        "inference_base_url": state.base_url,
        "cloud_key_configured": state.cloud_credential.configured().await,
        "uptime_secs": state.started_at.elapsed().as_secs(),
        "streaming": false,
    }))
    .into_response()
}

async fn get_health() -> Response {
    Json(json!({ "status": "ok" })).into_response()
}

/// `POST /v1/chat/completions` (and the `/oai_chat` alias). Forwards to the
/// gateway with the `cog_` bearer.
async fn post_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    raw: Bytes,
) -> Response {
    if !check_authorization(&state, &headers) {
        return error_json(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid per-cog token",
        );
    }
    // No cloud key ⇒ Tier-3 unavailable; agent falls back to sparse-LLM.
    let key = match state.cloud_credential.load().await {
        Ok(key) => key,
        Err(_) => {
            return error_json(
                StatusCode::SERVICE_UNAVAILABLE,
                "tier_unavailable",
                "Tier-3 cloud inference credential is unavailable",
            );
        }
    };
    let Ok(body) = serde_json::from_slice::<Value>(&raw) else {
        return error_json(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "body must be JSON",
        );
    };
    if !body.get("messages").map(|m| m.is_array()).unwrap_or(false) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing `messages` array",
        );
    }
    if body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        return error_json(
            StatusCode::BAD_REQUEST,
            "unsupported",
            "streaming is not supported in v1",
        );
    }

    let mut req = state
        .client
        .post(format!("{}/v1/chat/completions", state.base_url))
        .bearer_auth(key.as_str())
        .json(&build_forward_body(body));
    // Propagate the caller's tier bounds + sub-tenant attribution (ADR-217).
    for h in [
        "x-cognitum-min-tier",
        "x-cognitum-max-tier",
        "x-cognitum-sub-tenant",
    ] {
        if let Some(v) = headers.get(h).and_then(|v| v.to_str().ok()) {
            req = req.header(h, v);
        }
    }

    let resp = match req.send().await {
        Ok(r) => r,
        // Offline / DNS / TLS failure ⇒ honest 503 so the agent degrades.
        Err(e) => {
            let code = if e.is_timeout() {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            return error_json(
                code,
                "degraded",
                &format!("cloud upstream unreachable: {e}"),
            );
        }
    };
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return error_json(
                StatusCode::BAD_GATEWAY,
                "degraded",
                &format!("reading upstream body: {e}"),
            )
        }
    };

    // Fail safe on a wrong-but-2xx upstream (non-completion body).
    if status.is_success() {
        let ok = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .map(|v| is_completion_body(&v))
            .unwrap_or(false);
        if !ok {
            return error_json(
                StatusCode::BAD_GATEWAY,
                "bad_upstream",
                "cloud upstream returned a non-completion 2xx; check inference_base_url",
            );
        }
    }

    // Pass the upstream status + body through; propagate Retry-After on 402/429.
    let mut out = (status, bytes).into_response();
    out.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    if let Some(ra) = retry_after {
        if let Ok(hv) = ra.parse() {
            out.headers_mut()
                .insert(axum::http::header::RETRY_AFTER, hv);
        }
    }
    out
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/info", get(get_info))
        .route("/health", get(get_health))
        .route("/v1/chat/completions", post(post_completions))
        .route("/oai_chat", post(post_completions)) // legacy alias (cog.toml [api])
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let cloud_credential = CloudCredentialSource::from_env();
    if args.info {
        println!(
            "{COG_ID} v{COG_VERSION}\n  inference_base_url = {}\n  cloud_key_configured = {}",
            args.inference_base_url,
            cloud_credential.configured().await
        );
        return;
    }

    let expected_token = std::env::var("COGNITUM_COG_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    if expected_token.is_none() {
        eprintln!("[{COG_ID}] WARN: COGNITUM_COG_TOKEN unset — standalone-dev mode, Authorization not enforced");
    }
    if !cloud_credential.configured().await {
        eprintln!(
            "[{COG_ID}] WARN: no readable cloud credential — Tier-3 returns 503 until one is provisioned"
        );
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(args.timeout_secs))
        .build()
        .expect("reqwest client");

    let state = AppState {
        started_at: Instant::now(),
        expected_token,
        cloud_credential,
        base_url: args.inference_base_url.trim_end_matches('/').to_string(),
        client,
    };

    let addr: SocketAddr = ([127, 0, 0, 1], args.port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind loopback");
    eprintln!("[{COG_ID}] listening on http://{addr} → {}", state.base_url);
    axum::serve(listener, build_router(state))
        .await
        .expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn state(token: Option<&str>, key: Option<&str>) -> AppState {
        AppState {
            started_at: Instant::now(),
            expected_token: token.map(String::from),
            cloud_credential: key
                .map(|k| CloudCredentialSource::Static(Arc::new(k.to_string())))
                .unwrap_or(CloudCredentialSource::Missing),
            base_url: "http://127.0.0.1:0".into(),
            client: reqwest::Client::new(),
        }
    }

    fn credential_file(contents: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cog-cloud-inference-credential-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(contents.as_bytes()).unwrap();
        }
        #[cfg(not(unix))]
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn forward_body_forces_auto_model_and_caps_tokens() {
        let out = build_forward_body(json!({
            "model": "gpt-4o",
            "messages": [{ "role": "user", "content": "hi" }],
            "stream": true
        }));
        assert_eq!(out["model"], AUTO_MODEL);
        assert_eq!(out["stream"], false);
        assert_eq!(out["max_tokens"], DEFAULT_MAX_TOKENS);
        // A caller-set max_tokens is respected.
        let out2 = build_forward_body(json!({ "messages": [], "max_tokens": 64 }));
        assert_eq!(out2["max_tokens"], 64);
    }

    /// ADR-106 §2 / ADR-258 §5 R-b. Without this the cog inherits meta-llm's
    /// `fail_fast` default and 403s every prompt the router judges hard for the
    /// ~97% of owner keys that are `completions:low`-only.
    #[test]
    fn forward_body_defaults_fallback_policy_to_best_effort() {
        let out = build_forward_body(json!({ "messages": [] }));

        assert_eq!(out["fallback_policy"], FALLBACK_POLICY);
        assert_eq!(out["fallback_policy"], "best_effort");
    }

    #[test]
    fn forward_body_lets_an_explicit_fallback_policy_win() {
        // A caller that deliberately wants a hard failure keeps it.
        let out = build_forward_body(json!({ "messages": [], "fallback_policy": "fail_fast" }));

        assert_eq!(out["fallback_policy"], "fail_fast");
    }

    /// Both egress points (this cog and the v0 hub's `llm::prepare_cloud_body`)
    /// must normalize identically — a seed's answer shouldn't depend on whether
    /// it went standalone or through its hub.
    #[test]
    fn default_inference_base_url_is_the_completions_plane() {
        assert_eq!(DEFAULT_INFERENCE_BASE_URL, "https://api.cognitum.one");
    }

    #[test]
    fn completion_shape_guard() {
        assert!(is_completion_body(
            &json!({ "choices": [{ "message": {} }] })
        ));
        // The api.cognitum.one storefront 200 must NOT pass.
        assert!(!is_completion_body(
            &json!({ "status": "healthy", "endpoints": {} })
        ));
        assert!(!is_completion_body(&json!({})));
    }

    #[test]
    fn auth_dev_mode_accepts_anything() {
        let st = state(None, Some("cog_k"));
        assert!(check_authorization(&st, &HeaderMap::new()));
    }

    #[test]
    fn auth_paired_requires_matching_bearer() {
        let st = state(Some("secret"), Some("cog_k"));
        assert!(!check_authorization(&st, &HeaderMap::new()));
        let mut good = HeaderMap::new();
        good.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret"),
        );
        assert!(check_authorization(&st, &good));
        let mut bad = HeaderMap::new();
        bad.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer nope"),
        );
        assert!(!check_authorization(&st, &bad));
    }

    // No cloud key → 503 tier_unavailable (agent falls back to sparse-LLM).
    #[tokio::test]
    async fn no_key_yields_503() {
        let st = state(None, None);
        let resp = post_completions(
            State(st),
            HeaderMap::new(),
            Bytes::from(r#"{"messages":[{"role":"user","content":"hi"}]}"#),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn credential_file_is_reread_for_each_request() {
        use std::sync::Mutex;

        let seen_auth = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = seen_auth.clone();
        let app = Router::new().route(
            "/v1/chat/completions",
            post(move |headers: HeaderMap| {
                let captured = captured.clone();
                async move {
                    captured.lock().unwrap().push(
                        headers
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string(),
                    );
                    Json(json!({ "choices": [] }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let path = credential_file("oauth_token_one\n");
        let mut st = state(None, None);
        st.base_url = format!("http://{addr}");
        st.cloud_credential = CloudCredentialSource::File(Arc::new(path.clone()));
        let body = Bytes::from_static(br#"{"messages":[]}"#);

        let first = post_completions(State(st.clone()), HeaderMap::new(), body.clone()).await;
        assert_eq!(first.status(), StatusCode::OK);
        let rotated = credential_file("oauth_token_two\n");
        std::fs::rename(rotated, &path).unwrap();
        let second = post_completions(State(st), HeaderMap::new(), body).await;
        assert_eq!(second.status(), StatusCode::OK);

        assert_eq!(
            *seen_auth.lock().unwrap(),
            ["Bearer oauth_token_one", "Bearer oauth_token_two"]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn missing_empty_or_unsafe_credential_file_fails_closed() {
        let path = credential_file("token");
        let mut st = state(None, None);
        st.cloud_credential = CloudCredentialSource::File(Arc::new(path.clone()));
        let body = Bytes::from_static(br#"{"messages":[]}"#);

        std::fs::write(&path, " \n").unwrap();
        let empty = post_completions(State(st.clone()), HeaderMap::new(), body.clone()).await;
        assert_eq!(empty.status(), StatusCode::SERVICE_UNAVAILABLE);

        std::fs::remove_file(&path).unwrap();
        let missing = post_completions(State(st), HeaderMap::new(), body).await;
        assert_eq!(missing.status(), StatusCode::SERVICE_UNAVAILABLE);

        let relative = load_credential_file(Path::new("relative-token")).await;
        assert!(matches!(relative, Err(CredentialError::UnsafeFile)));

        let oversized = credential_file(&"x".repeat(MAX_CREDENTIAL_BYTES as usize + 1));
        assert!(matches!(
            load_credential_file(&oversized).await,
            Err(CredentialError::UnsafeFile)
        ));
        std::fs::remove_file(oversized).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn credential_file_rejects_symlinks_and_broad_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let path = credential_file("token");
        let link = path.with_extension("link");
        symlink(&path, &link).unwrap();
        assert!(matches!(
            load_credential_file(&link).await,
            Err(CredentialError::UnsafeFile)
        ));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(matches!(
            load_credential_file(&path).await,
            Err(CredentialError::UnsafeFile)
        ));
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn bad_body_and_stream_rejected() {
        let st = state(None, Some("cog_k"));
        let r1 = post_completions(State(st.clone()), HeaderMap::new(), Bytes::from("{")).await;
        assert_eq!(r1.status(), StatusCode::BAD_REQUEST);
        let r2 = post_completions(
            State(st.clone()),
            HeaderMap::new(),
            Bytes::from(r#"{"foo":1}"#),
        )
        .await;
        assert_eq!(r2.status(), StatusCode::BAD_REQUEST);
        let r3 = post_completions(
            State(st),
            HeaderMap::new(),
            Bytes::from(r#"{"messages":[],"stream":true}"#),
        )
        .await;
        assert_eq!(r3.status(), StatusCode::BAD_REQUEST);
    }

    // Integration: forward to a spawned mock gateway → 429 + Retry-After
    // round-trips, model forced to cognitum-auto, sub-tenant propagated.
    #[tokio::test]
    async fn forwards_and_propagates_retry_after() {
        use std::sync::Mutex;
        let seen_model = Arc::new(Mutex::new(String::new()));
        let seen_sub = Arc::new(Mutex::new(String::new()));
        let (sm, ss) = (seen_model.clone(), seen_sub.clone());

        let app = Router::new().route(
            "/v1/chat/completions",
            post(move |headers: HeaderMap, Json(b): Json<Value>| {
                let (sm, ss) = (sm.clone(), ss.clone());
                async move {
                    *sm.lock().unwrap() = b["model"].as_str().unwrap_or("").to_string();
                    *ss.lock().unwrap() = headers
                        .get("x-cognitum-sub-tenant")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("retry-after", "9")],
                        Json(json!({ "error": "rate_limited" })),
                    )
                }
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

        let mut st = state(None, Some("cog_test"));
        st.base_url = format!("http://{addr}");
        let mut h = HeaderMap::new();
        h.insert(
            "x-cognitum-sub-tenant",
            HeaderValue::from_static("cognitum-8b40"),
        );

        let resp = post_completions(
            State(st),
            h,
            Bytes::from(r#"{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}]}"#),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(resp.headers().get("retry-after").unwrap(), "9");
        assert_eq!(*seen_model.lock().unwrap(), AUTO_MODEL);
        assert_eq!(*seen_sub.lock().unwrap(), "cognitum-8b40");
    }
}
