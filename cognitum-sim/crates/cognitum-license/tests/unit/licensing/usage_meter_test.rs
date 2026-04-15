//! Unit tests for usage metering

use cognitum_license::meter::{InMemoryMeter, UsageMeter, UsageEvent, Period, QuotaResult};
use cognitum_license::{LicenseTier, Operation};

#[tokio::test]
async fn should_accumulate_usage() {
    let meter = InMemoryMeter::new();

    meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    meter.record("key1", UsageEvent::Simulation { cycles: 200 }).await.unwrap();

    let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

    assert_eq!(usage.simulations, 2);
    assert_eq!(usage.total_cycles, 300);
}

#[tokio::test]
async fn should_isolate_usage_by_key() {
    let meter = InMemoryMeter::new();

    meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    meter.record("key2", UsageEvent::Simulation { cycles: 200 }).await.unwrap();

    let usage1 = meter.get_usage("key1", Period::AllTime).await.unwrap();
    let usage2 = meter.get_usage("key2", Period::AllTime).await.unwrap();

    assert_eq!(usage1.total_cycles, 100);
    assert_eq!(usage2.total_cycles, 200);
}

#[tokio::test]
async fn should_track_api_requests() {
    let meter = InMemoryMeter::new();

    for i in 0..10 {
        meter.record("key1", UsageEvent::ApiRequest {
            endpoint: format!("/endpoint{}", i)
        }).await.unwrap();
    }

    let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

    assert_eq!(usage.api_requests, 10);
}

#[tokio::test]
async fn should_check_quota_against_limits() {
    let meter = InMemoryMeter::new();
    meter.set_quota_from_tier("free_key", LicenseTier::Free);

    // Use up quota (Free tier: 1000 simulations/month)
    for _ in 0..1000 {
        meter.record("free_key", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    }

    let result = meter.check_quota("free_key", Operation::RunSimulation).await.unwrap();

    assert!(matches!(result, QuotaResult::Exceeded { .. }));
}

#[tokio::test]
async fn should_allow_unlimited_for_enterprise() {
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
async fn should_reset_usage() {
    let meter = InMemoryMeter::new();

    meter.record("key1", UsageEvent::Simulation { cycles: 100 }).await.unwrap();
    meter.reset("key1").await.unwrap();

    let usage = meter.get_usage("key1", Period::AllTime).await.unwrap();

    assert_eq!(usage.simulations, 0);
    assert_eq!(usage.total_cycles, 0);
}

#[tokio::test]
async fn should_enforce_tile_limits() {
    let meter = InMemoryMeter::new();
    meter.set_quota_from_tier("free_key", LicenseTier::Free);

    // Free tier: max 32 tiles
    let result = meter.check_quota("free_key", Operation::CreateSimulation { tiles: 33 }).await.unwrap();

    assert!(matches!(result, QuotaResult::Exceeded { limit: 32, used: 33 }));
}
