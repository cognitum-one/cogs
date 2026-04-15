//! Binary message protocol types mirroring the v0 spec.
//!
//! All tile messages use identical binary framing regardless of transport.
//! This module defines the wire format so the benchmark harness can
//! generate, inspect, and corrupt messages without depending on the
//! production crate at compile time.

// ── Magic & Version ────────────────────────────────────────────────
pub const MAGIC: [u8; 4] = [0xC0, 0x67, 0x01, 0x00]; // "COG\0"
pub const PROTOCOL_VERSION: u8 = 1;

// ── Message Types ──────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    Hello         = 0x01,
    Heartbeat     = 0x02,
    TaskAssign    = 0x03,
    TaskResult    = 0x04,
    StateSnapshot = 0x05,
    Nack          = 0x06,
    Reset         = 0x07,
}

impl MessageType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Hello),
            0x02 => Some(Self::Heartbeat),
            0x03 => Some(Self::TaskAssign),
            0x04 => Some(Self::TaskResult),
            0x05 => Some(Self::StateSnapshot),
            0x06 => Some(Self::Nack),
            0x07 => Some(Self::Reset),
            _    => None,
        }
    }
}

// ── Header ─────────────────────────────────────────────────────────
/// Fixed-size header: 4 + 1 + 1 + 2 + 8 + 8 + 1 + 4 = 29 bytes
pub const HEADER_SIZE: usize = 29;

#[derive(Clone, Debug)]
pub struct MessageHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub msg_type: MessageType,
    pub flags: u16,
    pub epoch: u64,
    pub sequence: u64,
    pub sender_id: u8,
    pub payload_len: u32,
}

// ── Full Message ───────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
    pub crc32: u32,
    /// Optional Ed25519 signature (64 bytes) when strict mode enabled
    pub signature: Option<[u8; 64]>,
}

// ── Kernel Identifiers ─────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KernelId {
    K1SpikeStep       = 1,
    K2SparseFeature   = 2,
    K3BoundaryDelta   = 3,
    K4AnomalyDetect   = 4,
    K5Stabilization   = 5,
    K6HealthCompact   = 6,
}

impl KernelId {
    /// Maximum declared runtime in microseconds per the kernel catalog.
    pub fn max_runtime_us(&self) -> u64 {
        match self {
            Self::K1SpikeStep     => 200,
            Self::K2SparseFeature => 400,
            Self::K3BoundaryDelta => 300,
            Self::K4AnomalyDetect => 250,
            Self::K5Stabilization => 150,
            Self::K6HealthCompact => 100,
        }
    }

    /// Memory budget in bytes.
    pub fn memory_budget(&self) -> usize {
        match self {
            Self::K1SpikeStep     => 4096,
            Self::K2SparseFeature => 8192,
            Self::K3BoundaryDelta => 4096,
            Self::K4AnomalyDetect => 2048,
            Self::K5Stabilization => 1024,
            Self::K6HealthCompact => 512,
        }
    }
}

// ── Heartbeat Payload ──────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct HeartbeatPayload {
    pub tick_time_us: u32,
    pub queue_depth: u16,
    pub temperature_c: Option<f32>,
    pub last_seq_received: u64,
}

// ── Task Assignment Payload ────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct TaskAssignPayload {
    pub job_id: u64,
    pub kernel_id: KernelId,
    pub tick_budget_us: u32,
    pub input_data: Vec<u8>,
    pub output_schema_hash: [u8; 32],
    pub deadline_tick: u64,
}

// ── Task Result Payload ────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct TaskResultPayload {
    pub job_id: u64,
    pub kernel_id: KernelId,
    pub runtime_us: u32,
    pub output_data: Vec<u8>,
    pub output_hash: [u8; 32],
    pub witness_pointers: Vec<u64>,
}

// ── Nack Payload ───────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NackReason {
    BadMagic       = 1,
    BadVersion     = 2,
    BadCrc         = 3,
    ReplayDetected = 4,
    EpochMismatch  = 5,
    Unknown        = 255,
}

#[derive(Clone, Debug)]
pub struct NackPayload {
    pub reason: NackReason,
    pub last_good_seq: u64,
}

// ── Throttle Level (coherence gate output) ─────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ThrottleLevel {
    None   = 0,
    Light  = 1,
    Medium = 2,
    Heavy  = 3,
}
