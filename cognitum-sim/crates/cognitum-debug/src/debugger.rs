//! Interactive debugger

/// Debugger implementation
pub struct Debugger;

impl Debugger {
    /// Create a new debugger
    pub fn new() -> Self {
        Self
    }
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}
