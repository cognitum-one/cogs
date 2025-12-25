# Cognitum License System

A comprehensive licensing and usage metering system for the Cognitum chip simulator, featuring Ed25519 cryptographic signing, offline validation, and tier-based quota enforcement.

## Features

- **Ed25519 Cryptographic Signing**: Secure license validation using modern elliptic curve cryptography
- **Offline Validation**: Licenses can be validated without network access
- **Usage Metering**: Track simulations, cycles, and API requests with high precision
- **Quota Enforcement**: Tier-based limits on tiles, simulations, and API usage
- **Billing Integration**: Optional Stripe integration for subscription management (feature-gated)
- **Feature Gating**: Control access to advanced features by license tier
- **Thread-Safe**: All components are designed for concurrent access

## License Tiers

| Tier | Price | Max Tiles | Simulations/Month | API Requests/Month | Special Features |
|------|-------|-----------|-------------------|-------------------|------------------|
| **Free** | $0 | 32 | 1,000 | None | Open source use |
| **Developer** | $99/mo | 256 | Unlimited | 10,000 | API access, advanced debugging |
| **Professional** | $499/mo | 1,024 | Unlimited | 100,000 | Custom models, distributed sim, cloud deploy |
| **Enterprise** | Custom | Unlimited | Unlimited | Unlimited | HIPAA compliance, priority support, hardware export |

## Quick Start

### Basic Usage

```rust
use cognitum_license::{
    Ed25519Validator, Ed25519Generator, LicenseValidator, LicenseGenerator,
    LicenseRequest, LicenseTier, InMemoryStore
};
use ed25519_dalek::SigningKey;
use std::sync::Arc;

// Initialize keys (in production, load from secure storage)
let signing_key = SigningKey::from_bytes(&[/* 32 bytes */]);
let public_key = signing_key.verifying_key();

// Create store (use persistent store in production)
let store = Arc::new(InMemoryStore::new());

// Create validator and generator
let validator = Ed25519Validator::new(public_key, store.clone());
let generator = Ed25519Generator::new(signing_key, store);

// Generate a license
let license = generator.generate(LicenseRequest {
    tier: LicenseTier::Developer,
    organization: "Acme Corp".to_string(),
    email: "admin@acme.com".to_string(),
    duration_months: 12,
    ..Default::default()
})?;

println!("License key: {}", license.key);

// Validate the license
let validated = validator.validate(&license.key)?;
assert_eq!(validated.tier, LicenseTier::Developer);
assert_eq!(validated.max_tiles, 256);
```

### Usage Metering

```rust
use cognitum_license::meter::{InMemoryMeter, UsageMeter, UsageEvent, Period};
use cognitum_license::{LicenseTier, Operation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let meter = InMemoryMeter::new();
    let license_key = "lic_dev_abc123_xyz";

    // Set quota for license
    meter.set_quota_from_tier(license_key, LicenseTier::Developer);

    // Record usage
    meter.record(license_key, UsageEvent::Simulation { cycles: 10000 }).await?;
    meter.record(license_key, UsageEvent::ApiRequest {
        endpoint: "/simulations".to_string()
    }).await?;

    // Check quota before operation
    let quota_result = meter.check_quota(
        license_key,
        Operation::CreateSimulation { tiles: 128 }
    ).await?;

    match quota_result {
        cognitum_license::meter::QuotaResult::Allowed { remaining } => {
            println!("Operation allowed, remaining quota: {:?}", remaining);
        }
        cognitum_license::meter::QuotaResult::Exceeded { limit, used } => {
            println!("Quota exceeded: {}/{}", used, limit);
        }
    }

    // Get usage statistics
    let usage = meter.get_usage(license_key, Period::CurrentMonth).await?;
    println!("Simulations this month: {}", usage.simulations);
    println!("Total cycles: {}", usage.total_cycles);
    println!("API requests: {}", usage.api_requests);

    Ok(())
}
```

### Feature Gating

```rust
use cognitum_license::{Feature, LicenseValidator, Ed25519Validator, Operation};

// Check if license has specific feature
if validator.check_feature(&license, Feature::ApiAccess) {
    // Allow API access
}

if validator.check_feature(&license, Feature::HipaaCompliance) {
    // Enable HIPAA compliance features
}

// Check operation limits
validator.check_limits(&license, Operation::CreateSimulation { tiles: 64 })?;
```

### Billing Integration (Optional)

Enable the `billing` feature in your `Cargo.toml`:

```toml
cognitum-license = { version = "0.1", features = ["billing"] }
```

```rust
use cognitum_license::billing::{StripeBillingClient, BillingClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Stripe client
    let billing = StripeBillingClient::from_env()?;

    // Create checkout session
    let session = billing.create_checkout(LicenseTier::Developer).await?;
    println!("Checkout URL: {}", session.url);

    // Process webhook
    let result = billing.process_webhook(payload, signature).await?;

    Ok(())
}
```

## Architecture

### Components

- **License**: Core license structure with tier, organization, features, and cryptographic signature
- **LicenseValidator**: Validates license keys using Ed25519 signature verification
- **LicenseGenerator**: Generates and signs new licenses
- **UsageMeter**: Tracks usage events and enforces quotas
- **LicenseStore**: Persistent storage for licenses (in-memory and custom implementations)
- **BillingClient**: Integration with payment providers (Stripe)
- **FeatureChecker**: Manages feature availability by tier

### Security

- **Ed25519 Signatures**: All licenses are cryptographically signed using Ed25519
- **Offline Validation**: Licenses can be validated without internet connectivity
- **Tamper Detection**: Any modification to license data invalidates the signature
- **Revocation Support**: Licenses can be revoked server-side

## Testing

Run the comprehensive test suite:

```bash
# All tests
cargo test -p cognitum-license

# Unit tests only
cargo test -p cognitum-license --lib

# Acceptance tests only
cargo test -p cognitum-license --test '*'

# With billing features
cargo test -p cognitum-license --all-features
```

Run benchmarks:

```bash
cargo bench -p cognitum-license
```

## Performance

- License validation: ~50μs per operation
- Usage recording: ~5μs per event
- Quota checking: ~10μs per check
- Thread-safe with minimal lock contention

## License Key Format

License keys follow the format: `lic_{tier}_{random}_{checksum}`

Example: `lic_dev_a1b2c3d4e5f6g7h8_9i0j1k`

- **lic**: Fixed prefix
- **tier**: Tier code (free/dev/pro/ent)
- **random**: 32 hex characters (16 bytes)
- **checksum**: 6 hex characters for validation

## Production Deployment

### Key Management

```rust
// Generate signing key (do this once, securely)
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

let signing_key = SigningKey::generate(&mut OsRng);
let public_key = signing_key.verifying_key();

// Store keys securely (HSM, key vault, etc.)
std::fs::write("signing_key.secret", signing_key.to_bytes())?;
std::fs::write("public_key.pub", public_key.to_bytes())?;
```

### Persistent Storage

Implement the `LicenseStore` trait for your database:

```rust
use cognitum_license::store::LicenseStore;
use cognitum_license::{License, LicenseError};

struct PostgresStore {
    pool: sqlx::PgPool,
}

impl LicenseStore for PostgresStore {
    fn save(&self, license: &License) -> Result<(), LicenseError> {
        // Save to database
    }

    fn get(&self, key: &str) -> Result<License, LicenseError> {
        // Load from database
    }

    // Implement other methods...
}
```

## Contributing

See the main Cognitum project for contribution guidelines.

## TDD Implementation

This crate was developed using Test-Driven Development following the London School approach:

- **Acceptance Tests**: Define customer-visible behavior
- **Unit Tests**: Test components in isolation with mocks
- **Integration Tests**: Verify component interactions

Test coverage: >90%

## License

MIT OR Apache-2.0
