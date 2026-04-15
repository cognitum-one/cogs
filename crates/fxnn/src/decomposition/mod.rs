//! Domain Decomposition for Parallel Molecular Dynamics
//!
//! This module implements MinCut-based domain decomposition for efficient
//! parallel execution of molecular dynamics simulations. The key idea is to
//! partition atoms into spatial domains that minimize cross-boundary interactions,
//! thereby reducing communication overhead between threads.
//!
//! # Architecture
//!
//! The decomposition follows a three-level hierarchy:
//!
//! 1. **Interaction Graph**: Built from neighbor lists, representing atom-atom
//!    interactions as weighted edges (by distance/potential contribution)
//!
//! 2. **MinCut Partitioning**: Uses graph partitioning algorithms to find
//!    domain boundaries that minimize cross-boundary edges
//!
//! 3. **Domain Management**: Handles ghost atoms, boundary synchronization,
//!    and load balancing across threads
//!
//! # Performance Characteristics
//!
//! - Graph construction: O(N * k) where k is average neighbor count
//! - MinCut partitioning: O(N log N) approximate, O(N^2) exact
//! - Ghost atom updates: O(boundary_size) per timestep
//! - Rebalancing: Amortized O(1) with adaptive thresholds
//!
//! # Example
//!
//! ```rust,ignore
//! use fxnn::decomposition::{DomainDecomposer, DecompositionConfig};
//! use fxnn::neighbor::NeighborList;
//!
//! // Build interaction graph from neighbor list
//! let config = DecompositionConfig::default()
//!     .with_num_domains(8)
//!     .with_ghost_cutoff(3.0);
//!
//! let decomposer = DomainDecomposer::new(config);
//! let domains = decomposer.partition(&atoms, &neighbor_list);
//!
//! // Each domain can now be processed independently
//! domains.par_iter().for_each(|domain| {
//!     domain.compute_forces(&force_field);
//! });
//! ```

pub mod domain;
pub mod mincut;

pub use domain::{
    AtomDomain, BoundaryAtom, DomainBoundary, DomainDecomposer, DomainId, DomainStats,
    GhostAtom, GhostManager, LoadBalancer,
};
pub use mincut::{
    InteractionEdge, InteractionGraph, MinCutPartitioner, PartitionConfig,
    PartitionResult, PartitionStrategy,
};

/// Configuration for domain decomposition
#[derive(Debug, Clone)]
pub struct DecompositionConfig {
    /// Number of domains to partition into (typically = number of threads)
    pub num_domains: usize,
    /// Cutoff distance for ghost atoms (should be >= force cutoff)
    pub ghost_cutoff: f32,
    /// Maximum imbalance ratio before triggering rebalancing (e.g., 1.1 = 10% imbalance)
    pub max_imbalance: f32,
    /// Minimum atoms per domain before merging
    pub min_atoms_per_domain: usize,
    /// Whether to use approximate (faster) or exact MinCut
    pub use_approximate_mincut: bool,
    /// Edge weight strategy for building interaction graph
    pub edge_weight_strategy: EdgeWeightStrategy,
    /// Rebalancing frequency (in timesteps, 0 = never)
    pub rebalance_frequency: usize,
    /// Ghost atom skin distance for neighbor list updates
    pub ghost_skin: f32,
}

impl Default for DecompositionConfig {
    fn default() -> Self {
        Self {
            num_domains: 1,
            ghost_cutoff: 3.0,
            max_imbalance: 1.15,
            min_atoms_per_domain: 100,
            use_approximate_mincut: true,
            edge_weight_strategy: EdgeWeightStrategy::Uniform,
            rebalance_frequency: 100,
            ghost_skin: 0.5,
        }
    }
}

impl DecompositionConfig {
    /// Create a new configuration with specified number of domains
    pub fn with_num_domains(mut self, n: usize) -> Self {
        self.num_domains = n.max(1);
        self
    }

    /// Set the ghost atom cutoff distance
    pub fn with_ghost_cutoff(mut self, cutoff: f32) -> Self {
        self.ghost_cutoff = cutoff;
        self
    }

    /// Set the maximum allowed load imbalance
    pub fn with_max_imbalance(mut self, ratio: f32) -> Self {
        self.max_imbalance = ratio.max(1.0);
        self
    }

    /// Set whether to use approximate MinCut algorithm
    pub fn with_approximate(mut self, approximate: bool) -> Self {
        self.use_approximate_mincut = approximate;
        self
    }

    /// Set the edge weight strategy
    pub fn with_edge_weight_strategy(mut self, strategy: EdgeWeightStrategy) -> Self {
        self.edge_weight_strategy = strategy;
        self
    }

    /// Set the rebalancing frequency
    pub fn with_rebalance_frequency(mut self, freq: usize) -> Self {
        self.rebalance_frequency = freq;
        self
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> std::result::Result<(), DecompositionError> {
        if self.num_domains == 0 {
            return Err(DecompositionError::InvalidConfig(
                "num_domains must be > 0".to_string(),
            ));
        }
        if self.ghost_cutoff <= 0.0 {
            return Err(DecompositionError::InvalidConfig(
                "ghost_cutoff must be > 0".to_string(),
            ));
        }
        if self.max_imbalance < 1.0 {
            return Err(DecompositionError::InvalidConfig(
                "max_imbalance must be >= 1.0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Strategy for assigning edge weights in the interaction graph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeWeightStrategy {
    /// All edges have weight 1 (minimize number of cross-boundary pairs)
    Uniform,
    /// Weight by inverse distance (favor cutting distant interactions)
    InverseDistance,
    /// Weight by interaction strength (LJ potential magnitude)
    InteractionStrength,
    /// Weight by communication cost estimate
    CommunicationCost,
}

/// Errors that can occur during domain decomposition
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecompositionError {
    /// Invalid configuration parameters
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Not enough atoms to partition
    #[error("Insufficient atoms ({0}) for {1} domains")]
    InsufficientAtoms(usize, usize),

    /// Graph construction failed
    #[error("Failed to build interaction graph: {0}")]
    GraphBuildFailed(String),

    /// Partitioning algorithm failed
    #[error("Partitioning failed: {0}")]
    PartitionFailed(String),

    /// Domain boundary error
    #[error("Boundary error: {0}")]
    BoundaryError(String),

    /// Load balancing failed
    #[error("Load balancing failed: {0}")]
    LoadBalanceFailed(String),
}

/// Result type for decomposition operations
pub type Result<T> = std::result::Result<T, DecompositionError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DecompositionConfig::default();
        assert_eq!(config.num_domains, 1);
        assert!(config.ghost_cutoff > 0.0);
        assert!(config.max_imbalance >= 1.0);
    }

    #[test]
    fn test_config_builder() {
        let config = DecompositionConfig::default()
            .with_num_domains(8)
            .with_ghost_cutoff(5.0)
            .with_max_imbalance(1.2)
            .with_approximate(false);

        assert_eq!(config.num_domains, 8);
        assert_eq!(config.ghost_cutoff, 5.0);
        assert_eq!(config.max_imbalance, 1.2);
        assert!(!config.use_approximate_mincut);
    }

    #[test]
    fn test_config_validation() {
        let valid = DecompositionConfig::default();
        assert!(valid.validate().is_ok());

        let invalid = DecompositionConfig {
            num_domains: 0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());

        let invalid = DecompositionConfig {
            ghost_cutoff: -1.0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }
}
