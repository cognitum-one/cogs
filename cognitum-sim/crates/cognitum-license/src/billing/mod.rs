//! Billing integration (Stripe)

use async_trait::async_trait;
use mockall::automock;
use serde::{Deserialize, Serialize};
use crate::{BillingError, LicenseTier, Usage};

#[cfg(feature = "billing")]
pub mod stripe;

#[cfg(feature = "billing")]
pub use stripe::StripeBillingClient;

/// Billing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingInfo {
    /// Customer ID
    pub customer_id: String,

    /// Current subscription status
    pub subscription_status: SubscriptionStatus,

    /// Current period start
    pub period_start: chrono::DateTime<chrono::Utc>,

    /// Current period end
    pub period_end: chrono::DateTime<chrono::Utc>,

    /// Amount due in cents
    pub amount_due_cents: u64,
}

/// Subscription status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    /// Active subscription
    Active,

    /// Payment past due
    PastDue,

    /// Cancelled
    Cancelled,

    /// Trialing
    Trialing,

    /// Incomplete
    Incomplete,
}

/// Checkout session for payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    /// Session ID
    pub session_id: String,

    /// Checkout URL
    pub url: String,

    /// Amount in cents
    pub amount_cents: u64,

    /// License tier
    pub tier: LicenseTier,
}

/// Webhook event result
#[derive(Debug, Clone)]
pub enum WebhookResult {
    /// Payment succeeded
    PaymentSucceeded {
        customer_id: String,
        tier: LicenseTier,
    },

    /// Subscription created
    SubscriptionCreated {
        customer_id: String,
        subscription_id: String,
    },

    /// Subscription cancelled
    SubscriptionCancelled {
        customer_id: String,
    },

    /// Payment failed
    PaymentFailed {
        customer_id: String,
        reason: String,
    },

    /// Other event (not handled)
    Other {
        event_type: String,
    },
}

/// Trait for billing client
#[automock]
#[async_trait]
pub trait BillingClient: Send + Sync {
    /// Report usage to billing provider
    async fn report_usage(&self, license_key: &str, usage: &Usage) -> Result<(), BillingError>;

    /// Get current billing info
    async fn get_billing_info(&self, license_key: &str) -> Result<BillingInfo, BillingError>;

    /// Process webhook from billing provider
    async fn process_webhook(
        &self,
        payload: &[u8],
        signature: &str,
    ) -> Result<WebhookResult, BillingError>;

    /// Create checkout session
    async fn create_checkout(&self, tier: LicenseTier) -> Result<CheckoutSession, BillingError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_status() {
        assert_eq!(
            serde_json::to_string(&SubscriptionStatus::Active).unwrap(),
            r#""Active""#
        );
    }

    #[test]
    fn test_checkout_session_serialization() {
        let session = CheckoutSession {
            session_id: "sess_123".to_string(),
            url: "https://checkout.stripe.com/...".to_string(),
            amount_cents: 9900,
            tier: LicenseTier::Developer,
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: CheckoutSession = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.session_id, session.session_id);
        assert_eq!(deserialized.amount_cents, session.amount_cents);
    }
}
