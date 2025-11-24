//! Mesh network topology

/// Mesh network
pub struct Mesh {
    /// Width of mesh
    width: usize,
    /// Height of mesh
    height: usize,
}

impl Mesh {
    /// Create a new mesh network
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }
}
