//! Cognitum Cog: Time Series Forecast
//!
//! Predict future sensor values using exponential smoothing (Holt-Winters).
//! Track level, trend, and seasonality. Output forecast with confidence interval.
//!
//! Usage:
//!   cog-time-series-forecast --once
//!   cog-time-series-forecast --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;
const SEASON_LEN: usize = 12; // 12-sample seasonal period

/// Holt-Winters triple exponential smoothing
struct HoltWinters {
    /// Level smoothing parameter
    alpha: f64,
    /// Trend smoothing parameter
    beta: f64,
    /// Seasonal smoothing parameter
    gamma: f64,
    /// Current level
    level: f64,
    /// Current trend
    trend: f64,
    /// Seasonal factors
    season: [f64; SEASON_LEN],
    /// Position in season
    season_idx: usize,
    /// Initialized flag
    initialized: bool,
    /// History for initialization
    init_buf: Vec<f64>,
    /// Residuals for confidence interval
    residuals: Vec<f64>,
}

impl HoltWinters {
    fn new(alpha: f64, beta: f64, gamma: f64) -> Self {
        Self {
            alpha,
            beta,
            gamma,
            level: 0.0,
            trend: 0.0,
            season: [0.0; SEASON_LEN],
            season_idx: 0,
            initialized: false,
            init_buf: Vec::new(),
            residuals: Vec::new(),
        }
    }

    fn initialize(&mut self) {
        let n = self.init_buf.len();
        if n < SEASON_LEN {
            self.level = self.init_buf.iter().sum::<f64>() / n as f64;
            self.trend = 0.0;
            self.initialized = true;
            return;
        }

        // Initial level: mean of first season
        self.level = self.init_buf[..SEASON_LEN].iter().sum::<f64>() / SEASON_LEN as f64;

        // Initial trend: average slope across first two seasons
        if n >= 2 * SEASON_LEN {
            let sum1: f64 = self.init_buf[..SEASON_LEN].iter().sum();
            let sum2: f64 = self.init_buf[SEASON_LEN..2 * SEASON_LEN].iter().sum();
            self.trend = (sum2 - sum1) / (SEASON_LEN * SEASON_LEN) as f64;
        }

        // Initial seasonal factors
        for i in 0..SEASON_LEN {
            if self.level.abs() > 1e-10 {
                self.season[i] = self.init_buf[i] - self.level;
            }
        }

        self.initialized = true;
    }

    /// Update with new observation, returns one-step-ahead forecast
    fn update(&mut self, value: f64) -> f64 {
        if !self.initialized {
            self.init_buf.push(value);
            if self.init_buf.len() >= SEASON_LEN {
                self.initialize();
            }
            return value; // Can't forecast yet
        }

        let s_idx = self.season_idx % SEASON_LEN;
        let old_level = self.level;
        let old_season = self.season[s_idx];

        // Update level
        self.level = self.alpha * (value - old_season)
            + (1.0 - self.alpha) * (old_level + self.trend);

        // Update trend
        self.trend = self.beta * (self.level - old_level)
            + (1.0 - self.beta) * self.trend;

        // Update seasonal
        self.season[s_idx] = self.gamma * (value - self.level)
            + (1.0 - self.gamma) * old_season;

        self.season_idx += 1;

        // One-step forecast
        let next_s = self.season_idx % SEASON_LEN;
        let forecast = self.level + self.trend + self.season[next_s];

        // Track residual
        let residual = value - (old_level + self.trend + old_season);
        self.residuals.push(residual);
        if self.residuals.len() > 100 {
            self.residuals.remove(0);
        }

        forecast
    }

    /// Forecast h steps ahead
    fn forecast(&self, h: usize) -> Vec<f64> {
        (1..=h)
            .map(|i| {
                let s_idx = (self.season_idx + i) % SEASON_LEN;
                self.level + self.trend * i as f64 + self.season[s_idx]
            })
            .collect()
    }

    /// 95% confidence interval width
    fn confidence_interval(&self) -> f64 {
        if self.residuals.len() < 5 {
            return f64::MAX;
        }
        let mean = self.residuals.iter().sum::<f64>() / self.residuals.len() as f64;
        let var = self.residuals.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
            / self.residuals.len() as f64;
        1.96 * var.sqrt() // ~95% CI
    }
}

#[derive(serde::Serialize)]
struct ForecastResult {
    current_value: f64,
    one_step_forecast: f64,
    five_step_forecast: Vec<f64>,
    level: f64,
    trend: f64,
    confidence_interval: f64,
    forecast_error: f64,
    status: String,
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

fn run_once(hw: &mut HoltWinters) -> Result<ForecastResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    // Feed all values through the model
    let mut forecast = 0.0;
    for &v in &values {
        forecast = hw.update(v);
    }

    let current = *values.last().unwrap();
    let error = (current - forecast).abs();
    let five_step = hw.forecast(5);
    let ci = hw.confidence_interval();

    let status = if ci == f64::MAX {
        "initializing"
    } else if ci < 0.5 {
        "stable"
    } else if ci < 1.5 {
        "moderate_uncertainty"
    } else {
        "high_uncertainty"
    };

    let vector = [
        current,
        forecast,
        hw.level,
        hw.trend,
        ci.min(10.0) / 10.0,
        error.min(5.0) / 5.0,
        five_step.get(4).cloned().unwrap_or(0.0),
        values.len() as f64 / 100.0,
    ];

    let _ = store_vector(&vector);

    Ok(ForecastResult {
        current_value: current,
        one_step_forecast: forecast,
        five_step_forecast: five_step,
        level: hw.level,
        trend: hw.trend,
        confidence_interval: ci,
        forecast_error: error,
        status: status.into(),
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

    eprintln!("[cog-time-series-forecast] starting (interval={interval}s, once={once})");

    let mut hw = HoltWinters::new(0.3, 0.1, 0.2);

    loop {
        let start = Instant::now();
        match run_once(&mut hw) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.forecast_error > 2.0 {
                    eprintln!("[cog-time-series-forecast] ALERT: high forecast error ({:.2})", r.forecast_error);
                }
            }
            Err(e) => eprintln!("[cog-time-series-forecast] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
