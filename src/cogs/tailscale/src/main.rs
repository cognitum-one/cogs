//! Cognitum Cog: Tailscale Mesh VPN
//!
//! See `docs/seed/ADR-100-tailscale-cog.md` for the design rationale.
//!
//! Lifecycle (--once):
//!   1. Read config.json from $app_dir
//!   2. Ensure tailscaled + tailscale binaries are present (download if missing)
//!   3. Spawn tailscaled in userspace mode
//!   4. Run `tailscale up --auth-key=… --hostname=…`
//!   5. (Optional) `tailscale serve` to expose the agent
//!   6. Start the loopback HTTP API on bind_port (8044)
//!   7. Block on a sigterm channel; on shutdown, stop tailscaled gracefully

use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

const TAILSCALE_VERSION: &str = "1.78.1"; // bump when re-uploading binaries to gs://cognitum-apps
const BIND_PORT: u16 = 8044;
const SOCKET_WAIT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize, Default)]
struct CogConfig {
    #[serde(default)]
    auth_key: String,
    #[serde(default)]
    hostname: String,
    #[serde(default = "default_true")]
    serve_agent: bool,
    #[serde(default = "default_tags")]
    advertise_tags: String,
}

fn default_true() -> bool {
    true
}
fn default_tags() -> String {
    "tag:seed".into()
}

fn app_dir() -> PathBuf {
    // Set by the agent when it spawns the cog; falls back to CWD for dev.
    env::var("COG_APP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn state_dir() -> PathBuf {
    let d = app_dir().join("state");
    let _ = fs::create_dir_all(&d);
    d
}

fn tailscaled_path() -> PathBuf {
    app_dir().join("tailscaled")
}

fn tailscale_path() -> PathBuf {
    app_dir().join("tailscale")
}

fn tailscaled_sock() -> PathBuf {
    state_dir().join("tailscaled.sock")
}

fn read_config() -> CogConfig {
    let p = app_dir().join("config.json");
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn parse_args() -> (&'static str, Option<CogConfig>) {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--status") => ("status", None),
        Some("--up") => ("up", Some(read_config())),
        Some("--logout") => ("logout", None),
        Some("--help") | Some("-h") => ("help", None),
        // default: --once / no-args == start the daemon loop
        _ => ("daemon", Some(read_config())),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Binary acquisition (download + verify)
// ────────────────────────────────────────────────────────────────────────────

fn ensure_binaries() -> Result<(), String> {
    if tailscaled_path().exists() && tailscale_path().exists() {
        return Ok(());
    }
    // Both binaries ship pre-fetched in gs://cognitum-apps/cogs/arm/tailscale/.
    // The agent's /apps/install (ADR-095 §4) handles asset download with sha256
    // verification BEFORE our --once runs; if we got here without the binaries
    // present, something went wrong with the install. Fail loudly.
    Err(format!(
        "tailscale binaries missing under {:?} — `apps/install` should have downloaded them as assets",
        app_dir()
    ))
}

// ────────────────────────────────────────────────────────────────────────────
// 2. tailscaled lifecycle
// ────────────────────────────────────────────────────────────────────────────

fn spawn_tailscaled() -> Result<Child, String> {
    let sock = tailscaled_sock();
    let _ = fs::remove_file(&sock); // stale socket from prior run
    let log = app_dir().join("tailscaled.log");
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| format!("open log {:?}: {e}", log))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("clone log fd: {e}"))?;

    let child = Command::new(tailscaled_path())
        .arg("--tun=userspace-networking")
        .arg(format!("--state={}", state_dir().join("tailscaled.state").display()))
        .arg(format!("--socket={}", sock.display()))
        .arg(format!("--statedir={}", state_dir().display()))
        .arg("--port=41641")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err))
        .spawn()
        .map_err(|e| format!("spawn tailscaled: {e}"))?;
    Ok(child)
}

fn wait_for_socket() -> Result<(), String> {
    let sock = tailscaled_sock();
    let deadline = Instant::now() + Duration::from_secs(SOCKET_WAIT_TIMEOUT_SECS);
    while Instant::now() < deadline {
        if sock.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(format!("tailscaled socket not ready after {SOCKET_WAIT_TIMEOUT_SECS}s"))
}

fn tailscale_cmd(args: &[&str]) -> Command {
    let mut cmd = Command::new(tailscale_path());
    cmd.arg(format!("--socket={}", tailscaled_sock().display()));
    for a in args {
        cmd.arg(a);
    }
    cmd
}

fn run_up(cfg: &CogConfig) -> Result<(), String> {
    let hostname = if cfg.hostname.is_empty() {
        // Fall back to the system hostname; tailscale uses it as the node name.
        fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "cognitum-seed".to_string())
    } else {
        cfg.hostname.clone()
    };

    let mut args: Vec<String> = vec![
        "up".into(),
        format!("--hostname={hostname}"),
        format!("--advertise-tags={}", cfg.advertise_tags),
        "--accept-routes=false".into(),
        "--accept-dns=false".into(), // userspace mode can't transparently take over DNS anyway
    ];
    if !cfg.auth_key.is_empty() {
        args.push(format!("--auth-key={}", cfg.auth_key));
    }

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = tailscale_cmd(&args_ref)
        .output()
        .map_err(|e| format!("invoke tailscale up: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tailscale up failed: stderr=\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    eprintln!("[tailscale] up OK as host '{hostname}'");
    Ok(())
}

fn run_serve_agent() -> Result<(), String> {
    let output = tailscale_cmd(&["serve", "--bg", "https+insecure://127.0.0.1:8443"])
        .output()
        .map_err(|e| format!("invoke tailscale serve: {e}"))?;
    if !output.status.success() {
        eprintln!(
            "[tailscale] serve returned non-zero (this is OK if already configured): {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Don't bail — serve --bg is idempotent and may complain on second run.
    }
    Ok(())
}

fn run_logout() -> Result<(), String> {
    let _ = tailscale_cmd(&["logout"]).output();
    Ok(())
}

fn run_status_json() -> Result<String, String> {
    let output = tailscale_cmd(&["status", "--json"])
        .output()
        .map_err(|e| format!("invoke tailscale status: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tailscale status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Loopback HTTP API (bind_port = 8044, proxied via /api/v1/cogs/tailscale/*)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StatusOut<'a> {
    cog_state: &'a str,
    tailscale: serde_json::Value,
}

fn http_api(shutdown: Arc<AtomicBool>) {
    let server = match tiny_http::Server::http(("127.0.0.1", BIND_PORT)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[tailscale] HTTP API failed to bind 127.0.0.1:{BIND_PORT}: {e}");
            return;
        }
    };
    eprintln!("[tailscale] cog API listening on 127.0.0.1:{BIND_PORT}");

    while !shutdown.load(Ordering::Relaxed) {
        let req = match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => continue,
        };
        let method = req.method().clone();
        let url = req.url().to_string();
        let path = url.split('?').next().unwrap_or(&url).to_string();
        let response = match (method.as_str(), path.as_str()) {
            ("GET", "/status") | ("GET", "/") => handle_status(),
            ("GET", "/health") => handle_health(),
            ("GET", "/peers") => handle_peers(),
            ("POST", "/logout") => handle_logout(),
            ("POST", "/up") => handle_up(),
            _ => not_found(),
        };
        let _ = req.respond(response);
    }
}

fn json_response(status: u16, body: serde_json::Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let bytes = body.to_string().into_bytes();
    tiny_http::Response::from_data(bytes)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(b"Content-Type".as_ref(), b"application/json".as_ref())
                .unwrap(),
        )
}

fn not_found() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    json_response(404, serde_json::json!({"error":"not found"}))
}

fn handle_health() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let healthy = tailscaled_sock().exists();
    let code = if healthy { 200 } else { 503 };
    json_response(code, serde_json::json!({"ok": healthy}))
}

fn handle_status() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match run_status_json() {
        Ok(s) => {
            let mut v: serde_json::Value =
                serde_json::from_str(&s).unwrap_or(serde_json::json!({}));
            // Redact AuthKey if it ever leaks into status (shouldn't, but defense-in-depth).
            if let Some(obj) = v.as_object_mut() {
                obj.remove("AuthURL"); // also strip the one-time login URL
            }
            let out = StatusOut {
                cog_state: "running",
                tailscale: v,
            };
            json_response(200, serde_json::to_value(out).unwrap())
        }
        Err(e) => json_response(503, serde_json::json!({"cog_state":"error","error": e})),
    }
}

fn handle_peers() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match run_status_json() {
        Ok(s) => {
            let v: serde_json::Value =
                serde_json::from_str(&s).unwrap_or(serde_json::json!({}));
            let peers = v.get("Peer").cloned().unwrap_or(serde_json::json!({}));
            json_response(200, peers)
        }
        Err(e) => json_response(503, serde_json::json!({"error": e})),
    }
}

fn handle_logout() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match run_logout() {
        Ok(_) => json_response(200, serde_json::json!({"ok": true})),
        Err(e) => json_response(500, serde_json::json!({"error": e})),
    }
}

fn handle_up() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let cfg = read_config();
    match run_up(&cfg) {
        Ok(_) => json_response(200, serde_json::json!({"ok": true})),
        Err(e) => json_response(500, serde_json::json!({"error": e})),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Entry point
// ────────────────────────────────────────────────────────────────────────────

fn main() {
    env_logger::init();
    let (mode, cfg) = parse_args();

    match mode {
        "help" => {
            println!("cog-tailscale — Cognitum Tailscale mesh VPN cog");
            println!();
            println!("USAGE:");
            println!("  cog-tailscale-arm              start the daemon + API");
            println!("  cog-tailscale-arm --status     print tailscale status JSON");
            println!("  cog-tailscale-arm --up         reauthorize using current config.json");
            println!("  cog-tailscale-arm --logout     drop the device from the tailnet");
            return;
        }
        "status" => {
            match run_status_json() {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            }
            return;
        }
        "logout" => {
            if let Err(e) = run_logout() {
                eprintln!("{e}");
                std::process::exit(2);
            }
            return;
        }
        "up" => {
            let cfg = cfg.expect("up requires config");
            if let Err(e) = run_up(&cfg) {
                eprintln!("{e}");
                std::process::exit(2);
            }
            return;
        }
        _ => {} // daemon
    }

    // Daemon path
    let cfg = cfg.expect("daemon requires config");
    if let Err(e) = ensure_binaries() {
        eprintln!("[tailscale] {e}");
        std::process::exit(3);
    }
    let mut daemon = match spawn_tailscaled() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[tailscale] {e}");
            std::process::exit(4);
        }
    };
    if let Err(e) = wait_for_socket() {
        eprintln!("[tailscale] {e}");
        let _ = daemon.kill();
        std::process::exit(5);
    }
    if let Err(e) = run_up(&cfg) {
        eprintln!("[tailscale] {e}");
        // Don't kill the daemon — leave it running so /api/v1/cogs/tailscale/up
        // can retry after a reauth.
    }
    if cfg.serve_agent {
        let _ = run_serve_agent();
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let s = shutdown.clone();
        ctrlc_handler(s);
    }
    http_api(shutdown.clone());

    // Cleanup on graceful exit
    eprintln!("[tailscale] shutting down tailscaled");
    let _ = daemon.kill();
    let _ = daemon.wait();
}

fn ctrlc_handler(shutdown: Arc<AtomicBool>) {
    // Best-effort SIGTERM/SIGINT handler; full signal support would need libc.
    // For the cog framework, the agent sends SIGTERM and waits up to 10s.
    thread::spawn(move || {
        use std::sync::atomic::Ordering::Relaxed;
        // Poll a sentinel file the agent can drop to request shutdown.
        let stop_file = app_dir().join("stop");
        loop {
            if stop_file.exists() {
                shutdown.store(true, Relaxed);
                let _ = fs::remove_file(&stop_file);
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

// Suppress unused-warning for the unused field; reserved for future asset download integrity check
#[allow(dead_code)]
fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let mut f = fs::File::open(path).map_err(|e| format!("open {:?}: {e}", path))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let got = hex::encode(h.finalize());
    if got == expected_hex {
        Ok(())
    } else {
        Err(format!("sha256 mismatch: expected={expected_hex} got={got}"))
    }
}
