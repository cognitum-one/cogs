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
