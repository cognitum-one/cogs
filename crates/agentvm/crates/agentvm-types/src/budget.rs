//! Budget types - resource consumption limits and tracking

use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Resource budget for a capsule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Budget {
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Wall clock time in milliseconds
    pub wall_time_ms: u64,
    /// Memory in bytes
    pub memory_bytes: u64,
    /// Disk write in bytes
    pub disk_write_bytes: u64,
    /// Network transfer in bytes
    pub network_bytes: u64,
    /// Network request count
    pub network_requests: u64,
}

impl Budget {
    /// Zero budget
    pub const ZERO: Budget = Budget {
        cpu_time_ms: 0,
        wall_time_ms: 0,
        memory_bytes: 0,
        disk_write_bytes: 0,
        network_bytes: 0,
        network_requests: 0,
    };

    /// Unlimited budget
    pub const UNLIMITED: Budget = Budget {
        cpu_time_ms: u64::MAX,
        wall_time_ms: u64::MAX,
        memory_bytes: u64::MAX,
        disk_write_bytes: u64::MAX,
        network_bytes: u64::MAX,
        network_requests: u64::MAX,
    };

    /// Create a new budget with specified limits
    pub fn new(
        cpu_time_ms: u64,
        wall_time_ms: u64,
        memory_bytes: u64,
        disk_write_bytes: u64,
        network_bytes: u64,
        network_requests: u64,
    ) -> Self {
        Self {
            cpu_time_ms,
            wall_time_ms,
            memory_bytes,
            disk_write_bytes,
            network_bytes,
            network_requests,
        }
    }

    /// Check if this budget can satisfy the requirements
    pub fn can_satisfy(&self, required: &Budget) -> bool {
        self.cpu_time_ms >= required.cpu_time_ms
            && self.wall_time_ms >= required.wall_time_ms
            && self.memory_bytes >= required.memory_bytes
            && self.disk_write_bytes >= required.disk_write_bytes
            && self.network_bytes >= required.network_bytes
            && self.network_requests >= required.network_requests
    }

    /// Saturating subtraction
    pub fn saturating_sub(&self, other: &Budget) -> Budget {
        Budget {
            cpu_time_ms: self.cpu_time_ms.saturating_sub(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.saturating_sub(other.wall_time_ms),
            memory_bytes: self.memory_bytes.saturating_sub(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.saturating_sub(other.disk_write_bytes),
            network_bytes: self.network_bytes.saturating_sub(other.network_bytes),
            network_requests: self.network_requests.saturating_sub(other.network_requests),
        }
    }

    /// Check if any resource is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.cpu_time_ms == 0
            || self.wall_time_ms == 0
            || self.memory_bytes == 0
            || self.disk_write_bytes == 0
            || self.network_bytes == 0
            || self.network_requests == 0
    }
}

impl Default for Budget {
    fn default() -> Self {
        // Default: 5 min CPU, 1 hour wall, 2GB RAM, 1GB disk, 100MB network
        Self {
            cpu_time_ms: 300_000,
            wall_time_ms: 3_600_000,
            memory_bytes: 2_147_483_648,
            disk_write_bytes: 1_073_741_824,
            network_bytes: 104_857_600,
            network_requests: 1000,
        }
    }
}

impl Add for Budget {
    type Output = Budget;

    fn add(self, other: Budget) -> Budget {
        Budget {
            cpu_time_ms: self.cpu_time_ms.saturating_add(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.saturating_add(other.wall_time_ms),
            memory_bytes: self.memory_bytes.saturating_add(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.saturating_add(other.disk_write_bytes),
            network_bytes: self.network_bytes.saturating_add(other.network_bytes),
            network_requests: self.network_requests.saturating_add(other.network_requests),
        }
    }
}

impl AddAssign for Budget {
    fn add_assign(&mut self, other: Budget) {
        *self = *self + other;
    }
}

impl Sub for Budget {
    type Output = Budget;

    fn sub(self, other: Budget) -> Budget {
        self.saturating_sub(&other)
    }
}

impl SubAssign for Budget {
    fn sub_assign(&mut self, other: Budget) {
        *self = *self - other;
    }
}

/// Multi-dimensional budget vector (for tracking initial, used, remaining)
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetVector {
    /// Initial budget allocation
    pub initial: Budget,
    /// Amount consumed
    pub consumed: Budget,
}

impl BudgetVector {
    /// Create new budget vector with initial allocation
    pub fn new(initial: Budget) -> Self {
        Self {
            initial,
            consumed: Budget::ZERO,
        }
    }

    /// Get remaining budget
    pub fn remaining(&self) -> Budget {
        self.initial.saturating_sub(&self.consumed)
    }

    /// Consume budget, returns true if successful
    pub fn consume(&mut self, amount: &Budget) -> bool {
        let remaining = self.remaining();
        if remaining.can_satisfy(amount) {
            self.consumed += *amount;
            true
        } else {
            false
        }
    }

    /// Check if budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.remaining().is_exhausted()
    }

    /// Get utilization ratio (0.0 to 1.0)
    pub fn utilization(&self) -> f64 {
        if self.initial.cpu_time_ms == 0 {
            return 0.0;
        }
        self.consumed.cpu_time_ms as f64 / self.initial.cpu_time_ms as f64
    }
}

impl Default for BudgetVector {
    fn default() -> Self {
        Self::new(Budget::default())
    }
}

/// Budget consumed by a single operation
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuotaConsumed {
    /// Invocations (usually 1)
    pub invocations: u64,
    /// Bytes transferred
    pub bytes: u64,
    /// Duration in nanoseconds
    pub duration_ns: u64,
}

impl QuotaConsumed {
    /// Create with all zeros
    pub const ZERO: QuotaConsumed = QuotaConsumed {
        invocations: 0,
        bytes: 0,
        duration_ns: 0,
    };

    /// Create for a single invocation
    pub fn single(bytes: u64, duration_ns: u64) -> Self {
        Self {
            invocations: 1,
            bytes,
            duration_ns,
        }
    }
}
