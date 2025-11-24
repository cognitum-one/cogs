//! Router implementation

/// Network router
pub struct Router {
    /// Router ID
    id: usize,
}

impl Router {
    /// Create a new router
    pub fn new(id: usize) -> Self {
        Self { id }
    }
}
