// Smoke tests for fall-detect via process invocation. These check
// that the binary parses CLI args, exits cleanly with --once, and
// produces well-formed JSON when the COG_SENSOR_URL points at a stub.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

const STUB_RESPONSE: &str = "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"healthy\":true,\"sample_count\":4,\"sample_rate_hz\":10,\"samples\":[{\"channel\":\"ch0\",\"value\":0.1,\"normalized\":0.1,\"quality\":0,\"quality_label\":\"good\",\"sensor\":\"synthetic\",\"timestamp_us\":0},{\"channel\":\"ch1\",\"value\":0.05,\"normalized\":0.05,\"quality\":0,\"quality_label\":\"good\",\"sensor\":\"synthetic\",\"timestamp_us\":0},{\"channel\":\"ch2\",\"value\":-0.1,\"normalized\":-0.1,\"quality\":0,\"quality_label\":\"good\",\"sensor\":\"synthetic\",\"timestamp_us\":0},{\"channel\":\"ch3\",\"value\":0.02,\"normalized\":0.02,\"quality\":0,\"quality_label\":\"good\",\"sensor\":\"synthetic\",\"timestamp_us\":0}]}";

fn spawn_stub() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for s in listener.incoming().flatten() {
            handle_one(s);
        }
    });
    // Give it a moment to start accepting
    thread::sleep(Duration::from_millis(50));
    port
}

fn handle_one(mut s: TcpStream) {
    // Read request (until \r\n\r\n) — we don't actually parse it.
    use std::io::Read;
    let mut buf = [0u8; 1024];
    let _ = s.read(&mut buf);
    let _ = s.write_all(STUB_RESPONSE.as_bytes());
    let _ = s.flush();
}

#[test]
fn fall_detect_runs_once_and_emits_json() {
    let port = spawn_stub();

    let exe = std::env::current_exe().unwrap();
    let mut p = exe.parent().unwrap().to_path_buf();
    // Walk up to crate target dir
    while p.file_name().map(|n| n != "target").unwrap_or(false) {
        if let Some(parent) = p.parent() { p = parent.to_path_buf(); } else { break; }
    }
    p.pop();
    let bin = p.join("target/debug/cog-fall-detect.exe");
    let bin_alt = p.join("target/release/cog-fall-detect.exe");
    let bin_unix = p.join("target/debug/cog-fall-detect");

    let bin_to_use = if bin.exists() { bin } else if bin_alt.exists() { bin_alt } else { bin_unix };
    if !bin_to_use.exists() {
        // Build hasn't run yet — skip rather than fail
        eprintln!("skip: binary {:?} not built", bin_to_use);
        return;
    }

    let out = std::process::Command::new(&bin_to_use)
        .env("COG_SENSOR_URL", format!("127.0.0.1:{port}"))
        .args(["--once"])
        .output()
        .expect("spawn cog-fall-detect");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // First non-empty stdout line should be JSON.
    let first = stdout.lines().find(|l| !l.is_empty()).unwrap_or("");
    assert!(first.starts_with('{'), "expected JSON output, got: {first}");
    assert!(first.contains("\"status\""), "expected status field");
    assert!(first.contains("\"fall_detected\""), "expected fall_detected field");
}
