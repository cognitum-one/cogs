//! Information budget and bandwidth limiting for perception.
//!
//! This module provides mechanisms for limiting the information flow
//! from sensors to cognitive layers, implementing ADR-001 requirements
//! for entropy-bounded observations.
//!
//! # Overview
//!
//! Real agents have limited processing capacity and communication bandwidth.
//! This module provides:
//!
//! - **BandwidthLimiter**: Max bytes/second rate limiting
//! - **EntropyBudget**: Shannon entropy constraints
//! - **Downsampler**: Resolution reduction for large observations
//!
//! # Examples
//!
//! ```rust,no_run
//! use fxnn::perception::{BandwidthLimiter, EntropyBudget, Downsampler};
//!
//! // Limit to 1KB/second
//! let mut bandwidth = BandwidthLimiter::new(1024);
//!
//! // Limit to 8 bits of entropy
//! let entropy_budget = EntropyBudget::new(8.0);
//!
//! // Downsample to reduce resolution
//! let downsampler = Downsampler::new(0.5); // 50% resolution
//! ```

use super::observer::{Observation, ObservationData, SensorReading};
use serde::{Deserialize, Serialize};

/// Metrics for bandwidth usage tracking.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BandwidthMetrics {
    /// Total bytes transmitted in the current window.
    pub bytes_transmitted: usize,

    /// Total observations transmitted.
    pub observations_transmitted: usize,

    /// Bytes dropped due to rate limiting.
    pub bytes_dropped: usize,

    /// Observations dropped due to rate limiting.
    pub observations_dropped: usize,

    /// Current utilization [0, 1].
    pub utilization: f32,

    /// Average bytes per observation.
    pub avg_bytes_per_observation: f32,
}

impl BandwidthMetrics {
    /// Reset all metrics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Update utilization based on limit.
    pub fn update_utilization(&mut self, limit: usize) {
        if limit > 0 {
            self.utilization = self.bytes_transmitted as f32 / limit as f32;
        }
    }
}

/// Bandwidth limiter for controlling data rate.
///
/// Implements a token bucket algorithm for smooth rate limiting
/// with configurable burst capacity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthLimiter {
    /// Maximum bytes per second.
    bytes_per_second: usize,

    /// Burst capacity (max bytes that can be transmitted instantly).
    burst_capacity: usize,

    /// Current token count (available bandwidth).
    tokens: usize,

    /// Last update timestamp (simulation time).
    last_update: f64,

    /// Metrics for monitoring.
    metrics: BandwidthMetrics,
}

impl BandwidthLimiter {
    /// Create a new bandwidth limiter.
    ///
    /// # Arguments
    ///
    /// * `bytes_per_second` - Maximum sustained bandwidth
    pub fn new(bytes_per_second: usize) -> Self {
        Self {
            bytes_per_second,
            burst_capacity: bytes_per_second, // Default burst = 1 second of data
            tokens: bytes_per_second,
            last_update: 0.0,
            metrics: BandwidthMetrics::default(),
        }
    }

    /// Set burst capacity.
    pub fn with_burst_capacity(mut self, capacity: usize) -> Self {
        self.burst_capacity = capacity;
        self.tokens = capacity;
        self
    }

    /// Update token count based on elapsed time.
    pub fn update(&mut self, current_time: f64) {
        if current_time <= self.last_update {
            return;
        }

        let elapsed = (current_time - self.last_update) as f32;
        let new_tokens = (elapsed * self.bytes_per_second as f32) as usize;

        self.tokens = (self.tokens + new_tokens).min(self.burst_capacity);
        self.last_update = current_time;
    }

    /// Check if an observation can be transmitted.
    ///
    /// # Arguments
    ///
    /// * `observation` - The observation to check
    ///
    /// # Returns
    ///
    /// `true` if the observation fits within the current bandwidth budget.
    pub fn can_transmit(&self, observation: &Observation) -> bool {
        observation.info_bytes <= self.tokens
    }

    /// Attempt to transmit an observation, consuming bandwidth tokens.
    ///
    /// # Arguments
    ///
    /// * `observation` - The observation to transmit
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// `true` if transmission succeeded, `false` if rate limited.
    pub fn transmit(&mut self, observation: &Observation, current_time: f64) -> bool {
        self.update(current_time);

        if observation.info_bytes <= self.tokens {
            self.tokens -= observation.info_bytes;
            self.metrics.bytes_transmitted += observation.info_bytes;
            self.metrics.observations_transmitted += 1;
            self.metrics.update_utilization(self.bytes_per_second);

            if self.metrics.observations_transmitted > 0 {
                self.metrics.avg_bytes_per_observation = self.metrics.bytes_transmitted as f32
                    / self.metrics.observations_transmitted as f32;
            }

            true
        } else {
            self.metrics.bytes_dropped += observation.info_bytes;
            self.metrics.observations_dropped += 1;
            false
        }
    }

    /// Get available bandwidth tokens.
    pub fn available_tokens(&self) -> usize {
        self.tokens
    }

    /// Get current metrics.
    pub fn metrics(&self) -> &BandwidthMetrics {
        &self.metrics
    }

    /// Reset metrics for a new measurement period.
    pub fn reset_metrics(&mut self) {
        self.metrics.reset();
    }

    /// Get the maximum bytes per second.
    pub fn bytes_per_second(&self) -> usize {
        self.bytes_per_second
    }
}

/// Entropy budget for constraining observation information content.
///
/// Implements Shannon entropy limits to ensure observations don't
/// contain more information than the system can process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EntropyBudget {
    /// Maximum allowed Shannon entropy (in bits).
    max_entropy: f32,

    /// Current entropy usage.
    current_entropy: f32,

    /// Whether to allow partial observations when over budget.
    allow_partial: bool,
}

impl EntropyBudget {
    /// Create a new entropy budget.
    ///
    /// # Arguments
    ///
    /// * `max_entropy` - Maximum Shannon entropy in bits
    pub fn new(max_entropy: f32) -> Self {
        Self {
            max_entropy,
            current_entropy: 0.0,
            allow_partial: true,
        }
    }

    /// Disable partial observations (all-or-nothing).
    pub fn with_no_partial(mut self) -> Self {
        self.allow_partial = false;
        self
    }

    /// Check if an observation fits within the entropy budget.
    pub fn check(&self, observation: &Observation) -> bool {
        observation.entropy <= self.max_entropy
    }

    /// Consume entropy budget for an observation.
    ///
    /// # Returns
    ///
    /// Fraction of the observation that fits [0, 1].
    pub fn consume(&mut self, observation: &Observation) -> f32 {
        if observation.entropy <= self.max_entropy - self.current_entropy {
            self.current_entropy += observation.entropy;
            1.0
        } else if self.allow_partial {
            let available = self.max_entropy - self.current_entropy;
            let fraction = available / observation.entropy;
            self.current_entropy = self.max_entropy;
            fraction.max(0.0)
        } else {
            0.0
        }
    }

    /// Reset the entropy budget.
    pub fn reset(&mut self) {
        self.current_entropy = 0.0;
    }

    /// Get remaining entropy budget.
    pub fn remaining(&self) -> f32 {
        (self.max_entropy - self.current_entropy).max(0.0)
    }

    /// Get current utilization [0, 1].
    pub fn utilization(&self) -> f32 {
        self.current_entropy / self.max_entropy
    }
}

/// Downsampling strategy for reducing observation resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DownsampleStrategy {
    /// Uniform random sampling.
    Random {
        /// Fraction of observations to keep [0, 1].
        keep_fraction: f32,
        /// Random seed for reproducibility.
        seed: u64,
    },

    /// Keep every Nth observation.
    Stride {
        /// Stride interval (keep every Nth).
        n: usize,
    },

    /// Keep observations with highest confidence.
    TopConfidence {
        /// Maximum number to keep.
        max_count: usize,
    },

    /// Reduce position precision.
    Quantize {
        /// Quantization step size.
        step_size: f32,
    },

    /// Spatial binning (average nearby observations).
    SpatialBin {
        /// Bin size in each dimension.
        bin_size: f32,
    },
}

/// Downsampler for reducing observation resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Downsampler {
    /// Downsampling strategy.
    strategy: DownsampleStrategy,

    /// Target reduction ratio [0, 1] (1 = no reduction).
    target_ratio: f32,
}

impl Downsampler {
    /// Create a new downsampler with random sampling.
    ///
    /// # Arguments
    ///
    /// * `keep_fraction` - Fraction of observations to keep [0, 1]
    pub fn new(keep_fraction: f32) -> Self {
        Self {
            strategy: DownsampleStrategy::Random {
                keep_fraction,
                seed: 42,
            },
            target_ratio: keep_fraction,
        }
    }

    /// Create a downsampler with specified strategy.
    pub fn with_strategy(strategy: DownsampleStrategy) -> Self {
        let target_ratio = match &strategy {
            DownsampleStrategy::Random { keep_fraction, .. } => *keep_fraction,
            DownsampleStrategy::Stride { n } => 1.0 / *n as f32,
            DownsampleStrategy::TopConfidence { .. } => 1.0,
            DownsampleStrategy::Quantize { .. } => 1.0,
            DownsampleStrategy::SpatialBin { .. } => 1.0,
        };

        Self {
            strategy,
            target_ratio,
        }
    }

    /// Downsample observation data.
    pub fn downsample(&self, data: &ObservationData) -> ObservationData {
        let readings = match &self.strategy {
            DownsampleStrategy::Random {
                keep_fraction,
                seed,
            } => {
                use rand::{Rng, SeedableRng};
                use rand_xoshiro::Xoshiro256PlusPlus;

                let mut rng = Xoshiro256PlusPlus::seed_from_u64(*seed);
                data.readings
                    .iter()
                    .filter(|_| rng.gen::<f32>() < *keep_fraction)
                    .cloned()
                    .collect()
            }

            DownsampleStrategy::Stride { n } => data
                .readings
                .iter()
                .enumerate()
                .filter(|(i, _)| i % n == 0)
                .map(|(_, r)| r.clone())
                .collect(),

            DownsampleStrategy::TopConfidence { max_count } => {
                let mut sorted: Vec<_> = data.readings.iter().cloned().collect();
                sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
                sorted.truncate(*max_count);
                sorted
            }

            DownsampleStrategy::Quantize { step_size } => data
                .readings
                .iter()
                .map(|r| {
                    let mut reading = r.clone();
                    reading.position = [
                        (r.position[0] / step_size).round() * step_size,
                        (r.position[1] / step_size).round() * step_size,
                        (r.position[2] / step_size).round() * step_size,
                    ];
                    reading
                })
                .collect(),

            DownsampleStrategy::SpatialBin { bin_size } => {
                use std::collections::HashMap;

                // Group readings by bin
                let mut bins: HashMap<(i32, i32, i32), Vec<&SensorReading>> = HashMap::new();

                for reading in &data.readings {
                    let bin_key = (
                        (reading.position[0] / bin_size).floor() as i32,
                        (reading.position[1] / bin_size).floor() as i32,
                        (reading.position[2] / bin_size).floor() as i32,
                    );
                    bins.entry(bin_key).or_default().push(reading);
                }

                // Average readings within each bin
                bins.into_values()
                    .map(|readings| {
                        let n = readings.len() as f32;
                        let avg_pos = readings.iter().fold([0.0; 3], |mut acc, r| {
                            acc[0] += r.position[0];
                            acc[1] += r.position[1];
                            acc[2] += r.position[2];
                            acc
                        });

                        let avg_conf: f32 = readings.iter().map(|r| r.confidence).sum::<f32>() / n;

                        SensorReading {
                            atom_id: readings[0].atom_id, // Use first atom's ID
                            position: [avg_pos[0] / n, avg_pos[1] / n, avg_pos[2] / n],
                            position_uncertainty: [*bin_size; 3], // Uncertainty is bin size
                            velocity: None,
                            velocity_uncertainty: None,
                            distance: readings.iter().map(|r| r.distance).sum::<f32>() / n,
                            angle: readings.iter().map(|r| r.angle).sum::<f32>() / n,
                            confidence: avg_conf,
                            atom_type: readings[0].atom_type,
                        }
                    })
                    .collect()
            }
        };

        ObservationData {
            readings,
            observer_position: data.observer_position,
            observer_direction: data.observer_direction,
            config: data.config,
        }
    }

    /// Get the target reduction ratio.
    pub fn target_ratio(&self) -> f32 {
        self.target_ratio
    }

    /// Estimate the output size after downsampling.
    pub fn estimate_output_size(&self, input_count: usize) -> usize {
        match &self.strategy {
            DownsampleStrategy::Random { keep_fraction, .. } => {
                (input_count as f32 * keep_fraction) as usize
            }
            DownsampleStrategy::Stride { n } => input_count / n,
            DownsampleStrategy::TopConfidence { max_count } => (*max_count).min(input_count),
            DownsampleStrategy::Quantize { .. } => input_count, // Same count, less precision
            DownsampleStrategy::SpatialBin { .. } => input_count, // Can't estimate without data
        }
    }
}

// ============================================================================
// ADR-001 Agent Budget Enforcement
// ============================================================================

/// Compute budget tracker for limiting agent FLOPs per tick (ADR-001)
///
/// Implements compute budget enforcement from ADR-001 Part II-B:
/// - Max compute per tick (FLOPs)
/// - Action timeout on budget exceeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeBudget {
    /// Maximum FLOPs allowed per tick
    max_flops_per_tick: f64,
    /// Current FLOPs consumed this tick
    current_flops: f64,
    /// Total FLOPs consumed (cumulative)
    total_flops: f64,
    /// Number of times budget was exceeded
    budget_exceeded_count: u64,
}

impl ComputeBudget {
    /// Create a new compute budget.
    ///
    /// # Arguments
    ///
    /// * `max_flops_per_tick` - Maximum FLOPs allowed per tick
    pub fn new(max_flops_per_tick: f64) -> Self {
        Self {
            max_flops_per_tick,
            current_flops: 0.0,
            total_flops: 0.0,
            budget_exceeded_count: 0,
        }
    }

    /// Default budget: 1 GFLOP per tick
    pub fn default_agent_budget() -> Self {
        Self::new(1e9)
    }

    /// Check if operation with given FLOPs fits in budget
    pub fn can_execute(&self, flops: f64) -> bool {
        self.current_flops + flops <= self.max_flops_per_tick
    }

    /// Try to consume FLOPs from budget
    ///
    /// Returns true if operation was allowed, false if budget exceeded.
    pub fn consume(&mut self, flops: f64) -> bool {
        if self.can_execute(flops) {
            self.current_flops += flops;
            self.total_flops += flops;
            true
        } else {
            self.budget_exceeded_count += 1;
            false
        }
    }

    /// Force consume FLOPs (for tracking even when over budget)
    pub fn force_consume(&mut self, flops: f64) {
        self.current_flops += flops;
        self.total_flops += flops;
        if self.current_flops > self.max_flops_per_tick {
            self.budget_exceeded_count += 1;
        }
    }

    /// Reset for new tick
    pub fn reset_tick(&mut self) {
        self.current_flops = 0.0;
    }

    /// Get remaining FLOPs for this tick
    pub fn remaining(&self) -> f64 {
        (self.max_flops_per_tick - self.current_flops).max(0.0)
    }

    /// Get current utilization [0, 1+] (can exceed 1.0 if over budget)
    pub fn utilization(&self) -> f64 {
        self.current_flops / self.max_flops_per_tick
    }

    /// Check if budget has been exceeded
    pub fn is_exceeded(&self) -> bool {
        self.current_flops > self.max_flops_per_tick
    }

    /// Get statistics
    pub fn stats(&self) -> ComputeBudgetStats {
        ComputeBudgetStats {
            max_flops_per_tick: self.max_flops_per_tick,
            current_flops: self.current_flops,
            total_flops: self.total_flops,
            utilization: self.utilization(),
            exceeded_count: self.budget_exceeded_count,
        }
    }
}

/// Statistics for compute budget
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ComputeBudgetStats {
    pub max_flops_per_tick: f64,
    pub current_flops: f64,
    pub total_flops: f64,
    pub utilization: f64,
    pub exceeded_count: u64,
}

/// Memory write rate limiter for agents (ADR-001)
///
/// Implements memory write rate limiting from ADR-001 Part II-B:
/// - Max entries/second (or per tick)
/// - Write throttling and queue overflow handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWriteBudget {
    /// Maximum writes allowed per tick
    max_writes_per_tick: u32,
    /// Current writes this tick
    current_writes: u32,
    /// Queued writes (waiting for next tick)
    queued_writes: u32,
    /// Maximum queue size before overflow
    max_queue_size: u32,
    /// Total writes performed
    total_writes: u64,
    /// Writes dropped due to overflow
    writes_dropped: u64,
}

impl MemoryWriteBudget {
    /// Create a new memory write budget.
    ///
    /// # Arguments
    ///
    /// * `max_writes_per_tick` - Maximum memory writes allowed per tick
    pub fn new(max_writes_per_tick: u32) -> Self {
        Self {
            max_writes_per_tick,
            current_writes: 0,
            queued_writes: 0,
            max_queue_size: max_writes_per_tick * 10, // Default: 10 ticks worth
            total_writes: 0,
            writes_dropped: 0,
        }
    }

    /// Set maximum queue size
    pub fn with_queue_size(mut self, size: u32) -> Self {
        self.max_queue_size = size;
        self
    }

    /// Default budget: 10 writes per tick
    pub fn default_agent_budget() -> Self {
        Self::new(10)
    }

    /// Check if a write can be performed immediately
    pub fn can_write_now(&self) -> bool {
        self.current_writes < self.max_writes_per_tick
    }

    /// Check if a write can be queued
    pub fn can_queue(&self) -> bool {
        self.queued_writes < self.max_queue_size
    }

    /// Try to perform a write
    ///
    /// Returns:
    /// - `WriteResult::Immediate` if write executed immediately
    /// - `WriteResult::Queued` if write was queued for later
    /// - `WriteResult::Dropped` if queue is full
    pub fn write(&mut self) -> WriteResult {
        if self.can_write_now() {
            self.current_writes += 1;
            self.total_writes += 1;
            WriteResult::Immediate
        } else if self.can_queue() {
            self.queued_writes += 1;
            WriteResult::Queued
        } else {
            self.writes_dropped += 1;
            WriteResult::Dropped
        }
    }

    /// Try to perform multiple writes
    ///
    /// Returns the number of writes that were accepted (immediate + queued)
    pub fn write_batch(&mut self, count: u32) -> u32 {
        let mut accepted = 0;
        for _ in 0..count {
            match self.write() {
                WriteResult::Immediate | WriteResult::Queued => accepted += 1,
                WriteResult::Dropped => break,
            }
        }
        accepted
    }

    /// Reset for new tick, processing queued writes
    pub fn reset_tick(&mut self) {
        // Process queued writes up to budget
        let process_from_queue = self.queued_writes.min(self.max_writes_per_tick);
        self.total_writes += process_from_queue as u64;
        self.queued_writes = self.queued_writes.saturating_sub(process_from_queue);
        self.current_writes = 0;
    }

    /// Get remaining writes for this tick
    pub fn remaining(&self) -> u32 {
        self.max_writes_per_tick.saturating_sub(self.current_writes)
    }

    /// Get queue utilization [0, 1]
    pub fn queue_utilization(&self) -> f32 {
        self.queued_writes as f32 / self.max_queue_size as f32
    }

    /// Get statistics
    pub fn stats(&self) -> MemoryWriteStats {
        MemoryWriteStats {
            max_writes_per_tick: self.max_writes_per_tick,
            current_writes: self.current_writes,
            queued_writes: self.queued_writes,
            total_writes: self.total_writes,
            writes_dropped: self.writes_dropped,
            queue_utilization: self.queue_utilization(),
        }
    }
}

/// Result of a write attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteResult {
    /// Write executed immediately
    Immediate,
    /// Write was queued for later execution
    Queued,
    /// Write was dropped (queue overflow)
    Dropped,
}

/// Statistics for memory write budget
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MemoryWriteStats {
    pub max_writes_per_tick: u32,
    pub current_writes: u32,
    pub queued_writes: u32,
    pub total_writes: u64,
    pub writes_dropped: u64,
    pub queue_utilization: f32,
}

/// Combined agent budget enforcer (ADR-001 compliant)
///
/// This struct enforces all agent budgets from ADR-001 Part II-B:
/// - Observation bandwidth (bytes/second)
/// - Compute per tick (FLOPs)
/// - Memory write rate (entries/tick)
/// - Action magnitude (via governance layer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudgetEnforcer {
    /// Agent ID this enforcer applies to
    pub agent_id: u64,
    /// Bandwidth limiter
    bandwidth: BandwidthLimiter,
    /// Compute budget
    compute: ComputeBudget,
    /// Memory write budget
    memory: MemoryWriteBudget,
    /// Action magnitude limit
    max_action_magnitude: f64,
    /// Whether to enforce strictly (drop violations) or softly (log warnings)
    strict_enforcement: bool,
}

impl AgentBudgetEnforcer {
    /// Create a new agent budget enforcer with default budgets
    pub fn new(agent_id: u64) -> Self {
        Self {
            agent_id,
            bandwidth: BandwidthLimiter::new(1_000_000), // 1 MB/s
            compute: ComputeBudget::default_agent_budget(),
            memory: MemoryWriteBudget::default_agent_budget(),
            max_action_magnitude: 1000.0,
            strict_enforcement: true,
        }
    }

    /// Configure bandwidth limit
    pub fn with_bandwidth(mut self, bytes_per_second: usize) -> Self {
        self.bandwidth = BandwidthLimiter::new(bytes_per_second);
        self
    }

    /// Configure compute budget
    pub fn with_compute(mut self, max_flops_per_tick: f64) -> Self {
        self.compute = ComputeBudget::new(max_flops_per_tick);
        self
    }

    /// Configure memory write budget
    pub fn with_memory_writes(mut self, max_writes_per_tick: u32) -> Self {
        self.memory = MemoryWriteBudget::new(max_writes_per_tick);
        self
    }

    /// Configure action magnitude limit
    pub fn with_action_magnitude(mut self, max_magnitude: f64) -> Self {
        self.max_action_magnitude = max_magnitude;
        self
    }

    /// Set enforcement mode
    pub fn with_strict_enforcement(mut self, strict: bool) -> Self {
        self.strict_enforcement = strict;
        self
    }

    /// Check all budgets and return comprehensive status
    pub fn check_all(&self) -> AgentBudgetStatus {
        AgentBudgetStatus {
            agent_id: self.agent_id,
            bandwidth_available: self.bandwidth.available_tokens(),
            compute_remaining: self.compute.remaining(),
            memory_writes_remaining: self.memory.remaining(),
            is_bandwidth_limited: self.bandwidth.available_tokens() == 0,
            is_compute_limited: self.compute.is_exceeded(),
            has_queued_writes: self.memory.queued_writes > 0,
        }
    }

    /// Reset all budgets for new tick
    pub fn reset_tick(&mut self, current_time: f64) {
        self.bandwidth.update(current_time);
        self.compute.reset_tick();
        self.memory.reset_tick();
    }

    /// Clip action magnitude to within bounds
    pub fn clip_action_magnitude(&self, magnitude: f64) -> f64 {
        magnitude.min(self.max_action_magnitude)
    }

    /// Get bandwidth metrics
    pub fn bandwidth_metrics(&self) -> &BandwidthMetrics {
        self.bandwidth.metrics()
    }

    /// Get compute stats
    pub fn compute_stats(&self) -> ComputeBudgetStats {
        self.compute.stats()
    }

    /// Get memory write stats
    pub fn memory_stats(&self) -> MemoryWriteStats {
        self.memory.stats()
    }

    /// Access bandwidth limiter
    pub fn bandwidth(&mut self) -> &mut BandwidthLimiter {
        &mut self.bandwidth
    }

    /// Access compute budget
    pub fn compute(&mut self) -> &mut ComputeBudget {
        &mut self.compute
    }

    /// Access memory budget
    pub fn memory(&mut self) -> &mut MemoryWriteBudget {
        &mut self.memory
    }
}

/// Status of agent budgets
#[derive(Debug, Clone, Copy)]
pub struct AgentBudgetStatus {
    pub agent_id: u64,
    pub bandwidth_available: usize,
    pub compute_remaining: f64,
    pub memory_writes_remaining: u32,
    pub is_bandwidth_limited: bool,
    pub is_compute_limited: bool,
    pub has_queued_writes: bool,
}

// ============================================================================
// Original InformationBudget (unchanged)
// ============================================================================

/// Combined bandwidth and entropy limiter.
///
/// Provides a unified interface for applying both bandwidth and
/// entropy constraints to observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InformationBudget {
    /// Bandwidth limiter.
    bandwidth: BandwidthLimiter,

    /// Entropy budget.
    entropy: EntropyBudget,

    /// Optional downsampler for over-budget observations.
    downsampler: Option<Downsampler>,
}

impl InformationBudget {
    /// Create a new information budget.
    ///
    /// # Arguments
    ///
    /// * `bytes_per_second` - Maximum bandwidth
    /// * `max_entropy` - Maximum entropy in bits
    pub fn new(bytes_per_second: usize, max_entropy: f32) -> Self {
        Self {
            bandwidth: BandwidthLimiter::new(bytes_per_second),
            entropy: EntropyBudget::new(max_entropy),
            downsampler: None,
        }
    }

    /// Add a downsampler for handling over-budget observations.
    pub fn with_downsampler(mut self, downsampler: Downsampler) -> Self {
        self.downsampler = Some(downsampler);
        self
    }

    /// Check if an observation fits within both budgets.
    pub fn check(&self, observation: &Observation) -> bool {
        self.bandwidth.can_transmit(observation) && self.entropy.check(observation)
    }

    /// Process an observation, applying limits and downsampling if needed.
    ///
    /// # Returns
    ///
    /// The processed observation if successful, or `None` if dropped.
    pub fn process(
        &mut self,
        observation: Observation,
        current_time: f64,
    ) -> Option<Observation> {
        // Check bandwidth
        if !self.bandwidth.can_transmit(&observation) {
            // Try downsampling
            if let Some(ref downsampler) = self.downsampler {
                let downsampled_data = downsampler.downsample(&observation.data);
                let new_observation =
                    Observation::new(downsampled_data, observation.timestamp, observation.sequence);

                if self.bandwidth.transmit(&new_observation, current_time) {
                    let _ = self.entropy.consume(&new_observation);
                    return Some(new_observation);
                }
            }
            return None;
        }

        // Check entropy
        let entropy_fraction = self.entropy.consume(&observation);
        if entropy_fraction < 1.0 && self.downsampler.is_none() {
            // Can't fit full observation and no downsampler
            return None;
        }

        // Transmit
        if self.bandwidth.transmit(&observation, current_time) {
            Some(observation)
        } else {
            None
        }
    }

    /// Reset both budgets.
    pub fn reset(&mut self) {
        self.bandwidth.reset_metrics();
        self.entropy.reset();
    }

    /// Get bandwidth metrics.
    pub fn bandwidth_metrics(&self) -> &BandwidthMetrics {
        self.bandwidth.metrics()
    }

    /// Get entropy utilization.
    pub fn entropy_utilization(&self) -> f32 {
        self.entropy.utilization()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::observer::{ObserverConfig, SensorReading};

    fn create_test_observation(reading_count: usize) -> Observation {
        let readings: Vec<SensorReading> = (0..reading_count)
            .map(|i| SensorReading {
                atom_id: i as u32,
                position: [i as f32, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([1.0, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: i as f32,
                angle: 0.0,
                confidence: 0.9,
                atom_type: 0,
            })
            .collect();

        let data = ObservationData {
            readings,
            observer_position: [0.0; 3],
            observer_direction: [1.0, 0.0, 0.0],
            config: ObserverConfig::default(),
        };

        Observation::new(data, 0.0, 0)
    }

    #[test]
    fn test_bandwidth_limiter() {
        let mut limiter = BandwidthLimiter::new(1000);

        let observation = create_test_observation(10);
        assert!(observation.info_bytes < 1000);

        // Should be able to transmit
        assert!(limiter.transmit(&observation, 0.0));

        // Metrics should be updated
        assert_eq!(
            limiter.metrics().bytes_transmitted,
            observation.info_bytes
        );
    }

    #[test]
    fn test_bandwidth_rate_limiting() {
        // Each reading is ~62 bytes (with velocity), plus 24 bytes overhead
        // 2 readings = 2 * 62 + 24 = 148 bytes minimum
        let mut limiter = BandwidthLimiter::new(200).with_burst_capacity(200);

        // Create an observation that uses most of the budget
        let observation = create_test_observation(2);

        // First transmission should succeed (budget is 200, observation is ~148)
        assert!(limiter.transmit(&observation, 0.0));

        // Second immediate transmission may fail if over budget
        let available_before = limiter.available_tokens();

        // After time passes, tokens should refill
        limiter.update(1.0); // 1 second later
        assert!(limiter.available_tokens() >= available_before);
    }

    #[test]
    fn test_entropy_budget() {
        let mut budget = EntropyBudget::new(4.0);

        let observation = create_test_observation(10);

        if observation.entropy <= 4.0 {
            let fraction = budget.consume(&observation);
            assert!((fraction - 1.0).abs() < 0.01);
        } else {
            let fraction = budget.consume(&observation);
            assert!(fraction < 1.0);
        }
    }

    #[test]
    fn test_downsampler_random() {
        let downsampler = Downsampler::new(0.5);

        let observation = create_test_observation(100);
        let downsampled = downsampler.downsample(&observation.data);

        // Should have approximately 50% of readings
        assert!(downsampled.readings.len() < 100);
        assert!(downsampled.readings.len() > 20); // Some variance expected
    }

    #[test]
    fn test_downsampler_stride() {
        let downsampler =
            Downsampler::with_strategy(DownsampleStrategy::Stride { n: 2 });

        let observation = create_test_observation(100);
        let downsampled = downsampler.downsample(&observation.data);

        // Should have exactly 50% of readings
        assert_eq!(downsampled.readings.len(), 50);
    }

    #[test]
    fn test_downsampler_top_confidence() {
        let downsampler =
            Downsampler::with_strategy(DownsampleStrategy::TopConfidence { max_count: 10 });

        let observation = create_test_observation(100);
        let downsampled = downsampler.downsample(&observation.data);

        assert_eq!(downsampled.readings.len(), 10);
    }

    #[test]
    fn test_information_budget() {
        let mut budget = InformationBudget::new(10000, 8.0);

        let observation = create_test_observation(10);

        if budget.check(&observation) {
            let result = budget.process(observation, 0.0);
            assert!(result.is_some());
        }
    }

    // =========================================================================
    // ADR-001 Agent Budget Enforcement Tests
    // =========================================================================

    #[test]
    fn test_compute_budget_basic() {
        let mut budget = ComputeBudget::new(1000.0);

        assert!(budget.can_execute(500.0));
        assert!(budget.consume(500.0));
        assert_eq!(budget.remaining(), 500.0);

        assert!(budget.consume(400.0));
        assert_eq!(budget.remaining(), 100.0);

        // This should fail - not enough budget
        assert!(!budget.consume(200.0));
        assert_eq!(budget.stats().exceeded_count, 1);
    }

    #[test]
    fn test_compute_budget_reset() {
        let mut budget = ComputeBudget::new(100.0);

        budget.consume(90.0);
        assert!(budget.utilization() > 0.89);

        budget.reset_tick();
        assert_eq!(budget.remaining(), 100.0);
        assert_eq!(budget.utilization(), 0.0);
    }

    #[test]
    fn test_memory_write_budget_basic() {
        let mut budget = MemoryWriteBudget::new(5);

        // First 5 writes should be immediate
        for _ in 0..5 {
            assert_eq!(budget.write(), WriteResult::Immediate);
        }

        // Next writes should be queued
        assert_eq!(budget.write(), WriteResult::Queued);
        assert_eq!(budget.stats().queued_writes, 1);
    }

    #[test]
    fn test_memory_write_budget_queue_overflow() {
        let mut budget = MemoryWriteBudget::new(2).with_queue_size(3);

        // 2 immediate
        budget.write();
        budget.write();

        // 3 queued
        budget.write();
        budget.write();
        budget.write();

        // This should be dropped
        assert_eq!(budget.write(), WriteResult::Dropped);
        assert_eq!(budget.stats().writes_dropped, 1);
    }

    #[test]
    fn test_memory_write_budget_tick_reset() {
        let mut budget = MemoryWriteBudget::new(3);

        // Fill budget
        budget.write();
        budget.write();
        budget.write();

        // Queue some
        budget.write();
        budget.write();
        assert_eq!(budget.stats().queued_writes, 2);

        // Reset - should process queued writes
        budget.reset_tick();
        assert_eq!(budget.remaining(), 3);
        // Queued writes should be processed (up to budget)
    }

    #[test]
    fn test_agent_budget_enforcer() {
        let mut enforcer = AgentBudgetEnforcer::new(42)
            .with_compute(1000.0)
            .with_memory_writes(5)
            .with_action_magnitude(100.0);

        let status = enforcer.check_all();
        assert_eq!(status.agent_id, 42);
        assert!(!status.is_compute_limited);
        assert!(!status.is_bandwidth_limited);

        // Use compute budget
        enforcer.compute().consume(900.0);
        assert!(enforcer.compute_stats().utilization > 0.89);

        // Use memory budget
        for _ in 0..5 {
            enforcer.memory().write();
        }
        assert_eq!(enforcer.memory_stats().current_writes, 5);

        // Clip action magnitude
        assert_eq!(enforcer.clip_action_magnitude(50.0), 50.0);
        assert_eq!(enforcer.clip_action_magnitude(200.0), 100.0);
    }

    #[test]
    fn test_agent_budget_enforcer_reset() {
        let mut enforcer = AgentBudgetEnforcer::new(1);

        enforcer.compute().consume(500.0);
        enforcer.memory().write();
        enforcer.memory().write();

        enforcer.reset_tick(1.0);

        assert_eq!(enforcer.compute().remaining(), 1e9);
        assert_eq!(enforcer.memory().remaining(), 10);
    }
}
