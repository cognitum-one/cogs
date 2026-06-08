//! Cognitum Cog: DOOM
//!
//! Runs the portable `doomgeneric` engine on-device (software rendered, no GPU /
//! no X11 / no SDL) and serves a touch-friendly browser game client over a
//! loopback HTTP API. The Cognitum seed agent reverse-proxies
//! `https://seed:8443/api/v1/cogs/doom/<path>` -> `http://127.0.0.1:8066/<path>`,
//! injecting a per-cog bearer token via env `COGNITUM_COG_TOKEN`.
//!
//! Security model (ADR-095 / ADR-019):
//!   * Default = loopback bind (127.0.0.1) + bearer-token auth on the game
//!     endpoints (/frame, /input, /stream). Only `/` and `/health` are open.
//!   * `DOOM_OPEN=1` is an OPT-IN escape hatch that disables the token check so
//!     the game can be played directly over the LAN (no agent proxy). Combined
//!     with `DOOM_BIND=0.0.0.0` this exposes the game endpoints to anyone on the
//!     local network — a deliberate convenience/security tradeoff, OFF by default.
//!
//! Threading model:
//!   * ONE dedicated "engine" thread owns all of doomgeneric (the engine is not
//!     thread-safe). It calls doomgeneric_Create() once then loops _Tick().
//!   * The DG_* callbacks below run ON that engine thread. They publish frames
//!     into a shared Mutex<Frame> + AtomicU64 counter, and pop input from a
//!     shared Mutex<VecDeque>.
//!   * N HTTP worker threads share an Arc<Server> and only ever read the shared
//!     frame / push input — they never touch doomgeneric directly.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_uchar, CString};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tiny_http::{Header, Method, Request, Response, Server};

// Render at DOOM's native 320x200: ~1/4 the pixels of 640x400, so JPEG encoding
// (the framerate bottleneck on the Pi Zero 2W) is ~3-4x cheaper and frames are
// far smaller. The phone upscales it (classic chunky DOOM look). Must match the
// DOOMGENERIC_RESX/RESY defines in build.rs.
const RESX: usize = 320;
const RESY: usize = 200;
const DEFAULT_JPEG_QUALITY: u8 = 55;
const DEFAULT_PORT: u16 = 8066;

// JPEG quality is resolved once at startup (env DOOM_JPEG_QUALITY / config), then
// read here by the encoder. Stored as a static so the engine + HTTP threads agree.
static JPEG_QUALITY: OnceLock<u8> = OnceLock::new();
fn jpeg_quality() -> u8 {
    *JPEG_QUALITY.get().unwrap_or(&DEFAULT_JPEG_QUALITY)
}

// ──────────────────────────────────────────────────────────────────────────
// doomgeneric C FFI
// ──────────────────────────────────────────────────────────────────────────

extern "C" {
    fn doomgeneric_Create(argc: c_int, argv: *mut *mut c_char);
    fn doomgeneric_Tick();
    // XRGB8888 framebuffer, RESX*RESY u32s. Allocated by doomgeneric_Create.
    static mut DG_ScreenBuffer: *mut u32;
}

// ──────────────────────────────────────────────────────────────────────────
// Shared engine <-> HTTP state
// ──────────────────────────────────────────────────────────────────────────

struct Shared {
    // Latest decoded frame as packed RGB (RESX*RESY*3). Guarded by mutex.
    frame: Mutex<Vec<u8>>,
    // Monotonic frame id, bumped on every DG_DrawFrame.
    counter: AtomicU64,
    // Notifies /frame?since waiters and /stream of a new frame.
    cv: Condvar,
    // Dummy mutex paired with cv (we only care about notify, counter is atomic).
    cv_lock: Mutex<()>,
    // Pending input events: (pressed, doom_key).
    input: Mutex<VecDeque<(c_int, c_uchar)>>,
    // Engine start time, for DG_GetTicksMs.
    start: Instant,
    // Cache of the last JPEG-encoded frame keyed by counter, so concurrent
    // /frame requests reuse one encode.
    jpeg_cache: Mutex<Option<(u64, Arc<Vec<u8>>)>>,
}

fn shared() -> &'static Shared {
    static S: OnceLock<Shared> = OnceLock::new();
    S.get_or_init(|| Shared {
        frame: Mutex::new(vec![0u8; RESX * RESY * 3]),
        counter: AtomicU64::new(0),
        cv: Condvar::new(),
        cv_lock: Mutex::new(()),
        input: Mutex::new(VecDeque::new()),
        start: Instant::now(),
        jpeg_cache: Mutex::new(None),
    })
}

// ──────────────────────────────────────────────────────────────────────────
// DG_* platform backend (called on the engine thread)
// ──────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn DG_Init() {
    // Nothing to set up — our "display" is the shared RGB buffer.
}

#[no_mangle]
pub extern "C" fn DG_DrawFrame() {
    let s = shared();
    // Copy DG_ScreenBuffer (XRGB8888) -> shared packed RGB.
    // Safe: this runs on the single engine thread; DG_ScreenBuffer is valid
    // after doomgeneric_Create.
    let src = unsafe { DG_ScreenBuffer };
    if src.is_null() {
        return;
    }
    {
        let mut dst = s.frame.lock().unwrap();
        let pixels = unsafe { std::slice::from_raw_parts(src, RESX * RESY) };
        for (i, &px) in pixels.iter().enumerate() {
            let r = ((px >> 16) & 0xff) as u8;
            let g = ((px >> 8) & 0xff) as u8;
            let b = (px & 0xff) as u8;
            let o = i * 3;
            dst[o] = r;
            dst[o + 1] = g;
            dst[o + 2] = b;
        }
    }
    s.counter.fetch_add(1, Ordering::SeqCst);
    // Wake any /frame?since= or /stream waiters.
    let _g = s.cv_lock.lock().unwrap();
    s.cv.notify_all();
}

#[no_mangle]
pub extern "C" fn DG_SleepMs(ms: u32) {
    std::thread::sleep(Duration::from_millis(ms as u64));
}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    shared().start.elapsed().as_millis() as u32
}

#[no_mangle]
pub extern "C" fn DG_GetKey(pressed: *mut c_int, key: *mut c_uchar) -> c_int {
    let s = shared();
    let mut q = s.input.lock().unwrap();
    match q.pop_front() {
        Some((p, k)) => {
            unsafe {
                *pressed = p;
                *key = k;
            }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {
    // Headless — ignore.
}

// ──────────────────────────────────────────────────────────────────────────
// Key mapping (values from vendor/doomgeneric/doomgeneric/doomkeys.h)
// ──────────────────────────────────────────────────────────────────────────

const KEY_RIGHTARROW: u8 = 0xae;
const KEY_LEFTARROW: u8 = 0xac;
const KEY_UPARROW: u8 = 0xad;
const KEY_DOWNARROW: u8 = 0xaf;
const KEY_STRAFE_L: u8 = 0xa0;
const KEY_STRAFE_R: u8 = 0xa1;
const KEY_USE: u8 = 0xa2;
const KEY_FIRE: u8 = 0xa3;
const KEY_ESCAPE: u8 = 27;
const KEY_ENTER: u8 = 13;
const KEY_TAB: u8 = 9;
const KEY_RSHIFT: u8 = 0x80 + 0x36;

fn map_key(name: &str) -> Option<u8> {
    Some(match name.to_ascii_lowercase().as_str() {
        "left" => KEY_LEFTARROW,
        "right" => KEY_RIGHTARROW,
        "up" => KEY_UPARROW,
        "down" => KEY_DOWNARROW,
        "fire" | "ctrl" => KEY_FIRE,
        "use" => KEY_USE,
        "enter" | "return" => KEY_ENTER,
        "escape" | "esc" => KEY_ESCAPE,
        "tab" | "map" => KEY_TAB,
        "strafeleft" => KEY_STRAFE_L,
        "straferight" => KEY_STRAFE_R,
        "run" | "shift" => KEY_RSHIFT,
        "space" => b' ',
        "y" => b'y',
        "n" => b'n',
        "1" => b'1',
        "2" => b'2',
        "3" => b'3',
        "4" => b'4',
        "5" => b'5',
        "6" => b'6',
        "7" => b'7',
        _ => return None,
    })
}

fn push_input(pressed: bool, key: u8) {
    let s = shared();
    let mut q = s.input.lock().unwrap();
    // Bound the queue so a wild client can't grow it without limit.
    if q.len() < 256 {
        q.push_back((if pressed { 1 } else { 0 }, key));
    }
}

// ──────────────────────────────────────────────────────────────────────────
// WAD resolution + engine thread
// ──────────────────────────────────────────────────────────────────────────

fn data_dir() -> std::path::PathBuf {
    let base = std::env::var("COGNITUM_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("cog-doom"));
    let _ = std::fs::create_dir_all(&base);
    base
}

/// Resolve the IWAD path. Resolution order:
///   1. env `DOOM_WAD` (explicit path),
///   2. `<exe_dir>/freedoom1.wad` (asset placed next to the binary),
///   3. `./freedoom1.wad` (current working dir).
/// Returns the first that exists, or `None` if none do.
fn resolve_wad() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("DOOM_WAD") {
        let p = std::path::PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
        eprintln!("[doom] DOOM_WAD set to {} but no file there", p.display());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("freedoom1.wad");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let cwd = std::path::PathBuf::from("freedoom1.wad");
    if cwd.is_file() {
        return Some(cwd);
    }
    None
}

fn start_engine() -> Result<(), String> {
    let wad_path = resolve_wad().ok_or_else(|| {
        format!(
            "no IWAD found. The doom cog needs a DOOM-format IWAD (the cognitum-one/cogs \
             cog ships the freely-redistributable FreeDoom IWAD as an asset).\n\
             Looked for, in order:\n  \
             1. $DOOM_WAD\n  \
             2. <exe_dir>/freedoom1.wad\n  \
             3. ./freedoom1.wad\n\
             Place freedoom1.wad next to the binary or set DOOM_WAD=/path/to/freedoom1.wad."
        )
    })?;

    eprintln!("[doom] using IWAD: {}", wad_path.display());

    // chdir to the writable data dir so the engine writes its config/savegames
    // somewhere sane (it writes default.cfg + savegames relative to cwd).
    let dir = data_dir();
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir to data dir: {e}"))?;

    // The engine needs an absolute WAD path now that we've chdir'd.
    let wad_abs = std::fs::canonicalize(&wad_path)
        .unwrap_or(wad_path)
        .to_string_lossy()
        .into_owned();

    std::thread::Builder::new()
        .name("doom-engine".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            // Build argv: ["doom", "-iwad", <wadpath>]
            let argv_owned: Vec<CString> = ["doom", "-iwad", &wad_abs]
                .iter()
                .map(|s| CString::new(*s).unwrap())
                .collect();
            let mut argv_ptrs: Vec<*mut c_char> =
                argv_owned.iter().map(|c| c.as_ptr() as *mut c_char).collect();
            argv_ptrs.push(std::ptr::null_mut());

            unsafe {
                doomgeneric_Create((argv_owned.len()) as c_int, argv_ptrs.as_mut_ptr());
                loop {
                    doomgeneric_Tick();
                }
            }
        })
        .map_err(|e| format!("spawn engine thread: {e}"))?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────
// JPEG encoding
// ──────────────────────────────────────────────────────────────────────────

fn encode_current_jpeg() -> (u64, Arc<Vec<u8>>) {
    let s = shared();
    let id = s.counter.load(Ordering::SeqCst);
    // Reuse cached encode if it matches the current counter.
    {
        let cache = s.jpeg_cache.lock().unwrap();
        if let Some((cid, bytes)) = cache.as_ref() {
            if *cid == id {
                return (id, bytes.clone());
            }
        }
    }
    // Snapshot the RGB frame, then encode without holding the frame lock.
    let rgb: Vec<u8> = { s.frame.lock().unwrap().clone() };
    let mut out = Vec::with_capacity(64 * 1024);
    {
        use jpeg_encoder::{ColorType, Encoder};
        let enc = Encoder::new(&mut out, jpeg_quality());
        enc.encode(&rgb, RESX as u16, RESY as u16, ColorType::Rgb)
            .expect("jpeg encode");
    }
    let arc = Arc::new(out);
    let mut cache = s.jpeg_cache.lock().unwrap();
    *cache = Some((id, arc.clone()));
    (id, arc)
}

/// Wait up to `timeout` for the frame counter to exceed `since`. Returns the
/// current counter (which may still equal `since` on timeout).
fn wait_for_frame(since: u64, timeout: Duration) -> u64 {
    let s = shared();
    let deadline = Instant::now() + timeout;
    loop {
        let cur = s.counter.load(Ordering::SeqCst);
        if cur > since {
            return cur;
        }
        let now = Instant::now();
        if now >= deadline {
            return cur;
        }
        let g = s.cv_lock.lock().unwrap();
        let _ = s.cv.wait_timeout(g, deadline - now).unwrap();
    }
}

// ──────────────────────────────────────────────────────────────────────────
// HTTP
// ──────────────────────────────────────────────────────────────────────────

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn json_response(status: u16, body: serde_json::Value) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body.to_string().into_bytes())
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
}

fn bearer_ok(expected: &Option<String>, req: &Request) -> bool {
    let Some(exp) = expected.as_ref() else {
        return true; // open mode (DOOM_OPEN=1): no auth
    };
    for h in req.headers() {
        if h.field.equiv("Authorization") {
            let val = h.value.as_str();
            if let Some(tok) = val.strip_prefix("Bearer ") {
                use subtle::ConstantTimeEq;
                return tok.as_bytes().ct_eq(exp.as_bytes()).into();
            }
        }
    }
    false
}

fn parse_query_since(url: &str) -> Option<u64> {
    let q = url.split('?').nth(1)?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("since") {
            return it.next().and_then(|v| v.parse::<u64>().ok());
        }
    }
    None
}

fn handle_frame(req: Request) {
    let url = req.url().to_string();
    let since = parse_query_since(&url);
    if let Some(since) = since {
        let cur = wait_for_frame(since, Duration::from_millis(1000));
        if cur <= since {
            // No new frame within the timeout.
            let _ = req.respond(Response::empty(204));
            return;
        }
    }
    let (id, bytes) = encode_current_jpeg();
    let resp = Response::from_data((*bytes).clone())
        .with_status_code(200)
        // Force Content-Length (never chunked): the agent's TLS proxy leaks
        // chunk framing into the body, corrupting the JPEG for the browser.
        .with_chunked_threshold(usize::MAX)
        .with_header(header("Content-Type", "image/jpeg"))
        .with_header(header("X-Frame-Id", &id.to_string()))
        .with_header(header("Cache-Control", "no-store"));
    let _ = req.respond(resp);
}

fn handle_input(mut req: Request) {
    let mut body = String::new();
    if std::io::Read::read_to_string(req.as_reader(), &mut body).is_err() {
        let _ = req.respond(json_response(400, serde_json::json!({"error":"read body"})));
        return;
    }
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = req.respond(json_response(400, serde_json::json!({"error":"bad json"})));
            return;
        }
    };

    // Accept {"events":[...]}, a bare array [...], or a single {action,key}.
    let events: Vec<serde_json::Value> = if let Some(arr) = parsed.get("events").and_then(|e| e.as_array()) {
        arr.clone()
    } else if let Some(arr) = parsed.as_array() {
        arr.clone()
    } else if parsed.is_object() {
        vec![parsed.clone()]
    } else {
        vec![]
    };

    let mut accepted = 0;
    for ev in events {
        let action = ev.get("action").and_then(|a| a.as_str()).unwrap_or("");
        let key_name = ev.get("key").and_then(|k| k.as_str()).unwrap_or("");
        let pressed = match action {
            "down" | "press" => true,
            "up" | "release" => false,
            _ => continue,
        };
        if let Some(code) = map_key(key_name) {
            push_input(pressed, code);
            accepted += 1;
        }
    }
    let _ = req.respond(json_response(200, serde_json::json!({"ok": true, "accepted": accepted})));
}

fn handle_stream(req: Request) {
    // multipart/x-mixed-replace MJPEG. Best-effort; loops until the socket dies.
    // tiny_http's into_writer() hands us the raw socket with NO HTTP head
    // emitted, so we write the status line + headers ourselves, then stream parts.
    let boundary = "cogdoomframe";
    let mut writer = req.into_writer();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={boundary}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    if writer.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut last = 0u64;
    loop {
        wait_for_frame(last, Duration::from_millis(2000));
        let (id, bytes) = encode_current_jpeg();
        last = id;
        let part = format!(
            "--{boundary}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            bytes.len()
        );
        if writer.write_all(part.as_bytes()).is_err() {
            break;
        }
        if writer.write_all(&bytes).is_err() {
            break;
        }
        if writer.write_all(b"\r\n").is_err() {
            break;
        }
        if writer.flush().is_err() {
            break;
        }
    }
}

const INDEX_HTML: &str = include_str!("../assets/index.html");

fn handle_request(req: Request, expected: &Option<String>) {
    let method = req.method().clone();
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();

    // Open endpoints (no auth).
    match (&method, path.as_str()) {
        (Method::Get, "/") | (Method::Get, "/index.html") => {
            let resp = Response::from_data(INDEX_HTML.as_bytes().to_vec())
                .with_status_code(200)
                .with_header(header("Content-Type", "text/html; charset=utf-8"));
            let _ = req.respond(resp);
            return;
        }
        (Method::Get, "/health") => {
            let _ = req.respond(json_response(200, serde_json::json!({"status":"ok"})));
            return;
        }
        _ => {}
    }

    // Authenticated endpoints.
    if !bearer_ok(expected, &req) {
        let _ = req.respond(json_response(401, serde_json::json!({"error":"unauthorized"})));
        return;
    }

    match (&method, path.as_str()) {
        (Method::Get, "/frame") => handle_frame(req),
        (Method::Post, "/input") => handle_input(req),
        (Method::Get, "/stream") => handle_stream(req),
        _ => {
            let _ = req.respond(json_response(404, serde_json::json!({"error":"not found"})));
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// main
// ──────────────────────────────────────────────────────────────────────────

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!("cog-doom {VERSION} — Cognitum DOOM cog");
    println!();
    println!("USAGE:");
    println!("  cog-doom            start the engine + HTTP API (default 127.0.0.1:8066)");
    println!("  cog-doom --help     show this help");
    println!("  cog-doom --version  print version");
    println!();
    println!("ENV:");
    println!("  COGNITUM_COG_PORT    bind port (default 8066)");
    println!("  COGNITUM_COG_TOKEN   per-cog bearer token; REQUIRED on /frame,/input,/stream");
    println!("                       unless DOOM_OPEN=1. Injected by the seed agent.");
    println!("  COGNITUM_DATA_DIR    writable dir for savegames/config (default tmp/cog-doom)");
    println!("  DOOM_WAD             path to the IWAD (else <exe_dir>/freedoom1.wad, ./freedoom1.wad)");
    println!("  DOOM_BIND            bind address (default 127.0.0.1; set 0.0.0.0 for LAN)");
    println!("  DOOM_OPEN            =1 disables the bearer-token check (direct-LAN play). OFF by default.");
    println!("  DOOM_JPEG_QUALITY    JPEG quality 1-100 (default 55)");
}

fn main() {
    // CLI: --help / --version for the [console] contract.
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--help") | Some("-h") => {
            print_help();
            return;
        }
        Some("--version") | Some("-V") => {
            println!("cog-doom {VERSION}");
            return;
        }
        _ => {}
    }

    let port: u16 = std::env::var("COGNITUM_COG_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // JPEG quality (clamped to a sane range).
    let q: u8 = std::env::var("DOOM_JPEG_QUALITY")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(DEFAULT_JPEG_QUALITY);
    let _ = JPEG_QUALITY.set(q);

    // Auth model (ADR-095 default): require a bearer token on the game endpoints.
    // `DOOM_OPEN=1` is an explicit opt-out for direct-LAN play (no agent proxy).
    let open_mode = std::env::var("DOOM_OPEN").map(|v| v == "1").unwrap_or(false);
    let expected_token: Option<String> = if open_mode {
        eprintln!(
            "[doom] DOOM_OPEN=1 — token auth DISABLED; /frame,/input,/stream are open. \
             Anyone who can reach this port can play. Intended for direct-LAN use only."
        );
        None
    } else {
        match std::env::var("COGNITUM_COG_TOKEN") {
            Ok(t) if !t.is_empty() => Some(t),
            _ => {
                // No token AND not in open mode: the game endpoints are unreachable
                // (every request 401s). This is the safe default — the seed agent
                // injects COGNITUM_COG_TOKEN at /start. Warn loudly so a local dev
                // who forgot it understands why /frame returns 401.
                eprintln!(
                    "[doom] WARNING: COGNITUM_COG_TOKEN is not set and DOOM_OPEN is off — \
                     /frame,/input,/stream will reject all requests with 401. The seed agent \
                     normally injects the token; for local testing set COGNITUM_COG_TOKEN=... \
                     or DOOM_OPEN=1."
                );
                Some(String::new()) // empty token: nothing matches -> always 401
            }
        }
    };

    // Start the engine first so frames begin rendering immediately. If the WAD
    // can't be found, fail fast with a clear, actionable error.
    if let Err(e) = start_engine() {
        eprintln!("[doom] fatal: {e}");
        std::process::exit(1);
    }

    // Default to loopback (ADR-095 loopback-only). `DOOM_BIND=0.0.0.0` faces the
    // LAN directly (bypassing the agent proxy) — pair with DOOM_OPEN=1 for the
    // documented direct-LAN mode.
    let bind = std::env::var("DOOM_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let server = match Server::http((bind.as_str(), port)) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("[doom] failed to bind {bind}:{port}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "[doom] cog API listening on {bind}:{port} (auth: {})",
        if expected_token.is_none() { "OPEN" } else { "bearer-token" }
    );

    let expected = Arc::new(expected_token);
    let mut handles = Vec::new();
    for _ in 0..4 {
        let server = server.clone();
        let expected = expected.clone();
        handles.push(std::thread::spawn(move || loop {
            match server.recv() {
                Ok(req) => handle_request(req, &expected),
                Err(_) => break,
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
}
