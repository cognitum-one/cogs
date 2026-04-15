//! Budget types for resource allocation

use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Budget vector tracking multiple resource dimensions
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetVector {
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Wall clock time in milliseconds
    pub wall_time_ms: u64,
    /// Memory in bytes
    pub memory_bytes: u64,
    /// Disk write bytes
    pub disk_write_bytes: u64,
    /// Network bytes
    pub network_bytes: u64,
    /// Network request count
    pub network_requests: u64,
}

impl BudgetVector {
    /// Create a new budget vector
    pub const fn new(
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

    /// Create an unlimited budget
    pub const fn unlimited() -> Self {
        Self {
            cpu_time_ms: u64::MAX,
            wall_time_ms: u64::MAX,
            memory_bytes: u64::MAX,
            disk_write_bytes: u64::MAX,
            network_bytes: u64::MAX,
            network_requests: u64::MAX,
        }
    }

    /// Create a zero budget
    pub const fn zero() -> Self {
        Self {
            cpu_time_ms: 0,
            wall_time_ms: 0,
            memory_bytes: 0,
            disk_write_bytes: 0,
            network_bytes: 0,
            network_requests: 0,
        }
    }

    /// Check if this budget has any resources remaining
    pub fn has_remaining(&self) -> bool {
        self.cpu_time_ms > 0
            || self.wall_time_ms > 0
            || self.memory_bytes > 0
            || self.disk_write_bytes > 0
            || self.network_bytes > 0
            || self.network_requests > 0
    }

    /// Check if this budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        !self.has_remaining()
    }

    /// Saturating subtraction
    pub fn saturating_sub(&self, other: &Self) -> Self {
        Self {
            cpu_time_ms: self.cpu_time_ms.saturating_sub(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.saturating_sub(other.wall_time_ms),
            memory_bytes: self.memory_bytes.saturating_sub(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.saturating_sub(other.disk_write_bytes),
            network_bytes: self.network_bytes.saturating_sub(other.network_bytes),
            network_requests: self.network_requests.saturating_sub(other.network_requests),
        }
    }

    /// Saturating addition
    pub fn saturating_add(&self, other: &Self) -> Self {
        Self {
            cpu_time_ms: self.cpu_time_ms.saturating_add(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.saturating_add(other.wall_time_ms),
            memory_bytes: self.memory_bytes.saturating_add(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.saturating_add(other.disk_write_bytes),
            network_bytes: self.network_bytes.saturating_add(other.network_bytes),
            network_requests: self.network_requests.saturating_add(other.network_requests),
        }
    }

    /// Check if this budget can satisfy a request
    pub fn can_satisfy(&self, request: &Self) -> bool {
        self.cpu_time_ms >= request.cpu_time_ms
            && self.wall_time_ms >= request.wall_time_ms
            && self.memory_bytes >= request.memory_bytes
            && self.disk_write_bytes >= request.disk_write_bytes
            && self.network_bytes >= request.network_bytes
            && self.network_requests >= request.network_requests
    }

    /// Element-wise minimum
    pub fn min(&self, other: &Self) -> Self {
        Self {
            cpu_time_ms: self.cpu_time_ms.min(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.min(other.wall_time_ms),
            memory_bytes: self.memory_bytes.min(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.min(other.disk_write_bytes),
            network_bytes: self.network_bytes.min(other.network_bytes),
            network_requests: self.network_requests.min(other.network_requests),
        }
    }

    /// Element-wise maximum
    pub fn max(&self, other: &Self) -> Self {
        Self {
            cpu_time_ms: self.cpu_time_ms.max(other.cpu_time_ms),
            wall_time_ms: self.wall_time_ms.max(other.wall_time_ms),
            memory_bytes: self.memory_bytes.max(other.memory_bytes),
            disk_write_bytes: self.disk_write_bytes.max(other.disk_write_bytes),
            network_bytes: self.network_bytes.max(other.network_bytes),
            network_requests: self.network_requests.max(other.network_requests),
        }
    }

    /// Calculate utilization ratio (0.0 - 1.0) against a maximum
    pub fn utilization(&self, max: &Self) -> f64 {
        let ratios = [
            if max.cpu_time_ms > 0 {
                self.cpu_time_ms as f64 / max.cpu_time_ms as f64
            } else {
                0.0
            },
            if max.wall_time_ms > 0 {
                self.wall_time_ms as f64 / max.wall_time_ms as f64
            } else {
                0.0
            },
            if max.memory_bytes > 0 {
                self.memory_bytes as f64 / max.memory_bytes as f64
            } else {
                0.0
            },
            if max.disk_write_bytes > 0 {
                self.disk_write_bytes as f64 / max.disk_write_bytes as f64
            } else {
                0.0
            },
            if max.network_bytes > 0 {
                self.network_bytes as f64 / max.network_bytes as f64
            } else {
                0.0
            },
            if max.network_requests > 0 {
                self.network_requests as f64 / max.network_requests as f64
            } else {
                0.0
            },
        ];

        // Return the maximum utilization across all dimensions
        ratios.into_iter().fold(0.0f64, |a, b| a.max(b))
    }
}

impl Add for BudgetVector {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self.saturating_add(&other)
    }
}

impl AddAssign for BudgetVector {
    fn add_assign(&mut self, other: Self) {
        *self = self.saturating_add(&other);
    }
}

impl Sub for BudgetVector {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        self.saturating_sub(&other)
    }
}

impl SubAssign for BudgetVector {
    fn sub_assign(&mut self, other: Self) {
        *self = self.saturating_sub(&other);
    }
}

/// Budget with initial allocation and tracking
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Budget {
    /// Initial budget allocation
    pub initial: BudgetVector,
    /// Amount consumed
    pub consumed: BudgetVector,
}

impl Budget {
    /// Create a new budget with the given initial allocation
    pub fn new(initial: BudgetVector) -> Self {
        Self {
            initial,
            consumed: BudgetVector::zero(),
        }
    }

    /// Get remaining budget
    pub fn remaining(&self) -> BudgetVector {
        self.initial.saturating_sub(&self.consumed)
    }

    /// Check if the budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        !self.remaining().has_remaining()
    }

    /// Try to consume budget, returns error if insufficient
    pub fn try_consume(&mut self, amount: &BudgetVector) -> Result<(), BudgetExceededError> {
        let remaining = self.remaining();
        if !remaining.can_satisfy(amount) {
            return Err(BudgetExceededError::new(&remaining, amount));
        }
        self.consumed = self.consumed.saturating_add(amount);
        Ok(())
    }

    /// Consume budget without checking (saturating)
    pub fn consume(&mut self, amount: &BudgetVector) {
        self.consumed = self.consumed.saturating_add(amount);
    }

    /// Get utilization ratio (0.0 - 1.0)
    pub fn utilization(&self) -> f64 {
        self.consumed.utilization(&self.initial)
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(BudgetVector::unlimited())
    }
}

/// Error when budget is exceeded
#[derive(Debug, Clone)]
pub struct BudgetExceededError {
    pub available: BudgetVector,
    pub requested: BudgetVector,
}

impl BudgetExceededError {
    pub fn new(available: &BudgetVector, requested: &BudgetVector) -> Self {
        Self {
            available: *available,
            requested: *requested,
        }
    }
}

impl fmt::Display for BudgetExceededError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "budget exceeded: requested {:?}, available {:?}", self.requested, self.available)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BudgetExceededError {}
