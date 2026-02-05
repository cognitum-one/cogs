//! Scheduler metrics for monitoring and observability.
//!
//! This module provides:
//! - `SchedulerMetrics` for tracking scheduling performance
//! - Export functionality for Prometheus/OpenTelemetry integration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::node::Tier;

/// Histogram bucket for latency tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyHistogram {
    /// Count of values in each bucket (milliseconds).
    pub buckets: Vec<(u64, u64)>, // (upper_bound_ms, count)
    /// Total sum of all values.
    pub sum_ms: u64,
    /// Total count of observations.
    pub count: u64,
}

impl LatencyHistogram {
    /// Create a new histogram with default buckets.
    pub fn new() -> Self {
        // Default buckets: 1ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1000ms
        let buckets = vec![
            (1, 0),
            (5, 0),
            (10, 0),
            (25, 0),
            (50, 0),
            (100, 0),
            (250, 0),
            (500, 0),
            (1000, 0),
            (u64::MAX, 0), // +Inf bucket
        ];

        Self {
            buckets,
            sum_ms: 0,
            count: 0,
        }
    }

    /// Record a latency observation.
    pub fn observe(&mut self, duration: Duration) {
        let ms = duration.as_millis() as u64;
        self.sum_ms += ms;
        self.count += 1;

        // Update bucket counts
        for (bound, count) in &mut self.buckets {
            if ms <= *bound {
                *count += 1;
            }
        }
    }

    /// Get the average latency in milliseconds.
    pub fn average_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_ms as f64 / self.count as f64
    }

    /// Get an approximate percentile.
    pub fn percentile(&self, p: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }

        let target_count = (self.count as f64 * p / 100.0).ceil() as u64;

        for (bound, count) in &self.buckets {
            if *count >= target_count {
                return *bound;
            }
        }

        self.buckets.last().map(|(b, _)| *b).unwrap_or(0)
    }

    /// Get the p50 (median) latency.
    pub fn p50(&self) -> u64 {
        self.percentile(50.0)
    }

    /// Get the p95 latency.
    pub fn p95(&self) -> u64 {
        self.percentile(95.0)
    }

    /// Get the p99 latency.
    pub fn p99(&self) -> u64 {
        self.percentile(99.0)
    }
}

/// Scheduler metrics for monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerMetrics {
    /// Tasks scheduled per tier.
    pub tasks_by_tier: HashMap<Tier, u64>,

    /// Scheduling latency histogram.
    #[serde(skip)]
    pub scheduling_latency: LatencyHistogram,

    /// Filter rejection counts by reason.
    pub filter_rejections: HashMap<String, u64>,

    /// Current queue depth (pending tasks).
    pub queue_depth: u64,

    /// Node utilization by tier (0-100).
    pub tier_utilization: HashMap<Tier, f64>,

    /// Power consumption by tier (mW).
    pub tier_power_mw: HashMap<Tier, u64>,

    /// Total preemptions count.
    pub preemptions: u64,

    /// Total reschedules count (due to failures).
    pub reschedules: u64,

    /// Total scheduling attempts.
    pub total_scheduling_attempts: u64,

    /// Successful scheduling count.
    pub successful_schedules: u64,

    /// Failed scheduling count.
    pub failed_schedules: u64,

    /// Average scheduling latency (ms).
    pub avg_scheduling_latency_ms: f64,

    /// P99 scheduling latency (ms).
    pub p99_scheduling_latency_ms: u64,

    /// Scheduling throughput (tasks per second, rolling average).
    pub throughput_per_second: f64,

    /// Last update timestamp.
    pub last_update_epoch_ms: u64,
}

impl SchedulerMetrics {
    /// Create new empty metrics.
    pub fn new() -> Self {
        let mut tasks_by_tier = HashMap::new();
        tasks_by_tier.insert(Tier::Edge, 0);
        tasks_by_tier.insert(Tier::Host, 0);
        tasks_by_tier.insert(Tier::Accel, 0);

        let mut tier_utilization = HashMap::new();
        tier_utilization.insert(Tier::Edge, 0.0);
        tier_utilization.insert(Tier::Host, 0.0);
        tier_utilization.insert(Tier::Accel, 0.0);

        let mut tier_power_mw = HashMap::new();
        tier_power_mw.insert(Tier::Edge, 0);
        tier_power_mw.insert(Tier::Host, 0);
        tier_power_mw.insert(Tier::Accel, 0);

        Self {
            tasks_by_tier,
            scheduling_latency: LatencyHistogram::new(),
            filter_rejections: HashMap::new(),
            queue_depth: 0,
            tier_utilization,
            tier_power_mw,
            preemptions: 0,
            reschedules: 0,
            total_scheduling_attempts: 0,
            successful_schedules: 0,
            failed_schedules: 0,
            avg_scheduling_latency_ms: 0.0,
            p99_scheduling_latency_ms: 0,
            throughput_per_second: 0.0,
            last_update_epoch_ms: current_epoch_ms(),
        }
    }

    /// Record a successful scheduling.
    pub fn record_scheduling(&mut self, tier: Tier, duration: Duration) {
        *self.tasks_by_tier.entry(tier).or_insert(0) += 1;
        self.scheduling_latency.observe(duration);
        self.total_scheduling_attempts += 1;
        self.successful_schedules += 1;

        // Update derived metrics
        self.avg_scheduling_latency_ms = self.scheduling_latency.average_ms();
        self.p99_scheduling_latency_ms = self.scheduling_latency.p99();
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Record a filter rejection.
    pub fn record_filter_rejection(&mut self, reason: &str) {
        *self.filter_rejections.entry(reason.to_string()).or_insert(0) += 1;
    }

    /// Record a failed scheduling attempt.
    pub fn record_scheduling_failure(&mut self) {
        self.total_scheduling_attempts += 1;
        self.failed_schedules += 1;
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Record a preemption.
    pub fn record_preemption(&mut self, count: usize) {
        self.preemptions += count as u64;
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Record a reschedule.
    pub fn record_reschedule(&mut self) {
        self.reschedules += 1;
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Update queue depth.
    pub fn set_queue_depth(&mut self, depth: u64) {
        self.queue_depth = depth;
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Update tier utilization.
    pub fn set_tier_utilization(&mut self, tier: Tier, utilization: f64) {
        self.tier_utilization.insert(tier, utilization.clamp(0.0, 100.0));
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Update tier power consumption.
    pub fn set_tier_power(&mut self, tier: Tier, power_mw: u64) {
        self.tier_power_mw.insert(tier, power_mw);
        self.last_update_epoch_ms = current_epoch_ms();
    }

    /// Get success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_scheduling_attempts == 0 {
            return 1.0;
        }
        self.successful_schedules as f64 / self.total_scheduling_attempts as f64
    }

    /// Get total power consumption across all tiers.
    pub fn total_power_mw(&self) -> u64 {
        self.tier_power_mw.values().sum()
    }

    /// Export metrics in Prometheus format.
    #[cfg(feature = "metrics-export")]
    pub fn to_prometheus(&self) -> String {
        let mut output = String::new();

        // Tasks by tier
        for (tier, count) in &self.tasks_by_tier {
            output.push_str(&format!(
                "agentvm_scheduler_tasks_total{{tier=\"{:?}\"}} {}\n",
                tier, count
            ));
        }

        // Scheduling latency
        output.push_str(&format!(
            "agentvm_scheduler_latency_avg_ms {}\n",
            self.avg_scheduling_latency_ms
        ));
        output.push_str(&format!(
            "agentvm_scheduler_latency_p99_ms {}\n",
            self.p99_scheduling_latency_ms
        ));

        // Filter rejections
        for (reason, count) in &self.filter_rejections {
            output.push_str(&format!(
                "agentvm_scheduler_filter_rejections_total{{reason=\"{}\"}} {}\n",
                reason, count
            ));
        }

        // Other metrics
        output.push_str(&format!(
            "agentvm_scheduler_queue_depth {}\n",
            self.queue_depth
        ));
        output.push_str(&format!(
            "agentvm_scheduler_preemptions_total {}\n",
            self.preemptions
        ));
        output.push_str(&format!(
            "agentvm_scheduler_reschedules_total {}\n",
            self.reschedules
        ));
        output.push_str(&format!(
            "agentvm_scheduler_success_rate {}\n",
            self.success_rate()
        ));

        // Tier utilization
        for (tier, util) in &self.tier_utilization {
            output.push_str(&format!(
                "agentvm_scheduler_tier_utilization{{tier=\"{:?}\"}} {}\n",
                tier, util
            ));
        }

        // Tier power
        for (tier, power) in &self.tier_power_mw {
            output.push_str(&format!(
                "agentvm_scheduler_tier_power_mw{{tier=\"{:?}\"}} {}\n",
                tier, power
            ));
        }

        output
    }

    /// Export metrics as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Reset all metrics.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for SchedulerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current epoch time in milliseconds.
fn current_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Metrics aggregator for collecting metrics from multiple schedulers.
#[derive(Debug, Default)]
pub struct MetricsAggregator {
    metrics: Vec<SchedulerMetrics>,
}

impl MetricsAggregator {
    /// Create a new aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add metrics from a scheduler.
    pub fn add(&mut self, metrics: SchedulerMetrics) {
        self.metrics.push(metrics);
    }

    /// Aggregate all collected metrics.
    pub fn aggregate(&self) -> SchedulerMetrics {
        let mut result = SchedulerMetrics::new();

        for m in &self.metrics {
            // Sum task counts
            for (tier, count) in &m.tasks_by_tier {
                *result.tasks_by_tier.entry(*tier).or_insert(0) += count;
            }

            // Sum filter rejections
            for (reason, count) in &m.filter_rejections {
                *result.filter_rejections.entry(reason.clone()).or_insert(0) += count;
            }

            // Sum counters
            result.queue_depth += m.queue_depth;
            result.preemptions += m.preemptions;
            result.reschedules += m.reschedules;
            result.total_scheduling_attempts += m.total_scheduling_attempts;
            result.successful_schedules += m.successful_schedules;
            result.failed_schedules += m.failed_schedules;
        }

        // Average utilization
        if !self.metrics.is_empty() {
            let n = self.metrics.len() as f64;
            for tier in &[Tier::Edge, Tier::Host, Tier::Accel] {
                let sum: f64 = self
                    .metrics
                    .iter()
                    .filter_map(|m| m.tier_utilization.get(tier))
                    .sum();
                result.tier_utilization.insert(*tier, sum / n);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_histogram() {
        let mut hist = LatencyHistogram::new();

        // Add some observations
        hist.observe(Duration::from_millis(5));
        hist.observe(Duration::from_millis(10));
        hist.observe(Duration::from_millis(15));
        hist.observe(Duration::from_millis(100));
        hist.observe(Duration::from_millis(200));

        assert_eq!(hist.count, 5);
        assert_eq!(hist.sum_ms, 330);
        assert_eq!(hist.average_ms(), 66.0);

        // Check percentiles
        assert!(hist.p50() <= 25); // Median should be around 15ms
        assert!(hist.p95() <= 250);
    }

    #[test]
    fn test_scheduler_metrics() {
        let mut metrics = SchedulerMetrics::new();

        // Record some schedulings
        metrics.record_scheduling(Tier::Host, Duration::from_millis(5));
        metrics.record_scheduling(Tier::Host, Duration::from_millis(10));
        metrics.record_scheduling(Tier::Edge, Duration::from_millis(1));

        assert_eq!(*metrics.tasks_by_tier.get(&Tier::Host).unwrap(), 2);
        assert_eq!(*metrics.tasks_by_tier.get(&Tier::Edge).unwrap(), 1);
        assert_eq!(metrics.successful_schedules, 3);

        // Record failures
        metrics.record_scheduling_failure();
        assert_eq!(metrics.failed_schedules, 1);
        assert_eq!(metrics.total_scheduling_attempts, 4);

        // Success rate
        assert!((metrics.success_rate() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_filter_rejection_tracking() {
        let mut metrics = SchedulerMetrics::new();

        metrics.record_filter_rejection("tier: mismatch");
        metrics.record_filter_rejection("tier: mismatch");
        metrics.record_filter_rejection("resource: memory");

        assert_eq!(
            *metrics.filter_rejections.get("tier: mismatch").unwrap(),
            2
        );
        assert_eq!(
            *metrics.filter_rejections.get("resource: memory").unwrap(),
            1
        );
    }

    #[test]
    fn test_tier_utilization() {
        let mut metrics = SchedulerMetrics::new();

        metrics.set_tier_utilization(Tier::Host, 75.5);
        metrics.set_tier_utilization(Tier::Edge, 10.0);

        assert_eq!(*metrics.tier_utilization.get(&Tier::Host).unwrap(), 75.5);
        assert_eq!(*metrics.tier_utilization.get(&Tier::Edge).unwrap(), 10.0);

        // Clamp to valid range
        metrics.set_tier_utilization(Tier::Accel, 150.0);
        assert_eq!(*metrics.tier_utilization.get(&Tier::Accel).unwrap(), 100.0);
    }

    #[test]
    fn test_metrics_aggregator() {
        let mut agg = MetricsAggregator::new();

        let mut m1 = SchedulerMetrics::new();
        m1.record_scheduling(Tier::Host, Duration::from_millis(5));
        m1.preemptions = 2;

        let mut m2 = SchedulerMetrics::new();
        m2.record_scheduling(Tier::Host, Duration::from_millis(10));
        m2.record_scheduling(Tier::Edge, Duration::from_millis(1));
        m2.preemptions = 3;

        agg.add(m1);
        agg.add(m2);

        let result = agg.aggregate();
        assert_eq!(*result.tasks_by_tier.get(&Tier::Host).unwrap(), 2);
        assert_eq!(*result.tasks_by_tier.get(&Tier::Edge).unwrap(), 1);
        assert_eq!(result.preemptions, 5);
    }

    #[test]
    fn test_metrics_json_export() {
        let mut metrics = SchedulerMetrics::new();
        metrics.record_scheduling(Tier::Host, Duration::from_millis(5));

        let json = metrics.to_json().unwrap();
        assert!(json.contains("tasks_by_tier"));
        assert!(json.contains("Host"));
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = SchedulerMetrics::new();
        metrics.record_scheduling(Tier::Host, Duration::from_millis(5));
        metrics.preemptions = 10;

        metrics.reset();

        assert_eq!(metrics.successful_schedules, 0);
        assert_eq!(metrics.preemptions, 0);
    }
}
