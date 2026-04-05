//! Cognitum Cog: Network Firewall — Connection Monitor
//!
//! Monitors network connections by reading /proc/net/tcp on the seed.
//! Detects unauthorized outbound connections and alerts on unexpected ports.
//! Maintains an allowlist of known-good ports and destinations.
//!
//! Usage:
//!   cog-network-firewall --once
//!   cog-network-firewall --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

/// Known-good local ports for the Cognitum seed
const ALLOWED_LOCAL_PORTS: &[u16] = &[
    22,    // SSH
    80,    // HTTP (cognitum-agent)
    443,   // HTTPS
    5353,  // mDNS
];

/// Known-good remote ports
const ALLOWED_REMOTE_PORTS: &[u16] = &[
    53,    // DNS
    80,    // HTTP
    443,   // HTTPS
    8443,  // Alt HTTPS
    123,   // NTP
];

#[derive(Clone)]
struct TcpConnection {
    local_addr: u32,
    local_port: u16,
    remote_addr: u32,
    remote_port: u16,
    state: u8,
}

impl TcpConnection {
    fn ip_to_string(addr: u32) -> String {
        format!("{}.{}.{}.{}",
            addr & 0xFF,
            (addr >> 8) & 0xFF,
            (addr >> 16) & 0xFF,
            (addr >> 24) & 0xFF)
    }

    fn local_ip(&self) -> String { Self::ip_to_string(self.local_addr) }
    fn remote_ip(&self) -> String { Self::ip_to_string(self.remote_addr) }

    fn state_str(&self) -> &'static str {
        match self.state {
            1 => "ESTABLISHED",
            2 => "SYN_SENT",
            3 => "SYN_RECV",
            4 => "FIN_WAIT1",
            5 => "FIN_WAIT2",
            6 => "TIME_WAIT",
            7 => "CLOSE",
            8 => "CLOSE_WAIT",
            9 => "LAST_ACK",
            10 => "LISTEN",
            11 => "CLOSING",
            _ => "UNKNOWN",
        }
    }

    fn is_outbound(&self) -> bool {
        // Outbound: remote port is a well-known service port, local is ephemeral
        self.local_port > 1024 && self.state != 10 // not LISTEN
    }

    fn is_loopback(&self) -> bool {
        (self.remote_addr & 0xFF) == 127 || self.remote_addr == 0
    }
}

struct NetworkFirewall {
    known_connections: Vec<(u32, u16)>, // (remote_addr, remote_port) seen before
    total_violations: u64,
    total_scans: u64,
    allowed_remote_ports: Vec<u16>,
    allowed_local_ports: Vec<u16>,
}

impl NetworkFirewall {
    fn new() -> Self {
        Self {
            known_connections: Vec::new(),
            total_violations: 0,
            total_scans: 0,
            allowed_remote_ports: ALLOWED_REMOTE_PORTS.to_vec(),
            allowed_local_ports: ALLOWED_LOCAL_PORTS.to_vec(),
        }
    }

    fn scan(&mut self) -> ScanResult {
        self.total_scans += 1;
        let connections = match read_proc_tcp() {
            Ok(c) => c,
            Err(e) => return ScanResult::Error(e),
        };

        let mut violations = Vec::new();
        let mut active_connections = Vec::new();

        for conn in &connections {
            if conn.is_loopback() { continue; }
            if conn.state == 10 { continue; } // skip LISTEN

            let conn_info = ConnectionInfo {
                local: format!("{}:{}", conn.local_ip(), conn.local_port),
                remote: format!("{}:{}", conn.remote_ip(), conn.remote_port),
                state: conn.state_str().to_string(),
                outbound: conn.is_outbound(),
            };

            active_connections.push(conn_info.clone());

            // Check for violations
            if conn.is_outbound() {
                if !self.allowed_remote_ports.contains(&conn.remote_port) {
                    violations.push(Violation {
                        violation_type: "unauthorized_outbound_port".into(),
                        connection: conn_info,
                        severity: if conn.remote_port < 1024 { "high" } else { "medium" }.into(),
                    });
                }

                // Check if this is a new connection we haven't seen
                let key = (conn.remote_addr, conn.remote_port);
                if !self.known_connections.contains(&key) {
                    self.known_connections.push(key);
                    if self.total_scans > 3 { // after initial learning
                        violations.push(Violation {
                            violation_type: "new_outbound_connection".into(),
                            connection: ConnectionInfo {
                                local: format!("{}:{}", conn.local_ip(), conn.local_port),
                                remote: format!("{}:{}", conn.remote_ip(), conn.remote_port),
                                state: conn.state_str().to_string(),
                                outbound: true,
                            },
                            severity: "low".into(),
                        });
                    }
                }
            }

            // Detect unexpected listening ports
            if conn.state == 10 && !self.allowed_local_ports.contains(&conn.local_port) {
                violations.push(Violation {
                    violation_type: "unauthorized_listen".into(),
                    connection: ConnectionInfo {
                        local: format!("{}:{}", conn.local_ip(), conn.local_port),
                        remote: "0.0.0.0:0".into(),
                        state: "LISTEN".into(),
                        outbound: false,
                    },
                    severity: "high".into(),
                });
            }
        }

        self.total_violations += violations.len() as u64;

        // Keep known connections bounded
        if self.known_connections.len() > 500 {
            self.known_connections.drain(0..250);
        }

        ScanResult::Ok {
            connections: active_connections,
            violations,
            total_connections: connections.len(),
        }
    }
}

#[derive(Clone)]
struct ConnectionInfo {
    local: String,
    remote: String,
    state: String,
    outbound: bool,
}

struct Violation {
    violation_type: String,
    connection: ConnectionInfo,
    severity: String,
}

enum ScanResult {
    Ok {
        connections: Vec<ConnectionInfo>,
        violations: Vec<Violation>,
        total_connections: usize,
    },
    Error(String),
}

fn read_proc_tcp() -> Result<Vec<TcpConnection>, String> {
    let content = std::fs::read_to_string("/proc/net/tcp")
        .map_err(|e| format!("read /proc/net/tcp: {e}"))?;

    let mut connections = Vec::new();
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }

        let local = parse_addr_port(parts[1]);
        let remote = parse_addr_port(parts[2]);
        let state = u8::from_str_radix(parts[3], 16).unwrap_or(0);

        if let (Some((la, lp)), Some((ra, rp))) = (local, remote) {
            connections.push(TcpConnection {
                local_addr: la,
                local_port: lp,
                remote_addr: ra,
                remote_port: rp,
                state,
            });
        }
    }
    Ok(connections)
}

fn parse_addr_port(s: &str) -> Option<(u32, u16)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 { return None; }
    let addr = u32::from_str_radix(parts[0], 16).ok()?;
    let port = u16::from_str_radix(parts[1], 16).ok()?;
    Some((addr, port))
}

#[derive(serde::Serialize)]
struct FirewallReport {
    status: String,
    violations_found: bool,
    violation_count: usize,
    violations: Vec<ViolationJson>,
    active_connections: usize,
    outbound_connections: usize,
    known_destinations: usize,
    total_violations: u64,
    total_scans: u64,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ViolationJson {
    violation_type: String,
    local: String,
    remote: String,
    state: String,
    severity: String,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn store_vector(report: &FirewallReport) -> Result<(), String> {
    let vector = vec![
        if report.violations_found { 1.0 } else { 0.0 },
        report.violation_count as f64 / 10.0,
        report.active_connections as f64 / 100.0,
        report.outbound_connections as f64 / 50.0,
        report.known_destinations as f64 / 500.0,
        report.total_violations as f64 / 1000.0,
        report.total_scans as f64 / 10000.0,
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
        .unwrap_or(10);

    eprintln!("[cog-network-firewall] starting (interval={}s)", interval);
    let mut firewall = NetworkFirewall::new();

    loop {
        let start = Instant::now();
        let result = firewall.scan();

        let report = match result {
            ScanResult::Error(e) => {
                eprintln!("[cog-network-firewall] scan error: {e}");
                FirewallReport {
                    status: "error".into(),
                    violations_found: false, violation_count: 0,
                    violations: Vec::new(),
                    active_connections: 0, outbound_connections: 0,
                    known_destinations: firewall.known_connections.len(),
                    total_violations: firewall.total_violations,
                    total_scans: firewall.total_scans,
                    timestamp: now_ts(),
                }
            }
            ScanResult::Ok { connections, violations, total_connections } => {
                let outbound = connections.iter().filter(|c| c.outbound).count();
                let violation_jsons: Vec<ViolationJson> = violations.iter().map(|v| ViolationJson {
                    violation_type: v.violation_type.clone(),
                    local: v.connection.local.clone(),
                    remote: v.connection.remote.clone(),
                    state: v.connection.state.clone(),
                    severity: v.severity.clone(),
                }).collect();

                let has_violations = !violation_jsons.is_empty();
                let high_severity = violations.iter().any(|v| v.severity == "high");

                FirewallReport {
                    status: if high_severity { "VIOLATION".into() }
                        else if has_violations { "warning".into() }
                        else { "secure".into() },
                    violations_found: has_violations,
                    violation_count: violation_jsons.len(),
                    violations: violation_jsons,
                    active_connections: connections.len(),
                    outbound_connections: outbound,
                    known_destinations: firewall.known_connections.len(),
                    total_violations: firewall.total_violations,
                    total_scans: firewall.total_scans,
                    timestamp: now_ts(),
                }
            }
        };

        println!("{}", serde_json::to_string(&report).unwrap_or_default());
        if let Err(e) = store_vector(&report) {
            eprintln!("[cog-network-firewall] store error: {e}");
        }
        if report.violations_found {
            for v in &report.violations {
                eprintln!("[cog-network-firewall] {} [{}]: {} -> {} ({})",
                    v.severity.to_uppercase(), v.violation_type, v.local, v.remote, v.state);
            }
        }

        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
