//! Error types for FXNN

use thiserror::Error;

/// Result type alias for FXNN operations
pub type Result<T> = std::result::Result<T, FxnnError>;

/// Errors that can occur during simulation
#[derive(Error, Debug)]
pub enum FxnnError {
    /// Error during force field evaluation
    #[error("Force field error: {0}")]
    ForceField(String),

    /// Error during neighbor list construction
    #[error("Neighbor list error: {0}")]
    NeighborList(String),

    /// Error during integration
    #[error("Integration error: {0}")]
    Integration(String),

    /// Invalid simulation parameters
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// File I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Numerical error (NaN, infinity, etc.)
    #[error("Numerical error: {0}")]
    Numerical(String),

    /// Topology error
    #[error("Topology error: {0}")]
    Topology(String),

    /// Out of bounds error
    #[error("Index out of bounds: {0}")]
    OutOfBounds(String),

    /// Neural network error
    #[error("Neural network error: {0}")]
    NeuralNetwork(String),

    /// Domain decomposition error
    #[error("Decomposition error: {0}")]
    Decomposition(String),

    // =========================================================================
    // Memory Module Errors (Layer 4: Agency)
    // =========================================================================

    /// Memory write rate limit exceeded
    #[error("Memory write rate exceeded: {0}")]
    MemoryRateExceeded(String),

    /// Memory capacity exceeded
    #[error("Memory capacity exceeded: {0}")]
    MemoryCapacityExceeded(String),

    /// Vector dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        got: usize,
    },

    /// Invalid trajectory data
    #[error("Invalid trajectory: {0}")]
    InvalidTrajectory(String),

    /// Checkpoint/restore failure
    #[error("Checkpoint error: {0}")]
    CheckpointError(String),

    /// SONA adaptation error
    #[error("SONA adaptation error: {0}")]
    SonaError(String),

    /// EWC consolidation error
    #[error("EWC consolidation error: {0}")]
    EwcError(String),

    /// Reasoning bank error
    #[error("Reasoning bank error: {0}")]
    ReasoningBankError(String),
}

impl FxnnError {
    /// Create a force field error
    pub fn force_field(msg: impl Into<String>) -> Self {
        Self::ForceField(msg.into())
    }

    /// Create a neighbor list error
    pub fn neighbor_list(msg: impl Into<String>) -> Self {
        Self::NeighborList(msg.into())
    }

    /// Create an integration error
    pub fn integration(msg: impl Into<String>) -> Self {
        Self::Integration(msg.into())
    }

    /// Create an invalid parameter error
    pub fn invalid_parameter(msg: impl Into<String>) -> Self {
        Self::InvalidParameter(msg.into())
    }

    /// Create a numerical error
    pub fn numerical(msg: impl Into<String>) -> Self {
        Self::Numerical(msg.into())
    }

    /// Create a topology error
    pub fn topology(msg: impl Into<String>) -> Self {
        Self::Topology(msg.into())
    }

    /// Create an out of bounds error
    pub fn out_of_bounds(msg: impl Into<String>) -> Self {
        Self::OutOfBounds(msg.into())
    }

    /// Create a memory rate exceeded error
    pub fn memory_rate_exceeded(msg: impl Into<String>) -> Self {
        Self::MemoryRateExceeded(msg.into())
    }

    /// Create a memory capacity exceeded error
    pub fn memory_capacity_exceeded(msg: impl Into<String>) -> Self {
        Self::MemoryCapacityExceeded(msg.into())
    }

    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: usize, got: usize) -> Self {
        Self::DimensionMismatch { expected, got }
    }

    /// Create an invalid trajectory error
    pub fn invalid_trajectory(msg: impl Into<String>) -> Self {
        Self::InvalidTrajectory(msg.into())
    }

    /// Create a checkpoint error
    pub fn checkpoint_error(msg: impl Into<String>) -> Self {
        Self::CheckpointError(msg.into())
    }

    /// Create a SONA error
    pub fn sona_error(msg: impl Into<String>) -> Self {
        Self::SonaError(msg.into())
    }

    /// Create an EWC error
    pub fn ewc_error(msg: impl Into<String>) -> Self {
        Self::EwcError(msg.into())
    }

    /// Create a reasoning bank error
    pub fn reasoning_bank_error(msg: impl Into<String>) -> Self {
        Self::ReasoningBankError(msg.into())
    }
}

/// Check for NaN or infinite values
pub fn check_finite(value: f32, context: &str) -> Result<()> {
    if value.is_nan() {
        Err(FxnnError::numerical(format!("{context}: NaN detected")))
    } else if value.is_infinite() {
        Err(FxnnError::numerical(format!("{context}: Infinite value detected")))
    } else {
        Ok(())
    }
}

/// Check for NaN or infinite values in an array
pub fn check_finite_array(values: &[f32], context: &str) -> Result<()> {
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            return Err(FxnnError::numerical(format!("{context}[{i}]: NaN detected")));
        }
        if v.is_infinite() {
            return Err(FxnnError::numerical(format!(
                "{context}[{i}]: Infinite value detected"
            )));
        }
    }
    Ok(())
}
