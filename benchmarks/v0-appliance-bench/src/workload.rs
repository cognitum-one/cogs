//! Workload Generation
//!
//! Configurable workload profiles that drive the emulator under test.
//! Three primary axes:
//!   1. Spike rate     -- events/sec entering the host ingest path
//!   2. Feature density -- fraction of events requiring tile extraction
//!   3. Query load      -- queries/sec against Postgres + RuVector
//!
//! Profiles can be mixed for concurrent stress testing.

use std::time::{Duration, Instant};

use crate::protocol::KernelId;

// ── Spike Rate Profiles ────────────────────────────────────────────

/// Preset spike-rate envelopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpikeRateProfile {
    /// ~100 events/sec -- quiet background
    Low,
    /// ~1 000 events/sec -- nominal operating point
    Medium,
    /// ~10 000 events/sec -- burst / overload
    High,
    /// Ramp from Low to High over `ramp_duration`, hold, then back down
    Ramp,
    /// Poisson process with given mean inter-arrival (microseconds)
    Poisson { mean_interval_us: u64 },
    /// Fixed rate N events per second
    Fixed { events_per_sec: u64 },
}

impl SpikeRateProfile {
    /// Steady-state events/sec for this profile.
    pub fn nominal_rate(&self) -> u64 {
        match self {
            Self::Low        => 100,
            Self::Medium     => 1_000,
            Self::High       => 10_000,
            Self::Ramp       => 5_000, // average over ramp
            Self::Poisson { mean_interval_us } => {
                if *mean_interval_us == 0 { 0 } else { 1_000_000 / mean_interval_us }
            }
            Self::Fixed { events_per_sec } => *events_per_sec,
        }
    }
}

// ── Feature Extraction Density ─────────────────────────────────────

/// Controls what fraction of ingested events are dispatched to tiles
/// for feature extraction (kernel K2).
#[derive(Clone, Copy, Debug)]
pub struct FeatureDensity {
    /// Fraction in [0.0, 1.0] of events sent to K2.
    pub extraction_ratio: f64,
    /// Average output feature vector dimensionality.
    pub output_dim: usize,
}

impl Default for FeatureDensity {
    fn default() -> Self {
        Self {
            extraction_ratio: 0.8,
            output_dim: 128,
        }
    }
}

// ── Query Load ─────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryProfile {
    /// No queries -- pure ingest
    None,
    /// ~10 queries/sec (light dashboard polling)
    Light,
    /// ~100 queries/sec (active user session)
    Medium,
    /// ~1 000 queries/sec (batch analytics or parallel clients)
    Heavy,
    /// Fixed rate
    Fixed { queries_per_sec: u64 },
}

impl QueryProfile {
    pub fn nominal_rate(&self) -> u64 {
        match self {
            Self::None                     => 0,
            Self::Light                    => 10,
            Self::Medium                   => 100,
            Self::Heavy                    => 1_000,
            Self::Fixed { queries_per_sec } => *queries_per_sec,
        }
    }
}

// ── Kernel Mix ─────────────────────────────────────────────────────

/// Probability weights for kernel selection during task assignment.
/// Weights do not need to sum to 1.0; they are normalized at runtime.
#[derive(Clone, Debug)]
pub struct KernelMix {
    pub k1_spike_step:     f64,
    pub k2_sparse_feature: f64,
    pub k3_boundary_delta: f64,
    pub k4_anomaly_detect: f64,
    pub k5_stabilization:  f64,
    pub k6_health_compact: f64,
}

impl Default for KernelMix {
    fn default() -> Self {
        // Nominal operating mix per the EDF scheduling policy:
        //   coherence-critical > spike > feature > background
        Self {
            k1_spike_step:     0.30,
            k2_sparse_feature: 0.25,
            k3_boundary_delta: 0.15,
            k4_anomaly_detect: 0.10,
            k5_stabilization:  0.10,
            k6_health_compact: 0.10,
        }
    }
}

impl KernelMix {
    /// Return a kernel ID sampled from the distribution.
    /// `rand_val` must be in [0.0, 1.0).
    pub fn sample(&self, rand_val: f64) -> KernelId {
        let total = self.k1_spike_step
            + self.k2_sparse_feature
            + self.k3_boundary_delta
            + self.k4_anomaly_detect
            + self.k5_stabilization
            + self.k6_health_compact;

        let mut cumulative = 0.0;
        let threshold = rand_val * total;

        cumulative += self.k1_spike_step;
        if threshold < cumulative { return KernelId::K1SpikeStep; }

        cumulative += self.k2_sparse_feature;
        if threshold < cumulative { return KernelId::K2SparseFeature; }

        cumulative += self.k3_boundary_delta;
        if threshold < cumulative { return KernelId::K3BoundaryDelta; }

        cumulative += self.k4_anomaly_detect;
        if threshold < cumulative { return KernelId::K4AnomalyDetect; }

        cumulative += self.k5_stabilization;
        if threshold < cumulative { return KernelId::K5Stabilization; }

        KernelId::K6HealthCompact
    }

    /// Return a stabilization-only mix (used when coherence is low).
    pub fn stabilization_only() -> Self {
        Self {
            k1_spike_step:     0.0,
            k2_sparse_feature: 0.0,
            k3_boundary_delta: 0.0,
            k4_anomaly_detect: 0.0,
            k5_stabilization:  0.80,
            k6_health_compact: 0.20,
        }
    }
}

// ── Composite Workload ─────────────────────────────────────────────

/// Full workload configuration combining all axes.
#[derive(Clone, Debug)]
pub struct WorkloadConfig {
    /// Total duration of the workload run.
    pub duration: Duration,

    /// Spike rate envelope.
    pub spike_rate: SpikeRateProfile,

    /// Feature extraction density.
    pub feature_density: FeatureDensity,

    /// Concurrent query load.
    pub query_profile: QueryProfile,

    /// Kernel selection weights.
    pub kernel_mix: KernelMix,

    /// Per-tile task queue ceiling (spec: bounded).
    pub tile_queue_ceiling: usize,

    /// Number of tile simulators (default 7 per spec).
    pub tile_count: usize,

    /// Warmup period before metrics collection begins.
    pub warmup: Duration,

    /// Label for this workload (used in reports).
    pub label: String,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(60),
            spike_rate: SpikeRateProfile::Medium,
            feature_density: FeatureDensity::default(),
            query_profile: QueryProfile::Light,
            kernel_mix: KernelMix::default(),
            tile_queue_ceiling: 8,
            tile_count: 7,
            warmup: Duration::from_secs(5),
            label: String::from("default"),
        }
    }
}

impl WorkloadConfig {
    /// 30-minute endurance run at medium load (Acceptance Test 1).
    pub fn endurance_medium() -> Self {
        Self {
            duration: Duration::from_secs(30 * 60),
            spike_rate: SpikeRateProfile::Medium,
            query_profile: QueryProfile::Medium,
            warmup: Duration::from_secs(10),
            label: String::from("endurance-30min-medium"),
            ..Default::default()
        }
    }

    /// Short smoke test (60 s, medium load).
    pub fn smoke() -> Self {
        Self {
            duration: Duration::from_secs(60),
            spike_rate: SpikeRateProfile::Medium,
            warmup: Duration::from_secs(5),
            label: String::from("smoke-60s"),
            ..Default::default()
        }
    }

    /// High-load burst test.
    pub fn burst() -> Self {
        Self {
            duration: Duration::from_secs(5 * 60),
            spike_rate: SpikeRateProfile::High,
            query_profile: QueryProfile::Heavy,
            warmup: Duration::from_secs(10),
            label: String::from("burst-5min-high"),
            ..Default::default()
        }
    }
}

// ── Workload Generator (pseudo-code struct) ────────────────────────

/// Drives events into the emulator according to the configured profile.
///
/// # Lifecycle
///
/// ```text
///   new(config) -> warmup() -> run() -> drain() -> report()
/// ```
pub struct WorkloadGenerator {
    pub config: WorkloadConfig,

    // -- internal bookkeeping (pseudo-code) --
    /// Monotonically increasing sequence counter.
    next_sequence: u64,
    /// Current epoch (increments on reset or coherence event).
    epoch: u64,
    /// Job ID allocator.
    next_job_id: u64,
    /// Instant the run started.
    start: Option<Instant>,
    /// Count of events generated so far.
    events_generated: u64,
    /// Count of queries dispatched.
    queries_dispatched: u64,
}

impl WorkloadGenerator {
    pub fn new(config: WorkloadConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            epoch: 1,
            next_job_id: 1,
            start: None,
            events_generated: 0,
            queries_dispatched: 0,
        }
    }

    /// Block until the warmup period elapses, emitting HELLO + initial
    /// heartbeats so tiles reach steady state.
    pub fn warmup(&mut self) {
        self.start = Some(Instant::now());
        // pseudo: sleep(self.config.warmup) while sending HELLOs
        // In real implementation: async loop sending HELLO to each tile,
        // waiting for HEARTBEAT responses, verifying epoch agreement.
    }

    /// Main generation loop.  Returns when `config.duration` elapses.
    ///
    /// Each iteration of the inner loop:
    ///   1. Determine next event time from spike rate profile.
    ///   2. Build a TASK_ASSIGN message for a randomly selected kernel.
    ///   3. Route to the least-loaded tile (respecting queue ceiling).
    ///   4. Optionally dispatch a query if the query timer fires.
    ///   5. Collect TASK_RESULT and HEARTBEAT messages from tiles.
    ///   6. Feed results to MetricsCollector.
    pub fn run(&mut self) {
        // pseudo: see harness.rs for the orchestrated version
    }

    /// Drain outstanding responses after the run completes.
    pub fn drain(&mut self) {
        // pseudo: wait up to 2x tick period for final results
    }

    // -- helpers --

    pub fn alloc_job_id(&mut self) -> u64 {
        let id = self.next_job_id;
        self.next_job_id += 1;
        id
    }

    pub fn alloc_sequence(&mut self) -> u64 {
        let seq = self.next_sequence;
        self.next_sequence += 1;
        seq
    }

    pub fn events_generated(&self) -> u64 {
        self.events_generated
    }

    pub fn queries_dispatched(&self) -> u64 {
        self.queries_dispatched
    }
}
