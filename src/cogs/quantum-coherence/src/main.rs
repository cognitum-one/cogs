//! Cognitum Cog: Quantum Coherence
//!
//! Quantum-inspired signal processing. Use superposition-like state
//! representation where each channel is a "qubit" amplitude.
//! Interference patterns for enhanced detection.
//!
//! Usage:
//!   cog-quantum-coherence --once
//!   cog-quantum-coherence --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Quantum-inspired state: amplitudes (complex numbers as [real, imag] pairs)
struct QuantumState {
    amplitudes: Vec<(f64, f64)>, // (real, imaginary) per channel
}

impl QuantumState {
    fn from_channels(values: &[f64]) -> Self {
        let n = values.len();
        // Normalize to unit vector (quantum state normalization)
        let norm: f64 = values.iter().map(|v| v * v).sum::<f64>().sqrt();
        let scale = if norm > 1e-10 { 1.0 / norm } else { 0.0 };

        let amplitudes: Vec<(f64, f64)> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let real = v * scale;
                // Phase encoding: use position-dependent phase
                let phase = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                let imag = v * scale * phase.sin();
                let real_rot = real * phase.cos();
                (real_rot, imag)
            })
            .collect();

        Self { amplitudes }
    }

    /// Compute probability of measurement (|amplitude|^2)
    fn probabilities(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|(r, i)| r * r + i * i).collect()
    }

    /// Compute interference between two quantum states
    fn interference(&self, other: &QuantumState) -> f64 {
        let n = self.amplitudes.len().min(other.amplitudes.len());
        if n == 0 { return 0.0; }
        let mut total = 0.0;
        for i in 0..n {
            let (r1, i1) = self.amplitudes[i];
            let (r2, i2) = other.amplitudes[i];
            // Inner product: real part
            total += r1 * r2 + i1 * i2;
        }
        total
    }

    /// Von Neumann entropy estimate (quantum information content)
    fn entropy(&self) -> f64 {
        let probs = self.probabilities();
        let total: f64 = probs.iter().sum();
        if total < 1e-10 { return 0.0; }
        -probs.iter()
            .map(|&p| {
                let norm_p = p / total;
                if norm_p > 1e-10 { norm_p * norm_p.ln() } else { 0.0 }
            })
            .sum::<f64>()
    }

    /// Coherence measure: off-diagonal elements of density matrix
    fn coherence_measure(&self) -> f64 {
        let n = self.amplitudes.len();
        if n < 2 { return 0.0; }
        let mut off_diagonal = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let (ri, ii) = self.amplitudes[i];
                let (rj, ij) = self.amplitudes[j];
                // |rho_ij| = |a_i * conj(a_j)|
                let real = ri * rj + ii * ij;
                let imag = ii * rj - ri * ij;
                off_diagonal += (real * real + imag * imag).sqrt();
            }
        }
        off_diagonal / (n * (n - 1) / 2).max(1) as f64
    }

    /// Apply Hadamard-like transform to amplify differences
    fn hadamard_detect(&self) -> Vec<f64> {
        let n = self.amplitudes.len();
        let scale = 1.0 / (n as f64).sqrt();
        (0..n).map(|k| {
            let mut sum_r = 0.0;
            for (j, &(r, _i)) in self.amplitudes.iter().enumerate() {
                let angle = 2.0 * std::f64::consts::PI * k as f64 * j as f64 / n as f64;
                sum_r += r * angle.cos();
            }
            sum_r * scale
        }).collect()
    }
}

#[derive(serde::Serialize)]
struct QuantumResult {
    channel_count: usize,
    entropy: f64,
    coherence: f64,
    interference_score: f64,
    probabilities: Vec<f64>,
    detection_amplified: Vec<f64>,
    quantum_status: String,
    anomalies: Vec<String>,
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
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

fn run_once(prev_state: &mut Option<QuantumState>) -> Result<QuantumResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let state = QuantumState::from_channels(&values);
    let entropy = state.entropy();
    let coherence = state.coherence_measure();
    let probs = state.probabilities();
    let detected = state.hadamard_detect();

    let interference = if let Some(prev) = prev_state.as_ref() {
        state.interference(prev)
    } else {
        0.0
    };

    let quantum_status = if coherence > 0.5 {
        "high_coherence"
    } else if coherence > 0.1 {
        "partial_coherence"
    } else {
        "decoherent"
    };

    let mut anomalies = Vec::new();
    if entropy > 3.0 {
        anomalies.push(format!("HIGH_ENTROPY: quantum entropy={entropy:.3}"));
    }
    if interference.abs() > 0.8 {
        anomalies.push(format!("STRONG_INTERFERENCE: score={interference:.3}"));
    }

    let max_detected = detected.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let mut vector = [0.0; DIM];
    vector[0] = entropy / 5.0;
    vector[1] = coherence;
    vector[2] = interference.abs();
    vector[3] = max_detected.abs().min(1.0);
    for (i, &p) in probs.iter().enumerate().take(4) {
        vector[4 + i] = p;
    }

    let _ = store_vector(&vector);

    *prev_state = Some(state);

    Ok(QuantumResult {
        channel_count: values.len(),
        entropy,
        coherence,
        interference_score: interference,
        probabilities: probs,
        detection_amplified: detected,
        quantum_status: quantum_status.into(),
        anomalies,
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-quantum-coherence] starting (interval={interval}s, once={once})");

    let mut prev_state: Option<QuantumState> = None;

    loop {
        let start = Instant::now();
        match run_once(&mut prev_state) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-quantum-coherence] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-quantum-coherence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
