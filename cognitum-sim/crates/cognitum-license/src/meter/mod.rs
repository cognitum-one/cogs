//! Usage metering and quota enforcement

use async_trait::async_trait;
use chrono::{DateTime, Utc, Datelike, TimeZone};
use mockall::automock;
use serde::{Deserialize, Serialize};
use crate::MeterError;

pub mod in_memory;

pub use in_memory::InMemoryMeter;

/// Usage event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsageEvent {
    /// Simulation executed with cycle count
    Simulation { cycles: u64 },

    /// API request made
    ApiRequest { endpoint: String },

    /// Custom event
    Custom { event_type: String, count: u64 },
}

/// Time period for usage queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// Current month
    CurrentMonth,

    /// Last 30 days
    Last30Days,

    /// All time
    AllTime,

    /// Custom date range
    Range { start: DateTime<Utc>, end: DateTime<Utc> },
}

impl Period {
    /// Get start date for period
    pub fn start_date(&self) -> DateTime<Utc> {
        match self {
            Self::CurrentMonth => {
                let now = Utc::now();
                let year = now.year();
                let month = now.month();
                Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).unwrap()
            }
            Self::Last30Days => Utc::now() - chrono::Duration::days(30),
            Self::AllTime => DateTime::<Utc>::MIN_UTC,
            Self::Range { start, .. } => *start,
        }
    }

    /// Get end date for period
    pub fn end_date(&self) -> DateTime<Utc> {
        match self {
            Self::CurrentMonth | Self::Last30Days | Self::AllTime => Utc::now(),
            Self::Range { end, .. } => *end,
        }
    }
}

/// Usage statistics for a license
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Total simulations run
    pub simulations: u64,

    /// Total cycles executed
    pub total_cycles: u64,

    /// Total API requests
    pub api_requests: u64,

    /// Custom event counts
    #[serde(default)]
    pub custom_events: std::collections::HashMap<String, u64>,

    /// Period start
    pub period_start: Option<DateTime<Utc>>,

    /// Period end
    pub period_end: Option<DateTime<Utc>>,
}

/// Quota check result
#[derive(Debug, Clone, PartialEq)]
pub enum QuotaResult {
    /// Operation allowed
    Allowed {
        /// Remaining quota
        remaining: Option<u64>,
    },

    /// Quota exceeded
    Exceeded {
        /// Limit
        limit: u64,
        /// Current usage
        used: u64,
    },
}

/// Trait for usage metering
#[automock]
#[async_trait]
pub trait UsageMeter: Send + Sync {
    /// Record usage event
    async fn record(&self, license_key: &str, usage: UsageEvent) -> Result<(), MeterError>;

    /// Get current usage for period
    async fn get_usage(&self, license_key: &str, period: Period) -> Result<Usage, MeterError>;

    /// Check if quota exceeded
    async fn check_quota(
        &self,
        license_key: &str,
        operation: crate::Operation,
    ) -> Result<QuotaResult, MeterError>;

    /// Reset usage counters (for testing)
    async fn reset(&self, license_key: &str) -> Result<(), MeterError>;

    /// Flush pending writes (for persistent backends)
    async fn flush(&self) -> Result<(), MeterError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_period_current_month() {
        let period = Period::CurrentMonth;
        let start = period.start_date();
        let end = period.end_date();

        assert_eq!(start.day(), 1);
        assert_eq!(start.hour(), 0);
        assert!(end >= start);
    }

    #[test]
    fn test_period_last_30_days() {
        let period = Period::Last30Days;
        let start = period.start_date();
        let end = period.end_date();

        let diff = (end - start).num_days();
        assert!(diff >= 29 && diff <= 31);
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.simulations, 0);
        assert_eq!(usage.total_cycles, 0);
        assert_eq!(usage.api_requests, 0);
    }
}
