//! Common types for Ruvector SDK

use serde::{Deserialize, Serialize};

/// Embedding identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(pub u64);

/// Vector embedding
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    pub data: Vec<f32>,
}

impl Embedding {
    pub fn new(data: Vec<f32>) -> Self {
        Self { data }
    }

    pub fn dimension(&self) -> usize {
        self.data.len()
    }

    pub fn random(dim: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            data: (0..dim).map(|_| rng.gen::<f32>()).collect(),
        }
    }

    pub fn zeros(dim: usize) -> Self {
        Self {
            data: vec![0.0; dim],
        }
    }
}

/// Tile identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId(pub u32);

/// Group identifier for parallel operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u32);

/// Embedding metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub tile_id: Option<TileId>,
    pub timestamp: Option<u64>,
    pub cycle_count: Option<u64>,
    pub custom: std::collections::HashMap<String, String>,
}

/// Search result with similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: EmbeddingId,
    pub similarity: f32,
    pub metadata: Metadata,
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            id: EmbeddingId(0),
            similarity: 0.0,
            metadata: Metadata::default(),
        }
    }
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub num_vectors: usize,
    pub dimension: usize,
    pub memory_bytes: usize,
}

/// Task embedding for routing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEmbedding {
    pub data: Vec<f32>,
}

impl TaskEmbedding {
    pub fn new(data: Vec<f32>) -> Self {
        Self { data }
    }

    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            data: (0..256).map(|_| rng.gen::<f32>()).collect(),
        }
    }

    pub fn from_description(name: &str, params: &[usize]) -> Self {
        // Simple hash-based encoding for demo
        let mut data = vec![0.0; 256];
        let hash = name.as_bytes().iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        data[0] = (hash % 256) as f32 / 255.0;
        for (i, &p) in params.iter().enumerate().take(10) {
            data[i + 1] = (p % 256) as f32 / 255.0;
        }
        Self { data }
    }
}

/// Training metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub epochs: usize,
    pub final_loss: f32,
    pub accuracy: f32,
}

/// Execution trace for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub task_embedding: TaskEmbedding,
    pub actual_tile: TileId,
    pub execution_time_us: u64,
    pub success: bool,
}

impl ExecutionTrace {
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            task_embedding: TaskEmbedding::random(),
            actual_tile: TileId(rng.gen_range(0..16)),
            execution_time_us: rng.gen_range(100..10000),
            success: true,
        }
    }
}

/// Vector operations for parallel execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorOp {
    Sum,
    DotProduct,
    MatrixMultiply,
    Normalize,
}

/// Tile state for embedding generation
#[derive(Debug, Clone, Default)]
pub struct TileState {
    pub program_counter: u32,
    pub stack_pointer: u32,
    pub registers: [u8; 32],
    pub cycle_count: u64,
    pub message_count: u32,
}

impl TileState {
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut registers = [0u8; 32];
        for r in &mut registers {
            *r = rng.gen();
        }
        Self {
            program_counter: rng.gen(),
            stack_pointer: rng.gen_range(0..4096),
            registers,
            cycle_count: rng.gen(),
            message_count: rng.gen(),
        }
    }
}

/// Processor state (all registers)
#[derive(Debug, Clone, Default)]
pub struct ProcessorState {
    pub tiles: Vec<TileState>,
}

/// Ruvector configuration
#[derive(Debug, Clone)]
pub struct RuvectorConfig {
    pub embedding_dimension: usize,
    pub index_capacity: usize,
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub num_tiles: usize,
}

impl Default for RuvectorConfig {
    fn default() -> Self {
        Self {
            embedding_dimension: 256,
            index_capacity: 100_000,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            num_tiles: 16,
        }
    }
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Index error: {0}")]
    Backend(String),
    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },
    #[error("Not found: {0}")]
    NotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("Router error: {0}")]
    Training(String),
    #[error("Model error: {0}")]
    Model(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RaceWayError {
    #[error("RaceWay error: {0}")]
    Communication(String),
    #[error("Invalid group: {0}")]
    InvalidGroup(String),
    #[error("Timeout")]
    Timeout,
}

/// Cosine similarity helper function
pub fn cosine_similarity(a: &Embedding, b: &Embedding) -> f32 {
    if a.data.len() != b.data.len() {
        return 0.0;
    }

    let dot: f32 = a.data.iter().zip(&b.data).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.data.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.data.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
