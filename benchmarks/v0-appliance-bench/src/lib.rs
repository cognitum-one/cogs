//! Cognitum v0 Appliance Emulator -- Benchmark & Testing Harness
//!
//! Comprehensive benchmark, acceptance test, and fault injection framework
//! for validating the v0 appliance emulator against its specification.
//!
//! # Architecture
//!
//! ```text
//! +---------------------+       +---------------------+
//! |  WorkloadGenerator  |------>|   EmulatorUnderTest  |
//! |  (spike, extract,   |       |  (host + 7 tiles +   |
//! |   query profiles)   |       |   coherence gate)    |
//! +---------------------+       +---------------------+
//!          |                              |
//!          v                              v
//! +---------------------+       +---------------------+
//! |  FaultInjector      |       |  MetricsCollector    |
//! |  (drop, delay,      |       |  (latency, thru,     |
//! |   replay, corrupt)  |       |   coherence, memory) |
//! +---------------------+       +---------------------+
//!                                         |
//!                                         v
//!                               +---------------------+
//!                               |  ReportEngine        |
//!                               |  (dashboard, HTML,   |
//!                               |   JSON, regression)  |
//!                               +---------------------+
//! ```
//!
//! # Acceptance Targets (from spec)
//!
//! | Metric                         | Target              |
//! |--------------------------------|---------------------|
//! | Tick latency p95               | < 1 ms              |
//! | Coherence gate blocks writes   | within 1 tick       |
//! | Tile failure recovery          | < 2 s               |
//! | Audit log                      | complete witness chain |
//! | Endurance (30 min, medium)     | zero protocol errors |
//!
//! # Modules
//!
//! - [`protocol`]  -- Binary message framing and types
//! - [`workload`]  -- Configurable workload generation
//! - [`fault`]     -- Fault injection framework
//! - [`metrics`]   -- Real-time metrics collection and statistics
//! - [`coherence`] -- Coherence gate test helpers
//! - [`harness`]   -- Top-level benchmark orchestrator
//! - [`report`]    -- Reporting, regression detection, CI integration
//! - [`profile`]   -- Profiling and optimization helpers

pub mod protocol;
pub mod workload;
pub mod fault;
pub mod metrics;
pub mod coherence;
pub mod harness;
pub mod report;
pub mod profile;
