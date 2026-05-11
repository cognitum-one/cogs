// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! HTTP API endpoints for on-device sparse-LLM inference (ADR-094, issue #131 item 6).
//!
//! Endpoints:
//!   POST /api/v1/llm/sparse/generate              — generate text from a prompt
//!   GET  /api/v1/llm/sparse/info                  — capability descriptor
//!   GET  /api/v1/llm/sparse/models                — list installed models
//!   PUT  /api/v1/llm/sparse/model/{id}/{filename} — upload model.gguf or tokenizer.json

use crate::http::{Request, Response};
use crate::sparse_llm_runner::{SmolLm2Config, SparseLlmRunner};
use crate::sparse_llm_tokenizer::BpeTokenizer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Monotonic request counter — used to assign unique request IDs.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

// ── Weight + tokenizer cache ───────────────────────────────────────

/// Cached state shared across requests so weights are not reloaded from disk
/// on every call. Held inside a Mutex — inference serialises automatically,
/// which is correct for single-core Pi Zero hardware.
struct SparseCache {
    model_path: PathBuf,
    weights: Option<crate::sparse_llm_loader::LoadedWeights>,
    tokenizer: BpeTokenizer,
    config: SmolLm2Config,
}

static SPARSE_CACHE: Mutex<Option<SparseCache>> = Mutex::new(None);

// ── Constants ──────────────────────────────────────────────────────

const MAX_TOKENS_LIMIT: usize = 200;
const MAX_SEQ_LIMIT: usize = 512;
const MIN_SEQ_LIMIT: usize = 64;
const MAX_PROMPT_CHARS: usize = 1000;
/// Resolves the cog's data directory from the `COGNITUM_COG_DATA_DIR` env var
/// (set by the agent at /start), defaulting to the canonical sandbox path
/// per ADR-095 §4. Models live at `<dir>/<model_id>/model.gguf` +
/// `<dir>/<model_id>/tokenizer.json`. The agent's asset-download writes there too.
pub fn model_base_dir() -> &'static str {
    use std::sync::OnceLock;
    static MBD: OnceLock<String> = OnceLock::new();
    MBD.get_or_init(|| {
        std::env::var("COGNITUM_COG_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/cognitum/apps/cognitive-pipeline".to_string())
    })
    .as_str()
}

/// Drop any cached weights pinned to `<model_base_dir>/<model_id>/`. Called by
/// the streaming PUT handler in `main.rs` after a successful upload so the
/// next `/generate` reloads from the new GGUF instead of serving stale weights.
/// Returns `true` if a cache entry was actually evicted, `false` if nothing
/// matched or if `SPARSE_CACHE` was contended (a running request will reload
/// on its own when it next acquires the lock).
pub fn invalidate_model_cache(model_id: &str) -> bool {
    let model_dir = format!("{}/{}", model_base_dir(), model_id);
    if let Ok(mut guard) = SPARSE_CACHE.try_lock() {
        if guard.as_ref().map(|c| c.model_path.starts_with(&model_dir)).unwrap_or(false) {
            *guard = None;
            return true;
        }
    }
    false
}
/// 320 MB — matches PiZeroProfile::MAX_MODEL_BYTES
const MAX_UPLOAD_BYTES: usize = 320 * 1024 * 1024;
/// Allowed upload filenames.
const ALLOWED_FILENAMES: &[&str] = &["model.gguf", "tokenizer.json"];
/// Known model IDs.
const KNOWN_MODELS: &[&str] = &["smollm2-135m", "qwen2.5-0.5b-q4"];

// ── Request / response types ───────────────────────────────────────

/// Deserialize `stop` from either a string (`"\n"`) or an array (`["\n", "."]`).
/// OpenAI allows both forms; we normalise to Vec<String>.
fn deser_stop<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = serde::Deserialize::deserialize(de)?;
    Ok(match v {
        serde_json::Value::String(s) => vec![s],
        serde_json::Value::Array(arr) =>
            arr.into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    })
}

/// Deserialize `logprobs` from either bool (`true` → 5, `false` → None)
/// or integer (1–5 → Some(N), 0/null → None). Handles both chat and completions APIs.
fn deser_logprobs<'de, D>(de: D) -> Result<Option<usize>, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = serde::Deserialize::deserialize(de)?;
    Ok(match &v {
        serde_json::Value::Bool(true)    => Some(5),
        serde_json::Value::Bool(false)   => None,
        serde_json::Value::Number(n)     => n.as_u64().filter(|&x| x > 0).map(|x| x.min(5) as usize),
        serde_json::Value::Null          => None,
        _                                => None,
    })
}

/// A single message in a multi-turn conversation.
#[derive(Debug, serde::Deserialize)]
pub struct ChatMessage {
    /// Role: "system", "user", or "assistant"
    pub role: String,
    /// Message text content
    pub content: String,
}

/// Request body for POST /api/v1/llm/sparse/generate
#[derive(Debug, serde::Deserialize)]
pub struct SparseGenerateRequest {
    /// Model alias: "smollm2-135m" (default) or "qwen2.5-0.5b-q4"
    #[serde(default = "default_model")]
    pub model: String,
    /// Text prompt to complete (mutually exclusive with `messages`; messages takes priority).
    #[serde(default)]
    pub prompt: String,
    /// Multi-turn conversation messages (OpenAI-compatible). If non-empty, takes priority
    /// over `prompt`. Base models use plain-text formatting; instruct models use ChatML.
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// Number of tokens to generate (1–200, default 20)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Max sequence length (64–512, default 256)
    #[serde(default = "default_max_seq")]
    pub max_seq: usize,
    /// Sampling temperature: 0.0 = greedy/deterministic, 1.0 = standard (default 1.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Top-k filtering: 0 = all tokens, ≥1 = keep only the k highest-logit tokens (default 40)
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Nucleus (top-p) sampling: 0.0 = disabled, 0.9 = keep 90% probability mass (default 0.0)
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Repetition penalty: 1.0 = none, >1.0 discourages repeating seen tokens (default 1.1)
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f32,
    /// Optional system prompt prepended before the user prompt.
    /// For instruct models, formatted as the model's chat template.
    /// For base models, appended as plain text: "{system}\n\n{prompt}".
    #[serde(default)]
    pub system: Option<String>,
    /// RNG seed for reproducible sampling. None = time-seeded (non-deterministic).
    /// Same seed + same prompt → identical output on the same model.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Stop sequences: generation halts when any string appears in the output.
    /// Max 4 entries, each at most 32 characters. Empty strings are ignored.
    /// Accepts a single string or an array of strings (OpenAI allows both).
    #[serde(default, deserialize_with = "deser_stop")]
    pub stop: Vec<String>,
    /// If true, the formatted prompt text is echoed back in the response as `prompt_text`.
    #[serde(default)]
    pub echo: bool,
    /// HuggingFace alias for `max_tokens`. When present, overrides `max_tokens`.
    #[serde(default)]
    pub max_new_tokens: Option<usize>,
    /// OpenAI newer API alias for `max_tokens`. When present, overrides `max_tokens`.
    #[serde(default)]
    pub max_completion_tokens: Option<usize>,
    /// Return per-token log-probability distributions (1–5 top tokens per step).
    /// Accepts either a boolean (`true` → 5, `false` → disabled) or an integer 1–5.
    /// Chat API sends bool; Completions API sends int — both are handled.
    #[serde(default, deserialize_with = "deser_logprobs")]
    pub logprobs: Option<usize>,
    /// OpenAI chat API alias: number of top log-probs per token (1–5).
    /// When set, overrides `logprobs`. Ignored when `logprobs` is false.
    #[serde(default)]
    pub top_logprobs: Option<usize>,
    /// Min-p sampling threshold (0.0 = disabled). Removes tokens whose probability
    /// is below `min_p × max_token_probability` after softmax. Range [0, 1).
    /// Complements top-p: min-p is relative to the top token, top-p is absolute mass.
    #[serde(default)]
    pub min_p: f32,
    /// OpenAI-compatible additive presence penalty in [-2, 2].
    /// Subtracts this value from the logit of every token that appears at least once
    /// in the prompt or prior output. Positive values discourage repetition.
    #[serde(default)]
    pub presence_penalty: f32,
    /// OpenAI-compatible additive frequency penalty in [-2, 2].
    /// Subtracts `frequency_penalty × occurrence_count` from each token's logit.
    /// Positive values increasingly discourage high-frequency tokens.
    #[serde(default)]
    pub frequency_penalty: f32,
    /// Caller-supplied user ID for audit trail. Echoed back in the response unchanged.
    #[serde(default)]
    pub user: Option<String>,
    /// Text to append after the generated completion (after stop-sequence truncation).
    #[serde(default)]
    pub suffix: Option<String>,
    /// Number of completions to generate. Only 1 is supported on this hardware;
    /// values > 1 are accepted without error but still produce one completion.
    #[serde(default)]
    pub n: Option<u32>,
    /// OpenAI tool/function definitions — accepted for API compatibility, not executed.
    #[serde(default)]
    pub tools: serde_json::Value,
    /// OpenAI tool_choice — accepted for compatibility, ignored.
    #[serde(default)]
    pub tool_choice: serde_json::Value,
    /// OpenAI response_format — accepted for compatibility, ignored (always text).
    #[serde(default)]
    pub response_format: serde_json::Value,
    /// OpenAI parallel_tool_calls — accepted for compatibility, ignored.
    #[serde(default)]
    pub parallel_tool_calls: serde_json::Value,
    /// OpenAI stream_options (e.g. `{"include_usage": true}`) — accepted, usage is always included.
    #[serde(default)]
    pub stream_options: serde_json::Value,
    /// OpenAI best_of — accepted for compatibility, ignored (always 1 on this hardware).
    #[serde(default)]
    pub best_of: Option<u32>,
    /// Per-token logit bias map: string token IDs → additive bias applied after
    /// sampling penalties. Values clamped to [-100, 100]. Use -100 to hard-block
    /// a token (drives logit to effectively -infinity after temperature scaling)
    /// or +100 to strongly prefer a token. String keys match JSON serialization.
    #[serde(default)]
    pub logit_bias: std::collections::HashMap<String, f32>,
    /// If true, stream tokens as Server-Sent Events (SSE). The connection stays
    /// open; each token is emitted as `data: {json}\n\n`; ends with `data: [DONE]`.
    /// Handled by the SSE path in the connection handler — not via this function.
    #[serde(default)]
    pub stream: bool,
}

fn default_model() -> String { "smollm2-135m".into() }
fn default_max_tokens() -> usize { 20 }
fn default_max_seq() -> usize { 256 }
fn default_temperature() -> f32 { 1.0 }
fn default_top_k() -> usize { 40 }
fn default_top_p() -> f32 { 0.0 }
fn default_repetition_penalty() -> f32 { 1.1 }

/// OpenAI-compatible token usage statistics.
#[derive(serde::Serialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Response body for POST /api/v1/llm/sparse/generate
#[derive(serde::Serialize)]
pub struct SparseGenerateResponse {
    pub text: String,
    pub tokens_generated: usize,
    pub time_ms: u64,
    pub model: String,
    pub weight_mode: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub token_ids: Vec<u32>,
    /// Prompt token IDs for diagnostics (first 16, always shown)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prompt_token_ids: Vec<u32>,
    /// Number of prompt tokens consumed (after optional truncation).
    pub prompt_tokens: usize,
    /// Number of transformer layers loaded
    pub layers_loaded: usize,
    /// Whether output norm was loaded
    pub output_norm_loaded: bool,
    /// True if the prompt was truncated to fit within the context window.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub prompt_truncated: bool,
    /// Why generation stopped: "eos", "max_tokens", or "stop_sequence".
    pub stop_reason: String,
    /// OpenAI alias for stop_reason (same value).
    pub finish_reason: String,
    /// Tokens per second during generation (0.0 when time_ms = 0).
    pub tokens_per_second: f64,
    /// Formatted prompt text echoed back when `echo: true` was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_text: Option<String>,
    /// Unique monotonic request ID for tracing. Format: "req-{N}".
    pub request_id: String,
    /// Per-token log-probability distributions (present when `logprobs` was requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<TokenLogprob>>,
    /// OpenAI-compatible token usage breakdown.
    pub usage: Usage,
    /// Caller-supplied user ID echoed back (present when `user` was set in request).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// KV cache tier telemetry: hot/warm/cold token counts, RAM usage, compression ratio.
    pub kv_cache_stats: KvCacheStats,
}

/// Per-request KV cache tier statistics (included in every generate response).
#[derive(serde::Serialize, Default)]
pub struct KvCacheStats {
    pub hot_tokens: usize,
    pub warm_tokens: usize,
    pub cold_tokens: usize,
    pub ram_bytes: usize,
    pub fp32_equiv_bytes: usize,
    pub compression_ratio: f32,
}

/// One candidate token with its log-probability (inside a logprobs entry).
#[derive(serde::Serialize)]
pub struct TopToken {
    /// Decoded text of the candidate token.
    pub token: String,
    pub token_id: u32,
    /// Natural-log probability under softmax (always ≤ 0.0; closer to 0 = more likely).
    pub logprob: f32,
}

/// Log-probability record for one generated token.
#[derive(serde::Serialize)]
pub struct TokenLogprob {
    /// Decoded text of the token that was actually sampled.
    pub token: String,
    pub token_id: u32,
    /// Natural-log probability of the sampled token under softmax.
    pub logprob: f32,
    /// Top-N alternatives at this position (includes the sampled token).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub top_logprobs: Vec<TopToken>,
}

/// Error response
#[derive(serde::Serialize)]
pub struct SparseGenerateError {
    pub error: String,
    pub code: &'static str,
}

// ── Dispatch ───────────────────────────────────────────────────────

/// Dispatch sparse-LLM endpoints. Returns `Some(Response)` if matched.
///
/// `authorized` must be true (Bearer token, mTLS, or USB link-local trust)
/// for write operations. GET /info and GET /models are always permitted.
pub fn dispatch_sparse_llm(
    req: &Request,
    path: &str,
    state: &crate::api::DeviceState,
    authorized: bool,
) -> Option<Response> {
    match (req.method.as_str(), path) {
        ("POST", "/api/v1/llm/sparse/generate") => {
            if !authorized {
                let err = SparseGenerateError {
                    error: "authorization required".into(),
                    code: "UNAUTHORIZED",
                };
                let body = serde_json::to_string(&err).unwrap_or_default();
                return Some(raw_json_response(401, body));
            }
            let (status, body) = handle_sparse_generate(&req.body, state);
            Some(raw_json_response(status, body))
        }
        ("POST", "/api/v1/llm/sparse/tokenize") => {
            if !authorized {
                let err = SparseGenerateError { error: "authorization required".into(), code: "UNAUTHORIZED" };
                let body = serde_json::to_string(&err).unwrap_or_default();
                return Some(raw_json_response(401, body));
            }
            let (status, body) = handle_sparse_tokenize(&req.body);
            Some(raw_json_response(status, body))
        }
        ("GET", "/api/v1/llm/sparse/info") => {
            let (status, body) = handle_sparse_info();
            Some(raw_json_response(status, body))
        }
        ("GET", "/api/v1/llm/sparse/models") => {
            let (status, body) = handle_sparse_models();
            Some(raw_json_response(status, body))
        }
        ("PUT", p) if p.starts_with("/api/v1/llm/sparse/model/") => {
            if !authorized {
                let err = SparseGenerateError {
                    error: "authorization required".into(),
                    code: "UNAUTHORIZED",
                };
                let body = serde_json::to_string(&err).unwrap_or_default();
                return Some(raw_json_response(401, body));
            }
            let (status, body) = handle_model_upload(p, &req.body);
            Some(raw_json_response(status, body))
        }
        // Health check (used by load balancers and monitoring)
        ("GET", "/health") | ("GET", "/v1/health") => {
            let (status, body) = handle_health();
            Some(raw_json_response(status, body))
        }
        // OpenAI-compatible endpoints
        ("OPTIONS", _) if path.starts_with("/v1/") => {
            Some(crate::http::Response {
                status: 204,
                status_text: "No Content",
                content_type: "text/plain",
                body: Vec::new(),
                extra_headers: Vec::new(),
            })
        }
        ("GET", "/v1/models") => {
            let (status, body) = handle_oai_models();
            Some(raw_json_response(status, body))
        }
        ("GET", p) if p.starts_with("/v1/models/") => {
            let model_id = p.trim_start_matches("/v1/models/");
            let (status, body) = handle_oai_model_get(model_id);
            Some(raw_json_response(status, body))
        }
        ("POST", "/v1/completions") => {
            if !authorized {
                let err = SparseGenerateError { error: "authorization required".into(), code: "UNAUTHORIZED" };
                return Some(raw_json_response(401, serde_json::to_string(&err).unwrap_or_default()));
            }
            let (status, body) = handle_oai_completions(&req.body, state);
            Some(raw_json_response(status, body))
        }
        ("POST", "/v1/chat/completions") => {
            if !authorized {
                let err = SparseGenerateError { error: "authorization required".into(), code: "UNAUTHORIZED" };
                return Some(raw_json_response(401, serde_json::to_string(&err).unwrap_or_default()));
            }
            let (status, body) = handle_oai_chat_completions(&req.body, state);
            Some(raw_json_response(status, body))
        }
        // ── Cognitive microkernel pipeline (ADR-094 §pipeline) ────────────
        ("POST", "/api/v1/llm/sparse/pipeline") => {
            let (status, body) = crate::sparse_pipeline::handle_sparse_pipeline(req, state, authorized);
            Some(raw_json_response(status as u16, body))
        }
        ("GET", "/api/v1/llm/sparse/pipeline/status") => {
            let (status, body) = crate::sparse_pipeline::handle_pipeline_status();
            Some(raw_json_response(status as u16, body))
        }
        ("PUT", "/api/v1/llm/sparse/pipeline/weights") => {
            let (status, body) = crate::sparse_pipeline::handle_pipeline_weights_upload(req, authorized);
            Some(raw_json_response(status as u16, body))
        }
        ("GET", "/api/v1/llm/sparse/pipeline/weights") => {
            let (status, body) = crate::sparse_pipeline::handle_pipeline_weights_download(authorized);
            Some(raw_json_response(status as u16, body))
        }
        ("GET", "/api/v1/llm/sparse/pipeline/sensor-status") => {
            let (status, body) = crate::sparse_pipeline::handle_sensor_pipeline_status();
            Some(raw_json_response(status as u16, body))
        }
        _ if req.method == "GET" && req.path.starts_with("/api/v1/llm/sparse/pipeline/events") => {
            let (status, body) = crate::sparse_pipeline::handle_sensor_events(req);
            Some(raw_json_response(status as u16, body))
        }
        ("DELETE", "/api/v1/llm/sparse/pipeline/events") => {
            let (status, body) = crate::sparse_pipeline::handle_sensor_events_clear(authorized);
            Some(raw_json_response(status as u16, body))
        }
        // Streaming pipeline is handled in the connection loop (wants_pipeline_stream)
        // and does not go through the normal dispatch() path.
        _ => None,
    }
}

/// Build a Response from a raw pre-serialized JSON string.
fn raw_json_response(status: u16, body: String) -> Response {
    use crate::http::Response as R;
    // Build via the public json() constructor with a pre-parsed Value so we
    // avoid double-serializing. Fall back to error body on parse failure.
    let value: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({"error":"internal"}));
    R::json(status, &value)
}

// ── Handlers ───────────────────────────────────────────────────────

/// Format a messages array into a single prompt string.
///
/// Instruct models use ChatML (`<|im_start|>role\ncontent<|im_end|>\n`).
/// Base models use plain-text role prefixes separated by double newlines.
fn format_messages_as_prompt(messages: &[ChatMessage], model: &str) -> String {
    if model.contains("instruct") {
        let mut out = String::new();
        for msg in messages {
            out.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, msg.content));
        }
        out.push_str("<|im_start|>assistant\n");
        out
    } else {
        messages.iter().map(|m| match m.role.as_str() {
            "system"    => m.content.clone(),
            "user"      => format!("User: {}", m.content),
            "assistant" => format!("Assistant: {}", m.content),
            _           => m.content.clone(),
        }).collect::<Vec<_>>().join("\n\n")
    }
}

/// Parse, clamp, and validate a generate request body.
///
/// Separated from the handler so it can be tested without a full DeviceState.
/// Returns `Ok(req)` on success, or `Err((status, json_body))` for error responses.
fn parse_generate_request(body: &[u8]) -> Result<SparseGenerateRequest, (u16, String)> {
    let mut req: SparseGenerateRequest = serde_json::from_slice(body).map_err(|e| {
        let err = SparseGenerateError {
            error: format!("invalid request body: {}", e),
            code: "INVALID_REQUEST",
        };
        (400u16, serde_json::to_string(&err).unwrap_or_default())
    })?;

    // Reject unknown model aliases before any filesystem access.
    if !KNOWN_MODELS.contains(&req.model.as_str()) {
        let err = SparseGenerateError {
            error: format!("unknown model '{}'; available: {:?}", req.model, KNOWN_MODELS),
            code: "MODEL_NOT_FOUND",
        };
        return Err((404, serde_json::to_string(&err).unwrap_or_default()));
    }

    // HuggingFace alias: max_new_tokens overrides max_tokens when present.
    if let Some(mnt) = req.max_new_tokens {
        req.max_tokens = mnt;
    }
    // OpenAI newer alias: max_completion_tokens overrides max_tokens when present.
    if let Some(mct) = req.max_completion_tokens {
        req.max_tokens = mct;
    }
    req.max_tokens         = req.max_tokens.clamp(1, MAX_TOKENS_LIMIT);
    req.max_seq            = req.max_seq.clamp(MIN_SEQ_LIMIT, MAX_SEQ_LIMIT);
    req.temperature        = req.temperature.clamp(0.0, 5.0);
    req.top_k              = req.top_k.min(49152);
    req.top_p              = req.top_p.clamp(0.0, 1.0);
    req.repetition_penalty = req.repetition_penalty.clamp(1.0, 5.0);
    // Stop sequences: max 4 entries, each max 32 chars, drop empty.
    req.stop.truncate(4);
    req.stop.retain(|s| !s.is_empty() && s.chars().count() <= 32);
    // Logprobs: top_logprobs overrides logprobs when both are set.
    if let Some(tlp) = req.top_logprobs {
        req.logprobs = Some(tlp.clamp(1, 5));
    } else if let Some(lp) = req.logprobs {
        req.logprobs = Some(lp.clamp(1, 5));
    }
    req.min_p              = req.min_p.clamp(0.0, 1.0);
    req.presence_penalty   = req.presence_penalty.clamp(-2.0, 2.0);
    req.frequency_penalty  = req.frequency_penalty.clamp(-2.0, 2.0);

    // Build the effective prompt: messages takes priority over prompt.
    let effective_text = if !req.messages.is_empty() {
        format_messages_as_prompt(&req.messages, &req.model)
    } else {
        req.prompt.clone()
    };

    let char_count: usize = req.messages.iter().map(|m| m.content.chars().count()).sum::<usize>()
        + req.prompt.chars().count();
    if char_count > MAX_PROMPT_CHARS {
        let err = SparseGenerateError {
            error: format!("prompt/messages exceed {} character limit", MAX_PROMPT_CHARS),
            code: "PROMPT_TOO_LONG",
        };
        return Err((400, serde_json::to_string(&err).unwrap_or_default()));
    }

    if effective_text.is_empty() {
        let err = SparseGenerateError {
            error: "prompt or messages must not be empty".into(),
            code: "PROMPT_EMPTY",
        };
        return Err((400, serde_json::to_string(&err).unwrap_or_default()));
    }

    // Store the formatted effective prompt back so the handler can use it directly.
    req.prompt = effective_text;

    // Reject tool-call requests — not supported on this hardware.
    if req.tools.is_array() && !req.tools.as_array().unwrap().is_empty() {
        let err = SparseGenerateError {
            error: "tool_calls not supported on this model".into(),
            code: "FEATURE_NOT_SUPPORTED",
        };
        return Err((400, serde_json::to_string(&err).unwrap_or_default()));
    }

    // Reject n > 1 — single-core Pi Zero can only produce one completion at a time.
    if req.n.unwrap_or(1) > 1 {
        let err = SparseGenerateError {
            error: "n > 1 not supported: this model produces one completion per request".into(),
            code: "FEATURE_NOT_SUPPORTED",
        };
        return Err((400, serde_json::to_string(&err).unwrap_or_default()));
    }

    // Reject best_of > 1 — same hardware constraint as n > 1.
    if req.best_of.unwrap_or(1) > 1 {
        let err = SparseGenerateError {
            error: "best_of > 1 not supported: this model produces one completion per request".into(),
            code: "FEATURE_NOT_SUPPORTED",
        };
        return Err((400, serde_json::to_string(&err).unwrap_or_default()));
    }

    // Reject unsupported response_format types.
    match req.response_format.get("type").and_then(|v| v.as_str()) {
        None | Some("text") | Some("json_object") => {}
        Some(other) => {
            let err = SparseGenerateError {
                error: format!("response_format type '{}' not supported; use 'text' or 'json_object'", other),
                code: "FEATURE_NOT_SUPPORTED",
            };
            return Err((400, serde_json::to_string(&err).unwrap_or_default()));
        }
    }

    // JSON mode: inject a system-level instruction when response_format.type == "json_object".
    if req.response_format.get("type").and_then(|v| v.as_str()) == Some("json_object") {
        let json_hint = "Respond only with valid JSON. Do not include any text outside the JSON object.";
        let current = req.prompt.clone();
        req.prompt = format!("{}\n\n{}", json_hint, current);
        // Clamp to char limit after injection.
        if req.prompt.chars().count() > MAX_PROMPT_CHARS {
            req.prompt.truncate(MAX_PROMPT_CHARS * 4); // byte-safe overestimate; decoded chars are checked below
        }
    }

    Ok(req)
}

/// Handle POST /api/v1/llm/sparse/generate
///
/// Admission checks (mirror llm_inference.rs policy for Pi Zero):
/// - max_tokens clamped to 200
/// - max_seq clamped to 512
/// - prompt length checked: reject if > 1000 chars
/// - Hardware check: warn in response if not Pi Zero 2 W (don't reject)
pub fn handle_sparse_generate(
    body: &[u8],
    _state: &crate::api::DeviceState,
) -> (u16, String) {
    // 1–3. Parse + clamp + validate.
    let req = match parse_generate_request(body) {
        Ok(r) => r,
        Err((status, body)) => return (status, body),
    };

    // Hardware check (non-blocking warning, logged via weight_mode suffix)
    let on_pi_zero = crate::sparse_llm::detect_pi_zero();

    // Assign a unique request ID for tracing before any fallible work.
    let request_id = format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed));

    // 4–7. Try to acquire the inference mutex.
    //      Pi Zero 2 W is single-core: only one inference runs at a time.
    //      Use try_lock so concurrent requests fail-fast with 503 instead of
    //      queuing behind a multi-second inference and causing timeouts.
    let model_path = PathBuf::from(format!("{}/{}/model.gguf", model_base_dir(), req.model));
    let tokenizer_path = PathBuf::from(format!("{}/{}/tokenizer.json", model_base_dir(), req.model));

    let mut cache_guard = match SPARSE_CACHE.try_lock() {
        Ok(g) => g,
        Err(_) => {
            let err = SparseGenerateError {
                error: "server busy: inference in progress; retry shortly".into(),
                code: "BUSY",
            };
            return (503, serde_json::to_string(&err).unwrap_or_default());
        }
    };
    let cache_hit = cache_guard.as_ref().map(|c| c.model_path == model_path).unwrap_or(false);

    if !cache_hit {
        // Cold path: load weights and tokenizer from disk (only on first request
        // or when the model changes).
        let tokenizer = if tokenizer_path.exists() {
            BpeTokenizer::from_file(&tokenizer_path).unwrap_or_else(|_| BpeTokenizer::byte_fallback_stub())
        } else {
            BpeTokenizer::byte_fallback_stub()
        };

        let (weights, config) = if model_path.exists() {
            let mut tmp_runner = SparseLlmRunner::new(model_path.clone())
                .unwrap_or_else(|_| SparseLlmRunner::new(PathBuf::from("/nonexistent")).unwrap());
            let loaded_weights = if tmp_runner.load_header().is_ok() {
                tmp_runner.gguf_header.as_ref().and_then(|hdr| {
                    crate::sparse_llm_loader::load_weights(
                        &model_path, hdr.tensor_count, hdr.post_kv_file_offset,
                    ).ok()
                })
            } else { None };
            let cfg = tmp_runner.config.clone();
            (loaded_weights, cfg)
        } else {
            (None, SmolLm2Config::default())
        };

        *cache_guard = Some(SparseCache { model_path: model_path.clone(), weights, tokenizer, config });
    }

    let cached = cache_guard.as_ref().unwrap();

    // Per-request: fresh runner with an empty KV cache but correct config.
    let mut runner = match SparseLlmRunner::new(model_path.clone()) {
        Ok(mut r) => { r.config = cached.config.clone(); r }
        Err(e) => {
            let err = SparseGenerateError { error: format!("runner init: {}", e), code: "RUNNER_INIT_ERROR" };
            return (500, serde_json::to_string(&err).unwrap_or_default());
        }
    };

    // Build the final prompt to tokenize.
    // When messages were provided, req.prompt already contains the fully formatted
    // conversation text (system messages included); skip the standalone system prefix.
    // When only prompt is provided, apply req.system as a prefix.
    let full_prompt = if !req.messages.is_empty() {
        req.prompt.clone()
    } else {
        match &req.system {
            Some(sys) if !sys.is_empty() => {
                if req.model.contains("instruct") {
                    format!(
                        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                        sys, req.prompt
                    )
                } else {
                    format!("{}\n\n{}", sys, req.prompt)
                }
            }
            _ => req.prompt.clone(),
        }
    };

    let mut token_ids = cached.tokenizer.encode(&full_prompt);
    // Strip trailing EOS — its embedding is near-zero, driving all logits to ~0.
    if token_ids.last() == Some(&cached.tokenizer.eos_id) {
        token_ids.pop();
    }

    // Context-length protection: truncate prompt if it leaves no room for generation.
    let max_ctx = cached.config.max_seq_len;
    let prompt_truncated = if token_ids.len() + req.max_tokens > max_ctx {
        let keep = max_ctx.saturating_sub(req.max_tokens).saturating_sub(1);
        if keep == 0 {
            let err = SparseGenerateError {
                error: format!("prompt too long: {} tokens, max_ctx={}", token_ids.len(), max_ctx),
                code: "PROMPT_TOO_LONG_TOKENS",
            };
            return (400, serde_json::to_string(&err).unwrap_or_default());
        }
        let skip = token_ids.len() - keep;
        token_ids = token_ids[skip..].to_vec();
        true
    } else {
        false
    };

    let prompt_token_count = token_ids.len();
    let prompt_diag: Vec<u32> = token_ids.iter().take(16).cloned().collect();
    // Capture prompt text for echo before moving full_prompt.
    let echoed_prompt = if req.echo { Some(full_prompt.clone()) } else { None };

    let weights = cached.weights.as_ref();
    let layers_loaded = weights.map(|w| w.layers_raw.len()).unwrap_or(0);
    let output_norm_loaded = weights.map(|w| w.output_norm.is_some()).unwrap_or(false);
    let weight_mode = match (&weights, on_pi_zero) {
        (Some(w), _) if w.layers_raw.is_empty() => "gguf-embed-only".into(),
        (Some(w), _) if w.weight_tied => format!("gguf-tied[{}L{}]", w.layers_raw.len(), if w.output_norm.is_some() { "+norm" } else { "" }),
        (Some(w), _) => format!("gguf[{}L{}]", w.layers_raw.len(), if w.output_norm.is_some() { "+norm" } else { "" }),
        (None, true)  => "stub".into(),
        (None, false) => "stub(non-pi-zero-hw)".into(),
    };

    let logprobs_k = req.logprobs.unwrap_or(0);
    let logit_bias: std::collections::HashMap<u32, f32> = req.logit_bias.iter()
        .filter_map(|(k, &v)| k.parse::<u32>().ok().map(|id| (id, v.clamp(-100.0, 100.0))))
        .collect();

    // 8. Time the generate call (real weights if loaded, stub otherwise).
    let start = Instant::now();
    let (generated_ids, raw_logprobs) = match crate::sparse_llm_loader::generate_with_fallback(
        &mut runner,
        weights,
        &token_ids,
        req.max_tokens,
        cached.tokenizer.eos_id,
        req.temperature,
        req.top_k,
        req.top_p,
        req.min_p,
        req.repetition_penalty,
        req.presence_penalty,
        req.frequency_penalty,
        &logit_bias,
        req.seed,
        logprobs_k,
        &mut |_, _| {},  // non-streaming: no per-token callback
    ) {
        Ok(pair) => pair,
        Err(e) => {
            let err = SparseGenerateError {
                error: format!("inference error: {}", e),
                code: "INFERENCE_ERROR",
            };
            return (500, serde_json::to_string(&err).unwrap_or_default());
        }
    };
    let time_ms = start.elapsed().as_millis() as u64;
    let tokens_per_second = if time_ms > 0 {
        generated_ids.len() as f64 / (time_ms as f64 / 1000.0)
    } else {
        0.0
    };

    // Collect KV tier telemetry from the runner's quant cache.
    let kv_stats = {
        let s = runner.kv_quant.tier_stats();
        KvCacheStats {
            hot_tokens: s.hot_tokens,
            warm_tokens: s.warm_tokens,
            cold_tokens: s.cold_tokens,
            ram_bytes: s.ram_bytes,
            fp32_equiv_bytes: s.fp32_equiv_bytes,
            compression_ratio: s.compression_ratio(),
        }
    };

    // 9. Determine stop reason (before decoding — based on token count).
    //    If generated_ids.len() < req.max_tokens the model hit EOS; otherwise the
    //    budget was exhausted. Stop sequences may override this below.
    let mut stop_reason = if generated_ids.len() < req.max_tokens {
        "eos".to_string()
    } else {
        "max_tokens".to_string()
    };

    // 10. Decode tokens → text; apply stop sequences if any.
    let mut text = cached.tokenizer.decode(&generated_ids);
    if !req.stop.is_empty() {
        // Find the earliest byte position where any stop string appears.
        let earliest = req.stop.iter()
            .filter_map(|s| text.find(s.as_str()).map(|pos| pos))
            .min();
        if let Some(cut) = earliest {
            text.truncate(cut);
            stop_reason = "stop_sequence".to_string();
        }
    }
    // Append suffix (after stop-sequence truncation) when provided.
    if let Some(ref sfx) = req.suffix {
        if !sfx.is_empty() {
            text.push_str(sfx);
        }
    }
    let out_token_ids = if text.is_empty() { generated_ids.clone() } else { vec![] };

    // Convert raw (token_id, ln_prob) pairs → TokenLogprob records.
    let logprobs_resp: Option<Vec<TokenLogprob>> = if logprobs_k > 0 && !raw_logprobs.is_empty() {
        let records: Vec<TokenLogprob> = generated_ids.iter().zip(raw_logprobs.iter())
            .map(|(&tok_id, pairs)| {
                let sampled_lp = pairs.iter()
                    .find(|&&(id, _)| id == tok_id)
                    .map(|&(_, lp)| lp)
                    .unwrap_or(f32::NEG_INFINITY);
                let top_logprobs: Vec<TopToken> = pairs.iter()
                    .map(|&(id, lp)| TopToken {
                        token: cached.tokenizer.decode(&[id]),
                        token_id: id,
                        logprob: lp,
                    })
                    .collect();
                TokenLogprob {
                    token: cached.tokenizer.decode(&[tok_id]),
                    token_id: tok_id,
                    logprob: sampled_lp,
                    top_logprobs,
                }
            })
            .collect();
        Some(records)
    } else {
        None
    };

    let completion_tokens = generated_ids.len();
    let resp = SparseGenerateResponse {
        text,
        tokens_generated: completion_tokens,
        time_ms,
        tokens_per_second,
        model: req.model,
        weight_mode,
        token_ids: out_token_ids,
        prompt_token_ids: prompt_diag,
        prompt_tokens: prompt_token_count,
        layers_loaded,
        output_norm_loaded,
        prompt_truncated,
        finish_reason: stop_reason.clone(),
        stop_reason,
        prompt_text: echoed_prompt,
        request_id,
        logprobs: logprobs_resp,
        usage: Usage {
            prompt_tokens: prompt_token_count,
            completion_tokens,
            total_tokens: prompt_token_count + completion_tokens,
        },
        user: req.user,
        kv_cache_stats: kv_stats,
    };

    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(e) => {
            let err = SparseGenerateError {
                error: format!("serialization error: {}", e),
                code: "SERIALIZATION_ERROR",
            };
            (500, serde_json::to_string(&err).unwrap_or_default())
        }
    }
}

// ── SSE streaming handler ─────────────────────────────────────────────

/// Which SSE wire format to emit for streaming responses.
enum SseFormat {
    /// Custom cognitum format: `{"token":"...","token_id":N,"index":N}` per token.
    Custom,
    /// OpenAI text_completion chunk format: `{"choices":[{"text":"...","index":0,"finish_reason":null}]}`.
    OaiCompletion,
    /// OpenAI chat.completion.chunk format: `{"choices":[{"delta":{"content":"..."},"finish_reason":null}]}`.
    OaiChat,
}

/// Handle streaming SSE for any generate path.
///
/// `path` selects the wire format:
///   - `/api/v1/llm/sparse/generate` → cognitum custom chunks
///   - `/v1/completions`             → OpenAI text_completion chunks
///   - `/v1/chat/completions`        → OpenAI chat.completion.chunk
///
/// Writes HTTP headers + SSE events directly to `writer` (bypasses Content-Length).
/// Connection closes after `data: [DONE]`.
pub fn handle_sparse_generate_sse<W: std::io::Write>(
    body: &[u8],
    _state: &crate::api::DeviceState,
    authorized: bool,
    path: &str,
    writer: &mut W,
) {
    let sse_format = match path {
        "/v1/completions"      => SseFormat::OaiCompletion,
        "/v1/chat/completions" => SseFormat::OaiChat,
        _                      => SseFormat::Custom,
    };
    use std::io::Write as IoWrite;

    macro_rules! sse_err {
        ($status:expr, $text:expr, $msg:expr) => {{
            let b = format!("{{\"error\":{},\"code\":{}}}", serde_json::Value::String($msg.into()), serde_json::Value::String($text.into()));
            let _ = write!(writer, "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", $status, $text, b.len(), b);
            let _ = writer.flush();
            return;
        }};
    }

    if !authorized {
        sse_err!(401, "Unauthorized", "authorization required");
    }

    let req = match parse_generate_request(body) {
        Ok(r) => r,
        Err((status, body_str)) => {
            let st = match status {
                400 => "Bad Request",
                401 => "Unauthorized",
                404 => "Not Found",
                413 => "Payload Too Large",
                _ => "Error",
            };
            let _ = write!(writer, "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, st, body_str.len(), body_str);
            let _ = writer.flush();
            return;
        }
    };

    // Acquire the cache (non-streaming model loading is the same path).
    let model_path = PathBuf::from(format!("{}/{}/model.gguf", model_base_dir(), req.model));
    let tokenizer_path = PathBuf::from(format!("{}/{}/tokenizer.json", model_base_dir(), req.model));

    let mut cache_guard = match SPARSE_CACHE.try_lock() {
        Ok(g) => g,
        Err(_) => { sse_err!(503, "Service Unavailable", "server busy: inference in progress; retry shortly"); }
    };
    let cache_hit = cache_guard.as_ref().map(|c| c.model_path == model_path).unwrap_or(false);
    if !cache_hit {
        let tokenizer = if tokenizer_path.exists() {
            BpeTokenizer::from_file(&tokenizer_path).unwrap_or_else(|_| BpeTokenizer::byte_fallback_stub())
        } else {
            BpeTokenizer::byte_fallback_stub()
        };
        let (weights, config) = if model_path.exists() {
            let mut tmp_runner = SparseLlmRunner::new(model_path.clone())
                .unwrap_or_else(|_| SparseLlmRunner::new(PathBuf::from("/nonexistent")).unwrap());
            let loaded_weights = if tmp_runner.load_header().is_ok() {
                tmp_runner.gguf_header.as_ref().and_then(|hdr| {
                    crate::sparse_llm_loader::load_weights(&model_path, hdr.tensor_count, hdr.post_kv_file_offset).ok()
                })
            } else { None };
            let cfg = tmp_runner.config.clone();
            (loaded_weights, cfg)
        } else {
            (None, SmolLm2Config::default())
        };
        *cache_guard = Some(SparseCache { model_path: model_path.clone(), weights, tokenizer, config });
    }
    let cached = cache_guard.as_ref().unwrap();
    let mut runner = match SparseLlmRunner::new(model_path.clone()) {
        Ok(mut r) => { r.config = cached.config.clone(); r }
        Err(e) => { sse_err!(500, "Internal Server Error", &format!("runner init: {}", e)); }
    };

    // Build token IDs.
    let full_prompt = if !req.messages.is_empty() {
        req.prompt.clone()
    } else {
        match &req.system {
            Some(sys) if !sys.is_empty() && req.model.contains("instruct") =>
                format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", sys, req.prompt),
            Some(sys) if !sys.is_empty() =>
                format!("{}\n\n{}", sys, req.prompt),
            _ => req.prompt.clone(),
        }
    };
    let mut token_ids = cached.tokenizer.encode(&full_prompt);
    if token_ids.last() == Some(&cached.tokenizer.eos_id) { token_ids.pop(); }
    let max_ctx = cached.config.max_seq_len;
    if token_ids.len() + req.max_tokens > max_ctx {
        let keep = max_ctx.saturating_sub(req.max_tokens).saturating_sub(1);
        if keep == 0 { sse_err!(400, "Bad Request", "prompt too long"); }
        let skip = token_ids.len() - keep;
        token_ids = token_ids[skip..].to_vec();
    }
    let prompt_token_count = token_ids.len();

    // Write SSE response headers.
    let sse_hdr = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
    if writer.write_all(sse_hdr.as_bytes()).is_err() { return; }
    if writer.flush().is_err() { return; }

    let weights = cached.weights.as_ref();
    let request_id = format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed));
    let start = Instant::now();
    let created_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let req_model = req.model.clone();

    // OpenAI chat streaming: emit the initial role chunk before any content tokens.
    // The SDK reads this to know the assistant turn has begun.
    if matches!(sse_format, SseFormat::OaiChat) {
        let role_chunk = format!(
            "data: {{\"id\":{id},\"object\":\"chat.completion.chunk\",\"created\":{ts},\"model\":{model},\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"content\":\"\"}},\"finish_reason\":null}}]}}\n\n",
            id    = serde_json::to_string(&request_id).unwrap_or_default(),
            ts    = created_ts,
            model = serde_json::to_string(&req_model).unwrap_or_default(),
        );
        let _ = writer.write_all(role_chunk.as_bytes());
        let _ = writer.flush();
    }

    let mut token_index: u32 = 0;
    let logit_bias_sse: std::collections::HashMap<u32, f32> = req.logit_bias.iter()
        .filter_map(|(k, &v)| k.parse::<u32>().ok().map(|id| (id, v.clamp(-100.0, 100.0))))
        .collect();
    // Stop-sequence state: accumulate decoded text; halt emission when any stop
    // string appears. Uses a per-character rolling check so sequences can span
    // token boundaries.
    let stop_seqs_sse: Vec<String> = req.stop.clone();
    let mut sse_text_buf = String::new();
    let mut sse_stopped = false;

    let generated_ids = {
        let tok = &cached.tokenizer;
        let rid = request_id.clone();
        let sse_logprobs_k = req.logprobs.unwrap_or(0);
        let sse_req_logprobs = sse_logprobs_k > 0;
        match crate::sparse_llm_loader::generate_with_fallback(
            &mut runner, weights, &token_ids, req.max_tokens,
            cached.tokenizer.eos_id,
            req.temperature, req.top_k, req.top_p, req.min_p,
            req.repetition_penalty, req.presence_penalty, req.frequency_penalty,
            &logit_bias_sse,
            req.seed, sse_logprobs_k,
            &mut |token_id: u32, step_lp: &[(u32, f32)]| {
                if sse_stopped { return; }
                let text = tok.decode(&[token_id]);
                sse_text_buf.push_str(&text);
                if !stop_seqs_sse.is_empty() {
                    if stop_seqs_sse.iter().any(|s| sse_text_buf.contains(s.as_str())) {
                        sse_stopped = true;
                        return;
                    }
                }
                let id_json   = serde_json::to_string(&rid).unwrap_or_default();
                let text_json = serde_json::to_string(&text).unwrap_or_default();
                let model_json = serde_json::to_string(&req_model).unwrap_or_default();
                let chunk = match sse_format {
                    SseFormat::OaiCompletion => format!(
                        "data: {{\"id\":{id},\"object\":\"text_completion\",\"created\":{ts},\"model\":{model},\"choices\":[{{\"text\":{tok},\"index\":0,\"logprobs\":null,\"finish_reason\":null}}]}}\n\n",
                        id    = id_json,
                        ts    = created_ts,
                        model = model_json,
                        tok   = text_json,
                    ),
                    SseFormat::OaiChat => {
                        // Build per-token logprobs for chat streaming when requested.
                        let logprobs_field = if sse_req_logprobs && !step_lp.is_empty() {
                            let sampled_lp = step_lp.iter()
                                .find(|&&(id, _)| id == token_id)
                                .map(|&(_, lp)| lp)
                                .unwrap_or(f32::NEG_INFINITY);
                            let top_logprobs_arr: Vec<serde_json::Value> = step_lp.iter()
                                .map(|&(id, lp)| serde_json::json!({"token": tok.decode(&[id]), "logprob": lp, "bytes": null}))
                                .collect();
                            let lp_json = serde_json::json!({
                                "content": [{
                                    "token": text,
                                    "logprob": sampled_lp,
                                    "bytes": null,
                                    "top_logprobs": top_logprobs_arr,
                                }]
                            });
                            serde_json::to_string(&lp_json).unwrap_or_else(|_| "null".into())
                        } else {
                            "null".into()
                        };
                        format!(
                            "data: {{\"id\":{id},\"object\":\"chat.completion.chunk\",\"created\":{ts},\"model\":{model},\"choices\":[{{\"index\":0,\"delta\":{{\"content\":{tok}}},\"logprobs\":{lp},\"finish_reason\":null}}]}}\n\n",
                            id    = id_json,
                            ts    = created_ts,
                            model = model_json,
                            tok   = text_json,
                            lp    = logprobs_field,
                        )
                    },
                    SseFormat::Custom => format!(
                        "data: {{\"id\":{id},\"token\":{tok},\"token_id\":{tid},\"index\":{idx}}}\n\n",
                        id  = id_json,
                        tok = text_json,
                        tid = token_id,
                        idx = token_index,
                    ),
                };
                token_index += 1;
                let _ = writer.write_all(chunk.as_bytes());
                let _ = writer.flush();
            },
        ) {
            Ok((ids, _lp)) => ids,
            Err(_) => Vec::new(),
        }
    };
    let time_ms = start.elapsed().as_millis() as u64;
    let stop_reason = if sse_stopped {
        "stop_sequence"
    } else if generated_ids.len() < req.max_tokens {
        "eos"
    } else {
        "max_tokens"
    };
    let mut full_text = cached.tokenizer.decode(&generated_ids);
    if sse_stopped {
        // Truncate at the earliest stop sequence that appears in the full text.
        if let Some(cut) = stop_seqs_sse.iter().filter_map(|s| full_text.find(s.as_str())).min() {
            full_text.truncate(cut);
        }
    }
    // Append suffix when provided and not stopped mid-sequence.
    if !sse_stopped {
        if let Some(ref sfx) = req.suffix {
            if !sfx.is_empty() { full_text.push_str(sfx); }
        }
    }
    let finish_oai = match stop_reason {
        "eos" | "stop_sequence" => "stop",
        "max_tokens"            => "length",
        other                   => other,
    };
    let usage_obj = serde_json::json!({
        "prompt_tokens": prompt_token_count,
        "completion_tokens": generated_ids.len(),
        "total_tokens": prompt_token_count + generated_ids.len(),
    });
    let summary = match sse_format {
        SseFormat::OaiCompletion => serde_json::json!({
            "id": request_id,
            "object": "text_completion",
            "created": created_ts,
            "model": req_model,
            "choices": [{"text": full_text, "index": 0, "logprobs": null, "finish_reason": finish_oai}],
            "usage": usage_obj,
        }),
        SseFormat::OaiChat => serde_json::json!({
            "id": request_id,
            "object": "chat.completion.chunk",
            "created": created_ts,
            "model": req_model,
            "system_fingerprint": serde_json::Value::Null,
            "choices": [{"index": 0, "delta": {}, "finish_reason": finish_oai}],
            "usage": usage_obj,
        }),
        SseFormat::Custom => serde_json::json!({
            "id": request_id,
            "object": "text_completion",
            "text": full_text,
            "tokens_generated": generated_ids.len(),
            "time_ms": time_ms,
            "stop_reason": stop_reason,
            "finish_reason": finish_oai,
            "usage": usage_obj,
        }),
    };
    let _ = write!(writer, "data: {}\n\n", summary);
    let _ = writer.write_all(b"data: [DONE]\n\n");
    let _ = writer.flush();
}

/// Handle POST /api/v1/llm/sparse/tokenize
///
/// Tokenizes a text string using the cached tokenizer (or byte-fallback stub when
/// the real tokenizer is not loaded). Returns token IDs, decoded token strings,
/// and count — useful for measuring prompt length before calling /generate.
///
/// Request body: `{"text": "...", "model": "smollm2-135m"}`
/// Response:     `{"token_ids": [...], "tokens": [...], "count": N, "model": "..."}`
fn handle_sparse_tokenize(body: &[u8]) -> (u16, String) {
    #[derive(serde::Deserialize)]
    struct TokenizeRequest {
        text: String,
        #[serde(default = "default_model")]
        model: String,
    }

    let req: TokenizeRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => {
            let err = SparseGenerateError { error: format!("invalid request: {}", e), code: "INVALID_REQUEST" };
            return (400, serde_json::to_string(&err).unwrap_or_default());
        }
    };

    if req.text.is_empty() {
        let err = SparseGenerateError { error: "text must not be empty".into(), code: "PROMPT_EMPTY" };
        return (400, serde_json::to_string(&err).unwrap_or_default());
    }
    if req.text.chars().count() > MAX_PROMPT_CHARS {
        let err = SparseGenerateError {
            error: format!("text exceeds {} character limit", MAX_PROMPT_CHARS),
            code: "PROMPT_TOO_LONG",
        };
        return (400, serde_json::to_string(&err).unwrap_or_default());
    }

    // Acquire the cache to get the tokenizer (use blocking lock — tokenize is
    // read-only and ~microseconds, never contends with a slow generate call).
    let (token_ids, tokens) = {
        let cache_guard = SPARSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let stub;
        let tok: &crate::sparse_llm_tokenizer::BpeTokenizer = match cache_guard.as_ref() {
            Some(c) => &c.tokenizer,
            None => { stub = crate::sparse_llm_tokenizer::BpeTokenizer::byte_fallback_stub(); &stub }
        };
        let ids = tok.encode(&req.text);
        let toks: Vec<String> = ids.iter().map(|&id| tok.decode(&[id])).collect();
        (ids, toks)
    };
    let count = token_ids.len();

    let resp = serde_json::json!({
        "token_ids": token_ids,
        "tokens": tokens,
        "count": count,
        "model": req.model,
    });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error","code":"SERIALIZATION_ERROR"}"#.into()),
    }
}

/// Returns JSON describing the sparse-llm endpoint capabilities.
///
/// GET /api/v1/llm/sparse/info
pub fn handle_sparse_info() -> (u16, String) {
    let cached = SPARSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let layers = cached.as_ref().and_then(|c| c.weights.as_ref()).map(|w| w.layers_raw.len()).unwrap_or(0);
    let warm = cached.is_some();
    drop(cached);

    let info = serde_json::json!({
        "models": KNOWN_MODELS,
        "max_tokens": MAX_TOKENS_LIMIT,
        "max_seq": MAX_SEQ_LIMIT,
        "weights_cached": warm,
        "layers_loaded": layers,
        "sampling": {
            "temperature":        { "default": 1.0,  "range": [0.0, 5.0],  "note": "0=greedy" },
            "top_k":              { "default": 40,   "range": [0, 49152],  "note": "0=all tokens" },
            "top_p":              { "default": 0.0,  "range": [0.0, 1.0],  "note": "0=disabled; 0.9=nucleus" },
            "min_p":              { "default": 0.0,  "range": "0.0..1.0",  "note": "min-p filter: remove tokens below min_p × max_prob" },
            "repetition_penalty": { "default": 1.1,  "range": [1.0, 5.0],  "note": "1=none; multiplicative" },
            "presence_penalty":   { "default": 0.0,  "range": [-2.0, 2.0], "note": "OpenAI additive: subtract from each seen token once" },
            "frequency_penalty":  { "default": 0.0,  "range": [-2.0, 2.0], "note": "OpenAI additive: subtract penalty × count from each token" },
            "logit_bias":         { "default": {},   "note": "map of string token IDs to additive bias [-100,100]; -100 hard-blocks" },
            "seed":               { "default": null, "note": "u64 or null; null=time-seeded (non-deterministic)" },
            "stop":               { "default": [],   "note": "array of up to 4 strings (≤32 chars); truncates at first match, works in SSE" },
            "echo":               { "default": false, "note": "if true, formatted prompt is returned as prompt_text" },
            "messages":           { "default": [],   "note": "OpenAI-compatible [{role,content}] array; takes priority over prompt" },
            "max_tokens":         { "default": 20,   "range": [1, 200],    "note": "tokens to generate" },
            "max_new_tokens":     { "default": null, "note": "HuggingFace alias for max_tokens" },
            "max_completion_tokens": { "default": null, "note": "OpenAI newer alias for max_tokens" },
            "logprobs":           { "default": null, "note": "integer 1-5: top-N token log-probs per step" },
            "stream":             { "default": false, "note": "SSE streaming; all OpenAI paths support stream:true" }
        },
        "endpoints": {
            "generate":      "POST /api/v1/llm/sparse/generate",
            "tokenize":      "POST /api/v1/llm/sparse/tokenize",
            "info":          "GET  /api/v1/llm/sparse/info",
            "models":        "GET  /api/v1/llm/sparse/models",
            "upload":        "PUT  /api/v1/llm/sparse/model/{id}/{filename}",
            "oai_models":    "GET  /v1/models",
            "oai_model":     "GET  /v1/models/{id}",
            "oai_complete":  "POST /v1/completions",
            "oai_chat":      "POST /v1/chat/completions"
        },
        "response_fields": {
            "usage":         "OpenAI-compatible {prompt_tokens, completion_tokens, total_tokens}",
            "finish_reason": "OpenAI alias for stop_reason: eos|max_tokens|stop_sequence",
            "logprobs":      "per-token [{token, token_id, logprob, top_logprobs:[...]}] when requested",
            "request_id":    "monotonic req-N identifier for tracing"
        },
        "concurrency": "serialized",
        "busy_code": "503"
    });
    match serde_json::to_string(&info) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error","code":"SERIALIZATION_ERROR"}"#.into()),
    }
}

// ── Model file management ─────────────────────────────────────────

/// List installed models by scanning model_base_dir().
///
/// GET /api/v1/llm/sparse/models
pub fn handle_sparse_models() -> (u16, String) {
    let mut result = serde_json::json!({
        "models": serde_json::Value::Array(vec![]),
        "base_dir": model_base_dir(),
    });
    let models = result["models"].as_array_mut().unwrap();
    for &model_id in KNOWN_MODELS {
        let model_dir = format!("{}/{}", model_base_dir(), model_id);
        let gguf_path = format!("{}/model.gguf", model_dir);
        let tok_path  = format!("{}/tokenizer.json", model_dir);
        let has_gguf  = std::path::Path::new(&gguf_path).exists();
        let has_tok   = std::path::Path::new(&tok_path).exists();
        let gguf_bytes = if has_gguf {
            std::fs::metadata(&gguf_path).map(|m| m.len()).unwrap_or(0)
        } else { 0 };
        models.push(serde_json::json!({
            "id": model_id,
            "model_gguf": has_gguf,
            "tokenizer_json": has_tok,
            "gguf_bytes": gguf_bytes,
            "ready": has_gguf,
        }));
    }
    match serde_json::to_string(&result) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error"}"#.into()),
    }
}

/// Upload a model file (model.gguf or tokenizer.json) for a known model ID.
///
/// PUT /api/v1/llm/sparse/model/{model_id}/{filename}
///
/// The path segment after `/model/` must be `{id}/{filename}` where `id` ∈
/// `KNOWN_MODELS` and `filename` ∈ `ALLOWED_FILENAMES`.
/// Body is `application/octet-stream`; max 320 MB.
pub fn handle_model_upload(path: &str, body: &[u8]) -> (u16, String) {
    // Parse /api/v1/llm/sparse/model/{id}/{filename}
    let suffix = path.trim_start_matches("/api/v1/llm/sparse/model/");
    let parts: Vec<&str> = suffix.splitn(2, '/').collect();
    if parts.len() != 2 {
        let err = serde_json::json!({"error": "path must be /model/{id}/{filename}", "code": "INVALID_PATH"});
        return (400, err.to_string());
    }
    let (model_id, filename) = (parts[0], parts[1]);

    if !KNOWN_MODELS.contains(&model_id) {
        let err = serde_json::json!({
            "error": format!("unknown model id '{}'; known: {:?}", model_id, KNOWN_MODELS),
            "code": "UNKNOWN_MODEL"
        });
        return (400, err.to_string());
    }
    if !ALLOWED_FILENAMES.contains(&filename) {
        let err = serde_json::json!({
            "error": format!("filename must be one of {:?}", ALLOWED_FILENAMES),
            "code": "INVALID_FILENAME"
        });
        return (400, err.to_string());
    }
    if body.is_empty() {
        let err = serde_json::json!({"error": "empty body", "code": "EMPTY_BODY"});
        return (400, err.to_string());
    }
    if body.len() > MAX_UPLOAD_BYTES {
        let err = serde_json::json!({
            "error": format!("file too large: {} > {} bytes", body.len(), MAX_UPLOAD_BYTES),
            "code": "FILE_TOO_LARGE"
        });
        return (413, err.to_string());
    }

    // Create model directory and write file.
    let model_dir = format!("{}/{}", model_base_dir(), model_id);
    if let Err(e) = std::fs::create_dir_all(&model_dir) {
        let err = serde_json::json!({
            "error": format!("failed to create model dir: {}", e),
            "code": "FS_ERROR"
        });
        return (500, err.to_string());
    }
    let dest = format!("{}/{}", model_dir, filename);
    if let Err(e) = std::fs::write(&dest, body) {
        let err = serde_json::json!({
            "error": format!("failed to write {}: {}", dest, e),
            "code": "WRITE_ERROR"
        });
        return (500, err.to_string());
    }

    // Evict the weight cache so the next request reloads from the new file.
    // Use try_lock — if inference is running, skip eviction (the running request
    // has the old weights; the next acquire will reload when it's not busy).
    if let Ok(mut guard) = SPARSE_CACHE.try_lock() {
        if guard.as_ref().map(|c| c.model_path.starts_with(&model_dir)).unwrap_or(false) {
            *guard = None;
        }
    }

    let resp = serde_json::json!({
        "status": "ok",
        "model_id": model_id,
        "filename": filename,
        "bytes_written": body.len(),
        "path": dest,
        "cache_evicted": true,
    });
    (200, resp.to_string())
}

// ── OpenAI-compatible endpoints ───────────────────────────────────

/// Convert our internal `{"error":"...","code":"..."}` body to the OpenAI
/// `{"error":{"message":"...","type":"...","param":null,"code":"..."}}` envelope.
fn to_oai_error(status: u16, our_body: &str) -> String {
    let err_type = match status {
        429 | 503 => "server_error",
        500..=599 => "server_error",
        _         => "invalid_request_error",
    };
    let v: serde_json::Value = serde_json::from_str(our_body).unwrap_or_default();
    let message = v["error"].as_str().unwrap_or("internal error");
    let code    = v["code"].as_str().unwrap_or("internal_error");
    serde_json::json!({
        "error": { "message": message, "type": err_type, "param": null, "code": code }
    }).to_string()
}

/// Normalise a `/v1/completions` body so that `"prompt": ["text"]` (array form)
/// is converted to `"prompt": "text"` before parsing. Non-array prompts pass through.
fn normalize_completions_body(body: &[u8]) -> Vec<u8> {
    let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(body) else { return body.to_vec(); };
    let Some(arr) = v["prompt"].as_array() else { return body.to_vec(); };
    let joined: String = arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n");
    v["prompt"] = serde_json::Value::String(joined);
    serde_json::to_vec(&v).unwrap_or_else(|_| body.to_vec())
}

/// GET /health or /v1/health — lightweight health check for load balancers.
fn handle_health() -> (u16, String) {
    let cache = SPARSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let model_loaded = cache.as_ref().and_then(|c| c.weights.as_ref()).is_some();
    let layers = cache.as_ref().and_then(|c| c.weights.as_ref()).map(|w| w.layers_raw.len()).unwrap_or(0);
    drop(cache);
    let resp = serde_json::json!({
        "status": "ok",
        "model_loaded": model_loaded,
        "layers_loaded": layers,
    });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"status":"error"}"#.into()),
    }
}

/// GET /v1/models/{id} — return a single model in OpenAI object format.
fn handle_oai_model_get(model_id: &str) -> (u16, String) {
    if !KNOWN_MODELS.contains(&model_id) {
        let err = serde_json::json!({
            "error": {
                "message": format!("The model '{}' does not exist", model_id),
                "type": "invalid_request_error",
                "code": "model_not_found",
            }
        });
        return (404, err.to_string());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let resp = serde_json::json!({
        "id": model_id,
        "object": "model",
        "created": now,
        "owned_by": "cognitum",
    });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error"}"#.into()),
    }
}

/// GET /v1/models — return available models in OpenAI list format.
fn handle_oai_models() -> (u16, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let data: Vec<serde_json::Value> = KNOWN_MODELS.iter().map(|&id| {
        serde_json::json!({
            "id": id,
            "object": "model",
            "created": now,
            "owned_by": "cognitum",
        })
    }).collect();
    let resp = serde_json::json!({ "object": "list", "data": data });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error"}"#.into()),
    }
}

/// POST /v1/completions — OpenAI text completion.
///
/// Accepts the standard OpenAI completions request format and returns an
/// OpenAI-shaped response wrapping the sparse-LLM output.
fn handle_oai_completions(body: &[u8], state: &crate::api::DeviceState) -> (u16, String) {
    let body = normalize_completions_body(body);
    let (status, gen_body) = handle_sparse_generate(&body, state);
    if status != 200 {
        return (status, to_oai_error(status, &gen_body));
    }
    let gen: serde_json::Value = match serde_json::from_str(&gen_body) {
        Ok(v) => v,
        Err(_) => return (500, r#"{"error":"internal"}"#.into()),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let finish = gen["finish_reason"].as_str().unwrap_or("stop");
    let finish_oai = match finish {
        "eos" | "stop_sequence" => "stop",
        "max_tokens"            => "length",
        other                   => other,
    };
    // Convert internal [{token, logprob, top_logprobs:[...]}] to OAI completions format.
    let logprobs_oai: serde_json::Value = match gen.get("logprobs") {
        Some(serde_json::Value::Array(arr)) if !arr.is_empty() => {
            let mut tokens: Vec<serde_json::Value> = Vec::new();
            let mut token_logprobs: Vec<serde_json::Value> = Vec::new();
            let mut top_logprobs: Vec<serde_json::Value> = Vec::new();
            let mut text_offset: Vec<serde_json::Value> = Vec::new();
            let mut offset: usize = 0;
            for e in arr {
                let tok = e["token"].as_str().unwrap_or("");
                tokens.push(serde_json::Value::String(tok.to_string()));
                token_logprobs.push(e["logprob"].clone());
                text_offset.push(serde_json::Value::Number(serde_json::Number::from(offset)));
                offset += tok.len();
                let top: serde_json::Map<String, serde_json::Value> = e["top_logprobs"]
                    .as_array()
                    .map(|tl| tl.iter().filter_map(|t| {
                        let k = t["token"].as_str()?.to_string();
                        Some((k, t["logprob"].clone()))
                    }).collect())
                    .unwrap_or_default();
                top_logprobs.push(serde_json::Value::Object(top));
            }
            serde_json::json!({
                "tokens": tokens,
                "token_logprobs": token_logprobs,
                "top_logprobs": top_logprobs,
                "text_offset": text_offset,
            })
        }
        _ => serde_json::Value::Null,
    };
    // When echo:true the output text must be prompt + completion.
    let text_out = if gen["prompt_text"].is_string() {
        let pfx = gen["prompt_text"].as_str().unwrap_or("");
        let cmp = gen["text"].as_str().unwrap_or("");
        serde_json::Value::String(format!("{}{}", pfx, cmp))
    } else {
        gen["text"].clone()
    };
    let resp = serde_json::json!({
        "id": gen["request_id"],
        "object": "text_completion",
        "created": now,
        "model": gen["model"],
        "system_fingerprint": serde_json::Value::Null,
        "choices": [{
            "text": text_out,
            "index": 0,
            "logprobs": logprobs_oai,
            "finish_reason": finish_oai,
        }],
        "usage": gen["usage"],
    });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error"}"#.into()),
    }
}

/// POST /v1/chat/completions — OpenAI chat completion.
///
/// Accepts OpenAI chat request format (messages array) and returns an
/// OpenAI-shaped chat.completion response.
fn handle_oai_chat_completions(body: &[u8], state: &crate::api::DeviceState) -> (u16, String) {
    let (status, gen_body) = handle_sparse_generate(body, state);
    if status != 200 {
        return (status, to_oai_error(status, &gen_body));
    }
    let gen: serde_json::Value = match serde_json::from_str(&gen_body) {
        Ok(v) => v,
        Err(_) => return (500, r#"{"error":"internal"}"#.into()),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let finish = gen["finish_reason"].as_str().unwrap_or("stop");
    let finish_oai = match finish {
        "eos" | "stop_sequence" => "stop",
        "max_tokens"            => "length",
        other                   => other,
    };
    // Build per-token logprobs in OpenAI chat format when requested.
    // Our internal format is [{token, token_id, logprob, top_logprobs:[...]}].
    // OpenAI chat format: {content: [{token, logprob, top_logprobs:[{token,logprob,bytes}]}]}.
    let choice_logprobs: serde_json::Value = match gen.get("logprobs") {
        Some(serde_json::Value::Array(arr)) => serde_json::json!({
            "content": arr.iter().map(|e| serde_json::json!({
                "token": e["token"],
                "logprob": e["logprob"],
                "bytes": serde_json::Value::Null,
                "top_logprobs": e["top_logprobs"].as_array().map(|tl| {
                    tl.iter().map(|t| serde_json::json!({
                        "token": t["token"],
                        "logprob": t["logprob"],
                        "bytes": serde_json::Value::Null,
                    })).collect::<Vec<_>>()
                }).unwrap_or_default(),
            })).collect::<Vec<_>>()
        }),
        _ => serde_json::Value::Null,
    };
    let resp = serde_json::json!({
        "id": gen["request_id"],
        "object": "chat.completion",
        "created": now,
        "model": gen["model"],
        "system_fingerprint": serde_json::Value::Null,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": gen["text"],
            },
            "logprobs": choice_logprobs,
            "finish_reason": finish_oai,
        }],
        "usage": gen["usage"],
    });
    match serde_json::to_string(&resp) {
        Ok(json) => (200, json),
        Err(_) => (500, r#"{"error":"serialization error"}"#.into()),
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_request_deserialization() {
        let json = br#"{"prompt": "hello"}"#;
        let req: SparseGenerateRequest =
            serde_json::from_slice(json).expect("should parse minimal JSON");
        assert_eq!(req.prompt, "hello");
        assert_eq!(req.model, "smollm2-135m");
        assert_eq!(req.max_tokens, 20);
        assert_eq!(req.max_seq, 256);
        assert!(req.seed.is_none(), "seed should default to None");
        assert!(req.stop.is_empty(), "stop should default to empty vec");
    }

    #[test]
    fn test_seed_field_deserializes() {
        let json = br#"{"prompt": "hi", "seed": 12345}"#;
        let req: SparseGenerateRequest = serde_json::from_slice(json).unwrap();
        assert_eq!(req.seed, Some(12345u64));
    }

    #[test]
    fn test_stop_sequences_clamped_and_filtered() {
        // max 4 entries kept; empty strings dropped; entries >32 chars dropped
        let json = br#"{"prompt": "hi", "stop": [".", "", "a", "b", "c", "this-string-is-definitely-more-than-32-characters-long"]}"#;
        let req = parse_generate_request(json).expect("should parse");
        // empty string removed, >32 chars removed, truncated to 4
        assert!(!req.stop.contains(&"".to_string()), "empty stop must be removed");
        assert!(req.stop.len() <= 4, "stop must be clamped to 4");
        for s in &req.stop {
            assert!(s.chars().count() <= 32, "stop entry too long: {}", s);
        }
    }

    #[test]
    fn test_messages_format_base_model() {
        let msgs = vec![
            ChatMessage { role: "system".into(), content: "You are helpful.".into() },
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ];
        let text = format_messages_as_prompt(&msgs, "smollm2-135m");
        assert!(text.contains("You are helpful."), "system content missing");
        assert!(text.contains("User: Hello"), "user content missing");
    }

    #[test]
    fn test_messages_format_instruct_model() {
        let msgs = vec![
            ChatMessage { role: "system".into(), content: "Be concise.".into() },
            ChatMessage { role: "user".into(), content: "Hi".into() },
        ];
        let text = format_messages_as_prompt(&msgs, "smollm2-135m-instruct");
        assert!(text.contains("<|im_start|>system"), "ChatML system token missing");
        assert!(text.contains("<|im_start|>user"), "ChatML user token missing");
        assert!(text.ends_with("<|im_start|>assistant\n"), "must end with assistant prompt");
    }

    #[test]
    fn test_messages_field_accepted_in_request() {
        let json = br#"{"messages": [{"role": "user", "content": "hello world"}]}"#;
        let req = parse_generate_request(json).expect("messages-only request should parse");
        assert!(!req.prompt.is_empty(), "formatted messages should be stored in prompt");
    }

    #[test]
    fn test_echo_field_defaults_false() {
        let json = br#"{"prompt": "hi"}"#;
        let req: SparseGenerateRequest = serde_json::from_slice(json).unwrap();
        assert!(!req.echo, "echo should default to false");
    }

    #[test]
    fn test_max_new_tokens_overrides_max_tokens() {
        let json = br#"{"prompt": "hi", "max_new_tokens": 50}"#;
        let req = parse_generate_request(json).expect("should parse");
        assert_eq!(req.max_tokens, 50, "max_new_tokens should override max_tokens default");
    }

    #[test]
    fn test_max_new_tokens_clamped() {
        let json = br#"{"prompt": "hi", "max_new_tokens": 9999}"#;
        let req = parse_generate_request(json).expect("should parse");
        assert_eq!(req.max_tokens, MAX_TOKENS_LIMIT, "max_new_tokens must be clamped");
    }

    #[test]
    fn test_handle_sparse_info_has_seed_and_stop() {
        let (status, body) = handle_sparse_info();
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v["sampling"]["seed"].is_object(), "sampling.seed must be documented");
        assert!(v["sampling"]["stop"].is_object(), "sampling.stop must be documented");
    }

    #[test]
    fn test_max_tokens_clamped() {
        let json = br#"{"prompt": "test", "max_tokens": 999}"#;
        let mut req: SparseGenerateRequest =
            serde_json::from_slice(json).expect("should parse");
        req.max_tokens = req.max_tokens.clamp(1, MAX_TOKENS_LIMIT);
        assert_eq!(req.max_tokens, 200, "max_tokens 999 must be clamped to 200");
    }

    #[test]
    fn test_handle_sparse_info_returns_200() {
        let (status, body) = handle_sparse_info();
        assert_eq!(status, 200, "handle_sparse_info must return 200, body: {}", body);
        let v: serde_json::Value =
            serde_json::from_str(&body).expect("body must be valid JSON");
        assert!(v["models"].is_array(), "response must have models array");
        assert_eq!(v["max_tokens"], 200);
        assert_eq!(v["max_seq"], 512);
    }

    // ── parse_generate_request validation ─────────────────────────────────────

    #[test]
    fn test_invalid_json_returns_400() {
        let result = parse_generate_request(b"not json at all");
        let (status, body) = result.expect_err("should fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "INVALID_REQUEST");
    }

    #[test]
    fn test_empty_prompt_returns_400() {
        let result = parse_generate_request(br#"{"prompt": ""}"#);
        let (status, body) = result.expect_err("empty prompt should fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "PROMPT_EMPTY");
    }

    #[test]
    fn test_prompt_too_long_returns_400() {
        let long_prompt = "x".repeat(MAX_PROMPT_CHARS + 1);
        let json = format!(r#"{{"prompt": "{}"}}"#, long_prompt);
        let result = parse_generate_request(json.as_bytes());
        let (status, body) = result.expect_err("oversized prompt should fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "PROMPT_TOO_LONG");
    }

    #[test]
    fn test_valid_request_parses_and_clamps() {
        let json = br#"{"prompt": "hello world", "max_tokens": 500, "max_seq": 1024}"#;
        let req = parse_generate_request(json).expect("should parse successfully");
        assert_eq!(req.prompt, "hello world");
        assert_eq!(req.max_tokens, MAX_TOKENS_LIMIT, "max_tokens clamped to 200");
        assert_eq!(req.max_seq, MAX_SEQ_LIMIT, "max_seq clamped to 512");
    }

    #[test]
    fn test_max_seq_clamped_from_below() {
        let json = br#"{"prompt": "hi", "max_seq": 10}"#;
        let req = parse_generate_request(json).expect("should parse");
        assert_eq!(req.max_seq, MIN_SEQ_LIMIT, "max_seq clamped up to 64");
    }

    // ── handle_sparse_models ──────────────────────────────────────────────

    #[test]
    fn test_handle_sparse_models_returns_200_with_known_models() {
        let (status, body) = handle_sparse_models();
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let models = v["models"].as_array().unwrap();
        assert!(!models.is_empty(), "models array must not be empty");
        // Verify expected keys in first entry.
        let m = &models[0];
        assert!(m["id"].is_string());
        assert!(m["model_gguf"].is_boolean());
        assert!(m["tokenizer_json"].is_boolean());
        assert!(m["ready"].is_boolean());
    }

    // ── handle_model_upload ───────────────────────────────────────────────

    #[test]
    fn test_upload_unknown_model_returns_400() {
        let (status, body) = handle_model_upload(
            "/api/v1/llm/sparse/model/unknown-model/model.gguf", b"data",
        );
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "UNKNOWN_MODEL");
    }

    #[test]
    fn test_upload_invalid_filename_returns_400() {
        let (status, body) = handle_model_upload(
            "/api/v1/llm/sparse/model/smollm2-135m/evil.sh", b"data",
        );
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "INVALID_FILENAME");
    }

    #[test]
    fn test_upload_missing_filename_segment_returns_400() {
        let (status, body) = handle_model_upload(
            "/api/v1/llm/sparse/model/smollm2-135m", b"data",
        );
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "INVALID_PATH");
    }

    #[test]
    fn test_upload_empty_body_returns_400() {
        let (status, body) = handle_model_upload(
            "/api/v1/llm/sparse/model/smollm2-135m/model.gguf", b"",
        );
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "EMPTY_BODY");
    }

    #[test]
    fn test_upload_oversized_file_returns_413() {
        let big = vec![0u8; MAX_UPLOAD_BYTES + 1];
        let (status, body) = handle_model_upload(
            "/api/v1/llm/sparse/model/smollm2-135m/tokenizer.json", &big,
        );
        assert_eq!(status, 413);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "FILE_TOO_LARGE");
    }

    // ── model validation ──────────────────────────────────────────────────

    #[test]
    fn test_unknown_model_returns_404() {
        let json = br#"{"prompt": "hello", "model": "gpt-99"}"#;
        let (status, body) = parse_generate_request(json).expect_err("unknown model must fail");
        assert_eq!(status, 404);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "MODEL_NOT_FOUND");
    }

    #[test]
    fn test_path_traversal_attempt_returns_404() {
        let json = br#"{"prompt": "hello", "model": "../../etc/passwd"}"#;
        let (status, _body) = parse_generate_request(json).expect_err("path traversal must fail");
        assert_eq!(status, 404);
    }

    #[test]
    fn test_known_model_passes_validation() {
        let json = br#"{"prompt": "hello", "model": "smollm2-135m"}"#;
        let req = parse_generate_request(json).expect("known model should pass");
        assert_eq!(req.model, "smollm2-135m");
    }

    #[test]
    fn test_second_known_model_passes_validation() {
        let json = br#"{"prompt": "hello", "model": "qwen2.5-0.5b-q4"}"#;
        let req = parse_generate_request(json).expect("qwen model should pass");
        assert_eq!(req.model, "qwen2.5-0.5b-q4");
    }

    // ── deser_stop custom deserializer ───────────────────────────────────

    #[test]
    fn test_stop_as_string_deserializes() {
        let json = br#"{"prompt": "hello", "stop": "\n"}"#;
        let req = parse_generate_request(json).expect("stop as string must parse");
        assert_eq!(req.stop, vec!["\n"]);
    }

    #[test]
    fn test_stop_as_array_deserializes() {
        let json = br#"{"prompt": "hello", "stop": [".", "!"]}"#;
        let req = parse_generate_request(json).expect("stop as array must parse");
        assert!(req.stop.contains(&".".to_string()));
        assert!(req.stop.contains(&"!".to_string()));
    }

    #[test]
    fn test_stop_null_treated_as_empty() {
        let json = br#"{"prompt": "hello", "stop": null}"#;
        let req = parse_generate_request(json).expect("stop null must parse");
        assert!(req.stop.is_empty(), "null stop must yield empty vec");
    }

    // ── feature rejections ───────────────────────────────────────────────

    #[test]
    fn test_n_greater_than_1_returns_400() {
        let json = br#"{"prompt": "hello", "n": 3}"#;
        let (status, body) = parse_generate_request(json).expect_err("n>1 must fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "FEATURE_NOT_SUPPORTED");
    }

    #[test]
    fn test_n_equal_1_passes() {
        let json = br#"{"prompt": "hello", "n": 1}"#;
        parse_generate_request(json).expect("n=1 must pass");
    }

    #[test]
    fn test_best_of_greater_than_1_returns_400() {
        let json = br#"{"prompt": "hello", "best_of": 5}"#;
        let (status, body) = parse_generate_request(json).expect_err("best_of>1 must fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "FEATURE_NOT_SUPPORTED");
    }

    #[test]
    fn test_tools_nonempty_returns_400() {
        let json = br#"{"prompt": "hello", "tools": [{"type": "function", "function": {"name": "get_weather"}}]}"#;
        let (status, body) = parse_generate_request(json).expect_err("tools must fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "FEATURE_NOT_SUPPORTED");
    }

    #[test]
    fn test_tools_empty_array_passes() {
        let json = br#"{"prompt": "hello", "tools": []}"#;
        parse_generate_request(json).expect("empty tools array must pass");
    }

    #[test]
    fn test_response_format_json_schema_returns_400() {
        let json = br#"{"prompt": "hello", "response_format": {"type": "json_schema", "json_schema": {}}}"#;
        let (status, body) = parse_generate_request(json).expect_err("json_schema must fail");
        assert_eq!(status, 400);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "FEATURE_NOT_SUPPORTED");
    }

    #[test]
    fn test_response_format_text_passes() {
        let json = br#"{"prompt": "hello", "response_format": {"type": "text"}}"#;
        parse_generate_request(json).expect("response_format text must pass");
    }

    #[test]
    fn test_response_format_json_object_injects_system_hint() {
        let json = br#"{"prompt": "list users", "response_format": {"type": "json_object"}}"#;
        let req = parse_generate_request(json).expect("json_object must pass");
        assert!(
            req.prompt.contains("Respond only with valid JSON"),
            "json_object mode must inject JSON hint into prompt"
        );
        assert!(req.prompt.contains("list users"), "original prompt must be preserved");
    }
}

// ── Pipeline summary helper (used by sparse_pipeline.rs) ───────────────────

/// Run a quick non-streaming generation against the cached model weights.
/// Returns None if no model is loaded or tokenisation yields no tokens.
/// Called by the cognitive pipeline when an anomaly event fires.
/// Stream SmolLM2 summary token-by-token via a callback.
///
/// `on_text(text_piece)` is called for each decoded token; return `false` to stop
/// Auto-load the default model into SPARSE_CACHE if it is not yet warm.
///
/// Called at the start of both summary functions so the pipeline works without
/// a preceding `/api/v1/llm/sparse/generate` warm-up request.  The default
/// model is `smollm2-135m`; if no model files are found the cache stays None.
fn ensure_cache_warm(guard: &mut Option<SparseCache>) {
    if guard.is_some() { return; }
    let model_id   = "smollm2-135m";
    let model_path = PathBuf::from(format!("{}/{}/model.gguf",     model_base_dir(), model_id));
    let tok_path   = PathBuf::from(format!("{}/{}/tokenizer.json", model_base_dir(), model_id));
    if !model_path.exists() { return; }

    let tokenizer = if tok_path.exists() {
        BpeTokenizer::from_file(&tok_path).unwrap_or_else(|_| BpeTokenizer::byte_fallback_stub())
    } else {
        BpeTokenizer::byte_fallback_stub()
    };

    let (weights, config) = {
        let mut tmp = match SparseLlmRunner::new(model_path.clone()) {
            Ok(r) => r,
            Err(_) => return,
        };
        let w = if tmp.load_header().is_ok() {
            tmp.gguf_header.as_ref().and_then(|hdr| {
                crate::sparse_llm_loader::load_weights(
                    &model_path, hdr.tensor_count, hdr.post_kv_file_offset,
                ).ok()
            })
        } else { None };
        let cfg = tmp.config.clone();
        (w, cfg)
    };

    *guard = Some(SparseCache { model_path, weights, tokenizer, config });
}

/// early (e.g. after punctuation, timeout, or desired length reached).
///
/// Returns `(tokens_generated, elapsed_ms)`. Holds SPARSE_CACHE lock for the
/// full duration — acceptable on single-core Pi Zero 2W (no parallelism).
pub fn generate_summary_streaming(
    prompt: &str,
    max_tokens: usize,
    on_text: &mut dyn FnMut(&str) -> bool,
) -> (usize, f64) {
    let mut guard = SPARSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    ensure_cache_warm(&mut guard);
    let cache = match guard.as_mut() { Some(c) => c, None => return (0, 0.0) };
    let weights = cache.weights.as_ref();
    let mut token_ids = cache.tokenizer.encode(prompt);
    // Strip trailing EOS — its embedding drives all logits to ~0, causing immediate EOS generation.
    let eos_id  = cache.tokenizer.eos_id;
    if token_ids.last() == Some(&eos_id) { token_ids.pop(); }
    if token_ids.is_empty() { return (0, 0.0); }
    let model_path = cache.model_path.clone();
    let config     = cache.config.clone();
    // Borrow the tokenizer immutably (different field from weights).
    let tok = &cache.tokenizer;
    let mut runner = match crate::sparse_llm_runner::SparseLlmRunner::new(model_path) {
        Ok(mut r) => { r.config = config; r }
        Err(_) => return (0, 0.0),
    };
    let t0   = std::time::Instant::now();
    let mut n = 0usize;
    let mut stop = false;
    let _ = crate::sparse_llm_loader::generate_with_fallback(
        &mut runner,
        weights,
        &token_ids,
        max_tokens,
        eos_id,
        0.3, 0, 0.9, 0.0, 1.1, 0.0, 0.0,  // temp=0.3, top_p=0.9 for focused summaries
        &std::collections::HashMap::new(),
        None, 0,
        &mut |id, _| {
            if stop { return; }
            let text = tok.decode(&[id]);
            // <|im_end|> marks end-of-turn in ChatML; stop without emitting it.
            if text.contains("<|im_end|>") { stop = true; return; }
            if !text.is_empty() {
                n += 1;
                if !on_text(&text) { stop = true; }
            }
        },
    );
    (n, t0.elapsed().as_secs_f64() * 1000.0)
}

pub fn generate_summary_from_cache(prompt: &str, max_tokens: usize) -> Option<String> {
    let mut guard = SPARSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    ensure_cache_warm(&mut guard);
    let cache = guard.as_mut()?;
    let weights = cache.weights.as_ref();
    let mut token_ids = cache.tokenizer.encode(prompt);
    let eos_id = cache.tokenizer.eos_id;
    // Strip trailing EOS — its embedding drives all logits to ~0.
    if token_ids.last() == Some(&eos_id) { token_ids.pop(); }
    if token_ids.is_empty() {
        return None;
    }
    let model_path = cache.model_path.clone();
    let config = cache.config.clone();
    // SparseLlmRunner::new takes the model path; override config with cached values.
    let mut runner = crate::sparse_llm_runner::SparseLlmRunner::new(model_path)
        .map(|mut r| { r.config = config; r })
        .ok()?;
    let (ids, _) = crate::sparse_llm_loader::generate_with_fallback(
        &mut runner,
        weights,
        &token_ids,
        max_tokens,
        eos_id,
        0.1,  // temperature — low = factual/deterministic summaries
        0,    // top_k (disabled)
        0.95, // top_p
        0.0,  // min_p
        1.3,  // repetition_penalty — discourage repetitive loops
        0.0,  // presence_penalty
        0.0,  // frequency_penalty
        &std::collections::HashMap::new(),
        None, // seed
        0,    // logprobs_k
        &mut |_, _| {},
    ).ok()?;
    let mut out = cache.tokenizer.decode(&ids);
    // Strip ChatML turn-end marker.
    if let Some(pos) = out.find("<|im_end|>") { out.truncate(pos); }
    // Cap at 200 chars then extend to nearest word boundary.
    if out.len() > 200 {
        out.truncate(200);
        if let Some(pos) = out.rfind(' ') { out.truncate(pos); }
    }
    // Truncate after the first sentence-ending punctuation.
    if let Some(pos) = out.find(|c| c == '.' || c == '!' || c == '?') {
        out.truncate(pos + 1);
    } else if !out.is_empty() {
        out.push('.');
    }
    let out = out.trim().to_string();
    if out.is_empty() { None } else { Some(out) }
}
