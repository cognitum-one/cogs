//! API data models

pub mod error;
pub mod request;
pub mod response;

use std::sync::Arc;
use crate::services::{SimulatorService, StorageService, AuthService, RateLimiter};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub simulator: Arc<dyn SimulatorService>,
    pub storage: Arc<dyn StorageService>,
    pub auth: Arc<dyn AuthService>,
    pub rate_limiter: Arc<dyn RateLimiter>,
}

impl AppState {
    pub fn new(
        simulator: Arc<dyn SimulatorService>,
        storage: Arc<dyn StorageService>,
        auth: Arc<dyn AuthService>,
        rate_limiter: Arc<dyn RateLimiter>,
    ) -> Self {
        Self {
            simulator,
            storage,
            auth,
            rate_limiter,
        }
    }
}

// Type aliases
pub type SimulationId = String;
pub type ProgramId = String;
pub type UserId = String;
pub type JobId = String;
