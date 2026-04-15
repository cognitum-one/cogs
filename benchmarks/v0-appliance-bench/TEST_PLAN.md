# Cognitum v0 Appliance Emulator -- Complete Test Plan

## 1. Overview

This test plan covers the benchmark harness, acceptance tests, fault injection,
and optimization strategy for the Cognitum v0 Appliance Emulator as defined in
`docs/v0-appliance.md`.

**System under test:**
- 1 host (A57 in QEMU, running RuVector + Postgres + coherence gate)
- 7 STM32 tile simulators (separate processes, Unix socket transport)
- Binary message protocol with CRC32 and optional Ed25519 signatures
- MinCut-driven coherence gate controlling learning writes

---

## 2. Acceptance Tests

### Test 1: 30-Minute Endurance (Medium Load)

| Item | Detail |
|------|--------|
| **Duration** | 30 minutes |
| **Spike rate** | 1,000 events/sec (medium) |
| **Query load** | 100 queries/sec (medium) |
| **Tile count** | 7 |
| **Fault injection** | None |
| **Kernel mix** | Default (K1 30%, K2 25%, K3 15%, K4 10%, K5 10%, K6 10%) |

**Pass criteria:**

| # | Criterion | Target | How Measured |
|---|-----------|--------|--------------|
| 1 | Protocol errors | 0 | Count NACKs across all tiles |
| 2 | Tick latency p95 | < 1 ms (1,000 us) | LatencyHistogram.p95() |
| 3 | All tiles alive | 7/7 at end | Heartbeat liveness check |
| 4 | Query latency p95 | < 50 ms | DbMetrics.query_latency.p95() |
| 5 | Audit log complete | Witness chain valid | verify_audit_chain() |

**Procedure:**
1. Boot QEMU host image.
2. Start Postgres + ruvector-postgres extension.
3. Start all 5 host services (ruvector, tile router, coherence gate, audit, API).
4. Start 7 tile simulators with default config.
5. Run WorkloadGenerator with `endurance_medium()` config for 30 min.
6. Collect MetricsSnapshot every 1 second.
7. After 30 min, drain outstanding responses.
8. Verify all pass criteria.

---

### Test 2: Coherence Gate Response

| Item | Detail |
|------|--------|
| **Duration** | 15 minutes |
| **Spike rate** | 1,000 events/sec (medium) |
| **Fault plan** | Drop 90% of messages at t=5min; stop drops at t=10min |

**Pass criteria:**

| # | Criterion | Target | How Measured |
|---|-----------|--------|--------------|
| 1 | Writes blocked | Within 1 tick of threshold breach | SimulatedGate.verify_block_within(N+1) |
| 2 | Audit event | Recorded for block transition | Query audit_log table |
| 3 | Quarantine queue | Grows while blocked | Quarantine queue length > 0 |
| 4 | Scheduler switch | Stabilization kernels only | Only K5/K6 assigned during block |
| 5 | No replay accepted | 0 protocol errors | NACK with ReplayDetected = 0 |
| 6 | Writes re-enabled | After hysteresis recovery | SimulatedGate.first_recovery_tick() is Some |
| 7 | Throttle ramp-down | Gradual 3 -> 2 -> 1 -> 0 | GateOutput.throttle history |

**Procedure:**
1. Start system as in Test 1.
2. At t=0, begin medium workload.
3. At t=5 min, FaultInjector activates 90% message drop rate.
   - This causes tile heartbeats to stop arriving.
   - Coherence score drops below `block_threshold` (0.6).
   - Gate must block writes within `consecutive_ticks_to_block` ticks.
4. Verify: WRITE_ALLOW = false, BURST_ALLOW = false, THROTTLE = Heavy.
5. Verify: audit event logged with witness (cut value, partition witness).
6. Verify: scheduler only assigns K5 (stabilization) and K6 (health) kernels.
7. At t=10 min, FaultInjector sets drop rate to 0%.
   - Heartbeats resume, coherence score recovers above `unblock_threshold` (0.8).
8. Verify: writes re-enabled after `consecutive_ticks_to_unblock` consecutive ticks.
9. Verify: throttle ramps down gradually (3 -> 2 -> 1 -> 0).
10. Verify: no replayed messages were accepted during the transition.

---

### Test 3: Tile Failure Recovery

| Item | Detail |
|------|--------|
| **Duration** | 6 minutes |
| **Spike rate** | 1,000 events/sec (medium) |
| **Fault plan** | Freeze tile 3 at t=2min; restart at t=4min with stale epoch |

**Pass criteria:**

| # | Criterion | Target | How Measured |
|---|-----------|--------|--------------|
| 1 | Detection time | Within heartbeat timeout (100 ms) | Tile 3 marked dead within timeout |
| 2 | Work reassigned | No tasks dropped | Tasks re-routed to tiles 0-2, 4-6 |
| 3 | Audit event | Logged for tile failure | Query audit_log for component="tile_3" |
| 4 | Recovery time | < 2 seconds | MetricsCollector.recovery_times[3] |
| 5 | Epoch sync | Host sends RESET | Tile 3 receives RESET with current epoch |
| 6 | All tiles alive | 7/7 at end | Heartbeat liveness check |

**Procedure:**
1. Start system as in Test 1.
2. At t=2 min, FaultInjector freezes tile 3 (no heartbeats, no task results).
3. Verify: host detects tile 3 failure within `heartbeat_timeout` (100 ms).
4. Verify: host reassigns tile 3's pending tasks to other tiles.
5. Verify: audit event logged with tile_id=3, decision="failure_detected".
6. Continue workload with 6 tiles for 2 minutes.
7. At t=4 min, FaultInjector unfreezes tile 3 (simulates restart).
   - Tile 3 sends HELLO with stale epoch.
8. Verify: host sends RESET message to tile 3 with current epoch.
9. Verify: tile 3 responds with HEARTBEAT showing synced epoch.
10. Verify: recovery time (from unfreeze to first successful task result) < 2 s.
11. Verify: all 7 tiles alive at t=6 min.

---

## 3. Benchmark Profiles

### 3.1 Smoke (60 seconds)

Quick validation that the harness and emulator are functional.

```
WorkloadConfig::smoke()
- duration: 60s
- spike_rate: Medium (1,000/s)
- query: Light (10/s)
- faults: None
```

### 3.2 Endurance (30 minutes)

Long-running stability test.

```
WorkloadConfig::endurance_medium()
- duration: 30 min
- spike_rate: Medium (1,000/s)
- query: Medium (100/s)
- faults: None
```

### 3.3 Burst (5 minutes)

Overload test at 10x nominal rate.

```
WorkloadConfig::burst()
- duration: 5 min
- spike_rate: High (10,000/s)
- query: Heavy (1,000/s)
- faults: None
```

### 3.4 Chaos (4 minutes)

All fault types activated in sequence.

```
FaultPlan::chaos()
- t=0:30  light drops (5%)
- t=1:00  delay spike (5-50ms)
- t=1:30  replay attack (3x)
- t=2:00  corrupt payload (bit-flip)
- t=2:30  slow tile 5 (4x, 30s)
- t=3:00  malformed result from tile 1
- t=3:30  freeze tile 6 (20s)
```

---

## 4. Metrics Collection Matrix

| Metric | Source | Histogram/Counter | Target |
|--------|--------|-------------------|--------|
| Tick latency (ns) | tick_start.elapsed() | LatencyHistogram (1M samples) | p95 < 1 ms |
| Write-allowed % | CoherenceTracker.record_tick() | Sliding window (60s) | -- |
| Tasks/sec | TileMetrics.tasks_completed | ThroughputCounter (10s window) | -- |
| Events/sec | WorkloadGenerator.events_generated | ThroughputCounter (10s window) | -- |
| Coherence transitions | CoherenceTracker.transitions | Counter | -- |
| Recovery time | MetricsCollector.recovery_times | Duration map | < 2 s |
| CPU % per component | CpuAccounting.stop() | Per-component ns | -- |
| Postgres query latency | DbMetrics.query_latency | LatencyHistogram (100K) | -- |
| Memory per tile | /proc/PID/statm | TileMemoryProfile | < 2 MB |
| Transport msg/sec | TransportMetrics.messages_sent | ThroughputCounter (10s) | -- |
| Backpressure events | TransportMetrics.backpressure_events | Counter | 0 |
| Protocol errors | TileMetrics.protocol_errors | Counter | 0 |
| Heartbeat interval | TileMetrics.heartbeat_interval | LatencyHistogram (10K) | -- |

---

## 5. Fault Injection Matrix

| Fault | Parameters | Expected System Response |
|-------|------------|--------------------------|
| Message drop | rate: 0.05-0.90, filter: type/tile | NACK from receiver; host retransmits or detects failure |
| Message delay | min: 1-50ms, max: 10-100ms | Tasks may timeout; scheduler adjusts deadlines |
| Message replay | count: 1-10, interval: 1-100ms | Replay protection rejects; NACK with ReplayDetected |
| Payload corrupt (bit-flip) | bits: 1-8 | CRC mismatch; NACK with BadCrc |
| Payload corrupt (truncate) | max_bytes: 10 | Parse failure; NACK with BadMagic or similar |
| Payload corrupt (zero CRC) | -- | CRC mismatch; NACK with BadCrc |
| Payload corrupt (bad magic) | -- | Immediate rejection; NACK with BadMagic |
| Tile freeze | tile: 0-6, duration: 1s-5min | Host detects via missing heartbeats; reassigns work |
| Tile slowdown | tile: 0-6, factor: 2-10x, dur: 10-60s | Tasks return TIMEOUT; scheduler avoids tile |
| Malformed result (wrong hash) | tile: 0-6, count: 1-10 | Host rejects result; audit event; requeue task |
| Malformed result (unknown job) | tile: 0-6, count: 1-10 | Host logs error; no state corruption |
| Malformed result (kernel mismatch) | tile: 0-6, count: 1-10 | Host logs error; quarantine result |

---

## 6. Reporting Outputs

### 6.1 Real-Time Dashboard

Printed every 1 second during a run:

```
t=30s | ticks=30000 | p95=420us | p99=780us | max=1200us | write%=100.0 | gate_tx=0 | err=0 | tiles=7/7
t=31s | ticks=31000 | p95=415us | p99=770us | max=1200us | write%=100.0 | gate_tx=0 | err=0 | tiles=7/7
```

### 6.2 JSON Report

Machine-readable output for CI and regression tracking.
Contains all metrics, time-series data, fault events, and acceptance verdicts.

**CI integration:**
```yaml
# .github/workflows/benchmark.yml
- name: Run benchmarks
  run: cargo run --release --bin v0_bench -- --profile smoke --json --output-dir ./results

- name: Check regression
  run: cargo run --release --bin v0_bench -- --regression --baseline ./baseline.json

- name: Upload artifacts
  uses: actions/upload-artifact@v4
  with:
    name: benchmark-results
    path: ./results/
```

### 6.3 HTML Report

Human-readable report with:
- Acceptance criteria pass/fail table
- Tick latency distribution table
- Coherence gate timeline
- Transport statistics
- Fault injection event log
- Time-series data point count (charts rendered via embedded SVG)

### 6.4 Regression Detection

Compares current run against a baseline JSON report.
Flags regressions exceeding a configurable tolerance (default 5%).

Checked metrics:
- tick_latency_p95_us (lower is better)
- tick_latency_p99_us (lower is better)
- protocol_errors (fewer is better)
- write_allowed_pct (higher is better)
- db_query_latency_p95_us (lower is better)

Exit code: 0 = pass, 1 = regression detected.

---

## 7. Optimization Strategy

### 7.1 Profiling Approach

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `perf record + inferno` | CPU flamegraph | Identify hotspots in tick loop |
| `valgrind --tool=dhat` | Heap allocation profiling | Find excessive allocations |
| `tokio-console` | Async task inspection | Debug contention in async runtime |
| `/proc/PID/statm` | Per-process memory | Track tile process RSS |
| `CpuAccounting` (built-in) | Per-component CPU time | Attribute time to subsystems |
| `TransportAnalysis` (built-in) | Transport efficiency | Optimize message batching |

### 7.2 Key Optimization Targets

| Priority | Component | Bottleneck | Optimization | Expected Gain |
|----------|-----------|------------|--------------|---------------|
| P0 | Tick loop | Lock contention | Lock-free SPSC queues per tile | 2-3x latency reduction |
| P0 | Transport | Syscall overhead | io_uring or sendmmsg batching | 4-8x throughput |
| P1 | CRC32 | Per-message CPU | SIMD CRC32C (SSE4.2 / ARMv8) | 10x CRC speed |
| P1 | Message framing | Allocation per msg | Arena allocator + bump alloc | Zero alloc per tick |
| P2 | Coherence gate | Full MinCut per tick | Incremental cut updates | 50-100x for gate |
| P2 | Postgres writes | fsync latency | WAL batching (group commit) | 5-10x write throughput |
| P3 | Tile processes | Process overhead | Thread pool (avoid fork/exec) | Faster startup |
| P3 | Audit log | Append overhead | Memory-mapped append-only file | Reduced I/O |

### 7.3 Memory Optimization (7 Tile Processes)

Target: < 2 MB RSS per tile process (14 MB total for 7 tiles).

| Component | Budget | Technique |
|-----------|--------|-----------|
| Thread stack | 64 KB | `RLIMIT_STACK` or `pthread_attr_setstack` |
| Kernel working set | 1 MB | Fixed-size buffers per kernel catalog |
| Message ring buffer | 512 KB | Pre-allocated, reusable |
| Misc (TLS, libc) | ~50 KB | Static linking to reduce .so overhead |
| **Total per tile** | **~1.6 MB** | |
| **Total 7 tiles** | **~11.2 MB** | |

### 7.4 Transport Optimization

| Technique | Description | Syscall Reduction |
|-----------|-------------|-------------------|
| **sendmmsg / recvmmsg** | Batch multiple messages per syscall | 4-8x |
| **UNIX SEQPACKET** | Kernel-maintained message boundaries | Simpler framing |
| **io_uring** | Async I/O without epoll overhead | 2-4x for small messages |
| **Shared memory** | Bypass kernel for local transport | Near-zero latency |
| **Zero-copy** | sendmsg with MSG_ZEROCOPY | Avoid memcpy in send path |
| **Message coalescing** | Buffer N messages, flush every tick | Fewer sends per tick |

---

## 8. File Manifest

```
benchmarks/v0-appliance-bench/
  Cargo.toml                    -- Crate manifest
  TEST_PLAN.md                  -- This document
  src/
    lib.rs                      -- Crate root, module declarations
    main.rs                     -- CLI benchmark runner
    acceptance.rs               -- Acceptance test runner binary
    protocol.rs                 -- Binary message types (v0 spec mirror)
    workload.rs                 -- Workload generation (spike, query, kernel mix)
    fault.rs                    -- Fault injection framework
    metrics.rs                  -- Metrics collection (histograms, counters)
    coherence.rs                -- Coherence gate test helpers
    harness.rs                  -- Top-level orchestrator + acceptance tests
    report.rs                   -- Reporting (JSON, HTML, regression, CI)
    profile.rs                  -- Profiling and optimization helpers
```

---

## 9. Execution Checklist

### Pre-flight
- [ ] QEMU host image boots and Postgres starts
- [ ] ruvector-postgres extension loaded
- [ ] All 5 host services start via systemd
- [ ] 7 tile simulators connect over Unix sockets
- [ ] `v0_bench --profile smoke` completes without error

### Acceptance Gate
- [ ] Test 1 passes (30 min endurance, zero errors, p95 < 1 ms)
- [ ] Test 2 passes (coherence gate blocks within 1 tick, recovers with hysteresis)
- [ ] Test 3 passes (tile failure detected, recovery < 2 s, epoch sync)

### Performance Baseline
- [ ] Baseline JSON report saved to `benchmarks/v0-appliance-bench/baseline.json`
- [ ] CI regression check integrated into PR workflow
- [ ] Flamegraph captured and hotspots documented

### Optimization Validation
- [ ] Lock-free queues benchmarked vs mutex baseline
- [ ] Transport batching benchmarked (msgs/syscall > 4)
- [ ] Per-tile memory within 2 MB budget
- [ ] CRC32 using SIMD intrinsics
