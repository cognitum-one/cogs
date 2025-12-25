//! API request models

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateSimulationRequest {
    #[validate(nested)]
    pub config: SimulationConfig,
    pub program_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SimulationConfig {
    #[validate(range(min = 1, max = 256))]
    pub tiles: u32,

    #[validate(range(min = 1000, max = 10000000))]
    pub memory_per_tile: u64,

    #[serde(default)]
    pub enable_crypto: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            tiles: 16,
            memory_per_tile: 156000,
            enable_crypto: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RunRequest {
    #[validate(range(min = 1, max = 10000000))]
    pub cycles: Option<u64>,

    #[serde(default)]
    pub enable_tracing: bool,
}

impl Default for RunRequest {
    fn default() -> Self {
        Self {
            cycles: Some(100000),
            enable_tracing: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub size: usize,
}

impl Default for ProgramMetadata {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: "0.0.0".to_string(),
            description: None,
            size: 0,
        }
    }
}
