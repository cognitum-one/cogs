//! Benchmark command implementation
//!
//! Runs performance benchmarks and verifies acceptance criteria.
//!
//! Usage:
//!   agentvm benchmark run --iterations 30 --task <task>
//!   agentvm benchmark analyze <files...>
//!   agentvm benchmark verify <report> --p95-improvement 2.0 --cov-threshold 0.2

use crate::commands::run::EvidenceBundle;
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::{
    format_bytes, format_duration, format_percent, OutputFormat, OutputWriter, ProgressManager,
    TableDisplay,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Benchmark command action
#[derive(Debug, Clone)]
pub enum BenchmarkAction {
    /// Run benchmark suite
    Run {
        /// Number of iterations
        iterations: u32,
        /// Task to benchmark
        task: String,
        /// Workspace path
        workspace: Option<PathBuf>,
        /// Output path for results
        output: Option<PathBuf>,
        /// Warmup iterations
        warmup: Option<u32>,
    },
    /// Analyze benchmark results
    Analyze {
        /// Evidence files to analyze
        files: Vec<PathBuf>,
        /// Output path for report
        output: Option<PathBuf>,
    },
    /// Verify benchmark passes criteria
    Verify {
        /// Benchmark report path
        report: PathBuf,
        /// Required p95 improvement factor
        p95_improvement: f64,
        /// Maximum coefficient of variation
        cov_threshold: f64,
    },
}

/// Benchmark command arguments
#[derive(Debug, Clone)]
pub struct BenchmarkArgs {
    /// The action to perform
    pub action: BenchmarkAction,
    /// Output format
    pub output_format: OutputFormat,
}

/// Benchmark report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// Report version
    pub version: String,
    /// Report generation timestamp
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Task description
    pub task: String,
    /// Number of iterations
    pub iterations: u32,
    /// Warmup iterations
    pub warmup_iterations: u32,
    /// Statistics
    pub stats: BenchmarkStats,
    /// Individual run results
    pub runs: Vec<BenchmarkRun>,
    /// Baseline comparison (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineComparison>,
    /// Pass/fail criteria results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criteria: Option<CriteriaResults>,
}

/// Benchmark statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    /// Mean duration (ms)
    pub mean_ms: f64,
    /// Median duration (ms)
    pub median_ms: f64,
    /// Standard deviation (ms)
    pub std_dev_ms: f64,
    /// Minimum duration (ms)
    pub min_ms: f64,
    /// Maximum duration (ms)
    pub max_ms: f64,
    /// 5th percentile (ms)
    pub p5_ms: f64,
    /// 25th percentile (ms)
    pub p25_ms: f64,
    /// 75th percentile (ms)
    pub p75_ms: f64,
    /// 95th percentile (ms)
    pub p95_ms: f64,
    /// 99th percentile (ms)
    pub p99_ms: f64,
    /// Coefficient of variation
    pub cov: f64,
    /// Success rate
    pub success_rate: f64,
    /// Total network bytes
    pub total_network_bytes: u64,
    /// Total capability calls
    pub total_capability_calls: u64,
}

/// Individual benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    /// Run number
    pub run_number: u32,
    /// Run ID
    pub run_id: String,
    /// Duration (ms)
    pub duration_ms: u64,
    /// Exit code
    pub exit_code: i32,
    /// Success
    pub success: bool,
    /// Capability calls
    pub capability_calls: usize,
    /// Network bytes
    pub network_bytes: u64,
}

/// Baseline comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// Baseline mean (ms)
    pub baseline_mean_ms: f64,
    /// Baseline p95 (ms)
    pub baseline_p95_ms: f64,
    /// Improvement factor (mean)
    pub mean_improvement: f64,
    /// Improvement factor (p95)
    pub p95_improvement: f64,
}

/// Criteria verification results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaResults {
    /// Overall pass/fail
    pub passed: bool,
    /// p95 improvement check
    pub p95_improvement_pass: bool,
    /// p95 improvement value
    pub p95_improvement_value: f64,
    /// p95 improvement required
    pub p95_improvement_required: f64,
    /// CoV check
    pub cov_pass: bool,
    /// CoV value
    pub cov_value: f64,
    /// CoV threshold
    pub cov_threshold: f64,
    /// Success rate check
    pub success_rate_pass: bool,
    /// Success rate value
    pub success_rate_value: f64,
}

impl TableDisplay for BenchmarkReport {
    fn table_headers() -> Vec<String> {
        vec![
            "Metric".to_string(),
            "Value".to_string(),
        ]
    }

    fn table_row(&self) -> Vec<String> {
        vec![
            "Summary".to_string(),
            format!(
                "{} iterations, mean={:.2}ms, p95={:.2}ms, cov={:.2}",
                self.iterations,
                self.stats.mean_ms,
                self.stats.p95_ms,
                self.stats.cov
            ),
        ]
    }

    fn print_text(&self, writer: &OutputWriter) {
        writer.header("Benchmark Report");
        writer.kv("Task", &self.task);
        writer.kv("Generated", self.generated_at.format("%Y-%m-%d %H:%M:%S UTC"));
        writer.kv("Iterations", self.iterations);
        writer.kv("Warmup", self.warmup_iterations);

        writer.header("Statistics");
        writer.kv("Mean", format!("{:.2} ms", self.stats.mean_ms));
        writer.kv("Median", format!("{:.2} ms", self.stats.median_ms));
        writer.kv("Std Dev", format!("{:.2} ms", self.stats.std_dev_ms));
        writer.kv("CoV", format!("{:.4}", self.stats.cov));
        writer.separator();
        writer.kv("Min", format!("{:.2} ms", self.stats.min_ms));
        writer.kv("Max", format!("{:.2} ms", self.stats.max_ms));
        writer.separator();
        writer.kv("P5", format!("{:.2} ms", self.stats.p5_ms));
        writer.kv("P25", format!("{:.2} ms", self.stats.p25_ms));
        writer.kv("P75", format!("{:.2} ms", self.stats.p75_ms));
        writer.kv("P95", format!("{:.2} ms", self.stats.p95_ms));
        writer.kv("P99", format!("{:.2} ms", self.stats.p99_ms));
        writer.separator();
        writer.kv("Success Rate", format_percent(self.stats.success_rate));
        writer.kv("Total Network", format_bytes(self.stats.total_network_bytes));
        writer.kv("Total Capability Calls", self.stats.total_capability_calls);

        if let Some(baseline) = &self.baseline {
            writer.header("Baseline Comparison");
            writer.kv("Baseline Mean", format!("{:.2} ms", baseline.baseline_mean_ms));
            writer.kv("Baseline P95", format!("{:.2} ms", baseline.baseline_p95_ms));
            writer.kv("Mean Improvement", format!("{:.2}x", baseline.mean_improvement));
            writer.kv("P95 Improvement", format!("{:.2}x", baseline.p95_improvement));
        }

        if let Some(criteria) = &self.criteria {
            writer.header("Criteria Verification");
            writer.kv(
                "Overall",
                if criteria.passed { "PASSED" } else { "FAILED" },
            );
            writer.kv(
                "P95 Improvement",
                format!(
                    "{:.2}x (required: {:.2}x) - {}",
                    criteria.p95_improvement_value,
                    criteria.p95_improvement_required,
                    if criteria.p95_improvement_pass { "PASS" } else { "FAIL" }
                ),
            );
            writer.kv(
                "CoV",
                format!(
                    "{:.4} (max: {:.4}) - {}",
                    criteria.cov_value,
                    criteria.cov_threshold,
                    if criteria.cov_pass { "PASS" } else { "FAIL" }
                ),
            );
            writer.kv(
                "Success Rate",
                format!(
                    "{:.1}% - {}",
                    criteria.success_rate_value * 100.0,
                    if criteria.success_rate_pass { "PASS" } else { "FAIL" }
                ),
            );
        }
    }
}

/// Handle benchmark commands
pub async fn handle_benchmark(args: BenchmarkArgs, config: &Config) -> Result<()> {
    let writer = OutputWriter::new(args.output_format, config.general.color);

    match args.action {
        BenchmarkAction::Run {
            iterations,
            task,
            workspace,
            output,
            warmup,
        } => {
            handle_run(
                &writer,
                config,
                iterations,
                &task,
                workspace,
                output,
                warmup.unwrap_or(config.benchmark.warmup_iterations),
            )
            .await
        }
        BenchmarkAction::Analyze { files, output } => {
            handle_analyze(&writer, config, &files, output).await
        }
        BenchmarkAction::Verify {
            report,
            p95_improvement,
            cov_threshold,
        } => handle_verify(&writer, config, &report, p95_improvement, cov_threshold).await,
    }
}

/// Handle benchmark run
async fn handle_run(
    writer: &OutputWriter,
    config: &Config,
    iterations: u32,
    task: &str,
    workspace: Option<PathBuf>,
    output: Option<PathBuf>,
    warmup: u32,
) -> Result<()> {
    let progress = ProgressManager::new();

    let workspace = workspace.unwrap_or_else(|| PathBuf::from("."));
    if !workspace.exists() {
        return Err(CliError::WorkspaceNotFound { path: workspace });
    }

    writer.info(&format!(
        "Running benchmark: {} iterations (+{} warmup)",
        iterations, warmup
    ));
    writer.kv("Task", task);
    writer.kv("Workspace", workspace.display());

    let mut runs = Vec::new();

    // Warmup phase
    if warmup > 0 {
        let bar = progress.bar(warmup as u64, "Warmup phase");
        for i in 0..warmup {
            let _result = run_benchmark_iteration(task, &workspace, i, true).await?;
            bar.inc(1);
        }
        bar.finish_with_message("Warmup complete");
    }

    // Main benchmark phase
    let bar = progress.bar(iterations as u64, "Running benchmark");
    for i in 0..iterations {
        let result = run_benchmark_iteration(task, &workspace, i, false).await?;
        runs.push(result);
        bar.inc(1);
    }
    bar.finish_with_message("Benchmark complete");

    // Calculate statistics
    let stats = calculate_stats(&runs);

    // Create report
    let report = BenchmarkReport {
        version: "1.0".to_string(),
        generated_at: chrono::Utc::now(),
        task: task.to_string(),
        iterations,
        warmup_iterations: warmup,
        stats,
        runs,
        baseline: None,
        criteria: None,
    };

    // Save report
    let output_path = output.unwrap_or_else(|| {
        config.benchmark.output_dir.join(format!(
            "benchmark-{}.json",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ))
    });

    std::fs::create_dir_all(output_path.parent().unwrap_or(&PathBuf::from(".")))?;
    let content = serde_json::to_string_pretty(&report)?;
    std::fs::write(&output_path, content)?;

    writer.output(&report)?;
    writer.success(&format!("Report saved to: {}", output_path.display()));

    Ok(())
}

/// Handle benchmark analyze
async fn handle_analyze(
    writer: &OutputWriter,
    config: &Config,
    files: &[PathBuf],
    output: Option<PathBuf>,
) -> Result<()> {
    let progress = ProgressManager::new();

    if files.is_empty() {
        return Err(CliError::Benchmark("No files specified".to_string()));
    }

    writer.info(&format!("Analyzing {} evidence file(s)", files.len()));

    let spinner = progress.spinner("Loading evidence bundles...");
    let mut runs = Vec::new();

    for (i, file) in files.iter().enumerate() {
        let bundle = load_evidence_bundle(file)?;
        let network_bytes: u64 = bundle
            .execution
            .network_events
            .iter()
            .map(|e| e.bytes)
            .sum();

        runs.push(BenchmarkRun {
            run_number: i as u32,
            run_id: bundle.run_id.clone(),
            duration_ms: bundle.execution.duration_ns / 1_000_000,
            exit_code: bundle.outputs.exit_code,
            success: bundle.outputs.exit_code == 0,
            capability_calls: bundle.execution.capability_calls.len(),
            network_bytes,
        });
    }
    spinner.finish_with_message("Evidence loaded");

    // Calculate statistics
    let stats = calculate_stats(&runs);

    // Create report
    let report = BenchmarkReport {
        version: "1.0".to_string(),
        generated_at: chrono::Utc::now(),
        task: "analyzed".to_string(),
        iterations: runs.len() as u32,
        warmup_iterations: 0,
        stats,
        runs,
        baseline: None,
        criteria: None,
    };

    // Save report if output specified
    if let Some(output_path) = output {
        std::fs::create_dir_all(output_path.parent().unwrap_or(&PathBuf::from(".")))?;
        let content = serde_json::to_string_pretty(&report)?;
        std::fs::write(&output_path, content)?;
        writer.info(&format!("Report saved to: {}", output_path.display()));
    }

    writer.output(&report)?;

    Ok(())
}

/// Handle benchmark verify
async fn handle_verify(
    writer: &OutputWriter,
    _config: &Config,
    report_path: &Path,
    p95_improvement: f64,
    cov_threshold: f64,
) -> Result<()> {
    if !report_path.exists() {
        return Err(CliError::Benchmark(format!(
            "Report not found: {}",
            report_path.display()
        )));
    }

    let content = std::fs::read_to_string(report_path)?;
    let mut report: BenchmarkReport = serde_json::from_str(&content)?;

    writer.info("Verifying benchmark criteria...");

    // Calculate criteria results
    // Note: p95_improvement requires a baseline comparison
    // For now, we assume improvement of 1.0 (no baseline)
    let p95_improvement_value = report
        .baseline
        .as_ref()
        .map(|b| b.p95_improvement)
        .unwrap_or(1.0);

    let p95_improvement_pass = p95_improvement_value >= p95_improvement;
    let cov_pass = report.stats.cov <= cov_threshold;
    let success_rate_pass = report.stats.success_rate >= 0.95; // 95% success rate required

    let passed = p95_improvement_pass && cov_pass && success_rate_pass;

    let criteria = CriteriaResults {
        passed,
        p95_improvement_pass,
        p95_improvement_value,
        p95_improvement_required: p95_improvement,
        cov_pass,
        cov_value: report.stats.cov,
        cov_threshold,
        success_rate_pass,
        success_rate_value: report.stats.success_rate,
    };

    report.criteria = Some(criteria.clone());

    writer.output(&report)?;

    if !passed {
        let mut reasons = Vec::new();
        if !p95_improvement_pass {
            reasons.push(format!(
                "P95 improvement {:.2}x < required {:.2}x",
                p95_improvement_value, p95_improvement
            ));
        }
        if !cov_pass {
            reasons.push(format!(
                "CoV {:.4} > threshold {:.4}",
                report.stats.cov, cov_threshold
            ));
        }
        if !success_rate_pass {
            reasons.push(format!(
                "Success rate {:.1}% < required 95%",
                report.stats.success_rate * 100.0
            ));
        }

        return Err(CliError::BenchmarkCriteriaNotMet {
            criteria: reasons.join("; "),
        });
    }

    writer.success("All benchmark criteria passed");

    Ok(())
}

/// Run a single benchmark iteration
async fn run_benchmark_iteration(
    task: &str,
    workspace: &Path,
    iteration: u32,
    _is_warmup: bool,
) -> Result<BenchmarkRun> {
    let run_id = uuid::Uuid::now_v7().to_string();
    let start = std::time::Instant::now();

    // In a real implementation, this would:
    // 1. Spawn a capsule
    // 2. Execute the task
    // 3. Collect evidence
    // For now, we simulate by running a simple command

    let command_parts: Vec<&str> = task.split_whitespace().collect();
    let output = if command_parts.is_empty() {
        return Err(CliError::Benchmark("Empty task".to_string()));
    } else {
        tokio::process::Command::new(command_parts[0])
            .args(&command_parts[1..])
            .current_dir(workspace)
            .output()
            .await
            .map_err(|e| CliError::Benchmark(format!("Task execution failed: {}", e)))?
    };

    let duration = start.elapsed();

    Ok(BenchmarkRun {
        run_number: iteration,
        run_id,
        duration_ms: duration.as_millis() as u64,
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
        capability_calls: 0,
        network_bytes: 0,
    })
}

/// Load evidence bundle from file
fn load_evidence_bundle(path: &Path) -> Result<EvidenceBundle> {
    let content = std::fs::read_to_string(path)?;
    let bundle: EvidenceBundle = serde_json::from_str(&content)?;
    Ok(bundle)
}

/// Calculate statistics from runs
fn calculate_stats(runs: &[BenchmarkRun]) -> BenchmarkStats {
    if runs.is_empty() {
        return BenchmarkStats {
            mean_ms: 0.0,
            median_ms: 0.0,
            std_dev_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
            p5_ms: 0.0,
            p25_ms: 0.0,
            p75_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            cov: 0.0,
            success_rate: 0.0,
            total_network_bytes: 0,
            total_capability_calls: 0,
        };
    }

    let mut durations: Vec<f64> = runs.iter().map(|r| r.duration_ms as f64).collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = durations.len() as f64;
    let mean = durations.iter().sum::<f64>() / n;

    let variance = durations.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cov = if mean > 0.0 { std_dev / mean } else { 0.0 };

    let min = durations.first().copied().unwrap_or(0.0);
    let max = durations.last().copied().unwrap_or(0.0);
    let median = percentile(&durations, 50.0);
    let p5 = percentile(&durations, 5.0);
    let p25 = percentile(&durations, 25.0);
    let p75 = percentile(&durations, 75.0);
    let p95 = percentile(&durations, 95.0);
    let p99 = percentile(&durations, 99.0);

    let successful = runs.iter().filter(|r| r.success).count();
    let success_rate = successful as f64 / runs.len() as f64;

    let total_network_bytes: u64 = runs.iter().map(|r| r.network_bytes).sum();
    let total_capability_calls: u64 = runs.iter().map(|r| r.capability_calls as u64).sum();

    BenchmarkStats {
        mean_ms: mean,
        median_ms: median,
        std_dev_ms: std_dev,
        min_ms: min,
        max_ms: max,
        p5_ms: p5,
        p25_ms: p25,
        p75_ms: p75,
        p95_ms: p95,
        p99_ms: p99,
        cov,
        success_rate,
        total_network_bytes,
        total_capability_calls,
    }
}

/// Calculate percentile from sorted data
fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let rank = (p / 100.0) * (sorted_data.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let fraction = rank - lower as f64;

    if upper >= sorted_data.len() {
        sorted_data[sorted_data.len() - 1]
    } else if lower == upper {
        sorted_data[lower]
    } else {
        sorted_data[lower] * (1.0 - fraction) + sorted_data[upper] * fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert!((percentile(&data, 50.0) - 5.5).abs() < 0.01);
        assert!((percentile(&data, 0.0) - 1.0).abs() < 0.01);
        assert!((percentile(&data, 100.0) - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_stats() {
        let runs = vec![
            BenchmarkRun {
                run_number: 0,
                run_id: "a".to_string(),
                duration_ms: 100,
                exit_code: 0,
                success: true,
                capability_calls: 5,
                network_bytes: 1024,
            },
            BenchmarkRun {
                run_number: 1,
                run_id: "b".to_string(),
                duration_ms: 150,
                exit_code: 0,
                success: true,
                capability_calls: 6,
                network_bytes: 2048,
            },
            BenchmarkRun {
                run_number: 2,
                run_id: "c".to_string(),
                duration_ms: 120,
                exit_code: 1,
                success: false,
                capability_calls: 4,
                network_bytes: 512,
            },
        ];

        let stats = calculate_stats(&runs);
        assert!((stats.mean_ms - 123.33).abs() < 1.0);
        assert!((stats.success_rate - 0.666).abs() < 0.01);
        assert_eq!(stats.total_network_bytes, 3584);
        assert_eq!(stats.total_capability_calls, 15);
    }

    #[test]
    fn test_empty_stats() {
        let stats = calculate_stats(&[]);
        assert_eq!(stats.mean_ms, 0.0);
        assert_eq!(stats.success_rate, 0.0);
    }
}
