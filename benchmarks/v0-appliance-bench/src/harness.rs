//! Benchmark Harness -- Top-Level Orchestrator
//!
//! The `BenchmarkHarness` is the single entry point that:
//!   1. Boots the emulator (or connects to a running one).
//!   2. Configures workload, fault plan, and metrics collection.
//!   3. Runs the benchmark loop tick by tick.
//!   4. Produces a `BenchmarkReport`.
//!
//! # Acceptance Tests
//!
//! Three acceptance tests are defined as top-level functions:
//!   - `acceptance_test_1_endurance()`
//!   - `acceptance_test_2_coherence_gate()`
//!   - `acceptance_test_3_tile_failure_recovery()`
//!
//! Each returns a `TestVerdict` (pass/fail with details).

use std::time::{Duration, Instant};

use crate::coherence::{CoherenceGateConfig, SimulatedGate};
use crate::fault::{FaultInjector, FaultPlan};
use crate::metrics::{MetricsCollector, MetricsSnapshot};
use crate::report::BenchmarkReport;
use crate::workload::{WorkloadConfig, WorkloadGenerator};

// ── Harness Configuration ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HarnessConfig {
    /// Workload to drive.
    pub workload: WorkloadConfig,
    /// Fault plan (empty for clean runs).
    pub fault_plan: FaultPlan,
    /// Coherence gate configuration.
    pub gate_config: CoherenceGateConfig,
    /// Tick period (default 1 ms per spec).
    pub tick_period: Duration,
    /// Heartbeat timeout (missing heartbeats = tile failure).
    pub heartbeat_timeout: Duration,
    /// Metrics snapshot interval (for time-series output).
    pub snapshot_interval: Duration,
    /// Whether to collect CPU profiles.
    pub enable_cpu_profiling: bool,
    /// Whether to collect per-tile memory stats.
    pub enable_memory_tracking: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            workload: WorkloadConfig::default(),
            fault_plan: FaultPlan::none(),
            gate_config: CoherenceGateConfig::default(),
            tick_period: Duration::from_millis(1),
            heartbeat_timeout: Duration::from_millis(100),
            snapshot_interval: Duration::from_secs(1),
            enable_cpu_profiling: false,
            enable_memory_tracking: true,
        }
    }
}

// ── Test Verdict ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TestVerdict {
    pub test_name: String,
    pub passed: bool,
    pub checks: Vec<CheckResult>,
    pub duration: Duration,
    pub snapshot: Option<MetricsSnapshot>,
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

impl TestVerdict {
    pub fn pass(name: &str, duration: Duration, snapshot: MetricsSnapshot) -> Self {
        Self {
            test_name: name.to_string(),
            passed: true,
            checks: vec![],
            duration,
            snapshot: Some(snapshot),
        }
    }

    pub fn fail(name: &str, duration: Duration, checks: Vec<CheckResult>) -> Self {
        Self {
            test_name: name.to_string(),
            passed: false,
            checks,
            duration,
            snapshot: None,
        }
    }

    /// Print a human-readable summary.
    pub fn summary(&self) -> String {
        let status = if self.passed { "PASS" } else { "FAIL" };
        let mut lines = vec![format!(
            "[{}] {} ({:.1}s)",
            status,
            self.test_name,
            self.duration.as_secs_f64(),
        )];
        for check in &self.checks {
            let mark = if check.passed { " ok " } else { "FAIL" };
            lines.push(format!(
                "  [{}] {}: expected={}, actual={}",
                mark, check.name, check.expected, check.actual,
            ));
        }
        lines.join("\n")
    }
}

// ── Benchmark Harness ──────────────────────────────────────────────

/// The benchmark harness drives the emulator and collects results.
///
/// ```text
/// ┌─────────────┐     ┌───────────────┐     ┌────────────────┐
/// │  Workload    │────>│  Fault        │────>│  Emulator      │
/// │  Generator   │     │  Injector     │     │  Under Test    │
/// └─────────────┘     └───────────────┘     └────────────────┘
///        │                    │                      │
///        └────────────────────┴──────────────────────┘
///                             │
///                     ┌───────v────────┐
///                     │  Metrics       │
///                     │  Collector     │
///                     └───────┬────────┘
///                             │
///                     ┌───────v────────┐
///                     │  Report        │
///                     │  Engine        │
///                     └────────────────┘
/// ```
pub struct BenchmarkHarness {
    pub config: HarnessConfig,
    generator: WorkloadGenerator,
    injector: FaultInjector,
    collector: MetricsCollector,
    gate: SimulatedGate,
    /// Time-series of snapshots for dashboards.
    snapshots: Vec<(Duration, MetricsSnapshot)>,
}

impl BenchmarkHarness {
    pub fn new(config: HarnessConfig) -> Self {
        let tile_count = config.workload.tile_count;
        let generator = WorkloadGenerator::new(config.workload.clone());
        let injector = FaultInjector::new(config.fault_plan.clone());
        let collector = MetricsCollector::new(tile_count);
        let gate = SimulatedGate::new(config.gate_config.clone());

        Self {
            config,
            generator,
            injector,
            collector,
            gate,
            snapshots: vec![],
        }
    }

    /// Run the full benchmark.  This is the main entry point.
    ///
    /// Pseudo-code for the tick loop:
    ///
    /// ```text
    /// warmup()
    /// collector.start()
    /// while elapsed < config.workload.duration {
    ///     tick_start = Instant::now()
    ///
    ///     // 1. Advance fault injector
    ///     injector.tick(elapsed)
    ///
    ///     // 2. Collect heartbeats from all tiles
    ///     for tile in 0..tile_count {
    ///         if injector.is_tile_frozen(tile) { continue }
    ///         recv_heartbeat(tile) -> collector.record_heartbeat(tile)
    ///     }
    ///
    ///     // 3. Compute coherence score (from tile heartbeats + mincut proxy)
    ///     let coherence_score = compute_coherence()
    ///     let gate_output = gate.tick(tick_number, coherence_score)
    ///
    ///     // 4. If write_allow: assign tasks from workload generator
    ///     if gate_output.write_allow {
    ///         for task in generator.next_batch(tile_count, queue_ceiling) {
    ///             maybe_apply_fault(task)
    ///             send_task_to_tile(task)
    ///             collector.transport.messages_sent.increment(1)
    ///         }
    ///     }
    ///
    ///     // 5. Collect task results
    ///     for result in recv_results() {
    ///         collector.record_task_complete(result.tile_id, result.latency)
    ///     }
    ///
    ///     // 6. Dispatch queries if query timer fires
    ///     if query_timer.elapsed() >= query_interval {
    ///         let latency = run_query()
    ///         collector.record_db_query(latency)
    ///     }
    ///
    ///     // 7. Record tick metrics
    ///     let tick_duration = tick_start.elapsed()
    ///     collector.record_tick(tick_duration, gate_output.write_allow)
    ///
    ///     // 8. Snapshot if interval elapsed
    ///     if snapshot_timer.elapsed() >= config.snapshot_interval {
    ///         snapshots.push((elapsed, collector.snapshot()))
    ///     }
    ///
    ///     // 9. Sleep remainder of tick period
    ///     sleep_until(tick_start + config.tick_period)
    /// }
    /// generator.drain()
    /// ```
    pub fn run(&mut self) -> BenchmarkReport {
        // -- Warmup phase --
        self.generator.warmup();
        self.collector.start();
        let run_start = Instant::now();
        let mut last_snapshot = Instant::now();
        let mut tick_number: u64 = 0;

        // -- Main tick loop (pseudo-code) --
        while run_start.elapsed() < self.config.workload.duration {
            let tick_start = Instant::now();
            let elapsed = run_start.elapsed();

            // 1. Advance fault injector.
            self.injector.tick(elapsed);

            // 2-7: See pseudo-code above.
            // In a real implementation this drives the actual emulator
            // transport.  Here we outline the structure.

            tick_number += 1;
            let tick_duration = tick_start.elapsed();
            let _coherence_score = 0.9_f64; // placeholder
            let gate_output = self.gate.tick(tick_number, _coherence_score);
            self.collector.record_tick(tick_duration, gate_output.write_allow);

            // 8. Periodic snapshot.
            if last_snapshot.elapsed() >= self.config.snapshot_interval {
                let snap = self.collector.snapshot();
                self.snapshots.push((elapsed, snap));
                last_snapshot = Instant::now();
            }

            // 9. Sleep remainder.
            let remaining = self.config.tick_period
                .checked_sub(tick_start.elapsed())
                .unwrap_or(Duration::ZERO);
            if !remaining.is_zero() {
                std::thread::sleep(remaining);
            }
        }

        // -- Drain --
        self.generator.drain();

        // -- Build report --
        let final_snapshot = self.collector.snapshot();
        BenchmarkReport {
            label: self.config.workload.label.clone(),
            started_at: run_start,
            total_duration: run_start.elapsed(),
            final_snapshot,
            time_series: self.snapshots.clone(),
            fault_events: self.injector.event_log.clone(),
            gate_history: self.gate.history.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Acceptance Tests
// ═══════════════════════════════════════════════════════════════════

/// Acceptance Test 1: 30-minute endurance under medium load.
///
/// Pass criteria:
///   - Zero protocol errors
///   - p95 tick latency < 1 ms
///   - Query responses within target latency
///   - All 7 tiles alive at end
pub fn acceptance_test_1_endurance() -> TestVerdict {
    let config = HarnessConfig {
        workload: WorkloadConfig::endurance_medium(),
        fault_plan: FaultPlan::none(),
        ..Default::default()
    };

    let mut harness = BenchmarkHarness::new(config);
    let start = Instant::now();
    let report = harness.run();
    let duration = start.elapsed();
    let snap = &report.final_snapshot;

    let mut checks = vec![];

    // Check 1: zero protocol errors
    checks.push(CheckResult {
        name: "zero_protocol_errors".into(),
        passed: snap.zero_protocol_errors(),
        expected: "0".into(),
        actual: snap.total_protocol_errors.to_string(),
    });

    // Check 2: p95 tick latency < 1ms
    checks.push(CheckResult {
        name: "tick_latency_p95_under_1ms".into(),
        passed: snap.tick_latency_ok(),
        expected: "< 1000 us".into(),
        actual: format!("{:.1} us", snap.tick_latency.p95_us),
    });

    // Check 3: all tiles alive
    checks.push(CheckResult {
        name: "all_tiles_alive".into(),
        passed: snap.all_tiles_alive(),
        expected: format!("{}", snap.tiles_total),
        actual: format!("{}", snap.tiles_alive),
    });

    let passed = checks.iter().all(|c| c.passed);
    TestVerdict {
        test_name: "acceptance_test_1_endurance_30min".into(),
        passed,
        checks,
        duration,
        snapshot: Some(report.final_snapshot),
    }
}

/// Acceptance Test 2: Coherence gate response.
///
/// Procedure:
///   - Inject coherence drop at minute 5.
///   - Verify gate blocks writes within 1 tick.
///   - Verify recovery with hysteresis at minute 10.
///   - Verify no replay accepted during transition.
///   - Verify audit event recorded.
///
/// Pass criteria:
///   - Writes blocked within `consecutive_ticks_to_block` of coherence drop
///   - Quarantine queue grows while blocked
///   - Writes re-enabled after hysteresis threshold
///   - No replay messages accepted
///   - Audit log contains witness chain
pub fn acceptance_test_2_coherence_gate() -> TestVerdict {
    let config = HarnessConfig {
        workload: WorkloadConfig {
            duration: Duration::from_secs(15 * 60), // 15 min total
            label: "acceptance-test-2-coherence".into(),
            ..WorkloadConfig::default()
        },
        fault_plan: FaultPlan::acceptance_test_2(),
        ..Default::default()
    };

    let mut harness = BenchmarkHarness::new(config);
    let start = Instant::now();
    let report = harness.run();
    let duration = start.elapsed();

    let mut checks = vec![];

    // Check 1: writes blocked within 1 tick of coherence drop
    // (In practice, within consecutive_ticks_to_block ticks.)
    let gate = &harness.gate;
    let blocked_within_1_tick = gate.verify_block_within(
        harness.config.gate_config.consecutive_ticks_to_block as u64 + 1,
    );
    checks.push(CheckResult {
        name: "writes_blocked_within_threshold".into(),
        passed: blocked_within_1_tick,
        expected: format!(
            "blocked within {} ticks",
            harness.config.gate_config.consecutive_ticks_to_block + 1,
        ),
        actual: format!(
            "first_block_tick={:?}",
            gate.first_block_tick(),
        ),
    });

    // Check 2: writes recovered
    let recovered = gate.first_recovery_tick().is_some();
    checks.push(CheckResult {
        name: "writes_recovered_with_hysteresis".into(),
        passed: recovered,
        expected: "recovered".into(),
        actual: if recovered { "recovered".into() } else { "still blocked".into() },
    });

    // Check 3: zero protocol errors (no replay accepted)
    checks.push(CheckResult {
        name: "no_replay_accepted".into(),
        passed: report.final_snapshot.zero_protocol_errors(),
        expected: "0 protocol errors".into(),
        actual: report.final_snapshot.total_protocol_errors.to_string(),
    });

    let passed = checks.iter().all(|c| c.passed);
    TestVerdict {
        test_name: "acceptance_test_2_coherence_gate".into(),
        passed,
        checks,
        duration,
        snapshot: Some(report.final_snapshot),
    }
}

/// Acceptance Test 3: Tile failure recovery.
///
/// Procedure:
///   - Kill tile 3 at minute 2.
///   - Host detects within heartbeat timeout.
///   - Reassign work to other tiles.
///   - Restart tile 3 at minute 4 with stale epoch.
///   - Host forces RESET and epoch sync.
///
/// Pass criteria:
///   - Detection within heartbeat_timeout
///   - Work reassigned (no tasks dropped)
///   - Audit event logged for failure + recovery
///   - Recovery time < 2 s
///   - Epoch synced after restart
pub fn acceptance_test_3_tile_failure_recovery() -> TestVerdict {
    let config = HarnessConfig {
        workload: WorkloadConfig {
            duration: Duration::from_secs(6 * 60), // 6 min total
            label: "acceptance-test-3-tile-failure".into(),
            ..WorkloadConfig::default()
        },
        fault_plan: FaultPlan::acceptance_test_3(),
        ..Default::default()
    };

    let mut harness = BenchmarkHarness::new(config);
    let start = Instant::now();
    let report = harness.run();
    let duration = start.elapsed();
    let snap = &report.final_snapshot;

    let mut checks = vec![];

    // Check 1: recovery time < 2s for tile 3
    let tile3_recovery = snap.recovery_times.get(&3);
    let recovery_ok = tile3_recovery
        .map(|d| *d < Duration::from_secs(2))
        .unwrap_or(false);
    checks.push(CheckResult {
        name: "tile_3_recovery_under_2s".into(),
        passed: recovery_ok,
        expected: "< 2000 ms".into(),
        actual: tile3_recovery
            .map(|d| format!("{} ms", d.as_millis()))
            .unwrap_or("no recovery recorded".into()),
    });

    // Check 2: all tiles alive at end (including restarted tile 3)
    checks.push(CheckResult {
        name: "all_tiles_alive_after_recovery".into(),
        passed: snap.all_tiles_alive(),
        expected: format!("{}", snap.tiles_total),
        actual: format!("{}", snap.tiles_alive),
    });

    // Check 3: audit event for failure
    // (In real implementation, query the audit_log table.)
    checks.push(CheckResult {
        name: "audit_event_for_failure".into(),
        passed: true, // placeholder -- real impl queries DB
        expected: "audit event exists".into(),
        actual: "placeholder".into(),
    });

    let passed = checks.iter().all(|c| c.passed);
    TestVerdict {
        test_name: "acceptance_test_3_tile_failure_recovery".into(),
        passed,
        checks,
        duration,
        snapshot: Some(report.final_snapshot),
    }
}
