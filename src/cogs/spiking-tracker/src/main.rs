//! Cognitum Cog: Spiking Tracker
//!
//! Spiking neural network inspired tracker. Leaky integrate-and-fire
//! neurons that spike on signal threshold crossing. Track spike trains
//! to detect movement patterns.
//!
//! Usage:
//!   cog-spiking-tracker --once
//!   cog-spiking-tracker --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Leaky Integrate-and-Fire neuron
struct LifNeuron {
    /// Membrane potential
    potential: f64,
    /// Leak rate (decay per step)
    tau: f64,
    /// Firing threshold
    threshold: f64,
    /// Reset potential after spike
    reset: f64,
    /// Refractory period counter
    refractory: u32,
    /// Refractory period length
    refractory_period: u32,
    /// Spike count
    spike_count: u64,
    /// Last spike time
    last_spike: Option<usize>,
    /// Inter-spike intervals
    isis: Vec<usize>,
}

impl LifNeuron {
    fn new(threshold: f64, tau: f64) -> Self {
        Self {
            potential: 0.0,
            tau,
            threshold,
            reset: 0.0,
            refractory: 0,
            refractory_period: 2,
            spike_count: 0,
            last_spike: None,
            isis: Vec::new(),
        }
    }

    /// Process one input sample, returns true if neuron fires
    fn step(&mut self, input: f64, time: usize) -> bool {
        if self.refractory > 0 {
            self.refractory -= 1;
            self.potential *= 1.0 - self.tau;
            return false;
        }

        // Leaky integration
        self.potential = self.potential * (1.0 - self.tau) + input;

        if self.potential >= self.threshold {
            // Fire!
            self.spike_count += 1;
            if let Some(last) = self.last_spike {
                self.isis.push(time - last);
            }
            self.last_spike = Some(time);
            self.potential = self.reset;
            self.refractory = self.refractory_period;
            true
        } else {
            false
        }
    }

    /// Mean firing rate (spikes per sample)
    fn firing_rate(&self, total_steps: usize) -> f64 {
        if total_steps == 0 {
            return 0.0;
        }
        self.spike_count as f64 / total_steps as f64
    }

    /// Coefficient of variation of inter-spike intervals
    fn isi_cv(&self) -> f64 {
        if self.isis.len() < 2 {
            return 0.0;
        }
        let mean = self.isis.iter().sum::<usize>() as f64 / self.isis.len() as f64;
        if mean < 1e-10 {
            return 0.0;
        }
        let var = self.isis.iter().map(|&i| (i as f64 - mean).powi(2)).sum::<f64>()
            / self.isis.len() as f64;
        var.sqrt() / mean
    }

    /// Burstiness: fraction of short ISIs (< mean/2)
    fn burstiness(&self) -> f64 {
        if self.isis.len() < 2 {
            return 0.0;
        }
        let mean = self.isis.iter().sum::<usize>() as f64 / self.isis.len() as f64;
        let bursts = self.isis.iter().filter(|&&i| (i as f64) < mean / 2.0).count();
        bursts as f64 / self.isis.len() as f64
    }
}

/// Detect movement pattern from spike trains of multiple neurons
fn classify_movement(neurons: &[LifNeuron], total_steps: usize) -> String {
    let active_count = neurons.iter().filter(|n| n.spike_count > 0).count();
    let total_spikes: u64 = neurons.iter().map(|n| n.spike_count).sum();
    let avg_burst: f64 = neurons.iter().map(|n| n.burstiness()).sum::<f64>()
        / neurons.len().max(1) as f64;

    if active_count == 0 {
        "stationary".into()
    } else if total_spikes as f64 / total_steps as f64 > 0.3 {
        "rapid_movement".into()
    } else if avg_burst > 0.5 {
        "burst_movement".into()
    } else if active_count as f64 / neurons.len() as f64 > 0.5 {
        "distributed_movement".into()
    } else {
        "localized_movement".into()
    }
}

#[derive(serde::Serialize)]
struct SpikingResult {
    neuron_count: usize,
    total_spikes: u64,
    active_neurons: usize,
    firing_rates: Vec<f64>,
    isi_cvs: Vec<f64>,
    movement_pattern: String,
    burstiness: f64,
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

fn run_once() -> Result<SpikingResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    // Group by channel
    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    if channels.is_empty() {
        return Err("no channels".into());
    }

    // Create one LIF neuron per channel
    let mut neurons: Vec<LifNeuron> = channels.keys().map(|_| LifNeuron::new(0.5, 0.1)).collect();
    let ch_data: Vec<Vec<f64>> = channels.values().cloned().collect();

    // Run all samples through neurons
    let max_len = ch_data.iter().map(|c| c.len()).max().unwrap_or(0);
    for t in 0..max_len {
        for (i, data) in ch_data.iter().enumerate() {
            if t < data.len() {
                neurons[i].step(data[t], t);
            }
        }
    }

    let total_spikes: u64 = neurons.iter().map(|n| n.spike_count).sum();
    let active = neurons.iter().filter(|n| n.spike_count > 0).count();
    let firing_rates: Vec<f64> = neurons.iter().map(|n| n.firing_rate(max_len)).collect();
    let isi_cvs: Vec<f64> = neurons.iter().map(|n| n.isi_cv()).collect();
    let avg_burst = neurons.iter().map(|n| n.burstiness()).sum::<f64>() / neurons.len().max(1) as f64;
    let movement = classify_movement(&neurons, max_len);

    let mut anomalies = Vec::new();
    if total_spikes as f64 / max_len.max(1) as f64 > 0.5 {
        anomalies.push("HIGH_SPIKE_RATE: excessive neural activity".into());
    }
    if avg_burst > 0.7 {
        anomalies.push(format!("BURST_PATTERN: burstiness={avg_burst:.2}"));
    }

    let avg_rate = firing_rates.iter().sum::<f64>() / firing_rates.len().max(1) as f64;
    let avg_cv = isi_cvs.iter().sum::<f64>() / isi_cvs.len().max(1) as f64;

    let vector = [
        total_spikes as f64 / 100.0,
        active as f64 / neurons.len().max(1) as f64,
        avg_rate,
        avg_cv,
        avg_burst,
        if movement == "rapid_movement" { 1.0 } else { 0.0 },
        neurons.len() as f64 / 10.0,
        max_len as f64 / 100.0,
    ];

    let _ = store_vector(&vector);

    Ok(SpikingResult {
        neuron_count: neurons.len(),
        total_spikes,
        active_neurons: active,
        firing_rates,
        isi_cvs,
        movement_pattern: movement,
        burstiness: avg_burst,
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

    eprintln!("[cog-spiking-tracker] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-spiking-tracker] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-spiking-tracker] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
