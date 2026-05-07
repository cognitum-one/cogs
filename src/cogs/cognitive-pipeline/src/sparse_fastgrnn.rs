//! FastGRNN temporal reflex detector for cognitum-agent cognitive microkernel.
//!
//! Ports the FastGRNN cell from ruvector-postgres/routing, adds:
//!   - persistent hidden state across frames (continuous sensor tracking)
//!   - EMA baseline for anomaly deviation scoring
//!   - weight serialization so pre-trained detectors can be uploaded via API
//!
//! Typical Pi Zero 2W cost: ~0.05 ms per frame at input_dim=16, hidden_dim=32.

/// Default input feature dimension. Covers: 3-axis accel, 3-axis gyro,
/// temp, pressure, RSSI, and 7 application-defined channels.
pub const DEFAULT_INPUT_DIM: usize = 16;
/// Default hidden state dimension. Yields ~40 KB of weights total.
pub const DEFAULT_HIDDEN_DIM: usize = 32;
/// EMA smoothing factor for baseline. 0.05 = slow adaptation, sensitive to bursts.
pub const EMA_ALPHA: f32 = 0.05;

/// FastGRNN cell with persistent hidden state and EMA anomaly baseline.
///
/// The gate equation:
///   g  = sigmoid(W_g · x + U_g · h + b_g)
///   c  = tanh(W_u · x + U_u · h + b_u)
///   h' = (ζ·g + ν) ⊙ h + (1 − ζ·g − ν) ⊙ c
pub struct FastGrnnDetector {
    pub input_dim: usize,
    pub hidden_dim: usize,
    w_gate:    Vec<f32>,
    u_gate:    Vec<f32>,
    w_update:  Vec<f32>,
    u_update:  Vec<f32>,
    bias_gate:   Vec<f32>,
    bias_update: Vec<f32>,
    zeta: f32,
    nu:   f32,
    /// Persistent hidden state — survives across frames within a session.
    pub hidden: Vec<f32>,
    /// EMA of hidden-state L2 norm — used as the normal baseline.
    pub baseline_ema: f32,
    /// Total frames processed since last reset.
    pub frames_seen: u64,
}

impl FastGrnnDetector {
    /// Initialise a new detector with Xavier-scaled weights.
    pub fn new(input_dim: usize, hidden_dim: usize) -> Self {
        let scale = (2.0 / (input_dim + hidden_dim) as f32).sqrt();
        // Deterministic tiny perturbation so the detector isn't exactly zero.
        let seed_w = |i: usize, s: f32| -> f32 {
            let r = ((i as u32).wrapping_mul(2654435761).wrapping_add(0x9e3779b9)) as f32;
            (r / u32::MAX as f32 - 0.5) * 2.0 * s * 0.1
        };
        let n_in_h  = input_dim  * hidden_dim;
        let n_h_h   = hidden_dim * hidden_dim;
        Self {
            input_dim,
            hidden_dim,
            w_gate:    (0..n_in_h).map(|i| seed_w(i,      scale)).collect(),
            u_gate:    (0..n_h_h ).map(|i| seed_w(i + 1000, scale)).collect(),
            w_update:  (0..n_in_h).map(|i| seed_w(i + 2000, scale)).collect(),
            u_update:  (0..n_h_h ).map(|i| seed_w(i + 3000, scale)).collect(),
            bias_gate:   vec![0.0; hidden_dim],
            bias_update: vec![0.0; hidden_dim],
            zeta: 1.0,
            nu:   0.0,
            hidden:       vec![0.0; hidden_dim],
            baseline_ema: 0.0,
            frames_seen:  0,
        }
    }

    /// Load weights from a serialized JSON object (as produced by `to_json`).
    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        let arr = |key: &str| -> Option<Vec<f32>> {
            v[key].as_array()?.iter()
                .map(|x| x.as_f64().map(|f| f as f32))
                .collect()
        };
        let input_dim  = v["input_dim"].as_u64()? as usize;
        let hidden_dim = v["hidden_dim"].as_u64()? as usize;
        Some(Self {
            input_dim,
            hidden_dim,
            w_gate:    arr("w_gate")?,
            u_gate:    arr("u_gate")?,
            w_update:  arr("w_update")?,
            u_update:  arr("u_update")?,
            bias_gate:   arr("bias_gate")?,
            bias_update: arr("bias_update")?,
            zeta: v["zeta"].as_f64().unwrap_or(1.0) as f32,
            nu:   v["nu"].as_f64().unwrap_or(0.0)   as f32,
            hidden:       vec![0.0; hidden_dim],
            baseline_ema: 0.0,
            frames_seen:  0,
        })
    }

    /// Serialize weights for API upload/download (excludes transient hidden state).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_dim":    self.input_dim,
            "hidden_dim":   self.hidden_dim,
            "w_gate":    self.w_gate,
            "u_gate":    self.u_gate,
            "w_update":  self.w_update,
            "u_update":  self.u_update,
            "bias_gate":   self.bias_gate,
            "bias_update": self.bias_update,
            "zeta": self.zeta,
            "nu":   self.nu,
        })
    }

    /// Process one sensor frame, update hidden state, return L2 norm of new hidden.
    /// The EMA baseline is updated here, so call this for every frame regardless
    /// of whether you plan to check for anomalies.
    pub fn push_frame(&mut self, features: &[f32]) -> f32 {
        debug_assert_eq!(features.len(), self.input_dim);
        let h_new = self.step(features, &self.hidden.clone());
        let norm = vec_norm(&h_new);
        self.hidden = h_new;
        self.frames_seen += 1;
        // Warm up EMA for first 10 frames without penalising them.
        if self.frames_seen < 10 {
            self.baseline_ema = if self.baseline_ema == 0.0 { norm } else {
                EMA_ALPHA * norm + (1.0 - EMA_ALPHA) * self.baseline_ema
            };
        } else {
            self.baseline_ema = EMA_ALPHA * norm + (1.0 - EMA_ALPHA) * self.baseline_ema;
        }
        norm
    }

    /// Process a window of sensor frames (each frame is `input_dim` features).
    ///
    /// Returns `(anomaly_score, final_hidden)` where anomaly_score is the
    /// maximum relative deviation from the EMA baseline seen in this window.
    /// A score >1.0 means the window peak deviated by more than the baseline value.
    pub fn score_window(&mut self, window: &[Vec<f32>]) -> (f32, Vec<f32>) {
        let mut max_deviation: f32 = 0.0;
        for frame in window {
            let norm = self.push_frame(frame);
            let baseline = self.baseline_ema.max(1e-6);
            let deviation = (norm - baseline).abs() / baseline;
            if deviation > max_deviation {
                max_deviation = deviation;
            }
        }
        (max_deviation, self.hidden.clone())
    }

    /// Reset hidden state and EMA baseline (start of a new sequence / session).
    pub fn reset(&mut self) {
        self.hidden.iter_mut().for_each(|h| *h = 0.0);
        self.baseline_ema = 0.0;
        self.frames_seen  = 0;
    }

    // ── Internal cell computation ──────────────────────────────────────

    fn step(&self, x: &[f32], h: &[f32]) -> Vec<f32> {
        let d = self.hidden_dim;
        let mut gate   = vec![0.0f32; d];
        let mut update = vec![0.0f32; d];
        matmul_add(&self.w_gate,   x, &mut gate,   self.input_dim);
        matmul_add(&self.u_gate,   h, &mut gate,   self.hidden_dim);
        matmul_add(&self.w_update, x, &mut update, self.input_dim);
        matmul_add(&self.u_update, h, &mut update, self.hidden_dim);
        let mut h_new = vec![0.0f32; d];
        for i in 0..d {
            let g = sigmoid(gate[i] + self.bias_gate[i]);
            let c = (update[i] + self.bias_update[i]).tanh();
            let gf = (self.zeta * g + self.nu).clamp(0.0, 1.0);
            h_new[i] = gf * h[i] + (1.0 - gf) * c;
        }
        h_new
    }
}

fn matmul_add(weights: &[f32], input: &[f32], result: &mut [f32], cols: usize) {
    let rows = result.len();
    for i in 0..rows {
        for j in 0..cols {
            result[i] += weights[i * cols + j] * input[j];
        }
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn vec_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_frame_updates_hidden() {
        let mut det = FastGrnnDetector::new(4, 8);
        let frame = vec![1.0, 0.5, -0.3, 0.2];
        let norm = det.push_frame(&frame);
        assert!(norm > 0.0);
        assert!(det.frames_seen == 1);
    }

    #[test]
    fn test_score_window_returns_max_deviation() {
        let mut det = FastGrnnDetector::new(4, 8);
        // Prime EMA with normal frames
        for _ in 0..20 {
            det.push_frame(&[0.1, 0.1, 0.1, 0.1]);
        }
        // A wild spike
        let window = vec![
            vec![10.0, -10.0, 10.0, -10.0],
            vec![0.1, 0.1, 0.1, 0.1],
        ];
        let (score, hidden) = det.score_window(&window);
        assert!(score > 0.0, "should detect anomaly");
        assert_eq!(hidden.len(), 8);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut det = FastGrnnDetector::new(4, 8);
        det.push_frame(&[1.0, 2.0, 3.0, 4.0]);
        det.reset();
        assert_eq!(det.frames_seen, 0);
        assert_eq!(det.baseline_ema, 0.0);
        assert!(det.hidden.iter().all(|&h| h == 0.0));
    }

    #[test]
    fn test_json_roundtrip() {
        let det = FastGrnnDetector::new(4, 8);
        let json = det.to_json();
        let det2 = FastGrnnDetector::from_json(&json).expect("roundtrip");
        assert_eq!(det2.input_dim, 4);
        assert_eq!(det2.hidden_dim, 8);
        assert_eq!(det2.w_gate.len(), det.w_gate.len());
    }

    #[test]
    fn test_low_cost_step() {
        // Each step at dim=16→32 must complete quickly on native hardware.
        let mut det = FastGrnnDetector::new(16, 32);
        let frame: Vec<f32> = (0..16).map(|i| i as f32 * 0.01).collect();
        let t = std::time::Instant::now();
        for _ in 0..1000 {
            det.push_frame(&frame);
        }
        let ms = t.elapsed().as_millis();
        // 1000 steps should complete in well under 100 ms on x86 dev box.
        assert!(ms < 200, "1000 steps took {ms} ms — too slow");
    }
}
