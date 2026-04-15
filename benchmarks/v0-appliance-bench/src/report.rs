//! Reporting, Regression Detection, and CI Integration
//!
//! Produces:
//!   - Real-time dashboard output (line-by-line streaming)
//!   - Final HTML report with embedded charts
//!   - Final JSON report for machine consumption
//!   - Regression detection comparing two JSON reports
//!   - CI exit code (0 = pass, 1 = fail)

use std::time::{Duration, Instant};

use crate::coherence::GateOutput;
use crate::fault::FaultEvent;
use crate::metrics::MetricsSnapshot;

// ── Benchmark Report ───────────────────────────────────────────────

/// Complete output of a single benchmark run.
#[derive(Clone, Debug)]
pub struct BenchmarkReport {
    /// Human-readable label.
    pub label: String,
    /// When the run started.
    pub started_at: Instant,
    /// Total wall-clock duration.
    pub total_duration: Duration,
    /// Final metrics snapshot.
    pub final_snapshot: MetricsSnapshot,
    /// Time-series of periodic snapshots (elapsed, snapshot).
    pub time_series: Vec<(Duration, MetricsSnapshot)>,
    /// Fault injection events that occurred.
    pub fault_events: Vec<FaultEvent>,
    /// Coherence gate output history.
    pub gate_history: Vec<GateOutput>,
}

impl BenchmarkReport {
    /// Serialize to JSON for storage and regression comparison.
    pub fn to_json(&self) -> String {
        // In a real implementation, derive Serialize and use serde_json.
        // Here we produce a structured JSON string manually.
        let snap = &self.final_snapshot;
        format!(
            r#"{{
  "label": "{}",
  "total_duration_secs": {:.3},
  "total_ticks": {},
  "tick_latency": {{
    "count": {},
    "mean_us": {:.2},
    "p50_us": {:.2},
    "p95_us": {:.2},
    "p99_us": {:.2},
    "max_us": {:.2}
  }},
  "write_allowed_pct": {:.4},
  "coherence_transitions": {},
  "total_protocol_errors": {},
  "db_query_latency": {{
    "count": {},
    "mean_us": {:.2},
    "p50_us": {:.2},
    "p95_us": {:.2},
    "p99_us": {:.2},
    "max_us": {:.2}
  }},
  "transport": {{
    "msg_sent_rate": {:.1},
    "msg_recv_rate": {:.1},
    "backpressure_events": {}
  }},
  "tiles_alive": {},
  "tiles_total": {},
  "fault_events_count": {},
  "time_series_points": {},
  "acceptance": {{
    "tick_latency_p95_ok": {},
    "zero_protocol_errors": {},
    "recovery_times_ok": {},
    "all_tiles_alive": {}
  }}
}}"#,
            self.label,
            self.total_duration.as_secs_f64(),
            snap.total_ticks,
            snap.tick_latency.count,
            snap.tick_latency.mean_us,
            snap.tick_latency.p50_us,
            snap.tick_latency.p95_us,
            snap.tick_latency.p99_us,
            snap.tick_latency.max_us,
            snap.write_allowed_pct,
            snap.coherence_transitions,
            snap.total_protocol_errors,
            snap.db_query_latency.count,
            snap.db_query_latency.mean_us,
            snap.db_query_latency.p50_us,
            snap.db_query_latency.p95_us,
            snap.db_query_latency.p99_us,
            snap.db_query_latency.max_us,
            snap.transport_msg_sent_rate,
            snap.transport_msg_recv_rate,
            snap.transport_backpressure,
            snap.tiles_alive,
            snap.tiles_total,
            self.fault_events.len(),
            self.time_series.len(),
            snap.tick_latency_ok(),
            snap.zero_protocol_errors(),
            snap.recovery_times_ok(),
            snap.all_tiles_alive(),
        )
    }

    /// Produce a one-line dashboard summary suitable for periodic printing.
    pub fn dashboard_line(&self) -> String {
        let snap = &self.final_snapshot;
        format!(
            "t={:.0}s | ticks={} | p95={:.0}us | p99={:.0}us | max={:.0}us | write%={:.1} | gate_tx={} | err={} | tiles={}/{}",
            self.total_duration.as_secs_f64(),
            snap.total_ticks,
            snap.tick_latency.p95_us,
            snap.tick_latency.p99_us,
            snap.tick_latency.max_us,
            snap.write_allowed_pct * 100.0,
            snap.coherence_transitions,
            snap.total_protocol_errors,
            snap.tiles_alive,
            snap.tiles_total,
        )
    }

    /// Generate a full HTML report with embedded SVG charts.
    /// Returns the HTML string.
    pub fn to_html(&self) -> String {
        let snap = &self.final_snapshot;
        let pass_class = |ok: bool| if ok { "pass" } else { "fail" };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Cognitum v0 Benchmark Report -- {label}</title>
<style>
  body {{ font-family: monospace; margin: 2em; background: #1a1a2e; color: #e0e0e0; }}
  h1 {{ color: #00d4ff; }}
  table {{ border-collapse: collapse; margin: 1em 0; }}
  th, td {{ border: 1px solid #333; padding: 6px 12px; text-align: right; }}
  th {{ background: #16213e; }}
  .pass {{ color: #00ff88; font-weight: bold; }}
  .fail {{ color: #ff4444; font-weight: bold; }}
  .section {{ margin: 2em 0; }}
  svg {{ background: #0f0f23; border: 1px solid #333; }}
</style>
</head>
<body>
<h1>Cognitum v0 Appliance Emulator -- Benchmark Report</h1>
<p>Label: <b>{label}</b> | Duration: {dur:.1}s | Ticks: {ticks}</p>

<div class="section">
<h2>Acceptance Criteria</h2>
<table>
<tr><th>Criterion</th><th>Target</th><th>Actual</th><th>Status</th></tr>
<tr>
  <td>Tick latency p95</td><td>&lt; 1000 us</td>
  <td>{p95:.1} us</td>
  <td class="{p95c}">{p95s}</td>
</tr>
<tr>
  <td>Protocol errors</td><td>0</td>
  <td>{errs}</td>
  <td class="{errc}">{errs_s}</td>
</tr>
<tr>
  <td>Recovery times</td><td>&lt; 2000 ms</td>
  <td>see below</td>
  <td class="{recc}">{recs}</td>
</tr>
<tr>
  <td>Tiles alive</td><td>{ttotal}</td>
  <td>{talive}</td>
  <td class="{tilec}">{tiles}</td>
</tr>
</table>
</div>

<div class="section">
<h2>Tick Latency Distribution</h2>
<table>
<tr><th>Percentile</th><th>Latency (us)</th></tr>
<tr><td>p50</td><td>{p50:.1}</td></tr>
<tr><td>p95</td><td>{p95:.1}</td></tr>
<tr><td>p99</td><td>{p99:.1}</td></tr>
<tr><td>max</td><td>{pmax:.1}</td></tr>
<tr><td>mean</td><td>{pmean:.1}</td></tr>
</table>
</div>

<div class="section">
<h2>Coherence Gate</h2>
<p>Write-allowed: {wpct:.1}% | Transitions: {ctx}</p>
</div>

<div class="section">
<h2>Transport</h2>
<p>Sent: {msent:.0} msg/s | Recv: {mrecv:.0} msg/s | Backpressure: {bp}</p>
</div>

<div class="section">
<h2>Database</h2>
<table>
<tr><th>Metric</th><th>Value</th></tr>
<tr><td>Queries</td><td>{dbcount}</td></tr>
<tr><td>p50</td><td>{dbp50:.1} us</td></tr>
<tr><td>p95</td><td>{dbp95:.1} us</td></tr>
<tr><td>p99</td><td>{dbp99:.1} us</td></tr>
</table>
</div>

<div class="section">
<h2>Fault Injection Events</h2>
<p>{faults} events injected during this run.</p>
</div>

<div class="section">
<h2>Time Series ({ts_points} data points)</h2>
<p><i>Embed SVG sparklines here in production build.</i></p>
</div>

</body>
</html>"#,
            label = self.label,
            dur = self.total_duration.as_secs_f64(),
            ticks = snap.total_ticks,
            p95 = snap.tick_latency.p95_us,
            p95c = pass_class(snap.tick_latency_ok()),
            p95s = if snap.tick_latency_ok() { "PASS" } else { "FAIL" },
            errs = snap.total_protocol_errors,
            errc = pass_class(snap.zero_protocol_errors()),
            errs_s = if snap.zero_protocol_errors() { "PASS" } else { "FAIL" },
            recc = pass_class(snap.recovery_times_ok()),
            recs = if snap.recovery_times_ok() { "PASS" } else { "FAIL" },
            ttotal = snap.tiles_total,
            talive = snap.tiles_alive,
            tilec = pass_class(snap.all_tiles_alive()),
            tiles = if snap.all_tiles_alive() { "PASS" } else { "FAIL" },
            p50 = snap.tick_latency.p50_us,
            p99 = snap.tick_latency.p99_us,
            pmax = snap.tick_latency.max_us,
            pmean = snap.tick_latency.mean_us,
            wpct = snap.write_allowed_pct * 100.0,
            ctx = snap.coherence_transitions,
            msent = snap.transport_msg_sent_rate,
            mrecv = snap.transport_msg_recv_rate,
            bp = snap.transport_backpressure,
            dbcount = snap.db_query_latency.count,
            dbp50 = snap.db_query_latency.p50_us,
            dbp95 = snap.db_query_latency.p95_us,
            dbp99 = snap.db_query_latency.p99_us,
            faults = self.fault_events.len(),
            ts_points = self.time_series.len(),
        )
    }
}

// ── Regression Detection ───────────────────────────────────────────

/// Compare two benchmark reports and detect regressions.
#[derive(Clone, Debug)]
pub struct RegressionComparison {
    pub baseline_label: String,
    pub current_label: String,
    pub checks: Vec<RegressionCheck>,
}

#[derive(Clone, Debug)]
pub struct RegressionCheck {
    pub metric: String,
    pub baseline_value: f64,
    pub current_value: f64,
    /// Positive = improvement, negative = regression.
    pub delta_pct: f64,
    /// Whether this is a regression beyond the tolerance.
    pub is_regression: bool,
}

impl RegressionComparison {
    /// Compare two snapshots with a given tolerance (e.g., 0.05 for 5%).
    pub fn compare(
        baseline: &BenchmarkReport,
        current: &BenchmarkReport,
        tolerance_pct: f64,
    ) -> Self {
        let b = &baseline.final_snapshot;
        let c = &current.final_snapshot;
        let mut checks = vec![];

        // Tick latency p95 (lower is better).
        let delta_p95 = if b.tick_latency.p95_us > 0.0 {
            (b.tick_latency.p95_us - c.tick_latency.p95_us) / b.tick_latency.p95_us
        } else {
            0.0
        };
        checks.push(RegressionCheck {
            metric: "tick_latency_p95_us".into(),
            baseline_value: b.tick_latency.p95_us,
            current_value: c.tick_latency.p95_us,
            delta_pct: delta_p95 * 100.0,
            is_regression: delta_p95 < -tolerance_pct,
        });

        // Tick latency p99 (lower is better).
        let delta_p99 = if b.tick_latency.p99_us > 0.0 {
            (b.tick_latency.p99_us - c.tick_latency.p99_us) / b.tick_latency.p99_us
        } else {
            0.0
        };
        checks.push(RegressionCheck {
            metric: "tick_latency_p99_us".into(),
            baseline_value: b.tick_latency.p99_us,
            current_value: c.tick_latency.p99_us,
            delta_pct: delta_p99 * 100.0,
            is_regression: delta_p99 < -tolerance_pct,
        });

        // Protocol errors (fewer is better).
        checks.push(RegressionCheck {
            metric: "protocol_errors".into(),
            baseline_value: b.total_protocol_errors as f64,
            current_value: c.total_protocol_errors as f64,
            delta_pct: 0.0,
            is_regression: c.total_protocol_errors > b.total_protocol_errors,
        });

        // Write-allowed percentage (higher is better during normal ops).
        let delta_wpct = if b.write_allowed_pct > 0.0 {
            (c.write_allowed_pct - b.write_allowed_pct) / b.write_allowed_pct
        } else {
            0.0
        };
        checks.push(RegressionCheck {
            metric: "write_allowed_pct".into(),
            baseline_value: b.write_allowed_pct,
            current_value: c.write_allowed_pct,
            delta_pct: delta_wpct * 100.0,
            is_regression: delta_wpct < -tolerance_pct,
        });

        // DB query latency p95 (lower is better).
        let delta_db = if b.db_query_latency.p95_us > 0.0 {
            (b.db_query_latency.p95_us - c.db_query_latency.p95_us) / b.db_query_latency.p95_us
        } else {
            0.0
        };
        checks.push(RegressionCheck {
            metric: "db_query_latency_p95_us".into(),
            baseline_value: b.db_query_latency.p95_us,
            current_value: c.db_query_latency.p95_us,
            delta_pct: delta_db * 100.0,
            is_regression: delta_db < -tolerance_pct,
        });

        Self {
            baseline_label: baseline.label.clone(),
            current_label: current.label.clone(),
            checks,
        }
    }

    pub fn has_regressions(&self) -> bool {
        self.checks.iter().any(|c| c.is_regression)
    }

    /// Return CI exit code: 0 = pass, 1 = regression detected.
    pub fn exit_code(&self) -> i32 {
        if self.has_regressions() { 1 } else { 0 }
    }

    pub fn summary(&self) -> String {
        let mut lines = vec![format!(
            "Regression comparison: {} (baseline) vs {} (current)",
            self.baseline_label, self.current_label,
        )];
        for check in &self.checks {
            let arrow = if check.delta_pct > 0.0 { "+" } else { "" };
            let status = if check.is_regression { "REGRESS" } else { "ok" };
            lines.push(format!(
                "  [{:>7}] {:30} baseline={:>10.2}  current={:>10.2}  delta={}{:.1}%",
                status, check.metric,
                check.baseline_value, check.current_value,
                arrow, check.delta_pct,
            ));
        }
        if self.has_regressions() {
            lines.push("RESULT: REGRESSION DETECTED".into());
        } else {
            lines.push("RESULT: PASS (no regressions)".into());
        }
        lines.join("\n")
    }
}

// ── CI Integration ─────────────────────────────────────────────────

/// Write the JSON report to a file path (for CI artifact upload).
pub fn write_json_report(report: &BenchmarkReport, path: &str) {
    // pseudo: std::fs::write(path, report.to_json()).expect("write report");
    let _ = (report, path); // placeholder
}

/// Write the HTML report to a file path.
pub fn write_html_report(report: &BenchmarkReport, path: &str) {
    // pseudo: std::fs::write(path, report.to_html()).expect("write report");
    let _ = (report, path); // placeholder
}

/// Load a JSON report from a file (for regression baseline).
pub fn load_json_report(_path: &str) -> Option<BenchmarkReport> {
    // pseudo: parse JSON -> BenchmarkReport
    None // placeholder
}
