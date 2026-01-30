//! Metrics Collection and Statistical Analysis
//!
//! Collects all metrics specified in the v0 benchmark requirements:
//!
//! | Metric                        | Collection Method          | Target        |
//! |-------------------------------|----------------------------|---------------|
//! | Tick latency (p50/p95/p99/max)| Per-tick Instant diff      | p95 < 1 ms    |
//! | Write-allowed %               | Gate API sample/sec        | --            |
//! | Throughput (tasks/sec)        | Counter / elapsed          | --            |
//! | Throughput (events/sec)       | Counter / elapsed          | --            |
//! | Coherence gate transitions    | State-change counter       | --            |
//! | Recovery time                 | Duration(fail..recover)    | < 2 s         |
//! | CPU usage per component       | /proc/stat or getrusage    | --            |
//! | Postgres query latency        | Per-query Instant diff     | --            |
//! | Memory per tile simulator     | /proc/PID/statm            | --            |
//! | Transport msg throughput      | Counter / elapsed          | --            |
//! | Backpressure events           | Counter                    | --            |

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── Histogram ──────────────────────────────────────────────────────

/// HDR-style histogram for latency tracking.
/// Stores raw samples up to a cap, then switches to reservoir sampling.
pub struct LatencyHistogram {
    /// Sorted on demand for percentile queries.
    samples: Vec<u64>, // nanoseconds
    /// Maximum samples to retain (reservoir cap).
    max_samples: usize,
    /// Total observations (may exceed samples.len() after reservoir kicks in).
    total_count: u64,
    /// Running sum for mean calculation.
    running_sum: u64,
    /// Running max.
    running_max: u64,
    /// Running min.
    running_min: u64,
    /// Whether the sorted cache is stale.
    dirty: bool,
}

impl LatencyHistogram {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples.min(65536)),
            max_samples,
            total_count: 0,
            running_sum: 0,
            running_max: 0,
            running_min: u64::MAX,
            dirty: false,
        }
    }

    /// Record a single observation.
    pub fn record(&mut self, nanos: u64) {
        self.total_count += 1;
        self.running_sum += nanos;
        self.running_max = self.running_max.max(nanos);
        self.running_min = self.running_min.min(nanos);

        if self.samples.len() < self.max_samples {
            self.samples.push(nanos);
        } else {
            // Reservoir sampling: replace a random element.
            // pseudo: let idx = rng.gen_range(0..self.total_count);
            // if idx < self.max_samples as u64 {
            //     self.samples[idx as usize] = nanos;
            // }
            let idx = (self.total_count % self.max_samples as u64) as usize;
            self.samples[idx] = nanos;
        }
        self.dirty = true;
    }

    /// Record a Duration directly.
    pub fn record_duration(&mut self, d: Duration) {
        self.record(d.as_nanos() as u64);
    }

    fn ensure_sorted(&mut self) {
        if self.dirty {
            self.samples.sort_unstable();
            self.dirty = false;
        }
    }

    pub fn count(&self) -> u64 {
        self.total_count
    }

    pub fn mean_ns(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        self.running_sum as f64 / self.total_count as f64
    }

    pub fn max_ns(&self) -> u64 {
        self.running_max
    }

    pub fn min_ns(&self) -> u64 {
        if self.running_min == u64::MAX { 0 } else { self.running_min }
    }

    /// Return the value at the given percentile (0.0 - 1.0).
    pub fn percentile(&mut self, p: f64) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.ensure_sorted();
        let idx = ((self.samples.len() as f64 * p) as usize)
            .min(self.samples.len() - 1);
        self.samples[idx]
    }

    pub fn p50(&mut self) -> u64 { self.percentile(0.50) }
    pub fn p95(&mut self) -> u64 { self.percentile(0.95) }
    pub fn p99(&mut self) -> u64 { self.percentile(0.99) }

    /// Return a snapshot suitable for serialization.
    pub fn snapshot(&mut self) -> LatencySnapshot {
        LatencySnapshot {
            count: self.total_count,
            mean_us: self.mean_ns() / 1_000.0,
            p50_us: self.p50() as f64 / 1_000.0,
            p95_us: self.p95() as f64 / 1_000.0,
            p99_us: self.p99() as f64 / 1_000.0,
            max_us: self.max_ns() as f64 / 1_000.0,
            min_us: self.min_ns() as f64 / 1_000.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LatencySnapshot {
    pub count: u64,
    pub mean_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
    pub min_us: f64,
}

impl LatencySnapshot {
    /// Check whether p95 is under the target (microseconds).
    pub fn p95_under(&self, target_us: f64) -> bool {
        self.p95_us < target_us
    }
}

// ── Throughput Counter ─────────────────────────────────────────────

/// Sliding-window throughput counter.
pub struct ThroughputCounter {
    /// Ring buffer of (timestamp, count_delta) pairs.
    window: Vec<(Instant, u64)>,
    /// Window duration.
    window_size: Duration,
    /// Total count (never resets).
    total: u64,
}

impl ThroughputCounter {
    pub fn new(window_size: Duration) -> Self {
        Self {
            window: Vec::with_capacity(1024),
            window_size,
            total: 0,
        }
    }

    pub fn increment(&mut self, count: u64) {
        let now = Instant::now();
        self.window.push((now, count));
        self.total += count;
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        let cutoff = now - self.window_size;
        self.window.retain(|&(t, _)| t >= cutoff);
    }

    /// Current rate (per second) over the sliding window.
    pub fn rate_per_sec(&self) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.window.iter().map(|&(_, c)| c).sum();
        let elapsed = self.window.last().unwrap().0
            .duration_since(self.window.first().unwrap().0);
        let secs = elapsed.as_secs_f64().max(0.001);
        sum as f64 / secs
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

// ── Coherence Gate Tracker ─────────────────────────────────────────

/// Tracks coherence gate state transitions and timing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoherenceState {
    /// Writes allowed, normal operation.
    WriteAllowed,
    /// Writes blocked, stabilization mode.
    WriteBlocked,
    /// Transitioning (hysteresis ramp-up).
    Recovering,
}

pub struct CoherenceTracker {
    /// Current state.
    pub state: CoherenceState,
    /// Timestamp of the most recent state change.
    pub last_transition: Instant,
    /// History of (timestamp, old_state, new_state).
    pub transitions: Vec<(Instant, CoherenceState, CoherenceState)>,
    /// Sliding window of write_allowed samples (true/false per tick).
    write_allowed_window: Vec<(Instant, bool)>,
    /// Window duration for write_allowed percentage.
    window_size: Duration,
}

impl CoherenceTracker {
    pub fn new() -> Self {
        Self {
            state: CoherenceState::WriteAllowed,
            last_transition: Instant::now(),
            transitions: vec![],
            write_allowed_window: vec![],
            window_size: Duration::from_secs(60),
        }
    }

    /// Record a tick's write-allowed status.
    pub fn record_tick(&mut self, write_allowed: bool) {
        let now = Instant::now();
        self.write_allowed_window.push((now, write_allowed));

        // Detect state change.
        let new_state = if write_allowed {
            CoherenceState::WriteAllowed
        } else {
            CoherenceState::WriteBlocked
        };

        if new_state != self.state {
            self.transitions.push((now, self.state.clone(), new_state.clone()));
            self.state = new_state;
            self.last_transition = now;
        }

        // Prune old samples.
        let cutoff = now - self.window_size;
        self.write_allowed_window.retain(|&(t, _)| t >= cutoff);
    }

    /// Fraction of ticks in the window where writes were allowed.
    pub fn write_allowed_pct(&self) -> f64 {
        if self.write_allowed_window.is_empty() {
            return 1.0;
        }
        let allowed = self.write_allowed_window.iter().filter(|&&(_, w)| w).count();
        allowed as f64 / self.write_allowed_window.len() as f64
    }

    /// Total number of state transitions observed.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Time spent in current state.
    pub fn time_in_current_state(&self) -> Duration {
        self.last_transition.elapsed()
    }
}

// ── Per-Tile Metrics ───────────────────────────────────────────────

pub struct TileMetrics {
    pub tile_id: u8,
    /// Task completion latency histogram.
    pub task_latency: LatencyHistogram,
    /// Heartbeat interval histogram.
    pub heartbeat_interval: LatencyHistogram,
    /// Tasks completed counter.
    pub tasks_completed: ThroughputCounter,
    /// Protocol errors (NACKs received).
    pub protocol_errors: u64,
    /// Timeout count (task exceeded tick budget).
    pub timeouts: u64,
    /// Estimated memory usage in bytes.
    pub memory_bytes: usize,
    /// Whether the tile is currently alive.
    pub alive: bool,
    /// Timestamp of last heartbeat.
    pub last_heartbeat: Option<Instant>,
}

impl TileMetrics {
    pub fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            task_latency: LatencyHistogram::new(100_000),
            heartbeat_interval: LatencyHistogram::new(10_000),
            tasks_completed: ThroughputCounter::new(Duration::from_secs(10)),
            protocol_errors: 0,
            timeouts: 0,
            memory_bytes: 0,
            alive: true,
            last_heartbeat: None,
        }
    }
}

// ── Postgres Query Metrics ─────────────────────────────────────────

pub struct DbMetrics {
    pub query_latency: LatencyHistogram,
    pub queries_total: ThroughputCounter,
    pub write_gate_denials: u64,
    pub write_gate_allows: u64,
}

impl DbMetrics {
    pub fn new() -> Self {
        Self {
            query_latency: LatencyHistogram::new(100_000),
            queries_total: ThroughputCounter::new(Duration::from_secs(10)),
            write_gate_denials: 0,
            write_gate_allows: 0,
        }
    }
}

// ── Transport Metrics ──────────────────────────────────────────────

pub struct TransportMetrics {
    /// Messages sent (host -> tile).
    pub messages_sent: ThroughputCounter,
    /// Messages received (tile -> host).
    pub messages_received: ThroughputCounter,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
    /// Backpressure events (send buffer full).
    pub backpressure_events: u64,
}

impl TransportMetrics {
    pub fn new() -> Self {
        Self {
            messages_sent: ThroughputCounter::new(Duration::from_secs(10)),
            messages_received: ThroughputCounter::new(Duration::from_secs(10)),
            bytes_sent: 0,
            bytes_received: 0,
            backpressure_events: 0,
        }
    }
}

// ── Aggregate Metrics Collector ────────────────────────────────────

/// Top-level metrics collector aggregating all sub-collectors.
pub struct MetricsCollector {
    /// Global tick latency.
    pub tick_latency: LatencyHistogram,

    /// Per-tile metrics (indexed by tile_id).
    pub tiles: Vec<TileMetrics>,

    /// Coherence gate tracker.
    pub coherence: CoherenceTracker,

    /// Database metrics.
    pub db: DbMetrics,

    /// Transport metrics.
    pub transport: TransportMetrics,

    /// Recovery time measurements (tile_id -> duration).
    pub recovery_times: HashMap<u8, Duration>,

    /// CPU usage samples per component (component_name -> percentage).
    pub cpu_usage: HashMap<String, Vec<f64>>,

    /// Timestamp of collection start.
    pub started_at: Option<Instant>,

    /// Total ticks observed.
    pub total_ticks: u64,

    /// Total protocol errors across all tiles.
    pub total_protocol_errors: u64,
}

impl MetricsCollector {
    pub fn new(tile_count: usize) -> Self {
        let tiles = (0..tile_count)
            .map(|i| TileMetrics::new(i as u8))
            .collect();

        Self {
            tick_latency: LatencyHistogram::new(1_000_000), // 1M samples
            tiles,
            coherence: CoherenceTracker::new(),
            db: DbMetrics::new(),
            transport: TransportMetrics::new(),
            recovery_times: HashMap::new(),
            cpu_usage: HashMap::new(),
            started_at: None,
            total_ticks: 0,
            total_protocol_errors: 0,
        }
    }

    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    /// Called once per scheduler tick.
    pub fn record_tick(&mut self, tick_duration: Duration, write_allowed: bool) {
        self.total_ticks += 1;
        self.tick_latency.record_duration(tick_duration);
        self.coherence.record_tick(write_allowed);
    }

    /// Record a task completion on a specific tile.
    pub fn record_task_complete(&mut self, tile_id: u8, latency: Duration) {
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.task_latency.record_duration(latency);
            tile.tasks_completed.increment(1);
        }
    }

    /// Record a heartbeat from a tile.
    pub fn record_heartbeat(&mut self, tile_id: u8) {
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            if let Some(last) = tile.last_heartbeat {
                tile.heartbeat_interval.record_duration(last.elapsed());
            }
            tile.last_heartbeat = Some(Instant::now());
            tile.alive = true;
        }
    }

    /// Record that a tile has failed (missed heartbeats).
    pub fn record_tile_failure(&mut self, tile_id: u8) {
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.alive = false;
        }
    }

    /// Record that a tile has recovered.
    pub fn record_tile_recovery(&mut self, tile_id: u8, recovery_duration: Duration) {
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.alive = true;
        }
        self.recovery_times.insert(tile_id, recovery_duration);
    }

    /// Record a protocol error (NACK).
    pub fn record_protocol_error(&mut self, tile_id: u8) {
        if let Some(tile) = self.tiles.get_mut(tile_id as usize) {
            tile.protocol_errors += 1;
        }
        self.total_protocol_errors += 1;
    }

    /// Record a database query.
    pub fn record_db_query(&mut self, latency: Duration) {
        self.db.query_latency.record_duration(latency);
        self.db.queries_total.increment(1);
    }

    /// Record a write gate decision.
    pub fn record_write_gate(&mut self, allowed: bool) {
        if allowed {
            self.db.write_gate_allows += 1;
        } else {
            self.db.write_gate_denials += 1;
        }
    }

    /// Wall-clock duration since start.
    pub fn elapsed(&self) -> Duration {
        self.started_at
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Produce a full metrics snapshot.
    pub fn snapshot(&mut self) -> MetricsSnapshot {
        MetricsSnapshot {
            elapsed: self.elapsed(),
            total_ticks: self.total_ticks,
            tick_latency: self.tick_latency.snapshot(),
            write_allowed_pct: self.coherence.write_allowed_pct(),
            coherence_transitions: self.coherence.transition_count(),
            total_protocol_errors: self.total_protocol_errors,
            db_query_latency: self.db.query_latency.snapshot(),
            transport_msg_sent_rate: self.transport.messages_sent.rate_per_sec(),
            transport_msg_recv_rate: self.transport.messages_received.rate_per_sec(),
            transport_backpressure: self.transport.backpressure_events,
            recovery_times: self.recovery_times.clone(),
            tiles_alive: self.tiles.iter().filter(|t| t.alive).count(),
            tiles_total: self.tiles.len(),
        }
    }
}

// ── Serializable Snapshot ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MetricsSnapshot {
    pub elapsed: Duration,
    pub total_ticks: u64,
    pub tick_latency: LatencySnapshot,
    pub write_allowed_pct: f64,
    pub coherence_transitions: usize,
    pub total_protocol_errors: u64,
    pub db_query_latency: LatencySnapshot,
    pub transport_msg_sent_rate: f64,
    pub transport_msg_recv_rate: f64,
    pub transport_backpressure: u64,
    pub recovery_times: HashMap<u8, Duration>,
    pub tiles_alive: usize,
    pub tiles_total: usize,
}

impl MetricsSnapshot {
    /// Check acceptance target: p95 tick latency < 1ms (1000 us).
    pub fn tick_latency_ok(&self) -> bool {
        self.tick_latency.p95_under(1_000.0)
    }

    /// Check acceptance target: zero protocol errors.
    pub fn zero_protocol_errors(&self) -> bool {
        self.total_protocol_errors == 0
    }

    /// Check acceptance target: all recovery times < 2s.
    pub fn recovery_times_ok(&self) -> bool {
        self.recovery_times
            .values()
            .all(|d| *d < Duration::from_secs(2))
    }

    /// Check acceptance target: all tiles alive.
    pub fn all_tiles_alive(&self) -> bool {
        self.tiles_alive == self.tiles_total
    }
}
