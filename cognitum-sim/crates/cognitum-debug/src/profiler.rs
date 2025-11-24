//! Performance profiler

use std::collections::HashMap;

/// Profiler for tracking performance metrics
pub struct Profiler {
    /// Performance counters
    counters: HashMap<String, u64>,
}

impl Profiler {
    /// Create a new profiler
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    /// Increment a counter
    pub fn increment(&mut self, name: &str) {
        *self.counters.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Get counter value
    pub fn get(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}
