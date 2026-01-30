//! Profiling and Optimization Helpers
//!
//! Provides integration points for:
//!   - CPU flamegraph capture (via perf + inferno)
//!   - Memory profiling (via jemalloc / DHAT)
//!   - tokio-console for async runtime inspection
//!   - Per-component CPU accounting
//!   - Transport optimization analysis (batching, zero-copy)
//!
//! # Optimization Strategy
//!
//! ## Phase 1: Identify Hotspots
//!
//! ```text
//! perf record -g --call-graph dwarf -- ./target/release/v0-bench ...
//! perf script | inferno-collapse-perf | inferno-flamegraph > flame.svg
//! ```
//!
//! ## Phase 2: Key Optimization Targets
//!
//! | Component              | Expected Bottleneck        | Optimization                |
//! |------------------------|----------------------------|-----------------------------|
//! | Scheduler tick loop    | Lock contention on queues  | Lock-free SPSC per tile     |
//! | Transport (Unix sock)  | Syscall overhead           | io_uring batched I/O        |
//! | Message framing        | Allocation per message     | Arena allocator + zero-copy |
//! | Coherence gate         | MinCut computation         | Incremental updates         |
//! | Postgres writes        | fsync latency              | WAL batching                |
//! | CRC32 computation      | Per-message CPU            | SIMD CRC32C intrinsics      |
//! | Tile processes (x7)    | Process spawn overhead     | Thread pool alternative     |
//!
//! ## Phase 3: Memory Optimization for 7 Tile Processes
//!
//! Each tile process should use < 2 MB resident:
//!   - Stack: 64 KB (reduced from default 8 MB)
//!   - Heap: ~1 MB for kernel working sets
//!   - Message buffers: ~512 KB (pre-allocated ring)
//!   - Total per tile: ~1.6 MB
//!   - Total 7 tiles: ~11.2 MB
//!
//! ## Phase 4: Transport Optimization
//!
//! | Technique          | Benefit                        | Complexity |
//! |--------------------|--------------------------------|------------|
//! | Message batching   | Reduce syscall count           | Low        |
//! | Zero-copy buffers  | Eliminate memcpy in send path   | Medium     |
//! | io_uring           | Async I/O without epoll         | High       |
//! | Shared memory      | Eliminate kernel crossing       | Medium     |
//! | UNIX SEQPACKET     | Message boundaries in kernel    | Low        |

use std::time::{Duration, Instant};

// ── CPU Profile Capture ────────────────────────────────────────────

/// Configuration for flamegraph capture.
#[derive(Clone, Debug)]
pub struct FlamegraphConfig {
    /// Duration to capture.
    pub capture_duration: Duration,
    /// Output SVG path.
    pub output_path: String,
    /// Sampling frequency (Hz).
    pub sample_freq: u32,
    /// Whether to include kernel stacks.
    pub kernel_stacks: bool,
}

impl Default for FlamegraphConfig {
    fn default() -> Self {
        Self {
            capture_duration: Duration::from_secs(30),
            output_path: String::from("benchmarks/v0-appliance-bench/output/flame.svg"),
            sample_freq: 99,
            kernel_stacks: false,
        }
    }
}

/// Start a flamegraph capture in a background thread.
///
/// In production, this shells out to `perf record` and `inferno-flamegraph`.
/// Here we define the interface.
pub fn start_flamegraph(config: &FlamegraphConfig, pid: u32) -> FlamegraphHandle {
    FlamegraphHandle {
        config: config.clone(),
        pid,
        started: Instant::now(),
    }
}

pub struct FlamegraphHandle {
    config: FlamegraphConfig,
    pid: u32,
    started: Instant,
}

impl FlamegraphHandle {
    /// Block until capture is complete and the SVG is written.
    pub fn wait(self) -> Result<String, String> {
        // pseudo:
        //   std::thread::sleep(self.config.capture_duration);
        //   Command::new("perf").args(["record", "-g", ...]).status()?;
        //   Command::new("perf").arg("script").pipe("inferno-collapse-perf")
        //     .pipe("inferno-flamegraph").arg("-o").arg(&self.config.output_path).status()?;
        Ok(self.config.output_path.clone())
    }
}

// ── Per-Component CPU Accounting ───────────────────────────────────

/// Tracks CPU time consumed by each logical component of the emulator.
pub struct CpuAccounting {
    components: Vec<ComponentCpu>,
}

struct ComponentCpu {
    name: String,
    /// Cumulative nanoseconds spent in this component.
    total_ns: u64,
    /// Number of invocations.
    invocations: u64,
    /// Currently timing?
    timer_start: Option<Instant>,
}

impl CpuAccounting {
    pub fn new(component_names: &[&str]) -> Self {
        Self {
            components: component_names
                .iter()
                .map(|name| ComponentCpu {
                    name: name.to_string(),
                    total_ns: 0,
                    invocations: 0,
                    timer_start: None,
                })
                .collect(),
        }
    }

    /// Standard component names for the v0 emulator.
    pub fn v0_components() -> Self {
        Self::new(&[
            "scheduler_tick",
            "coherence_gate",
            "transport_send",
            "transport_recv",
            "message_framing",
            "crc_compute",
            "postgres_query",
            "audit_log",
            "tile_dispatch",
            "heartbeat_check",
        ])
    }

    pub fn start(&mut self, component_idx: usize) {
        if let Some(c) = self.components.get_mut(component_idx) {
            c.timer_start = Some(Instant::now());
        }
    }

    pub fn stop(&mut self, component_idx: usize) {
        if let Some(c) = self.components.get_mut(component_idx) {
            if let Some(start) = c.timer_start.take() {
                c.total_ns += start.elapsed().as_nanos() as u64;
                c.invocations += 1;
            }
        }
    }

    /// Return a summary of CPU time per component.
    pub fn summary(&self) -> Vec<CpuComponentSummary> {
        let grand_total: u64 = self.components.iter().map(|c| c.total_ns).sum();
        self.components
            .iter()
            .map(|c| CpuComponentSummary {
                name: c.name.clone(),
                total_ns: c.total_ns,
                invocations: c.invocations,
                avg_ns: if c.invocations > 0 {
                    c.total_ns / c.invocations
                } else {
                    0
                },
                pct_of_total: if grand_total > 0 {
                    c.total_ns as f64 / grand_total as f64 * 100.0
                } else {
                    0.0
                },
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct CpuComponentSummary {
    pub name: String,
    pub total_ns: u64,
    pub invocations: u64,
    pub avg_ns: u64,
    pub pct_of_total: f64,
}

// ── Memory Profiling ───────────────────────────────────────────────

/// Per-tile memory budget tracker.
#[derive(Clone, Debug)]
pub struct TileMemoryProfile {
    pub tile_id: u8,
    /// Stack size in bytes.
    pub stack_bytes: usize,
    /// Heap usage in bytes.
    pub heap_bytes: usize,
    /// Message buffer pool size.
    pub msg_buffer_bytes: usize,
}

impl TileMemoryProfile {
    pub fn total(&self) -> usize {
        self.stack_bytes + self.heap_bytes + self.msg_buffer_bytes
    }

    /// Check if within the target of 2 MB per tile.
    pub fn within_budget(&self) -> bool {
        self.total() <= 2 * 1024 * 1024
    }
}

/// Aggregate memory profile for all tile processes.
pub fn profile_tile_memory(tile_count: usize) -> Vec<TileMemoryProfile> {
    // In production: read /proc/<pid>/statm for each tile process.
    // Here: return estimated profiles.
    (0..tile_count)
        .map(|i| TileMemoryProfile {
            tile_id: i as u8,
            stack_bytes: 64 * 1024,         // 64 KB reduced stack
            heap_bytes: 1024 * 1024,         // ~1 MB
            msg_buffer_bytes: 512 * 1024,    // 512 KB ring buffer
        })
        .collect()
}

// ── Transport Optimization Analysis ────────────────────────────────

/// Analyze transport performance and suggest optimizations.
#[derive(Clone, Debug)]
pub struct TransportAnalysis {
    /// Average messages per syscall (higher = better batching).
    pub msgs_per_syscall: f64,
    /// Bytes wasted by message padding/headers.
    pub overhead_pct: f64,
    /// Whether zero-copy is possible (message lifetime allows it).
    pub zero_copy_eligible: bool,
    /// Recommendations.
    pub recommendations: Vec<String>,
}

pub fn analyze_transport(
    messages_sent: u64,
    syscalls: u64,
    total_bytes: u64,
    payload_bytes: u64,
) -> TransportAnalysis {
    let msgs_per_syscall = if syscalls > 0 {
        messages_sent as f64 / syscalls as f64
    } else {
        0.0
    };

    let overhead_pct = if total_bytes > 0 {
        (total_bytes - payload_bytes) as f64 / total_bytes as f64 * 100.0
    } else {
        0.0
    };

    let mut recommendations = vec![];

    if msgs_per_syscall < 4.0 {
        recommendations.push(
            "Enable message batching: coalesce multiple messages per sendmsg() call".into(),
        );
    }

    if overhead_pct > 20.0 {
        recommendations.push(
            "Consider header compression or larger payloads to reduce framing overhead".into(),
        );
    }

    recommendations.push(
        "Consider UNIX SEQPACKET sockets for kernel-maintained message boundaries".into(),
    );

    if msgs_per_syscall < 2.0 {
        recommendations.push(
            "Evaluate io_uring for batched async I/O to reduce syscall count".into(),
        );
    }

    TransportAnalysis {
        msgs_per_syscall,
        overhead_pct,
        zero_copy_eligible: true, // messages are consumed immediately
        recommendations,
    }
}
