//! Acceptance tests for usage metering

use cognitum_license::meter::{InMemoryMeter, UsageMeter, UsageEvent, Period, QuotaResult};
use cognitum_license::{LicenseTier, Operation};

#[tokio::test]
async fn should_track_simulation_usage() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_usage_key";

    // Record simulations
    meter.record(key, UsageEvent::Simulation { cycles: 1000 }).await.unwrap();
    meter.record(key, UsageEvent::Simulation { cycles: 2000 }).await.unwrap();

    let usage = meter.get_usage(key, Period::AllTime).await.unwrap();

    assert_eq!(usage.simulations, 2);
    assert_eq!(usage.total_cycles, 3000);
}

#[tokio::test]
async fn should_enforce_monthly_quota() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_free_key";
    meter.set_quota_from_tier(key, LicenseTier::Free);

    // Use up quota (Free tier: 1000 sims/month)
    for _ in 0..1000 {
        meter.record(key, UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    }

    // Check quota
    let result = meter.check_quota(key, Operation::RunSimulation).await.unwrap();

    assert!(matches!(result, QuotaResult::Exceeded { .. }));
}

#[tokio::test]
async fn should_track_api_requests() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_dev_key";

    for i in 0..100 {
        meter.record(key, UsageEvent::ApiRequest {
            endpoint: format!("/simulations/{}", i)
        }).await.unwrap();
    }

    let usage = meter.get_usage(key, Period::AllTime).await.unwrap();

    assert_eq!(usage.api_requests, 100);
}

#[tokio::test]
async fn should_allow_unlimited_for_enterprise() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_enterprise_key";
    meter.set_quota_from_tier(key, LicenseTier::Enterprise);

    // Record massive usage
    for _ in 0..10_000 {
        meter.record(key, UsageEvent::Simulation { cycles: 1_000_000 }).await.unwrap();
    }

    let result = meter.check_quota(key, Operation::RunSimulation).await.unwrap();

    assert!(matches!(result, QuotaResult::Allowed { .. }));
}

#[tokio::test]
async fn should_track_usage_by_period() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_period_key";

    // Record some usage
    for _ in 0..50 {
        meter.record(key, UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    }

    let usage_all_time = meter.get_usage(key, Period::AllTime).await.unwrap();
    let usage_current_month = meter.get_usage(key, Period::CurrentMonth).await.unwrap();

    assert_eq!(usage_all_time.simulations, 50);
    assert_eq!(usage_current_month.simulations, 50);
}

#[tokio::test]
async fn should_reset_usage_counters() {
    let meter = InMemoryMeter::new();
    let key = "lic_test_reset_key";

    meter.record(key, UsageEvent::Simulation { cycles: 100 }).await.unwrap();

    let usage_before = meter.get_usage(key, Period::AllTime).await.unwrap();
    assert_eq!(usage_before.simulations, 1);

    meter.reset(key).await.unwrap();

    let usage_after = meter.get_usage(key, Period::AllTime).await.unwrap();
    assert_eq!(usage_after.simulations, 0);
}
