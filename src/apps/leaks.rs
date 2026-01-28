//! Memory leak detection
//!
//! Monitors processes over time to detect potential memory leaks:
//! - Tracks memory usage history
//! - Detects consistent memory growth
//! - Identifies processes with abnormal memory patterns
//! - Provides recommendations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, ProcessesToUpdate, Pid};

/// Memory sample for a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySample {
    pub timestamp: u64,
    pub memory_mb: f64,
    pub cpu_percent: f32,
}

/// Process memory history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessHistory {
    pub pid: u32,
    pub name: String,
    pub samples: Vec<MemorySample>,
    pub start_memory_mb: f64,
    pub current_memory_mb: f64,
    pub peak_memory_mb: f64,
    pub growth_rate_mb_per_hour: f64,
    pub is_likely_leak: bool,
    pub confidence: f64,
}

impl ProcessHistory {
    /// Create new process history
    pub fn new(pid: u32, name: String, initial_memory_mb: f64) -> Self {
        Self {
            pid,
            name,
            samples: vec![MemorySample {
                timestamp: current_timestamp(),
                memory_mb: initial_memory_mb,
                cpu_percent: 0.0,
            }],
            start_memory_mb: initial_memory_mb,
            current_memory_mb: initial_memory_mb,
            peak_memory_mb: initial_memory_mb,
            growth_rate_mb_per_hour: 0.0,
            is_likely_leak: false,
            confidence: 0.0,
        }
    }

    /// Add a memory sample
    pub fn add_sample(&mut self, memory_mb: f64, cpu_percent: f32) {
        let sample = MemorySample {
            timestamp: current_timestamp(),
            memory_mb,
            cpu_percent,
        };

        self.samples.push(sample);
        self.current_memory_mb = memory_mb;

        if memory_mb > self.peak_memory_mb {
            self.peak_memory_mb = memory_mb;
        }

        // Keep only last 100 samples
        if self.samples.len() > 100 {
            self.samples.remove(0);
        }

        self.analyze();
    }

    /// Analyze memory pattern for leaks
    fn analyze(&mut self) {
        if self.samples.len() < 5 {
            return;
        }

        // Calculate growth rate using linear regression
        let n = self.samples.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;

        let base_time = self.samples[0].timestamp;

        for sample in &self.samples {
            let x = (sample.timestamp - base_time) as f64 / 3600.0; // Hours
            let y = sample.memory_mb;

            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        self.growth_rate_mb_per_hour = slope;

        // Calculate R-squared for confidence
        let mean_y = sum_y / n;
        let mut ss_tot = 0.0;
        let mut ss_res = 0.0;

        for sample in &self.samples {
            let x = (sample.timestamp - base_time) as f64 / 3600.0;
            let y = sample.memory_mb;
            let predicted = self.samples[0].memory_mb + slope * x;

            ss_tot += (y - mean_y).powi(2);
            ss_res += (y - predicted).powi(2);
        }

        let r_squared = if ss_tot > 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        self.confidence = r_squared.max(0.0).min(1.0);

        // Determine if likely leak
        // Criteria: consistent growth (high R²), significant rate, not just startup
        let memory_doubled = self.current_memory_mb > self.start_memory_mb * 2.0;
        let significant_growth = self.growth_rate_mb_per_hour > 10.0; // >10 MB/hour
        let consistent = self.confidence > 0.7;
        let enough_samples = self.samples.len() >= 10;

        self.is_likely_leak = (memory_doubled || significant_growth) && consistent && enough_samples;
    }

    /// Get memory growth percentage
    pub fn growth_percent(&self) -> f64 {
        if self.start_memory_mb > 0.0 {
            ((self.current_memory_mb - self.start_memory_mb) / self.start_memory_mb) * 100.0
        } else {
            0.0
        }
    }

    /// Get severity level (0-3)
    pub fn severity(&self) -> u8 {
        if !self.is_likely_leak {
            return 0;
        }

        if self.growth_rate_mb_per_hour > 100.0 || self.growth_percent() > 500.0 {
            3 // Critical
        } else if self.growth_rate_mb_per_hour > 50.0 || self.growth_percent() > 200.0 {
            2 // High
        } else {
            1 // Medium
        }
    }
}

/// Leak detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakReport {
    pub process_name: String,
    pub pid: u32,
    pub current_memory_mb: f64,
    pub start_memory_mb: f64,
    pub growth_rate_mb_per_hour: f64,
    pub growth_percent: f64,
    pub confidence: f64,
    pub severity: u8,
    pub recommendation: String,
}

/// Memory leak detector
pub struct LeakDetector {
    system: System,
    process_history: HashMap<u32, ProcessHistory>,
    monitoring_duration_secs: u64,
    sample_interval_secs: u64,
    last_sample: std::time::Instant,
}

impl LeakDetector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);

        Self {
            system,
            process_history: HashMap::new(),
            monitoring_duration_secs: 0,
            sample_interval_secs: 30,
            last_sample: std::time::Instant::now(),
        }
    }

    /// Set sample interval
    pub fn set_sample_interval(&mut self, secs: u64) {
        self.sample_interval_secs = secs;
    }

    /// Take a sample of all processes
    pub fn sample(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);

        let mut seen_pids = Vec::new();

        for (pid, process) in self.system.processes() {
            let pid_u32 = pid.as_u32();
            let name = process.name().to_string_lossy().to_string();
            let memory_mb = process.memory() as f64 / (1024.0 * 1024.0);
            let cpu_percent = process.cpu_usage();

            seen_pids.push(pid_u32);

            if let Some(history) = self.process_history.get_mut(&pid_u32) {
                history.add_sample(memory_mb, cpu_percent);
            } else {
                // Only track processes using >50MB
                if memory_mb > 50.0 {
                    self.process_history.insert(
                        pid_u32,
                        ProcessHistory::new(pid_u32, name, memory_mb),
                    );
                }
            }
        }

        // Remove dead processes
        self.process_history.retain(|pid, _| seen_pids.contains(pid));

        self.last_sample = std::time::Instant::now();
        self.monitoring_duration_secs += self.sample_interval_secs;
    }

    /// Check if enough time has passed for next sample
    pub fn should_sample(&self) -> bool {
        self.last_sample.elapsed().as_secs() >= self.sample_interval_secs
    }

    /// Get processes with likely memory leaks
    pub fn get_leaks(&self) -> Vec<LeakReport> {
        let mut leaks: Vec<LeakReport> = self
            .process_history
            .values()
            .filter(|h| h.is_likely_leak)
            .map(|h| LeakReport {
                process_name: h.name.clone(),
                pid: h.pid,
                current_memory_mb: h.current_memory_mb,
                start_memory_mb: h.start_memory_mb,
                growth_rate_mb_per_hour: h.growth_rate_mb_per_hour,
                growth_percent: h.growth_percent(),
                confidence: h.confidence,
                severity: h.severity(),
                recommendation: self.get_recommendation(h),
            })
            .collect();

        // Sort by severity (highest first)
        leaks.sort_by(|a, b| b.severity.cmp(&a.severity));

        leaks
    }

    /// Get all monitored processes sorted by memory growth
    pub fn get_all_monitored(&self) -> Vec<&ProcessHistory> {
        let mut procs: Vec<_> = self.process_history.values().collect();
        procs.sort_by(|a, b| {
            b.growth_rate_mb_per_hour
                .partial_cmp(&a.growth_rate_mb_per_hour)
                .unwrap()
        });
        procs
    }

    /// Get top memory growing processes
    pub fn get_top_growing(&self, count: usize) -> Vec<&ProcessHistory> {
        let mut procs: Vec<_> = self
            .process_history
            .values()
            .filter(|h| h.samples.len() >= 3 && h.growth_rate_mb_per_hour > 0.0)
            .collect();

        procs.sort_by(|a, b| {
            b.growth_rate_mb_per_hour
                .partial_cmp(&a.growth_rate_mb_per_hour)
                .unwrap()
        });

        procs.into_iter().take(count).collect()
    }

    /// Get recommendation for a process
    fn get_recommendation(&self, history: &ProcessHistory) -> String {
        match history.severity() {
            3 => format!(
                "CRITICAL: {} is growing at {:.0} MB/hour. Restart immediately!",
                history.name, history.growth_rate_mb_per_hour
            ),
            2 => format!(
                "HIGH: {} has grown {:.0}%. Consider restarting soon.",
                history.name, history.growth_percent()
            ),
            1 => format!(
                "MEDIUM: {} shows gradual memory growth. Monitor closely.",
                history.name
            ),
            _ => String::from("No action needed"),
        }
    }

    /// Get monitoring stats
    pub fn stats(&self) -> LeakDetectorStats {
        let total_processes = self.process_history.len();
        let leaking = self.process_history.values().filter(|h| h.is_likely_leak).count();
        let growing = self
            .process_history
            .values()
            .filter(|h| h.growth_rate_mb_per_hour > 1.0)
            .count();

        LeakDetectorStats {
            total_processes,
            leaking_processes: leaking,
            growing_processes: growing,
            monitoring_duration_secs: self.monitoring_duration_secs,
            sample_count: self
                .process_history
                .values()
                .map(|h| h.samples.len())
                .max()
                .unwrap_or(0),
        }
    }

    /// Print leak detection summary
    pub fn print_summary(&self) {
        let stats = self.stats();
        let leaks = self.get_leaks();
        let top_growing = self.get_top_growing(5);

        println!("\n🔍 Memory Leak Detection\n");
        println!(
            "Monitoring {} processes for {} minutes ({} samples)\n",
            stats.total_processes,
            stats.monitoring_duration_secs / 60,
            stats.sample_count
        );

        if !leaks.is_empty() {
            println!("⚠️  DETECTED MEMORY LEAKS:\n");
            println!("┌──────────────────────┬───────────┬───────────┬──────────┬──────────┐");
            println!("│ Process              │ Current   │ Growth/hr │ Growth % │ Severity │");
            println!("├──────────────────────┼───────────┼───────────┼──────────┼──────────┤");

            for leak in &leaks {
                let severity_icon = match leak.severity {
                    3 => "🔴 Crit",
                    2 => "🟠 High",
                    1 => "🟡 Med",
                    _ => "🟢 Low",
                };

                println!(
                    "│ {:20} │ {:>7.0} MB │ {:>+7.0} MB │ {:>+7.0}% │ {:8} │",
                    truncate(&leak.process_name, 20),
                    leak.current_memory_mb,
                    leak.growth_rate_mb_per_hour,
                    leak.growth_percent,
                    severity_icon
                );
            }

            println!("└──────────────────────┴───────────┴───────────┴──────────┴──────────┘");

            println!("\n💡 Recommendations:");
            for leak in leaks.iter().take(3) {
                println!("   • {}", leak.recommendation);
            }
        } else if !top_growing.is_empty() {
            println!("No confirmed leaks detected, but monitoring these growing processes:\n");
            println!("┌──────────────────────┬───────────┬───────────┬──────────────┐");
            println!("│ Process              │ Current   │ Growth/hr │ Confidence   │");
            println!("├──────────────────────┼───────────┼───────────┼──────────────┤");

            for proc in &top_growing {
                println!(
                    "│ {:20} │ {:>7.0} MB │ {:>+7.1} MB │ {:>10.0}%  │",
                    truncate(&proc.name, 20),
                    proc.current_memory_mb,
                    proc.growth_rate_mb_per_hour,
                    proc.confidence * 100.0
                );
            }

            println!("└──────────────────────┴───────────┴───────────┴──────────────┘");
        } else {
            println!("✅ No memory leaks or unusual growth patterns detected.");
        }

        println!(
            "\nTip: Run with longer duration for better detection accuracy."
        );
    }
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Leak detector statistics
#[derive(Debug, Clone)]
pub struct LeakDetectorStats {
    pub total_processes: usize,
    pub leaking_processes: usize,
    pub growing_processes: usize,
    pub monitoring_duration_secs: u64,
    pub sample_count: usize,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:width$}", s, width = max)
    } else {
        format!("{}...", &s[..max - 3])
    }
}
