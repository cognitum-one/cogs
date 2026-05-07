//! Minimal Request/Response/DeviceState compat layer.
//!
//! The sparse-LLM and sparse-pipeline modules were lifted from the cognitum-agent
//! (cognitum-one/seed#133), where they consumed a hand-rolled `crate::http`
//! Request/Response and a `crate::api::DeviceState` parameter. Inside this cog,
//! axum handlers in `main.rs` translate axum requests into these compat types
//! and call the moved code unchanged.

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub peer_addr: Option<String>,
    pub client_cn: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Response {
    pub status: u16,
    pub status_text: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub extra_headers: Vec<(String, String)>,
}

impl Response {
    pub fn json(status: u16, body: &impl serde::Serialize) -> Self {
        let body = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
        Self {
            status,
            status_text: status_text(status),
            content_type: "application/json",
            body,
            extra_headers: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn json_ok(body: &impl serde::Serialize) -> Self {
        Self::json(200, body)
    }

    #[allow(dead_code)]
    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, &serde_json::json!({ "error": message }))
    }

    #[allow(dead_code)]
    pub fn not_found() -> Self {
        Self::error(404, "not found")
    }
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

/// Empty stand-in for the agent's `DeviceState`.
///
/// All `state` parameters in the moved code are `_state` (unused) — they were
/// kept on signatures for future use. The cog has no equivalent (the agent
/// owns paired/mesh/store state and gates the proxy at the agent boundary).
#[derive(Debug, Default, Clone)]
pub struct DeviceState;
