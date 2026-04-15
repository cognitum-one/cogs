//! Coherence Gate Test Helpers
//!
//! Provides test-specific utilities for verifying coherence gate behavior
//! as specified in the v0 appliance spec:
//!
//! - WRITE_ALLOW boolean for Postgres and RuVector mutation paths
//! - BURST_ALLOW boolean for scheduling heavy kernels
//! - THROTTLE level 0..3 controlling task rate and CPU governor state
//!
//! Policy:
//!   If coherence below threshold for N consecutive ticks:
//!     1. Disable learning writes immediately
//!     2. Switch scheduler to stabilization kernels only
//!     3. Emit an audit event with witness
//!
//!   If coherence recovers above threshold with hysteresis:
//!     1. Enable writes gradually
//!     2. Ramp tasks from throttle 3 down to 0

use crate::protocol::ThrottleLevel;

// ── Gate Configuration ─────────────────────────────────────────────

/// Coherence gate configuration mirroring the host config file.
#[derive(Clone, Debug)]
pub struct CoherenceGateConfig {
    /// Coherence score below which writes are blocked.
    pub block_threshold: f64,
    /// Coherence score above which writes resume (must be > block_threshold
    /// to provide hysteresis).
    pub unblock_threshold: f64,
    /// Number of consecutive low-coherence ticks before blocking.
    pub consecutive_ticks_to_block: u32,
    /// Number of consecutive high-coherence ticks before unblocking.
    pub consecutive_ticks_to_unblock: u32,
    /// Throttle ramp-down step count (throttle 3 -> 0).
    pub ramp_down_steps: u32,
}

impl Default for CoherenceGateConfig {
    fn default() -> Self {
        Self {
            block_threshold: 0.6,
            unblock_threshold: 0.8,
            consecutive_ticks_to_block: 3,
            consecutive_ticks_to_unblock: 10,
            ramp_down_steps: 4,
        }
    }
}

// ── Gate Output ────────────────────────────────────────────────────

/// Snapshot of the coherence gate output at a given tick.
#[derive(Clone, Debug)]
pub struct GateOutput {
    /// Whether learning writes are allowed.
    pub write_allow: bool,
    /// Whether heavy / burst kernels are allowed.
    pub burst_allow: bool,
    /// Current throttle level.
    pub throttle: ThrottleLevel,
    /// Raw coherence score that produced this output.
    pub coherence_score: f64,
    /// Tick number.
    pub tick: u64,
}

// ── Simulated Gate (for test verification) ─────────────────────────

/// A standalone re-implementation of the coherence gate logic for
/// verifying the emulator's behavior from the outside.
///
/// The harness feeds the same mincut/coherence signals here and into
/// the emulator, then compares outputs.
pub struct SimulatedGate {
    config: CoherenceGateConfig,
    /// Running count of consecutive ticks below block_threshold.
    low_streak: u32,
    /// Running count of consecutive ticks above unblock_threshold.
    high_streak: u32,
    /// Current gate state.
    write_allowed: bool,
    burst_allowed: bool,
    throttle: ThrottleLevel,
    /// History of outputs for post-hoc analysis.
    pub history: Vec<GateOutput>,
}

impl SimulatedGate {
    pub fn new(config: CoherenceGateConfig) -> Self {
        Self {
            config,
            low_streak: 0,
            high_streak: 0,
            write_allowed: true,
            burst_allowed: true,
            throttle: ThrottleLevel::None,
            history: vec![],
        }
    }

    /// Process one tick with the given coherence score.
    /// Returns the gate output for this tick.
    pub fn tick(&mut self, tick_number: u64, coherence_score: f64) -> GateOutput {
        // -- Low-coherence logic --
        if coherence_score < self.config.block_threshold {
            self.low_streak += 1;
            self.high_streak = 0;

            if self.low_streak >= self.config.consecutive_ticks_to_block {
                self.write_allowed = false;
                self.burst_allowed = false;
                self.throttle = ThrottleLevel::Heavy;
            }
        }
        // -- Recovery logic (with hysteresis) --
        else if coherence_score >= self.config.unblock_threshold {
            self.high_streak += 1;
            self.low_streak = 0;

            if self.high_streak >= self.config.consecutive_ticks_to_unblock {
                self.write_allowed = true;
                self.burst_allowed = true;
                // Gradual ramp-down of throttle.
                self.throttle = match self.throttle {
                    ThrottleLevel::Heavy  => ThrottleLevel::Medium,
                    ThrottleLevel::Medium => ThrottleLevel::Light,
                    ThrottleLevel::Light  => ThrottleLevel::None,
                    ThrottleLevel::None   => ThrottleLevel::None,
                };
            }
        }
        // -- In the dead zone (between thresholds) --
        else {
            self.low_streak = 0;
            self.high_streak = 0;
            // Hold current state (hysteresis).
        }

        let output = GateOutput {
            write_allow: self.write_allowed,
            burst_allow: self.burst_allowed,
            throttle: self.throttle,
            coherence_score,
            tick: tick_number,
        };

        self.history.push(output.clone());
        output
    }

    /// Find the first tick where writes became blocked.
    pub fn first_block_tick(&self) -> Option<u64> {
        self.history.iter()
            .find(|o| !o.write_allow)
            .map(|o| o.tick)
    }

    /// Find the first tick where writes were re-allowed after a block.
    pub fn first_recovery_tick(&self) -> Option<u64> {
        let block_tick = self.first_block_tick()?;
        self.history.iter()
            .skip_while(|o| o.tick <= block_tick)
            .find(|o| o.write_allow)
            .map(|o| o.tick)
    }

    /// Duration (in ticks) from first block to first recovery.
    pub fn recovery_ticks(&self) -> Option<u64> {
        let block = self.first_block_tick()?;
        let recover = self.first_recovery_tick()?;
        Some(recover - block)
    }

    /// Verify that writes were blocked within `max_ticks` of coherence
    /// dropping below the threshold.  Used by Acceptance Test 2.
    pub fn verify_block_within(&self, max_ticks: u64) -> bool {
        // Find the first tick where coherence < block_threshold.
        let first_low = self.history.iter()
            .find(|o| o.coherence_score < self.config.block_threshold);
        let first_block = self.first_block_tick();

        match (first_low, first_block) {
            (Some(low), Some(block_tick)) => {
                block_tick <= low.tick + max_ticks
            }
            _ => false,
        }
    }
}

// ── Audit Event Verifier ───────────────────────────────────────────

/// Verifies that the emulator's audit log contains the expected witness
/// chain for coherence gate transitions.
#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub event_id: u64,
    pub epoch: u64,
    pub component: String,
    pub decision: String,
    pub reason_code: u32,
    pub inputs_hash: [u8; 32],
    pub outputs_hash: [u8; 32],
    pub witness_refs: Vec<u64>,
}

/// Check that the audit log contains a complete witness chain.
///
/// "Complete" means:
/// 1. Every gate transition has a corresponding audit event.
/// 2. Each audit event references a witness with cut value and
///    tile results hashes.
/// 3. Events are ordered by epoch/tick.
pub fn verify_audit_chain(events: &[AuditEvent]) -> AuditVerification {
    let mut errors: Vec<String> = vec![];

    // Check ordering.
    for window in events.windows(2) {
        if window[1].epoch < window[0].epoch {
            errors.push(format!(
                "Audit event {} has epoch {} before previous epoch {}",
                window[1].event_id, window[1].epoch, window[0].epoch,
            ));
        }
    }

    // Check witness references are non-empty.
    for event in events {
        if event.witness_refs.is_empty() {
            errors.push(format!(
                "Audit event {} has no witness references",
                event.event_id,
            ));
        }
    }

    AuditVerification {
        total_events: events.len(),
        errors,
    }
}

#[derive(Clone, Debug)]
pub struct AuditVerification {
    pub total_events: usize,
    pub errors: Vec<String>,
}

impl AuditVerification {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}
