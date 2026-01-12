//! Agency Layer Tests
//!
//! Tests for agent-related functionality in the FXNN framework:
//! - Agent creation and initialization
//! - Sensor observation (position, velocity, force sensing)
//! - Actuator actions (applying forces, setting velocities)
//! - Policy forward pass (decision making based on state)
//! - Goal evaluation (measuring progress toward objectives)
//!
//! Note: FXNN is primarily a physics simulation library. These tests
//! model "agents" as atoms with specific behaviors and policies.

use fxnn::{
    Simulation, SimulationBox, LennardJones, VelocityVerlet,
    generators::{fcc_lattice, maxwell_boltzmann_velocities},
    types::Atom,
    observable,
};

// ============================================================================
// Agent Creation Tests
// ============================================================================

/// Agent wrapper around Atom for agent-based modeling
struct Agent {
    atom: Atom,
    goal_position: Option<[f32; 3]>,
    reward_accumulated: f64,
    steps_taken: usize,
}

impl Agent {
    fn new(id: u32, atom_type: u16, mass: f32) -> Self {
        Self {
            atom: Atom::new(id, atom_type, mass),
            goal_position: None,
            reward_accumulated: 0.0,
            steps_taken: 0,
        }
    }

    fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.atom = self.atom.with_position(x, y, z);
        self
    }

    fn with_velocity(mut self, vx: f32, vy: f32, vz: f32) -> Self {
        self.atom = self.atom.with_velocity(vx, vy, vz);
        self
    }

    fn with_goal(mut self, goal: [f32; 3]) -> Self {
        self.goal_position = Some(goal);
        self
    }

    fn position(&self) -> [f32; 3] {
        self.atom.position
    }

    fn velocity(&self) -> [f32; 3] {
        self.atom.velocity
    }

    fn force(&self) -> [f32; 3] {
        self.atom.force
    }
}

/// Test basic agent creation with proper initialization
#[test]
fn test_agent_creation() {
    let agent = Agent::new(0, 0, 1.0)
        .with_position(5.0, 5.0, 5.0)
        .with_velocity(1.0, 0.0, 0.0)
        .with_goal([10.0, 5.0, 5.0]);

    assert_eq!(agent.atom.id, 0);
    assert_eq!(agent.atom.mass, 1.0);
    assert_eq!(agent.position(), [5.0, 5.0, 5.0]);
    assert_eq!(agent.velocity(), [1.0, 0.0, 0.0]);
    assert!(agent.goal_position.is_some());
    assert_eq!(agent.goal_position.unwrap(), [10.0, 5.0, 5.0]);
    assert_eq!(agent.reward_accumulated, 0.0);
    assert_eq!(agent.steps_taken, 0);
}

/// Test multiple agent creation with different properties
#[test]
fn test_multi_agent_creation() {
    let agents: Vec<Agent> = (0..10)
        .map(|i| {
            Agent::new(i, 0, 1.0 + i as f32 * 0.1)
                .with_position(i as f32, 0.0, 0.0)
                .with_goal([10.0 - i as f32, 0.0, 0.0])
        })
        .collect();

    assert_eq!(agents.len(), 10);

    for (i, agent) in agents.iter().enumerate() {
        assert_eq!(agent.atom.id, i as u32);
        assert!((agent.atom.mass - (1.0 + i as f32 * 0.1)).abs() < 1e-6);
        assert_eq!(agent.position()[0], i as f32);
    }
}

// ============================================================================
// Sensor Observation Tests
// ============================================================================

/// Sensor trait for observing the environment
trait Sensor {
    fn observe(&self, atom: &Atom, environment: &SensorEnvironment) -> Observation;
}

/// Environment information available to sensors
struct SensorEnvironment {
    neighbors: Vec<Atom>,
    box_: SimulationBox,
}

/// Observation data from sensors
#[derive(Debug, Clone)]
struct Observation {
    local_density: f32,
    nearest_neighbor_distance: f32,
    average_neighbor_velocity: [f32; 3],
    force_magnitude: f32,
}

/// Position and local density sensor
struct DensitySensor {
    sensing_radius: f32,
}

impl Sensor for DensitySensor {
    fn observe(&self, atom: &Atom, env: &SensorEnvironment) -> Observation {
        let mut count = 0;
        let mut nearest_dist = f32::MAX;
        let mut avg_vel = [0.0f32; 3];

        for neighbor in &env.neighbors {
            let dist = env.box_.distance(&atom.position, &neighbor.position);
            if dist < self.sensing_radius && dist > 1e-6 {
                count += 1;
                nearest_dist = nearest_dist.min(dist);
                avg_vel[0] += neighbor.velocity[0];
                avg_vel[1] += neighbor.velocity[1];
                avg_vel[2] += neighbor.velocity[2];
            }
        }

        if count > 0 {
            avg_vel[0] /= count as f32;
            avg_vel[1] /= count as f32;
            avg_vel[2] /= count as f32;
        }

        let volume = 4.0 / 3.0 * std::f32::consts::PI * self.sensing_radius.powi(3);
        let local_density = count as f32 / volume;

        let force_mag = (atom.force[0].powi(2) + atom.force[1].powi(2) + atom.force[2].powi(2)).sqrt();

        Observation {
            local_density,
            nearest_neighbor_distance: if nearest_dist == f32::MAX { -1.0 } else { nearest_dist },
            average_neighbor_velocity: avg_vel,
            force_magnitude: force_mag,
        }
    }
}

/// Test sensor observation of local environment
#[test]
fn test_sensor_observation() {
    let atoms = vec![
        Atom::new(0, 0, 1.0).with_position(5.0, 5.0, 5.0).with_velocity(0.0, 0.0, 0.0),
        Atom::new(1, 0, 1.0).with_position(6.0, 5.0, 5.0).with_velocity(1.0, 0.0, 0.0),
        Atom::new(2, 0, 1.0).with_position(5.0, 6.0, 5.0).with_velocity(0.0, 1.0, 0.0),
        Atom::new(3, 0, 1.0).with_position(15.0, 15.0, 15.0).with_velocity(0.0, 0.0, 0.0), // Far away
    ];

    let box_ = SimulationBox::cubic(20.0);
    let sensor = DensitySensor { sensing_radius: 2.5 };

    let env = SensorEnvironment {
        neighbors: atoms[1..].to_vec(),
        box_,
    };

    let obs = sensor.observe(&atoms[0], &env);

    // Should observe 2 neighbors within radius 2.5
    assert!(obs.local_density > 0.0, "Should detect neighbors");
    assert!(
        (obs.nearest_neighbor_distance - 1.0).abs() < 0.01,
        "Nearest neighbor at distance 1.0"
    );
    assert!(
        obs.average_neighbor_velocity[0] > 0.0 || obs.average_neighbor_velocity[1] > 0.0,
        "Should detect neighbor velocities"
    );
}

/// Test sensor with varying radius
#[test]
fn test_sensor_radius_sensitivity() {
    let atoms: Vec<Atom> = (0..20)
        .map(|i| Atom::new(i, 0, 1.0).with_position(i as f32 * 0.5, 0.0, 0.0))
        .collect();

    let box_ = SimulationBox::cubic(20.0);

    // Small radius
    let small_sensor = DensitySensor { sensing_radius: 1.0 };
    let env_small = SensorEnvironment {
        neighbors: atoms[1..].to_vec(),
        box_,
    };
    let obs_small = small_sensor.observe(&atoms[0], &env_small);

    // Large radius
    let large_sensor = DensitySensor { sensing_radius: 5.0 };
    let env_large = SensorEnvironment {
        neighbors: atoms[1..].to_vec(),
        box_,
    };
    let obs_large = large_sensor.observe(&atoms[0], &env_large);

    // Larger radius should observe more neighbors (higher density before normalization)
    // but density is normalized by volume, so this is about the effective sensing
    println!("Small radius density: {}", obs_small.local_density);
    println!("Large radius density: {}", obs_large.local_density);

    // Nearest neighbor should be the same regardless of radius
    assert!(
        (obs_small.nearest_neighbor_distance - obs_large.nearest_neighbor_distance).abs() < 0.01,
        "Nearest neighbor distance should be independent of radius"
    );
}

// ============================================================================
// Actuator Action Tests
// ============================================================================

/// Actuator trait for applying actions
trait Actuator {
    fn apply(&self, atom: &mut Atom, action: &Action);
}

/// Action that an agent can take
#[derive(Debug, Clone)]
enum Action {
    ApplyForce([f32; 3]),
    SetVelocity([f32; 3]),
    Accelerate([f32; 3]),
    Stop,
}

/// Force-based actuator
struct ForceActuator {
    max_force: f32,
}

impl Actuator for ForceActuator {
    fn apply(&self, atom: &mut Atom, action: &Action) {
        match action {
            Action::ApplyForce(f) => {
                // Clamp force magnitude
                let mag = (f[0].powi(2) + f[1].powi(2) + f[2].powi(2)).sqrt();
                let scale = if mag > self.max_force {
                    self.max_force / mag
                } else {
                    1.0
                };

                atom.force[0] += f[0] * scale;
                atom.force[1] += f[1] * scale;
                atom.force[2] += f[2] * scale;
            }
            Action::SetVelocity(v) => {
                atom.velocity = *v;
            }
            Action::Accelerate(a) => {
                atom.velocity[0] += a[0];
                atom.velocity[1] += a[1];
                atom.velocity[2] += a[2];
            }
            Action::Stop => {
                atom.velocity = [0.0; 3];
            }
        }
    }
}

/// Test actuator force application
#[test]
fn test_actuator_apply_force() {
    let mut atom = Atom::new(0, 0, 1.0);
    let actuator = ForceActuator { max_force: 10.0 };

    actuator.apply(&mut atom, &Action::ApplyForce([1.0, 2.0, 3.0]));

    assert_eq!(atom.force, [1.0, 2.0, 3.0]);
}

/// Test actuator force clamping
#[test]
fn test_actuator_force_clamping() {
    let mut atom = Atom::new(0, 0, 1.0);
    let actuator = ForceActuator { max_force: 5.0 };

    // Apply force with magnitude > 5
    actuator.apply(&mut atom, &Action::ApplyForce([10.0, 0.0, 0.0]));

    let force_mag = (atom.force[0].powi(2) + atom.force[1].powi(2) + atom.force[2].powi(2)).sqrt();
    assert!(
        (force_mag - 5.0).abs() < 0.01,
        "Force should be clamped to max_force, got {}",
        force_mag
    );
}

/// Test velocity actions
#[test]
fn test_actuator_velocity_actions() {
    let mut atom = Atom::new(0, 0, 1.0).with_velocity(1.0, 2.0, 3.0);
    let actuator = ForceActuator { max_force: 10.0 };

    // Set velocity
    actuator.apply(&mut atom, &Action::SetVelocity([5.0, 0.0, 0.0]));
    assert_eq!(atom.velocity, [5.0, 0.0, 0.0]);

    // Accelerate
    actuator.apply(&mut atom, &Action::Accelerate([1.0, 1.0, 0.0]));
    assert_eq!(atom.velocity, [6.0, 1.0, 0.0]);

    // Stop
    actuator.apply(&mut atom, &Action::Stop);
    assert_eq!(atom.velocity, [0.0, 0.0, 0.0]);
}

// ============================================================================
// Policy Forward Tests
// ============================================================================

/// Policy trait for decision making
trait Policy {
    fn forward(&self, observation: &Observation, goal: Option<&[f32; 3]>, current_pos: &[f32; 3]) -> Action;
}

/// Simple goal-seeking policy
struct GoalSeekingPolicy {
    gain: f32,
    max_speed: f32,
}

impl Policy for GoalSeekingPolicy {
    fn forward(&self, _observation: &Observation, goal: Option<&[f32; 3]>, current_pos: &[f32; 3]) -> Action {
        match goal {
            Some(g) => {
                // Move towards goal
                let dx = g[0] - current_pos[0];
                let dy = g[1] - current_pos[1];
                let dz = g[2] - current_pos[2];

                let dist = (dx*dx + dy*dy + dz*dz).sqrt();

                if dist < 0.1 {
                    Action::Stop
                } else {
                    // Desired velocity towards goal
                    let speed = (dist * self.gain).min(self.max_speed);
                    Action::SetVelocity([
                        dx / dist * speed,
                        dy / dist * speed,
                        dz / dist * speed,
                    ])
                }
            }
            None => Action::Stop,
        }
    }
}

/// Test policy forward pass generates valid actions
#[test]
fn test_policy_forward() {
    let policy = GoalSeekingPolicy {
        gain: 1.0,
        max_speed: 2.0,
    };

    let obs = Observation {
        local_density: 0.0,
        nearest_neighbor_distance: -1.0,
        average_neighbor_velocity: [0.0; 3],
        force_magnitude: 0.0,
    };

    // With goal
    let goal = [10.0, 5.0, 5.0];
    let current = [5.0, 5.0, 5.0];
    let action = policy.forward(&obs, Some(&goal), &current);

    match action {
        Action::SetVelocity(v) => {
            // Should move in +x direction towards goal
            assert!(v[0] > 0.0, "Should move towards goal in x");
            assert!(v[1].abs() < 0.01, "Should have minimal y velocity");
            assert!(v[2].abs() < 0.01, "Should have minimal z velocity");
        }
        _ => panic!("Expected SetVelocity action"),
    }

    // Without goal
    let action_no_goal = policy.forward(&obs, None, &current);
    match action_no_goal {
        Action::Stop => {}
        _ => panic!("Expected Stop action without goal"),
    }
}

/// Test policy at goal position
#[test]
fn test_policy_at_goal() {
    let policy = GoalSeekingPolicy {
        gain: 1.0,
        max_speed: 2.0,
    };

    let obs = Observation {
        local_density: 0.0,
        nearest_neighbor_distance: -1.0,
        average_neighbor_velocity: [0.0; 3],
        force_magnitude: 0.0,
    };

    let goal = [5.0, 5.0, 5.0];
    let current = [5.0, 5.0, 5.0]; // Already at goal

    let action = policy.forward(&obs, Some(&goal), &current);

    match action {
        Action::Stop => {}
        _ => panic!("Should stop when at goal"),
    }
}

// ============================================================================
// Goal Evaluation Tests
// ============================================================================

/// Goal evaluator for measuring progress
struct GoalEvaluator {
    distance_weight: f32,
    speed_penalty: f32,
    energy_penalty: f32,
}

impl GoalEvaluator {
    fn evaluate(&self, agent: &Agent, box_: &SimulationBox) -> f64 {
        match agent.goal_position {
            Some(goal) => {
                // Distance to goal (negative reward)
                let dist = box_.distance(&agent.position(), &goal);

                // Speed penalty
                let speed = (agent.velocity()[0].powi(2)
                    + agent.velocity()[1].powi(2)
                    + agent.velocity()[2].powi(2))
                .sqrt();

                // Simple reward: closer is better, penalize excess speed
                let reward = -self.distance_weight * dist
                    - self.speed_penalty * (speed - 1.0).max(0.0); // Penalty for speed > 1

                reward as f64
            }
            None => 0.0,
        }
    }

    fn goal_reached(&self, agent: &Agent, threshold: f32, box_: &SimulationBox) -> bool {
        match agent.goal_position {
            Some(goal) => {
                let dist = box_.distance(&agent.position(), &goal);
                dist < threshold
            }
            None => false,
        }
    }
}

/// Test goal evaluation at different positions
#[test]
fn test_goal_evaluation() {
    let evaluator = GoalEvaluator {
        distance_weight: 1.0,
        speed_penalty: 0.5,
        energy_penalty: 0.0,
    };

    let box_ = SimulationBox::cubic(20.0);

    // Far from goal
    let agent_far = Agent::new(0, 0, 1.0)
        .with_position(0.0, 0.0, 0.0)
        .with_goal([10.0, 0.0, 0.0]);

    // Close to goal
    let agent_close = Agent::new(1, 0, 1.0)
        .with_position(9.0, 0.0, 0.0)
        .with_goal([10.0, 0.0, 0.0]);

    let reward_far = evaluator.evaluate(&agent_far, &box_);
    let reward_close = evaluator.evaluate(&agent_close, &box_);

    println!("Reward far: {}", reward_far);
    println!("Reward close: {}", reward_close);

    assert!(
        reward_close > reward_far,
        "Agent closer to goal should have higher reward"
    );
}

/// Test goal reached detection
#[test]
fn test_goal_reached() {
    let evaluator = GoalEvaluator {
        distance_weight: 1.0,
        speed_penalty: 0.0,
        energy_penalty: 0.0,
    };

    let box_ = SimulationBox::cubic(20.0);

    let agent_at_goal = Agent::new(0, 0, 1.0)
        .with_position(10.0, 5.0, 5.0)
        .with_goal([10.0, 5.0, 5.0]);

    let agent_far = Agent::new(1, 0, 1.0)
        .with_position(0.0, 0.0, 0.0)
        .with_goal([10.0, 5.0, 5.0]);

    assert!(evaluator.goal_reached(&agent_at_goal, 0.5, &box_));
    assert!(!evaluator.goal_reached(&agent_far, 0.5, &box_));
}

/// Test goal evaluation with speed penalty
#[test]
fn test_goal_evaluation_speed_penalty() {
    let evaluator = GoalEvaluator {
        distance_weight: 1.0,
        speed_penalty: 1.0,
        energy_penalty: 0.0,
    };

    let box_ = SimulationBox::cubic(20.0);

    // Same position, different speeds
    let agent_slow = Agent::new(0, 0, 1.0)
        .with_position(5.0, 0.0, 0.0)
        .with_velocity(0.5, 0.0, 0.0)
        .with_goal([10.0, 0.0, 0.0]);

    let agent_fast = Agent::new(1, 0, 1.0)
        .with_position(5.0, 0.0, 0.0)
        .with_velocity(5.0, 0.0, 0.0)
        .with_goal([10.0, 0.0, 0.0]);

    let reward_slow = evaluator.evaluate(&agent_slow, &box_);
    let reward_fast = evaluator.evaluate(&agent_fast, &box_);

    println!("Reward slow: {}", reward_slow);
    println!("Reward fast: {}", reward_fast);

    // Fast agent should have lower reward due to speed penalty
    assert!(
        reward_slow > reward_fast,
        "Agent with lower speed should have higher reward when penalty is applied"
    );
}

// ============================================================================
// Integration: Agent in Physical Simulation
// ============================================================================

/// Test agent behavior in physical simulation
#[test]
fn test_agent_in_physical_simulation() {
    // Create a physical system where "agents" (atoms) interact
    let mut atoms = fcc_lattice(2, 2, 2, 1.5);
    let box_ = SimulationBox::cubic(3.0);

    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);
    observable::remove_com_velocity(&mut atoms);

    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001);

    // Track first "agent"
    let initial_pos = sim.atoms()[0].position;

    // Run simulation
    sim.run(100);

    let final_pos = sim.atoms()[0].position;

    // Agent should have moved due to physical interactions
    let displacement = (
        (final_pos[0] - initial_pos[0]).powi(2)
        + (final_pos[1] - initial_pos[1]).powi(2)
        + (final_pos[2] - initial_pos[2]).powi(2)
    ).sqrt();

    // With temperature 1.0, atom should move
    assert!(
        displacement > 0.01,
        "Agent should move in physical simulation"
    );
}
