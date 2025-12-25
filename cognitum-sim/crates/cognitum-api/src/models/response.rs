//! API response models

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use super::request::{SimulationConfig, ProgramMetadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSimulationResponse {
    pub id: String,
    pub status: String,
    pub config: SimulationConfig,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResponse {
    pub id: String,
    pub status: String,
    pub config: SimulationConfig,
    pub owner: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
    pub job_id: String,
    pub status: String,
    pub estimated_completion: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStatusResponse {
    pub simulation_id: String,
    pub status: String,
    pub progress: Option<f32>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResults {
    pub simulation_id: String,
    pub status: String,
    pub results: ExecutionResults,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResults {
    pub cycles_executed: u64,
    pub instructions_executed: u64,
    pub tiles_used: u32,
    pub memory_bytes_accessed: u64,
    pub execution_time_ms: u64,
    pub exit_reason: String,
}

impl Default for ExecutionResults {
    fn default() -> Self {
        Self {
            cycles_executed: 0,
            instructions_executed: 0,
            tiles_used: 0,
            memory_bytes_accessed: 0,
            execution_time_ms: 0,
            exit_reason: "not_started".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramResponse {
    pub id: String,
    pub size: usize,
    pub metadata: ProgramMetadata,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    Cycle(CycleData),
    Complete(ExecutionResults),
    Error(ErrorData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleData {
    pub cycle: u64,
    pub active_tiles: u32,
    pub instructions_this_cycle: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: String,
    pub message: String,
}
