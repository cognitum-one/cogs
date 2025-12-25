//! In-memory usage meter implementation

use super::{UsageMeter, UsageEvent, Usage, Period, QuotaResult};
use crate::{MeterError, Operation, LicenseTier};
use crate::license::UsageQuota;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Record of a single usage event
#[derive(Debug, Clone)]
struct UsageRecord {
    timestamp: DateTime<Utc>,
    event: UsageEvent,
}

/// In-memory usage meter
pub struct InMemoryMeter {
    /// Usage records by license key
    records: Arc<RwLock<HashMap<String, Vec<UsageRecord>>>>,

    /// Quota limits by license key
    quotas: Arc<RwLock<HashMap<String, UsageQuota>>>,
}

impl InMemoryMeter {
    /// Create a new in-memory meter
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set quota for a license key
    pub fn set_quota(&self, license_key: &str, quota: UsageQuota) {
        self.quotas.write().insert(license_key.to_string(), quota);
    }

    /// Set quota from tier
    pub fn set_quota_from_tier(&self, license_key: &str, tier: LicenseTier) {
        self.set_quota(license_key, UsageQuota::from_tier(tier));
    }

    /// Calculate usage from records
    fn calculate_usage(&self, records: &[UsageRecord], period: Period) -> Usage {
        let start = period.start_date();
        let end = period.end_date();

        let mut usage = Usage {
            period_start: Some(start),
            period_end: Some(end),
            ..Default::default()
        };

        for record in records {
            if record.timestamp >= start && record.timestamp <= end {
                match &record.event {
                    UsageEvent::Simulation { cycles } => {
                        usage.simulations += 1;
                        usage.total_cycles += cycles;
                    }
                    UsageEvent::ApiRequest { .. } => {
                        usage.api_requests += 1;
                    }
                    UsageEvent::Custom { event_type, count } => {
                        *usage.custom_events.entry(event_type.clone()).or_insert(0) += count;
                    }
                }
            }
        }

        usage
    }

    /// Check quota with specific limits
    pub async fn check_quota_with_limits(
        &self,
        license_key: &str,
        limits: &UsageQuota,
    ) -> Result<QuotaResult, MeterError> {
        let usage = self.get_usage(license_key, Period::CurrentMonth).await?;

        // Check simulation quota
        if let Some(max_sims) = limits.max_simulations {
            if usage.simulations >= max_sims {
                return Ok(QuotaResult::Exceeded {
                    limit: max_sims,
                    used: usage.simulations,
                });
            }
        }

        // Check cycle quota
        if let Some(max_cycles) = limits.max_cycles {
            if usage.total_cycles >= max_cycles {
                return Ok(QuotaResult::Exceeded {
                    limit: max_cycles,
                    used: usage.total_cycles,
                });
            }
        }

        // Check API request quota
        if let Some(max_api) = limits.max_api_requests {
            if usage.api_requests >= max_api {
                return Ok(QuotaResult::Exceeded {
                    limit: max_api,
                    used: usage.api_requests,
                });
            }
        }

        // Calculate remaining quota (use minimum remaining if multiple limits)
        let remaining = [
            limits.max_simulations.map(|m| m.saturating_sub(usage.simulations)),
            limits.max_api_requests.map(|m| m.saturating_sub(usage.api_requests)),
        ]
        .iter()
        .filter_map(|&x| x)
        .min();

        Ok(QuotaResult::Allowed { remaining })
    }
}

impl Default for InMemoryMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UsageMeter for InMemoryMeter {
    async fn record(&self, license_key: &str, usage: UsageEvent) -> Result<(), MeterError> {
        let record = UsageRecord {
            timestamp: Utc::now(),
            event: usage,
        };

        self.records
            .write()
            .entry(license_key.to_string())
            .or_insert_with(Vec::new)
            .push(record);

        Ok(())
    }

    async fn get_usage(&self, license_key: &str, period: Period) -> Result<Usage, MeterError> {
        let records = self.records.read();
        let key_records = records.get(license_key).map(|v| v.as_slice()).unwrap_or(&[]);

        Ok(self.calculate_usage(key_records, period))
    }

    async fn check_quota(
        &self,
        license_key: &str,
        operation: Operation,
    ) -> Result<QuotaResult, MeterError> {
        // Get quota for this license
        let quota = {
            let quotas = self.quotas.read();
            quotas.get(license_key).ok_or_else(|| {
                MeterError::LicenseNotFound {
                    key: license_key.to_string(),
                }
            })?.clone()
        }; // Lock is dropped here

        // Check tile limits for create simulation
        if let Operation::CreateSimulation { tiles } = operation {
            if tiles > quota.max_tiles {
                return Ok(QuotaResult::Exceeded {
                    limit: quota.max_tiles as u64,
                    used: tiles as u64,
                });
            }
        }

        // Check usage quotas
        self.check_quota_with_limits(license_key, &quota).await
    }

    async fn reset(&self, license_key: &str) -> Result<(), MeterError> {
        self.records.write().remove(license_key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_usage() {
        let meter = InMemoryMeter::new();

        meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
        meter.record("key1", UsageEvent::Simulation { cycles: 200 }).await.unwrap();

        let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

        assert_eq!(usage.simulations, 2);
        assert_eq!(usage.total_cycles, 300);
    }

    #[tokio::test]
    async fn test_isolate_by_key() {
        let meter = InMemoryMeter::new();

        meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
        meter.record("key2", UsageEvent::Simulation { cycles: 200 }).await.unwrap();

        let usage1 = meter.get_usage("key1", Period::AllTime).await.unwrap();
        let usage2 = meter.get_usage("key2", Period::AllTime).await.unwrap();

        assert_eq!(usage1.total_cycles, 100);
        assert_eq!(usage2.total_cycles, 200);
    }

    #[tokio::test]
    async fn test_api_request_tracking() {
        let meter = InMemoryMeter::new();

        for _ in 0..10 {
            meter.record("key1", UsageEvent::ApiRequest {
                endpoint: "/test".to_string()
            }).await.unwrap();
        }

        let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

        assert_eq!(usage.api_requests, 10);
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let meter = InMemoryMeter::new();
        meter.set_quota_from_tier("free_key", LicenseTier::Free);

        // Record up to quota
        for _ in 0..1000 {
            meter.record("free_key", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
        }

        let result = meter.check_quota("free_key", Operation::RunSimulation).await.unwrap();

        assert!(matches!(result, QuotaResult::Exceeded { .. }));
    }

    #[tokio::test]
    async fn test_unlimited_quota() {
        let meter = InMemoryMeter::new();
        meter.set_quota_from_tier("ent_key", LicenseTier::Enterprise);

        // Record massive usage
        for _ in 0..10_000 {
            meter.record("ent_key", UsageEvent::Simulation { cycles: 1000 }).await.unwrap();
        }

        let result = meter.check_quota("ent_key", Operation::RunSimulation).await.unwrap();

        assert!(matches!(result, QuotaResult::Allowed { .. }));
    }

    #[tokio::test]
    async fn test_reset() {
        let meter = InMemoryMeter::new();

        meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
        meter.reset("key1").await.unwrap();

        let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

        assert_eq!(usage.simulations, 0);
    }
}
