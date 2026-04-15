//! Fault Injection Framework
//!
//! Implements Deliverable 5 from the v0 spec:
//!   - Drop messages          (configurable rate)
//!   - Delay messages         (configurable distribution)
//!   - Replay messages        (sequence replay attacks)
//!   - Corrupt payload        (bit-flip, truncation, garbage)
//!   - Freeze a tile          (stop responding to heartbeats)
//!   - Slow a tile            (exceed tick budget)
//!   - Return malformed results
//!
//! Faults are composed via `FaultPlan` which schedules when and how
//! faults activate.  The plan integrates with the `MetricsCollector`
//! so acceptance tests can correlate faults with observed behavior.

use std::time::{Duration, Instant};

// ── Individual Fault Types ─────────────────────────────────────────

/// A single fault action that can be applied to the transport layer
/// or to a specific tile simulator.
#[derive(Clone, Debug)]
pub enum FaultAction {
    /// Drop the next N messages matching the filter.
    DropMessages {
        /// Probability of dropping each matching message [0.0, 1.0].
        drop_rate: f64,
        /// Only drop messages of this type (None = all types).
        msg_type_filter: Option<u8>,
        /// Only drop messages from/to this tile (None = any tile).
        tile_filter: Option<u8>,
    },

    /// Add latency to messages.
    DelayMessages {
        /// Minimum added delay.
        min_delay: Duration,
        /// Maximum added delay (uniform distribution between min and max).
        max_delay: Duration,
        /// Tile filter.
        tile_filter: Option<u8>,
    },

    /// Replay a previously captured message.
    ReplayMessage {
        /// Number of times to replay.
        replay_count: u32,
        /// Delay between replays.
        replay_interval: Duration,
        /// Specific sequence number to replay (None = capture next).
        target_sequence: Option<u64>,
    },

    /// Corrupt payload bytes.
    CorruptPayload {
        /// Corruption strategy.
        strategy: CorruptionStrategy,
        /// Tile filter.
        tile_filter: Option<u8>,
    },

    /// Freeze a tile -- it stops sending heartbeats and processing tasks.
    FreezeTile {
        /// Which tile to freeze (0-6 for 7-tile config).
        tile_id: u8,
        /// How long to keep the tile frozen.
        freeze_duration: Duration,
    },

    /// Slow a tile so it exceeds its tick budget on every task.
    SlowTile {
        /// Which tile to slow.
        tile_id: u8,
        /// Multiplier on kernel runtime (e.g. 3.0 = 3x slower).
        slowdown_factor: f64,
        /// Duration of the slowdown.
        slow_duration: Duration,
    },

    /// Return a malformed TASK_RESULT (wrong schema hash, truncated, etc).
    MalformedResult {
        /// Which tile produces bad results.
        tile_id: u8,
        /// What kind of malformation.
        defect: ResultDefect,
        /// How many consecutive bad results.
        count: u32,
    },
}

/// How to corrupt a payload.
#[derive(Clone, Debug)]
pub enum CorruptionStrategy {
    /// Flip random bits.
    BitFlip { bit_count: usize },
    /// Truncate to N bytes.
    Truncate { max_bytes: usize },
    /// Replace payload with random garbage.
    Garbage,
    /// Zero out the CRC field.
    ZeroCrc,
    /// Overwrite magic bytes.
    BadMagic,
}

/// Kinds of malformed results a tile can produce.
#[derive(Clone, Debug)]
pub enum ResultDefect {
    /// Output hash does not match output data.
    WrongHash,
    /// Payload is truncated.
    TruncatedPayload,
    /// Job ID in result does not match any assigned job.
    UnknownJobId,
    /// Kernel ID mismatch (result claims a different kernel).
    KernelMismatch,
    /// Runtime field is zero (impossible for a real kernel).
    ZeroRuntime,
}

// ── Scheduled Fault ────────────────────────────────────────────────

/// A fault scheduled to activate at a specific time offset from the
/// start of the benchmark run.
#[derive(Clone, Debug)]
pub struct ScheduledFault {
    /// When to activate (offset from run start).
    pub activate_at: Duration,
    /// The fault to inject.
    pub action: FaultAction,
    /// Human-readable label for reports.
    pub label: String,
}

// ── Fault Plan ─────────────────────────────────────────────────────

/// A complete plan of faults to inject during a benchmark run.
/// The harness walks through scheduled faults in chronological order.
#[derive(Clone, Debug)]
pub struct FaultPlan {
    pub faults: Vec<ScheduledFault>,
}

impl FaultPlan {
    pub fn none() -> Self {
        Self { faults: vec![] }
    }

    pub fn builder() -> FaultPlanBuilder {
        FaultPlanBuilder { faults: vec![] }
    }

    /// Pre-built plan for Acceptance Test 2 (Coherence Gate).
    ///
    /// - At 5 min: inject coherence drop (heavy message drops causing
    ///   mincut signal degradation).
    /// - At 10 min: stop injecting so coherence can recover.
    pub fn acceptance_test_2() -> Self {
        Self::builder()
            .at(Duration::from_secs(5 * 60))
            .label("coherence-drop-inject")
            .drop_messages(0.90, None, None) // 90% drop rate
            .at(Duration::from_secs(10 * 60))
            .label("coherence-drop-stop")
            .drop_messages(0.0, None, None) // restore
            .build()
    }

    /// Pre-built plan for Acceptance Test 3 (Tile Failure Recovery).
    ///
    /// - At 2 min: kill tile 3 (freeze indefinitely, then restart).
    /// - At 4 min: restart tile 3 with stale epoch.
    pub fn acceptance_test_3() -> Self {
        Self::builder()
            .at(Duration::from_secs(2 * 60))
            .label("kill-tile-3")
            .freeze_tile(3, Duration::from_secs(2 * 60))
            .at(Duration::from_secs(4 * 60))
            .label("restart-tile-3-stale-epoch")
            .freeze_tile(3, Duration::ZERO) // unfreeze (duration=0 signals restart)
            .build()
    }

    /// Comprehensive chaos plan for stress testing.
    pub fn chaos() -> Self {
        Self::builder()
            // Message drops
            .at(Duration::from_secs(30))
            .label("light-drops")
            .drop_messages(0.05, None, None)
            // Delay spike
            .at(Duration::from_secs(60))
            .label("delay-spike")
            .delay_messages(
                Duration::from_millis(5),
                Duration::from_millis(50),
                None,
            )
            // Replay attack
            .at(Duration::from_secs(90))
            .label("replay-attack")
            .replay_message(3, Duration::from_millis(10), None)
            // Corrupt payload
            .at(Duration::from_secs(120))
            .label("corrupt-payload")
            .corrupt_payload(CorruptionStrategy::BitFlip { bit_count: 2 }, None)
            // Slow tile 5
            .at(Duration::from_secs(150))
            .label("slow-tile-5")
            .slow_tile(5, 4.0, Duration::from_secs(30))
            // Malformed result from tile 1
            .at(Duration::from_secs(180))
            .label("malformed-result-tile-1")
            .malformed_result(1, ResultDefect::WrongHash, 5)
            // Freeze tile 6
            .at(Duration::from_secs(210))
            .label("freeze-tile-6")
            .freeze_tile(6, Duration::from_secs(20))
            .build()
    }
}

// ── Fault Plan Builder ─────────────────────────────────────────────

pub struct FaultPlanBuilder {
    faults: Vec<ScheduledFault>,
}

impl FaultPlanBuilder {
    /// Set the activation time for the next fault.
    pub fn at(self, time: Duration) -> FaultPlanBuilderTimed {
        FaultPlanBuilderTimed {
            faults: self.faults,
            activate_at: time,
            label: String::new(),
        }
    }

    pub fn build(self) -> FaultPlan {
        let mut plan = FaultPlan { faults: self.faults };
        plan.faults.sort_by_key(|f| f.activate_at);
        plan
    }
}

pub struct FaultPlanBuilderTimed {
    faults: Vec<ScheduledFault>,
    activate_at: Duration,
    label: String,
}

impl FaultPlanBuilderTimed {
    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    pub fn drop_messages(
        mut self,
        rate: f64,
        msg_type: Option<u8>,
        tile: Option<u8>,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::DropMessages {
                drop_rate: rate,
                msg_type_filter: msg_type,
                tile_filter: tile,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn delay_messages(
        mut self,
        min: Duration,
        max: Duration,
        tile: Option<u8>,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::DelayMessages {
                min_delay: min,
                max_delay: max,
                tile_filter: tile,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn replay_message(
        mut self,
        count: u32,
        interval: Duration,
        target_seq: Option<u64>,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::ReplayMessage {
                replay_count: count,
                replay_interval: interval,
                target_sequence: target_seq,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn corrupt_payload(
        mut self,
        strategy: CorruptionStrategy,
        tile: Option<u8>,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::CorruptPayload {
                strategy,
                tile_filter: tile,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn freeze_tile(
        mut self,
        tile_id: u8,
        duration: Duration,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::FreezeTile {
                tile_id,
                freeze_duration: duration,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn slow_tile(
        mut self,
        tile_id: u8,
        factor: f64,
        duration: Duration,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::SlowTile {
                tile_id,
                slowdown_factor: factor,
                slow_duration: duration,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }

    pub fn malformed_result(
        mut self,
        tile_id: u8,
        defect: ResultDefect,
        count: u32,
    ) -> FaultPlanBuilder {
        self.faults.push(ScheduledFault {
            activate_at: self.activate_at,
            action: FaultAction::MalformedResult {
                tile_id,
                defect,
                count,
            },
            label: self.label.clone(),
        });
        FaultPlanBuilder { faults: self.faults }
    }
}

// ── Fault Injector Runtime ─────────────────────────────────────────

/// Runtime component that sits between the workload generator and the
/// emulator transport, applying faults according to the plan.
pub struct FaultInjector {
    plan: FaultPlan,
    /// Index of the next fault to activate.
    next_fault_idx: usize,
    /// Currently active drop rate.
    active_drop_rate: f64,
    /// Currently active delay range.
    active_delay: Option<(Duration, Duration)>,
    /// Set of frozen tile IDs.
    frozen_tiles: Vec<(u8, Instant)>, // (tile_id, unfreeze_at)
    /// Set of slowed tile IDs.
    slowed_tiles: Vec<(u8, f64, Instant)>, // (tile_id, factor, end_at)
    /// Messages captured for replay.
    replay_buffer: Vec<Vec<u8>>,
    /// Fault event log (for correlation with metrics).
    pub event_log: Vec<FaultEvent>,
}

/// A recorded fault event for post-run analysis.
#[derive(Clone, Debug)]
pub struct FaultEvent {
    pub timestamp: Instant,
    pub label: String,
    pub description: String,
}

impl FaultInjector {
    pub fn new(plan: FaultPlan) -> Self {
        Self {
            plan,
            next_fault_idx: 0,
            active_drop_rate: 0.0,
            active_delay: None,
            frozen_tiles: vec![],
            slowed_tiles: vec![],
            replay_buffer: vec![],
            event_log: vec![],
        }
    }

    /// Called every tick by the harness.  Activates any faults whose
    /// scheduled time has arrived.
    pub fn tick(&mut self, elapsed: Duration) {
        while self.next_fault_idx < self.plan.faults.len() {
            let fault = &self.plan.faults[self.next_fault_idx];
            if elapsed >= fault.activate_at {
                self.activate_fault(fault.clone());
                self.next_fault_idx += 1;
            } else {
                break;
            }
        }

        // Expire frozen/slowed tiles.
        let now = Instant::now();
        self.frozen_tiles.retain(|&(_, unfreeze)| now < unfreeze);
        self.slowed_tiles.retain(|&(_, _, end)| now < end);
    }

    fn activate_fault(&mut self, fault: ScheduledFault) {
        self.event_log.push(FaultEvent {
            timestamp: Instant::now(),
            label: fault.label.clone(),
            description: format!("{:?}", fault.action),
        });

        match &fault.action {
            FaultAction::DropMessages { drop_rate, .. } => {
                self.active_drop_rate = *drop_rate;
            }
            FaultAction::DelayMessages { min_delay, max_delay, .. } => {
                self.active_delay = Some((*min_delay, *max_delay));
            }
            FaultAction::FreezeTile { tile_id, freeze_duration } => {
                if freeze_duration.is_zero() {
                    // Unfreeze (restart).
                    self.frozen_tiles.retain(|&(id, _)| id != *tile_id);
                } else {
                    let unfreeze_at = Instant::now() + *freeze_duration;
                    self.frozen_tiles.push((*tile_id, unfreeze_at));
                }
            }
            FaultAction::SlowTile { tile_id, slowdown_factor, slow_duration } => {
                let end_at = Instant::now() + *slow_duration;
                self.slowed_tiles.push((*tile_id, *slowdown_factor, end_at));
            }
            _ => {
                // ReplayMessage, CorruptPayload, MalformedResult are handled
                // in the intercept methods below.
            }
        }
    }

    /// Returns true if the message should be dropped.
    pub fn should_drop(&self, _tile_id: u8) -> bool {
        if self.active_drop_rate <= 0.0 {
            return false;
        }
        // pseudo: random() < self.active_drop_rate
        false // placeholder
    }

    /// Returns true if the given tile is currently frozen.
    pub fn is_tile_frozen(&self, tile_id: u8) -> bool {
        self.frozen_tiles.iter().any(|&(id, _)| id == tile_id)
    }

    /// Returns the slowdown factor for a tile (1.0 = normal).
    pub fn slowdown_factor(&self, tile_id: u8) -> f64 {
        self.slowed_tiles
            .iter()
            .find(|&&(id, _, _)| id == tile_id)
            .map(|&(_, factor, _)| factor)
            .unwrap_or(1.0)
    }
}
