//! Stripe billing integration

use super::{BillingClient, BillingInfo, CheckoutSession, WebhookResult, SubscriptionStatus};
use crate::{BillingError, LicenseTier, Usage};
use async_trait::async_trait;

/// Stripe billing client
pub struct StripeBillingClient {
    /// Stripe API key
    api_key: String,

    /// Webhook signing secret
    webhook_secret: String,
}

impl StripeBillingClient {
    /// Create a new Stripe billing client
    pub fn new(api_key: String, webhook_secret: String) -> Self {
        Self {
            api_key,
            webhook_secret,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, BillingError> {
        let api_key = std::env::var("STRIPE_API_KEY")
            .map_err(|_| BillingError::Configuration("STRIPE_API_KEY not set".to_string()))?;

        let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
            .map_err(|_| BillingError::Configuration("STRIPE_WEBHOOK_SECRET not set".to_string()))?;

        Ok(Self::new(api_key, webhook_secret))
    }
}

#[async_trait]
impl BillingClient for StripeBillingClient {
    async fn report_usage(&self, _license_key: &str, _usage: &Usage) -> Result<(), BillingError> {
        // In a real implementation, this would:
        // 1. Get customer ID from license key
        // 2. Create usage record in Stripe
        // 3. Handle metered billing

        // For now, return success
        Ok(())
    }

    async fn get_billing_info(&self, _license_key: &str) -> Result<BillingInfo, BillingError> {
        // In a real implementation, this would:
        // 1. Get customer ID from license key
        // 2. Fetch subscription from Stripe
        // 3. Parse subscription details

        // For now, return mock data
        Ok(BillingInfo {
            customer_id: "cus_mock".to_string(),
            subscription_status: SubscriptionStatus::Active,
            period_start: chrono::Utc::now(),
            period_end: chrono::Utc::now() + chrono::Duration::days(30),
            amount_due_cents: 9900,
        })
    }

    async fn process_webhook(
        &self,
        _payload: &[u8],
        _signature: &str,
    ) -> Result<WebhookResult, BillingError> {
        // In a real implementation, this would:
        // 1. Verify webhook signature
        // 2. Parse webhook event
        // 3. Handle different event types

        // For now, return a mock result
        Ok(WebhookResult::Other {
            event_type: "test".to_string(),
        })
    }

    async fn create_checkout(&self, tier: LicenseTier) -> Result<CheckoutSession, BillingError> {
        // In a real implementation, this would:
        // 1. Create Stripe checkout session
        // 2. Set price based on tier
        // 3. Configure success/cancel URLs

        let amount_cents = tier.price_cents()
            .ok_or_else(|| BillingError::Configuration("Tier has no price".to_string()))?;

        Ok(CheckoutSession {
            session_id: "sess_mock_123".to_string(),
            url: "https://checkout.stripe.com/mock".to_string(),
            amount_cents,
            tier,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_checkout() {
        let client = StripeBillingClient::new(
            "sk_test_mock".to_string(),
            "whsec_mock".to_string(),
        );

        let session = client.create_checkout(LicenseTier::Developer).await.unwrap();

        assert_eq!(session.amount_cents, 9900);
        assert_eq!(session.tier, LicenseTier::Developer);
    }

    #[tokio::test]
    async fn test_get_billing_info() {
        let client = StripeBillingClient::new(
            "sk_test_mock".to_string(),
            "whsec_mock".to_string(),
        );

        let info = client.get_billing_info("test_key").await.unwrap();

        assert_eq!(info.subscription_status, SubscriptionStatus::Active);
    }
}
