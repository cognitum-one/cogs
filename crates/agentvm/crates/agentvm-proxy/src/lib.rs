//! # agentvm-proxy
//!
//! Capability proxy for Agentic VM implementing ADR-005.
//!
//! This crate provides the host-side capability proxy that:
//! - Manages capability grants and revocations
//! - Validates capability invocations against scope and quota
//! - Executes operations through type-specific executors
//! - Logs all operations for evidence chain
//! - Communicates with guest capsules over vsock
//!
//! ## Architecture
//!
//! ```text
//! +------------------+     +-----------------+     +------------------+
//! |   Guest Capsule  | --> |  vsock Channel  | --> | Capability Proxy |
//! +------------------+     +-----------------+     +--------+---------+
//!                                                          |
//!                          +-------------------------------+
//!                          |                |              |
//!                          v                v              v
//!                   +----------+     +-----------+   +----------+
//!                   | Network  |     | Filesystem|   | Secrets  |
//!                   | Executor |     | Executor  |   | Provider |
//!                   +----------+     +-----------+   +----------+
//! ```
//!
//! ## Usage
//!
//! ```no_run
//! use agentvm_proxy::{CapabilityProxy, ProxyConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ProxyConfig::from_env()?;
//!     let proxy = CapabilityProxy::new(config).await?;
//!     proxy.run().await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod config;
pub mod error;
pub mod evidence;
pub mod executor;
pub mod proxy;
pub mod types;
pub mod vsock;
pub mod wire;

// Re-exports
pub use config::ProxyConfig;
pub use error::{GrantError, InvokeError, ProxyError, RevokeError};
pub use evidence::EvidenceLogger;
pub use executor::{Executor, ExecutorResult};
pub use proxy::CapabilityProxy;
pub use types::*;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Wire protocol magic number
pub const WIRE_MAGIC: u32 = 0x43415056; // "CAPV"

/// Wire protocol version
pub const WIRE_VERSION: u16 = 1;
