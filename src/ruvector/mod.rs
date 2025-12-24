//! Ruvector SDK Integration for Cognitum Chip
//!
//! Provides vector embedding, indexing, routing, and parallel operations
//! for the Cognitum chip architecture.

pub mod embedding;
pub mod index;
pub mod router;
pub mod snn_router;
pub mod bridge;
pub mod facade;
pub mod types;
pub mod partitioning;
pub mod fusion;
pub mod simulator_bridge;
pub mod quantization;

// Re-export main types
pub use embedding::{EmbeddingGenerator, DefaultEmbeddingGenerator};
pub use index::{VectorIndex, HnswVectorIndex};
pub use router::{TaskRouter, TinyDancerRouter};
pub use snn_router::{SnnRouter, LifNeuron, StdpRule, SpikingLayer, simd_integrate_batch};
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
pub use simulator_bridge::{
    SimulatorBridge, BridgeError, PerformanceMetrics, RebalanceResult,
};
pub use quantization::{
    ScalarQuantizer, ProductQuantizer, QuantizedHnswIndex,
    QuantizedVector, PQCode, DistanceTables,
    QuantizationType, MemoryEstimate, estimate_memory,
    QuantizedHnswConfig,
};
