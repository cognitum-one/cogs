//! Network Pruning
//!
//! Remove low-weight connections to reduce memory and computation.
//! Implements magnitude-based, gradient-based, and structured pruning.
//!
//! Benefits:
//! - Reduced memory footprint
//! - Faster inference
//! - Lower power consumption
//! - Maintained accuracy with proper thresholds

use heapless::Vec as HVec;

/// Maximum connections to track
const MAX_CONNECTIONS: usize = 512;

/// Maximum neurons
const MAX_NEURONS: usize = 64;

/// Pruning strategy
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PruningStrategy {
    /// Remove connections below absolute threshold
    Magnitude,
    /// Remove bottom percentage of connections
    Percentile,
    /// Remove entire neurons with low activity
    Structured,
    /// Gradual pruning over time
    Gradual,
    /// Activity-based (remove unused connections)
    ActivityBased,
}

/// Connection weight with metadata
#[derive(Clone, Copy, Debug)]
pub struct Connection {
    /// Source neuron ID
    pub source: u16,
    /// Target neuron ID
    pub target: u16,
    /// Weight (Q8 fixed-point)
    pub weight: i8,
    /// Usage count (for activity-based pruning)
    pub usage: u16,
    /// Is connection active (not pruned)
    pub active: bool,
}

impl Connection {
    /// Create a new connection
    pub fn new(source: u16, target: u16, weight: i8) -> Self {
        Self {
            source,
            target,
            weight,
            usage: 0,
            active: true,
        }
    }

    /// Get absolute weight magnitude
    pub fn magnitude(&self) -> u8 {
        self.weight.unsigned_abs()
    }
}

/// Pruning configuration
#[derive(Clone, Copy, Debug)]
pub struct PruningConfig {
    /// Pruning strategy
    pub strategy: PruningStrategy,
    /// Magnitude threshold (for Magnitude strategy)
    pub magnitude_threshold: u8,
    /// Percentile to prune (0.0 to 1.0, for Percentile strategy)
    pub prune_percentile: f32,
    /// Activity threshold (for ActivityBased strategy)
    pub activity_threshold: u16,
    /// Gradual pruning rate (connections per update)
    pub gradual_rate: u16,
    /// Minimum connections to keep
    pub min_connections: usize,
    /// Allow pruning of excitatory connections
    pub prune_excitatory: bool,
    /// Allow pruning of inhibitory connections
    pub prune_inhibitory: bool,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            strategy: PruningStrategy::Magnitude,
            magnitude_threshold: 10,
            prune_percentile: 0.2,
            activity_threshold: 5,
            gradual_rate: 10,
            min_connections: 32,
            prune_excitatory: true,
            prune_inhibitory: true,
        }
    }
}

/// Pruning statistics
#[derive(Clone, Copy, Debug, Default)]
pub struct PruningStats {
    /// Total connections
    pub total_connections: u32,
    /// Active connections
    pub active_connections: u32,
    /// Pruned connections
    pub pruned_connections: u32,
    /// Sparsity achieved (0.0 to 1.0)
    pub sparsity: f32,
    /// Memory savings estimate (bytes)
    pub memory_saved: u32,
}

/// Network pruning controller
///
/// Manages connection pruning for neural network optimization.
pub struct NetworkPruner {
    config: PruningConfig,
    /// All connections
    connections: HVec<Connection, MAX_CONNECTIONS>,
    /// Neuron activity counts
    neuron_activity: HVec<u16, MAX_NEURONS>,
    /// Pruning statistics
    stats: PruningStats,
    /// Gradual pruning cursor
    gradual_cursor: usize,
}

impl NetworkPruner {
    /// Create a new network pruner
    pub fn new(config: PruningConfig, num_neurons: usize) -> Self {
        let mut neuron_activity = HVec::new();
        for _ in 0..num_neurons.min(MAX_NEURONS) {
            let _ = neuron_activity.push(0);
        }

        Self {
            config,
            connections: HVec::new(),
            neuron_activity,
            stats: PruningStats::default(),
            gradual_cursor: 0,
        }
    }

    /// Add a connection
    pub fn add_connection(&mut self, source: u16, target: u16, weight: i8) -> bool {
        if self.connections.is_full() {
            return false;
        }

        let conn = Connection::new(source, target, weight);
        self.connections.push(conn).is_ok()
    }

    /// Record usage of a connection
    pub fn record_usage(&mut self, source: u16, target: u16) {
        for conn in self.connections.iter_mut() {
            if conn.source == source && conn.target == target && conn.active {
                conn.usage = conn.usage.saturating_add(1);
                break;
            }
        }

        // Record neuron activity
        if let Some(activity) = self.neuron_activity.get_mut(source as usize) {
            *activity = activity.saturating_add(1);
        }
        if let Some(activity) = self.neuron_activity.get_mut(target as usize) {
            *activity = activity.saturating_add(1);
        }
    }

    /// Run pruning based on configured strategy
    pub fn prune(&mut self) -> PruningStats {
        match self.config.strategy {
            PruningStrategy::Magnitude => self.prune_by_magnitude(),
            PruningStrategy::Percentile => self.prune_by_percentile(),
            PruningStrategy::Structured => self.prune_structured(),
            PruningStrategy::Gradual => self.prune_gradual(),
            PruningStrategy::ActivityBased => self.prune_by_activity(),
        }

        self.update_stats();
        self.stats
    }

    /// Prune by absolute magnitude threshold
    fn prune_by_magnitude(&mut self) {
        let threshold = self.config.magnitude_threshold;
        let min_active = self.config.min_connections;

        let mut active_count = self.connections.iter().filter(|c| c.active).count();

        for conn in self.connections.iter_mut() {
            if !conn.active {
                continue;
            }
            if active_count <= min_active {
                break;
            }

            let can_prune = (conn.weight > 0 && self.config.prune_excitatory)
                || (conn.weight < 0 && self.config.prune_inhibitory);

            if can_prune && conn.magnitude() < threshold {
                conn.active = false;
                active_count -= 1;
            }
        }
    }

    /// Prune bottom percentile of connections
    fn prune_by_percentile(&mut self) {
        let active_conns: HVec<usize, MAX_CONNECTIONS> = self.connections
            .iter()
            .enumerate()
            .filter(|(_, c)| c.active)
            .map(|(i, _)| i)
            .collect();

        if active_conns.is_empty() {
            return;
        }

        // Sort indices by magnitude
        let mut sorted_indices: HVec<usize, MAX_CONNECTIONS> = active_conns.clone();

        // Simple bubble sort (heapless doesn't have sort)
        for i in 0..sorted_indices.len() {
            for j in i + 1..sorted_indices.len() {
                let mag_i = self.connections[sorted_indices[i]].magnitude();
                let mag_j = self.connections[sorted_indices[j]].magnitude();
                if mag_i > mag_j {
                    sorted_indices.swap(i, j);
                }
            }
        }

        // Prune bottom percentile
        let prune_count = ((sorted_indices.len() as f32 * self.config.prune_percentile) as usize)
            .min(sorted_indices.len().saturating_sub(self.config.min_connections));

        for &idx in sorted_indices.iter().take(prune_count) {
            let conn = &self.connections[idx];
            let can_prune = (conn.weight > 0 && self.config.prune_excitatory)
                || (conn.weight < 0 && self.config.prune_inhibitory);

            if can_prune {
                self.connections[idx].active = false;
            }
        }
    }

    /// Structured pruning - remove entire neurons
    fn prune_structured(&mut self) {
        // Find neurons with low activity
        let mut low_activity_neurons: HVec<u16, MAX_NEURONS> = HVec::new();

        for (i, &activity) in self.neuron_activity.iter().enumerate() {
            if activity < self.config.activity_threshold {
                let _ = low_activity_neurons.push(i as u16);
            }
        }

        // Remove all connections to/from low-activity neurons
        for conn in self.connections.iter_mut() {
            if conn.active {
                if low_activity_neurons.contains(&conn.source)
                    || low_activity_neurons.contains(&conn.target)
                {
                    conn.active = false;
                }
            }
        }
    }

    /// Gradual pruning - remove a few connections per call
    fn prune_gradual(&mut self) {
        let mut pruned = 0u16;
        let active_count = self.connections.iter().filter(|c| c.active).count();

        if active_count <= self.config.min_connections {
            return;
        }

        // Find lowest magnitude active connections
        let mut candidates: HVec<(usize, u8), 64> = HVec::new();

        for (i, conn) in self.connections.iter().enumerate() {
            if conn.active {
                let can_prune = (conn.weight > 0 && self.config.prune_excitatory)
                    || (conn.weight < 0 && self.config.prune_inhibitory);
                if can_prune {
                    let _ = candidates.push((i, conn.magnitude()));
                }
            }
        }

        // Sort by magnitude
        for i in 0..candidates.len() {
            for j in i + 1..candidates.len() {
                if candidates[i].1 > candidates[j].1 {
                    candidates.swap(i, j);
                }
            }
        }

        // Prune up to gradual_rate connections
        for &(idx, _) in candidates.iter() {
            if pruned >= self.config.gradual_rate {
                break;
            }
            if self.connections.iter().filter(|c| c.active).count() <= self.config.min_connections {
                break;
            }

            self.connections[idx].active = false;
            pruned += 1;
        }
    }

    /// Activity-based pruning - remove unused connections
    fn prune_by_activity(&mut self) {
        let threshold = self.config.activity_threshold;
        let min_active = self.config.min_connections;

        let mut active_count = self.connections.iter().filter(|c| c.active).count();

        for conn in self.connections.iter_mut() {
            if !conn.active {
                continue;
            }
            if active_count <= min_active {
                break;
            }

            if conn.usage < threshold {
                conn.active = false;
                active_count -= 1;
            }
        }
    }

    /// Update statistics
    fn update_stats(&mut self) {
        let total = self.connections.len() as u32;
        let active = self.connections.iter().filter(|c| c.active).count() as u32;
        let pruned = total - active;

        self.stats = PruningStats {
            total_connections: total,
            active_connections: active,
            pruned_connections: pruned,
            sparsity: if total > 0 { pruned as f32 / total as f32 } else { 0.0 },
            memory_saved: pruned * 4, // Approximate bytes per connection
        };
    }

    /// Get current statistics
    pub fn stats(&self) -> PruningStats {
        self.stats
    }

    /// Get number of active connections
    pub fn active_connection_count(&self) -> usize {
        self.connections.iter().filter(|c| c.active).count()
    }

    /// Get active connections
    pub fn active_connections(&self) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(|c| c.active)
    }

    /// Restore a pruned connection
    pub fn restore_connection(&mut self, source: u16, target: u16) -> bool {
        for conn in self.connections.iter_mut() {
            if conn.source == source && conn.target == target && !conn.active {
                conn.active = true;
                return true;
            }
        }
        false
    }

    /// Restore all pruned connections
    pub fn restore_all(&mut self) {
        for conn in self.connections.iter_mut() {
            conn.active = true;
        }
        self.update_stats();
    }

    /// Reset usage counters
    pub fn reset_usage(&mut self) {
        for conn in self.connections.iter_mut() {
            conn.usage = 0;
        }
        for activity in self.neuron_activity.iter_mut() {
            *activity = 0;
        }
    }

    /// Get connection weight
    pub fn get_weight(&self, source: u16, target: u16) -> Option<i8> {
        self.connections
            .iter()
            .find(|c| c.source == source && c.target == target && c.active)
            .map(|c| c.weight)
    }

    /// Update connection weight
    pub fn update_weight(&mut self, source: u16, target: u16, weight: i8) -> bool {
        for conn in self.connections.iter_mut() {
            if conn.source == source && conn.target == target {
                conn.weight = weight;
                return true;
            }
        }
        false
    }

    /// Get sparsity level
    pub fn sparsity(&self) -> f32 {
        self.stats.sparsity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_pruner() {
        let config = PruningConfig::default();
        let pruner = NetworkPruner::new(config, 8);

        assert_eq!(pruner.active_connection_count(), 0);
    }

    #[test]
    fn test_add_connection() {
        let config = PruningConfig::default();
        let mut pruner = NetworkPruner::new(config, 8);

        assert!(pruner.add_connection(0, 1, 50));
        assert!(pruner.add_connection(1, 2, 30));
        assert!(pruner.add_connection(2, 3, 10));

        assert_eq!(pruner.active_connection_count(), 3);
    }

    #[test]
    fn test_magnitude_pruning() {
        let config = PruningConfig {
            strategy: PruningStrategy::Magnitude,
            magnitude_threshold: 20,
            min_connections: 0,
            ..Default::default()
        };
        let mut pruner = NetworkPruner::new(config, 8);

        pruner.add_connection(0, 1, 50);  // Keep (50 > 20)
        pruner.add_connection(1, 2, 10);  // Prune (10 < 20)
        pruner.add_connection(2, 3, 5);   // Prune (5 < 20)

        let stats = pruner.prune();

        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.pruned_connections, 2);
    }

    #[test]
    fn test_percentile_pruning() {
        let config = PruningConfig {
            strategy: PruningStrategy::Percentile,
            prune_percentile: 0.5, // Prune bottom 50%
            min_connections: 0,
            ..Default::default()
        };
        let mut pruner = NetworkPruner::new(config, 8);

        for i in 0..10 {
            pruner.add_connection(i, i + 1, (i * 10) as i8);
        }

        let stats = pruner.prune();

        // Should prune about half
        assert!(stats.pruned_connections >= 4);
        assert!(stats.active_connections <= 6);
    }

    #[test]
    fn test_activity_pruning() {
        let config = PruningConfig {
            strategy: PruningStrategy::ActivityBased,
            activity_threshold: 5,
            min_connections: 0,
            ..Default::default()
        };
        let mut pruner = NetworkPruner::new(config, 8);

        pruner.add_connection(0, 1, 50);
        pruner.add_connection(1, 2, 50);

        // Record usage for first connection
        for _ in 0..10 {
            pruner.record_usage(0, 1);
        }

        let stats = pruner.prune();

        // First connection should survive, second should be pruned
        assert_eq!(stats.active_connections, 1);
        assert!(pruner.get_weight(0, 1).is_some());
        assert!(pruner.get_weight(1, 2).is_none());
    }

    #[test]
    fn test_min_connections() {
        let config = PruningConfig {
            strategy: PruningStrategy::Magnitude,
            magnitude_threshold: 100, // High threshold
            min_connections: 5,
            ..Default::default()
        };
        let mut pruner = NetworkPruner::new(config, 8);

        for i in 0..10 {
            pruner.add_connection(i, i + 1, 10); // All below threshold
        }

        let stats = pruner.prune();

        // Should keep at least min_connections
        assert!(stats.active_connections >= 5);
    }

    #[test]
    fn test_restore_connection() {
        let config = PruningConfig {
            strategy: PruningStrategy::Magnitude,
            magnitude_threshold: 50,
            min_connections: 0,
            ..Default::default()
        };
        let mut pruner = NetworkPruner::new(config, 8);

        pruner.add_connection(0, 1, 10); // Will be pruned
        pruner.prune();

        assert_eq!(pruner.active_connection_count(), 0);

        // Restore
        assert!(pruner.restore_connection(0, 1));
        assert_eq!(pruner.active_connection_count(), 1);
    }
}
