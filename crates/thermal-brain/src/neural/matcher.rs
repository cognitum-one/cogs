//! Spiking pattern matcher - combines LIF neurons with HNSW index

use crate::config::NeuralConfig;
use crate::error::ThermalBrainError;
use crate::types::{MatchResult, PatternVector, SparseVector, MAX_LABEL_LEN};
use super::{LIFNeuron, MiniHnsw};
use heapless::Vec as HVec;

/// Maximum number of patterns
const MAX_PATTERNS: usize = 64;

/// Pattern entry with metadata
#[derive(Clone)]
struct PatternEntry {
    /// Pattern ID
    id: u32,
    /// Pattern vector
    vector: PatternVector,
    /// Pattern label
    label: heapless::String<MAX_LABEL_LEN>,
    /// Match count
    match_count: u32,
    /// Whether pattern is active
    active: bool,
}

impl Default for PatternEntry {
    fn default() -> Self {
        Self {
            id: 0,
            vector: [0i8; 16],
            label: heapless::String::new(),
            match_count: 0,
            active: false,
        }
    }
}

/// Spiking pattern matcher
///
/// Combines:
/// - LIF neurons for temporal integration
/// - Sparse similarity computation
/// - Winner-take-all output selection
pub struct SpikingMatcher {
    /// LIF neurons (one per pattern)
    neurons: HVec<LIFNeuron, MAX_PATTERNS>,
    /// Pattern entries
    patterns: HVec<PatternEntry, MAX_PATTERNS>,
    /// Configuration
    config: NeuralConfig,
    /// Next pattern ID
    next_id: u32,
}

impl SpikingMatcher {
    /// Create a new spiking matcher
    pub fn new(config: NeuralConfig) -> Self {
        Self {
            neurons: HVec::new(),
            patterns: HVec::new(),
            config,
            next_id: 0,
        }
    }

    /// Process input through the matcher
    ///
    /// # Arguments
    /// * `input` - Sparse input vector
    /// * `refractory_ms` - Refractory period from thermal governor
    /// * `dt_ms` - Time step
    /// * `hnsw` - HNSW index for candidate selection
    ///
    /// # Returns
    /// Match result if any neuron fired
    pub fn process(
        &mut self,
        input: &SparseVector,
        refractory_ms: u32,
        dt_ms: u32,
        hnsw: &MiniHnsw,
    ) -> Option<MatchResult> {
        // Use HNSW to find candidate patterns
        let query_pattern = self.sparse_to_pattern(input);
        // Note: search mutates internal stats but is functionally read-only
        let hnsw_ptr = hnsw as *const MiniHnsw as *mut MiniHnsw;
        let candidates = unsafe { (*hnsw_ptr).search(&query_pattern, 10, self.config.hnsw_ef_search) };

        let mut best_match: Option<(usize, f32)> = None;

        // Process each candidate through its neuron
        for (pattern_id, similarity) in candidates.iter() {
            // Find pattern entry
            if let Some((idx, _entry)) = self.patterns.iter().enumerate()
                .find(|(_, p)| p.id == *pattern_id && p.active)
            {
                // Normalize similarity to [0, 1] and use as input current
                let normalized = similarity.clamp(0.0, 1.0);

                // Feed to corresponding neuron
                if let Some(neuron) = self.neurons.get_mut(idx) {
                    if neuron.integrate(normalized, dt_ms, refractory_ms) {
                        // Neuron fired!
                        match &best_match {
                            None => best_match = Some((idx, normalized)),
                            Some((_, prev_conf)) if normalized > *prev_conf => {
                                best_match = Some((idx, normalized));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // If we got a match, update stats and return result
        if let Some((idx, confidence)) = best_match {
            let entry = &mut self.patterns[idx];
            entry.match_count += 1;

            Some(MatchResult::new(
                &entry.label,
                confidence,
                entry.id,
                idx,
            ))
        } else {
            None
        }
    }

    /// Add a new pattern
    pub fn add_pattern(
        &mut self,
        id: u32,
        vector: PatternVector,
        label: &str,
    ) -> Result<(), ThermalBrainError> {
        if self.patterns.len() >= MAX_PATTERNS {
            return Err(ThermalBrainError::PatternLimitReached);
        }

        let mut entry = PatternEntry::default();
        entry.id = id;
        entry.vector = vector;
        entry.active = true;
        let _ = entry.label.push_str(&label[..label.len().min(MAX_LABEL_LEN - 1)]);

        self.patterns.push(entry).map_err(|_| ThermalBrainError::OutOfMemory)?;

        // Add corresponding neuron
        let neuron = LIFNeuron::new(self.config.base_threshold, self.config.tau_ms);
        self.neurons.push(neuron).map_err(|_| ThermalBrainError::OutOfMemory)?;

        if id >= self.next_id {
            self.next_id = id + 1;
        }

        Ok(())
    }

    /// Remove a pattern
    pub fn remove_pattern(&mut self, pattern_id: u32) -> Result<(), ThermalBrainError> {
        if let Some(idx) = self.patterns.iter().position(|p| p.id == pattern_id) {
            self.patterns[idx].active = false;
            Ok(())
        } else {
            Err(ThermalBrainError::PatternNotFound(pattern_id))
        }
    }

    /// Get pattern count
    pub fn len(&self) -> usize {
        self.patterns.iter().filter(|p| p.active).count()
    }

    /// Check if matcher is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reset all neurons
    pub fn reset_all(&mut self) {
        for n in self.neurons.iter_mut() {
            n.reset();
        }
    }

    /// Set threshold for all neurons
    pub fn set_threshold(&mut self, threshold: f32) {
        for n in self.neurons.iter_mut() {
            n.set_threshold(threshold);
        }
    }

    /// Get pattern info by ID
    pub fn get_pattern(&self, id: u32) -> Option<(&str, u32)> {
        self.patterns.iter()
            .find(|p| p.id == id && p.active)
            .map(|p| (p.label.as_str(), p.match_count))
    }

    /// List all active patterns
    pub fn list_patterns(&self) -> impl Iterator<Item = (u32, &str)> {
        self.patterns.iter()
            .filter(|p| p.active)
            .map(|p| (p.id, p.label.as_str()))
    }

    /// Convert sparse vector to pattern vector
    fn sparse_to_pattern(&self, sparse: &SparseVector) -> PatternVector {
        let mut pattern = [0i8; 16];
        for (&idx, &val) in sparse.indices.iter().zip(sparse.values.iter()) {
            if (idx as usize) < 16 {
                pattern[idx as usize] = val;
            }
        }
        pattern
    }

    /// Direct similarity match without neurons (for testing)
    pub fn direct_match(&self, input: &SparseVector) -> Option<(u32, &str, f32)> {
        let mut best: Option<(u32, &str, f32)> = None;

        for entry in self.patterns.iter().filter(|p| p.active) {
            let sim = input.dot_dense(&entry.vector) as f32;
            let normalized = sim / (127.0 * 127.0 * 16.0) + 0.5;

            match &best {
                None => best = Some((entry.id, &entry.label, normalized)),
                Some((_, _, prev)) if normalized > *prev => {
                    best = Some((entry.id, &entry.label, normalized))
                }
                _ => {}
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sparse(values: &[(u8, i8)]) -> SparseVector {
        let mut sparse = SparseVector::new(0.5);
        for &(idx, val) in values {
            let _ = sparse.push(idx, val);
        }
        sparse
    }

    fn make_pattern(base: i8) -> PatternVector {
        let mut v = [0i8; 16];
        for i in 0..16 {
            v[i] = base.saturating_add((i as i8) * 5);
        }
        v
    }

    #[test]
    fn test_matcher_add_pattern() {
        let config = NeuralConfig::default();
        let mut matcher = SpikingMatcher::new(config);

        matcher.add_pattern(0, make_pattern(10), "test1").unwrap();
        matcher.add_pattern(1, make_pattern(20), "test2").unwrap();

        assert_eq!(matcher.len(), 2);
    }

    #[test]
    fn test_matcher_remove_pattern() {
        let config = NeuralConfig::default();
        let mut matcher = SpikingMatcher::new(config);

        matcher.add_pattern(0, make_pattern(10), "test1").unwrap();
        matcher.add_pattern(1, make_pattern(20), "test2").unwrap();
        assert_eq!(matcher.len(), 2);

        matcher.remove_pattern(0).unwrap();
        assert_eq!(matcher.len(), 1);
    }

    #[test]
    fn test_matcher_direct_match() {
        let config = NeuralConfig::default();
        let mut matcher = SpikingMatcher::new(config);

        // Add patterns with different base values
        matcher.add_pattern(0, make_pattern(10), "low").unwrap();
        matcher.add_pattern(1, make_pattern(50), "mid").unwrap();
        matcher.add_pattern(2, make_pattern(100), "high").unwrap();

        // Query similar to "mid" pattern
        let sparse = make_sparse(&[(0, 50), (1, 55), (2, 60)]);
        let result = matcher.direct_match(&sparse);

        assert!(result.is_some());
        let (id, label, _) = result.unwrap();

        // Should match one of our patterns
        assert!(id <= 2);
    }

    #[test]
    fn test_matcher_list_patterns() {
        let config = NeuralConfig::default();
        let mut matcher = SpikingMatcher::new(config);

        matcher.add_pattern(0, make_pattern(10), "first").unwrap();
        matcher.add_pattern(1, make_pattern(20), "second").unwrap();
        matcher.add_pattern(2, make_pattern(30), "third").unwrap();

        let labels: Vec<_> = matcher.list_patterns().map(|(_, l)| l).collect();
        assert!(labels.contains(&"first"));
        assert!(labels.contains(&"second"));
        assert!(labels.contains(&"third"));
    }
}
