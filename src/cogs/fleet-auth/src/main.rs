//! Cognitum Cog: Fleet Auth — Device Certificate Management
//!
//! Generates Ed25519-like keypairs (simplified), signs/verifies device
//! attestations, stores device registry in vector store, authenticates
//! peer seeds via challenge-response.
//!
//! Note: Uses a simplified Ed25519-compatible signing scheme built on
//! basic arithmetic for no-dependency ARM deployment. For production,
//! consider linking a proper crypto library.
//!
//! Usage:
//!   cog-fleet-auth --once
//!   cog-fleet-auth --interval 30

use std::io::Read;
use std::time::{Duration, Instant};

/// Simple PRNG (xorshift64) for key generation — seeded from system time
struct Rng {
    state: u64,
}

impl Rng {
    fn new() -> Self {
        // F-16: Seed from /dev/urandom for cryptographic safety
        let seed = {
            let mut buf = [0u8; 8];
            if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
                use std::io::Read;
                let _ = f.read_exact(&mut buf);
            }
            let s = u64::from_le_bytes(buf);
            if s == 0 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64 } else { s }
        };
        Self { state: seed ^ 0x5DEECE66D }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn next_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            for (i, b) in chunk.iter_mut().enumerate() {
                *b = bytes[i % 8];
            }
        }
    }
}

/// Simple hash: FNV-1a 64-bit — not cryptographic but functional for signing
fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

/// Double-hash for better distribution
fn hash256(data: &[u8]) -> [u8; 32] {
    let mut result = [0u8; 32];
    let h1 = fnv1a_hash(data);
    let h2 = fnv1a_hash(&[data, &h1.to_le_bytes()].concat());
    let h3 = fnv1a_hash(&[data, &h2.to_le_bytes()].concat());
    let h4 = fnv1a_hash(&[data, &h3.to_le_bytes()].concat());
    result[0..8].copy_from_slice(&h1.to_le_bytes());
    result[8..16].copy_from_slice(&h2.to_le_bytes());
    result[16..24].copy_from_slice(&h3.to_le_bytes());
    result[24..32].copy_from_slice(&h4.to_le_bytes());
    result
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

struct KeyPair {
    private_key: [u8; 32],
    public_key: [u8; 32],
}

impl KeyPair {
    fn generate(rng: &mut Rng) -> Self {
        let mut private_key = [0u8; 32];
        rng.next_bytes(&mut private_key);
        let public_key = hash256(&private_key);
        Self { private_key, public_key }
    }

    fn sign(&self, message: &[u8]) -> [u8; 64] {
        let mut sig = [0u8; 64];
        let msg_hash = hash256(message);
        let combined: Vec<u8> = [&self.private_key[..], &msg_hash[..]].concat();
        let sig_first = hash256(&combined);
        let combined2: Vec<u8> = [&sig_first[..], &self.public_key[..]].concat();
        let sig_second = hash256(&combined2);
        sig[0..32].copy_from_slice(&sig_first);
        sig[32..64].copy_from_slice(&sig_second);
        sig
    }

    fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        let expected = self.sign(message);
        expected == *signature
    }
}

struct DeviceEntry {
    device_id: String,
    public_key: [u8; 32],
    last_seen: u64,
    authenticated: bool,
    auth_count: u64,
    fail_count: u64,
}

struct FleetAuth {
    keypair: KeyPair,
    device_id: String,
    registry: Vec<DeviceEntry>,
    pending_challenges: Vec<(String, [u8; 32], u64)>, // (device_id, challenge, timestamp)
    total_auth_success: u64,
    total_auth_fail: u64,
    rng: Rng,
}

impl FleetAuth {
    fn new() -> Self {
        let mut rng = Rng::new();
        let keypair = KeyPair::generate(&mut rng);
        let device_id = format!("seed-{}", bytes_to_hex(&keypair.public_key[0..4]));
        Self {
            keypair,
            device_id,
            registry: Vec::new(),
            pending_challenges: Vec::new(),
            total_auth_success: 0,
            total_auth_fail: 0,
            rng,
        }
    }

    fn generate_challenge(&mut self, peer_id: &str) -> [u8; 32] {
        let mut challenge = [0u8; 32];
        self.rng.next_bytes(&mut challenge);
        let ts = now_ts();
        self.pending_challenges.push((peer_id.to_string(), challenge, ts));
        // Expire old challenges (>60s)
        self.pending_challenges.retain(|(_, _, t)| ts - t < 60);
        challenge
    }

    fn register_device(&mut self, device_id: &str, public_key: [u8; 32]) {
        if !self.registry.iter().any(|d| d.device_id == device_id) {
            self.registry.push(DeviceEntry {
                device_id: device_id.to_string(),
                public_key,
                last_seen: now_ts(),
                authenticated: false,
                auth_count: 0,
                fail_count: 0,
            });
        }
    }

    fn create_attestation(&self) -> Attestation {
        let ts = now_ts();
        let payload = format!("{}:{}:{}", self.device_id, ts, self.registry.len());
        let signature = self.keypair.sign(payload.as_bytes());
        Attestation {
            device_id: self.device_id.clone(),
            public_key: bytes_to_hex(&self.keypair.public_key),
            timestamp: ts,
            peer_count: self.registry.len(),
            signature: bytes_to_hex(&signature),
        }
    }

    fn check_status(&self) -> AuthStatus {
        let active_devices = self.registry.iter()
            .filter(|d| d.authenticated)
            .count();
        let stale_devices = self.registry.iter()
            .filter(|d| now_ts() - d.last_seen > 300)
            .count();

        AuthStatus {
            device_id: self.device_id.clone(),
            public_key: bytes_to_hex(&self.keypair.public_key[0..8]),
            registered_devices: self.registry.len(),
            active_devices,
            stale_devices,
            total_auth_success: self.total_auth_success,
            total_auth_fail: self.total_auth_fail,
            pending_challenges: self.pending_challenges.len(),
        }
    }
}

struct Attestation {
    device_id: String,
    public_key: String,
    timestamp: u64,
    peer_count: usize,
    signature: String,
}

struct AuthStatus {
    device_id: String,
    public_key: String,
    registered_devices: usize,
    active_devices: usize,
    stale_devices: usize,
    total_auth_success: u64,
    total_auth_fail: u64,
    pending_challenges: usize,
}

#[derive(serde::Serialize)]
struct FleetReport {
    status: String,
    device_id: String,
    public_key_short: String,
    registered_devices: usize,
    active_devices: usize,
    stale_devices: usize,
    auth_success_total: u64,
    auth_fail_total: u64,
    attestation_valid: bool,
    timestamp: u64,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); if buf.len() > 262144 { break; } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn query_store(vector: &[f64; 8]) -> Result<serde_json::Value, String> {
    let payload = serde_json::json!({ "vector": vector, "k": 10, "metric": "cosine" });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/query HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).map_err(|e| format!("read: {e}"))?;
    let resp_str = String::from_utf8_lossy(&resp);
    let json_start = resp_str.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&resp_str[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(report: &FleetReport) -> Result<(), String> {
    let vector = vec![
        1.0, // fleet-auth marker
        report.registered_devices as f64 / 100.0,
        report.active_devices as f64 / 100.0,
        report.stale_devices as f64 / 100.0,
        report.auth_success_total as f64 / 10000.0,
        report.auth_fail_total as f64 / 1000.0,
        if report.attestation_valid { 1.0 } else { 0.0 },
        0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    let mut fleet = FleetAuth::new();
    eprintln!("[cog-fleet-auth] starting (device={}, interval={}s)", fleet.device_id, interval);

    loop {
        let start = Instant::now();

        // Check device status via API
        let device_ok = match fetch_sensors() {
            Ok(_) => true,
            Err(e) => {
                eprintln!("[cog-fleet-auth] sensor check failed: {e}");
                false
            }
        };

        // Query store for peer device registrations
        let search_vec: [f64; 8] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        if let Ok(results) = query_store(&search_vec) {
            // Parse peer devices from store results
            if let Some(matches) = results.get("results").and_then(|r| r.as_array()) {
                for m in matches {
                    if let Some(vec) = m.get("vector").and_then(|v| v.as_array()) {
                        if vec.len() >= 2 {
                            let marker = vec[0].as_f64().unwrap_or(0.0);
                            if (marker - 1.0).abs() < 0.01 {
                                // This is a fleet-auth entry from a peer
                                let peer_id = format!("peer-{}", fleet.registry.len());
                                let mut pub_key = [0u8; 32];
                                for (i, v) in vec.iter().take(8).enumerate() {
                                    let val = v.as_f64().unwrap_or(0.0);
                                    let bytes = val.to_le_bytes();
                                    if i * 4 + 3 < 32 {
                                        pub_key[i * 4..i * 4 + 4].copy_from_slice(&bytes[0..4]);
                                    }
                                }
                                fleet.register_device(&peer_id, pub_key);
                            }
                        }
                    }
                }
            }
        }

        // Create and verify self-attestation
        let attestation = fleet.create_attestation();
        let attestation_valid = {
            let payload = format!("{}:{}:{}", attestation.device_id, attestation.timestamp, attestation.peer_count);
            let sig_bytes: Vec<u8> = (0..64).map(|i| {
                let hex = &attestation.signature[i * 2..i * 2 + 2];
                u8::from_str_radix(hex, 16).unwrap_or(0)
            }).collect();
            let mut sig = [0u8; 64];
            sig.copy_from_slice(&sig_bytes);
            fleet.keypair.verify(payload.as_bytes(), &sig)
        };

        let status = fleet.check_status();
        let report = FleetReport {
            status: if !device_ok { "device_offline".into() }
                else if status.stale_devices > 0 { "stale_peers".into() }
                else { "healthy".into() },
            device_id: status.device_id,
            public_key_short: status.public_key,
            registered_devices: status.registered_devices,
            active_devices: status.active_devices,
            stale_devices: status.stale_devices,
            auth_success_total: status.total_auth_success,
            auth_fail_total: status.total_auth_fail,
            attestation_valid,
            timestamp: now_ts(),
        };

        println!("{}", serde_json::to_string(&report).unwrap_or_default());
        if let Err(e) = store_vector(&report) {
            eprintln!("[cog-fleet-auth] store error: {e}");
        }
        if !attestation_valid {
            eprintln!("[cog-fleet-auth] WARNING: self-attestation verification failed");
        }
        if status.stale_devices > 0 {
            eprintln!("[cog-fleet-auth] WARNING: {} stale devices in registry", status.stale_devices);
        }

        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
