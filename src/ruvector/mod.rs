//! Ruvector SDK Integration for Cognitum Chip
//!
//! Provides vector embedding, indexing, routing, and parallel operations
//! for the Cognitum chip architecture.

pub mod embedding;
pub mod index;
pub mod router;
pub mod bridge;
pub mod facade;
pub mod types;
pub mod partitioning;
pub mod fusion;

// Re-export main types
pub use embedding::{EmbeddingGenerator, DefaultEmbeddingGenerator};
pub use index::{VectorIndex, HnswVectorIndex};
pub use router::{TaskRouter, TinyDancerRouter};
pub use bridge::{RaceWayBridge, DefaultRaceWayBridge};
pub use facade::CognitumRuvector;
pub use types::*;
pub use partitioning::{
    MinCutPartitioner, KernighanLinPartitioner, WorkloadPartitioner,
    TileGraph, TileNode, Partition, PartitionId, PartitionError,
};
pub use fusion::{
    FusedEdgeWeight, RelationType, BrittlenessMonitor, GraphOptimizer,
    OptimizerAction, HealthSignal, FusionGraph,
};
