//! Job queue for async simulation execution

use serde::{Deserialize, Serialize};
use crate::models::{JobId, SimulationId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub job_type: JobType,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobType {
    RunSimulation {
        simulation_id: SimulationId,
        cycles: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}
