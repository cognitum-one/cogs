//! Comprehensive tests for NEWS neuromorphic coprocessor

use cognitum_coprocessor::news::*;

/// Test basic neuron creation and parameters
#[test]
fn test_neuron_creation_and_params() {
    let neuron = LeakyIntegrateFireNeuron::new(5120, 240);

    assert_eq!(neuron.potential(), 0, "Initial potential should be 0");
    assert_eq!(neuron.threshold(), 5120, "Threshold should match input");
    assert!(!neuron.is_refractory(), "Should not be in refractory period");
}

/// Test custom neuron parameters
#[test]
fn test_neuron_custom_params() {
    let neuron = LeakyIntegrateFireNeuron::with_params(
        10000,  // threshold
        200,    // leak_rate
        -1000,  // resting_potential
        32,     // learning_rate
    );

    assert_eq!(neuron.potential(), -1000, "Should match resting potential");
    assert_eq!(neuron.threshold(), 10000);
}

/// Test synaptic weight setting and retrieval
#[test]
fn test_weight_setting() {
    let mut neuron = LeakyIntegrateFireNeuron::new(5120, 240);

    neuron.set_weight(0, 1000);
    neuron.set_weight(5, -500);
    neuron.set_weight(255, 750);

    assert_eq!(neuron.get_weight(0), 1000);
    assert_eq!(neuron.get_weight(5), -500);
    assert_eq!(neuron.get_weight(255), 750);
    assert_eq!(neuron.get_weight(100), 0, "Unset weight should be 0");
}

/// Test membrane potential leak over time
#[test]
fn test_membrane_leak() {
    let mut neuron = LeakyIntegrateFireNeuron::new(10000, 240); // ~6% leak per step

    // Inject current
    neuron.set_weight(0, 5000);
    neuron.receive_spike(0, 0, None);

    let initial = neuron.potential();
    assert!(initial > 0, "Potential should increase after spike");

    // Run several steps to observe leak
    for i in 1..=10 {
        neuron.update(i);
    }

    let final_potential = neuron.potential();
    assert!(
        final_potential < initial,
        "Potential should decay: {} -> {}",
        initial,
        final_potential
    );
    assert!(
        final_potential > 0,
        "Should not decay below resting potential quickly"
    );
}

/// Test spike generation when threshold is reached
#[test]
fn test_spike_generation() {
    let mut neuron = LeakyIntegrateFireNeuron::new(1000, 255); // No leak for simplicity

    // Set strong weight and deliver spike
    neuron.set_weight(0, 1500);
    neuron.receive_spike(0, 0, None);

    // Neuron should fire
    let fired = neuron.update(1);
    assert!(fired, "Neuron should spike when above threshold");

    // After spiking, potential should reset
    assert_eq!(neuron.potential(), 0, "Potential should reset after spike");
    assert!(neuron.is_refractory(), "Should enter refractory period");
}

/// Test refractory period behavior
#[test]
fn test_refractory_period() {
    let mut neuron = LeakyIntegrateFireNeuron::new(1000, 255);

    // Trigger spike
    neuron.set_weight(0, 2000);
    neuron.receive_spike(0, 0, None);
    assert!(neuron.update(1));
    assert!(neuron.is_refractory());

    // During refractory, should not integrate spikes
    neuron.receive_spike(0, 2, None);
    assert_eq!(
        neuron.potential(),
        0,
        "Should not integrate during refractory"
    );

    // After refractory period (5 cycles), should be responsive again
    for i in 3..=7 {
        neuron.update(i);
    }
    assert!(
        !neuron.is_refractory(),
        "Should exit refractory after 5 cycles"
    );

    // Now should integrate spikes again
    neuron.receive_spike(0, 8, None);
    assert!(neuron.potential() > 0, "Should integrate after refractory");
}

/// Test inhibitory connections (negative weights)
#[test]
fn test_inhibitory_connections() {
    let mut neuron = LeakyIntegrateFireNeuron::new(5000, 255);

    // Add excitatory input
    neuron.set_weight(0, 3000);
    neuron.receive_spike(0, 0, None);
    let after_excitation = neuron.potential();
    assert!(after_excitation > 0);

    // Add inhibitory input
    neuron.set_weight(1, -2000);
    neuron.receive_spike(1, 1, None);
    let after_inhibition = neuron.potential();

    assert!(
        after_inhibition < after_excitation,
        "Inhibitory spike should reduce potential"
    );
}

/// Test coprocessor creation and basic properties
#[test]
fn test_coprocessor_creation() {
    let news = NewsCoprocessor::new();

    assert_eq!(news.neuron_count(), MAX_NEURONS);
    assert_eq!(news.time(), 0);
    assert_eq!(news.total_spikes(), 0);
    assert_eq!(news.queue_length(), 0);
}

/// Test neuron access through coprocessor
#[test]
fn test_neuron_access() {
    let mut news = NewsCoprocessor::new();

    // Test read access
    let neuron = news.neuron(0);
    assert!(neuron.is_some());

    let neuron = news.neuron(255);
    assert!(neuron.is_some());

    // Test write access
    if let Some(neuron) = news.neuron_mut(10) {
        neuron.set_weight(5, 1000);
    }

    assert_eq!(news.neuron(10).unwrap().get_weight(5), 1000);
}

/// Test synaptic connection setup
#[test]
fn test_connection_setup() {
    let mut news = NewsCoprocessor::new();

    // Create connections
    news.connect(0, 1, 1000);
    news.connect(1, 2, 1500);
    news.connect(2, 0, -500);

    assert_eq!(news.neuron(1).unwrap().get_weight(0), 1000);
    assert_eq!(news.neuron(2).unwrap().get_weight(1), 1500);
    assert_eq!(news.neuron(0).unwrap().get_weight(2), -500);
}

/// Test external spike injection
#[test]
fn test_spike_injection() {
    let mut news = NewsCoprocessor::new();

    // Inject spike that should cause firing
    news.inject_spike(0, 6000);

    // Step simulation
    let spikes = news.step();

    // Neuron 0 should fire
    assert!(!spikes.is_empty(), "Should generate output spikes");
    assert_eq!(spikes[0].source, 0);
}

/// Test spike propagation through network
#[test]
fn test_spike_propagation() {
    let mut news = NewsCoprocessor::new();

    // Create chain: 0 -> 1 -> 2
    news.connect(0, 1, 3000);
    news.connect(1, 2, 3000);

    // Inject spike to neuron 0
    news.inject_spike(0, 6000);

    // Step 1: Neuron 0 fires
    let spikes_t0 = news.step();
    assert_eq!(spikes_t0.len(), 1);
    assert_eq!(spikes_t0[0].source, 0);

    // Step 2: Neuron 1 should fire
    let spikes_t1 = news.step();
    // Note: Might need multiple steps for propagation
    news.step();

    assert!(
        news.total_spikes() >= 2,
        "At least 2 spikes should propagate"
    );
}

/// Test simple oscillatory network (two mutually connected neurons)
#[test]
fn test_oscillatory_network() {
    let mut news = NewsCoprocessor::new();

    // Create reciprocal excitatory connections
    news.connect(0, 1, 2500);
    news.connect(1, 0, 2500);

    // Kick-start oscillation
    news.inject_spike(0, 6000);

    // Run for several cycles
    let mut spike_pattern = Vec::new();
    for _ in 0..20 {
        let spikes = news.step();
        spike_pattern.push(spikes.len());
    }

    // Should see periodic activity
    let total_activity: usize = spike_pattern.iter().sum();
    assert!(
        total_activity > 10,
        "Should sustain oscillatory activity: got {} spikes",
        total_activity
    );
}

/// Test synchronization in coupled neurons
#[test]
fn test_synchronization() {
    let mut news = NewsCoprocessor::new();

    // Create all-to-all connections for neurons 0-3
    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                news.connect(i, j, 1000);
            }
        }
    }

    // Inject spikes to all 4 neurons with slight timing differences
    news.inject_spike(0, 5000);
    news.inject_spike(1, 4500);
    news.inject_spike(2, 4800);
    news.inject_spike(3, 4600);

    // Run simulation
    for _ in 0..50 {
        news.step();
    }

    // Should generate significant activity
    assert!(
        news.total_spikes() > 20,
        "Coupled network should generate sustained activity"
    );
}

/// Test winner-take-all network
#[test]
fn test_winner_take_all() {
    let mut news = NewsCoprocessor::new();

    // Create lateral inhibition: each neuron inhibits others
    for i in 0..5 {
        for j in 0..5 {
            if i != j {
                news.connect(i, j, -2000); // Strong inhibition
            }
        }
    }

    // Give neuron 2 strongest input
    news.inject_spike(0, 3000);
    news.inject_spike(1, 3500);
    news.inject_spike(2, 5000); // Strongest
    news.inject_spike(3, 3200);
    news.inject_spike(4, 3100);

    // Step simulation
    for _ in 0..5 {
        let spikes = news.step();
        if !spikes.is_empty() {
            // First spike should be from neuron 2
            assert_eq!(spikes[0].source, 2, "Strongest input should win");
            break;
        }
    }
}

/// Test STDP learning (weight changes over time)
#[test]
fn test_stdp_learning() {
    let mut news = NewsCoprocessor::new();

    // Set initial small weight
    news.connect(0, 1, 500);

    let initial_weight = news.neuron(1).unwrap().get_weight(0);

    // Create causal spike pattern (pre before post)
    // This should potentiate the synapse
    for _ in 0..10 {
        news.inject_spike(0, 4000); // Pre-synaptic
        news.step();
        news.inject_spike(1, 2000); // Post-synaptic (ensure it fires)
        news.step();
        news.step(); // Gap
    }

    let final_weight = news.neuron(1).unwrap().get_weight(0);

    // Weight should have changed due to STDP
    // Note: Exact change depends on learning rate and timing
    assert_ne!(
        initial_weight, final_weight,
        "STDP should modify synaptic weights"
    );
}

/// Test long-running simulation stability
#[test]
fn test_long_simulation() {
    let mut news = NewsCoprocessor::new();

    // Create sparse random network
    for i in 0..20 {
        for j in 0..20 {
            if i != j && (i * 7 + j * 11) % 3 == 0 {
                news.connect(i, j, 1500);
            }
        }
    }

    // Inject initial spikes
    for i in 0..5 {
        news.inject_spike(i, 5000);
    }

    // Run for 1000 cycles
    let spike_count = news.run(1000);

    assert_eq!(news.time(), 1000, "Should run for full duration");
    assert!(spike_count > 0, "Should generate some activity");

    // Check that firing rate is reasonable (not exploding or dying)
    let firing_rate = news.average_firing_rate();
    assert!(
        firing_rate > 0.0 && firing_rate < 1.0,
        "Firing rate should be reasonable: {}",
        firing_rate
    );
}

/// Test reset functionality
#[test]
fn test_reset() {
    let mut news = NewsCoprocessor::new();

    // Create activity
    news.connect(0, 1, 2000);
    news.inject_spike(0, 6000);
    news.run(10);

    assert!(news.time() > 0);
    assert!(news.total_spikes() > 0);

    // Reset
    news.reset();

    assert_eq!(news.time(), 0, "Time should reset");
    assert_eq!(news.total_spikes(), 0, "Spike count should reset");
    assert_eq!(news.queue_length(), 0, "Queue should be empty");

    // All neurons should be at resting state
    for i in 0..MAX_NEURONS {
        let neuron = news.neuron(i as u8).unwrap();
        assert!(!neuron.is_refractory(), "Neuron {} should not be refractory", i);
    }
}

/// Test average firing rate calculation
#[test]
fn test_firing_rate_calculation() {
    let mut news = NewsCoprocessor::new();

    // Initially should be zero
    assert_eq!(news.average_firing_rate(), 0.0);

    // Create some activity
    for i in 0..10 {
        news.inject_spike(i, 6000);
    }
    news.run(100);

    let rate = news.average_firing_rate();
    assert!(rate > 0.0, "Should have non-zero firing rate");
    assert!(rate < 1.0, "Firing rate should be less than 1.0");
}

/// Test sparse network (realistic connectivity)
#[test]
fn test_sparse_network() {
    let mut news = NewsCoprocessor::new();

    // Create sparse connectivity (~10% connection probability)
    let mut connection_count = 0;
    for i in 0..MAX_NEURONS {
        for j in 0..MAX_NEURONS {
            if i != j && (i * 13 + j * 17) % 10 == 0 {
                news.connect(i as u8, j as u8, 1200);
                connection_count += 1;
            }
        }
    }

    assert!(
        connection_count > 1000,
        "Should create sparse network: {} connections",
        connection_count
    );

    // Inject random inputs
    for i in (0..MAX_NEURONS).step_by(10) {
        news.inject_spike(i as u8, 5000);
    }

    // Run simulation
    let spikes = news.run(100);
    assert!(spikes > 20, "Sparse network should propagate activity");
}

/// Test maximum neuron capacity
#[test]
fn test_max_neurons() {
    let news = NewsCoprocessor::new();
    assert_eq!(news.neuron_count(), MAX_NEURONS);

    // All 256 neurons should be accessible
    for i in 0..MAX_NEURONS {
        assert!(
            news.neuron(i as u8).is_some(),
            "Neuron {} should exist",
            i
        );
    }
}

/// Benchmark-style test: measure performance
#[test]
fn test_performance_baseline() {
    let mut news = NewsCoprocessor::new();

    // Create moderately connected network
    for i in 0..50 {
        for j in 0..50 {
            if i != j && (i + j) % 5 == 0 {
                news.connect(i, j, 1500);
            }
        }
    }

    // Inject inputs
    for i in (0..50).step_by(5) {
        news.inject_spike(i, 5000);
    }

    // Measure simulation speed
    use std::time::Instant;
    let start = Instant::now();
    news.run(10000);
    let duration = start.elapsed();

    println!(
        "Simulated 10,000 cycles in {:?} ({:.2} cycles/ms)",
        duration,
        10000.0 / duration.as_millis() as f64
    );

    // Basic performance assertion
    assert!(
        duration.as_millis() < 5000,
        "Should simulate 10k cycles in under 5 seconds"
    );
}
