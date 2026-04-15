//! Agent Maze Navigation Example
//!
//! This example demonstrates the Agency layer (Layer 4) of FXNN's Five-Layer
//! Reality Stack, where decision-making entities perceive and act within the
//! physical simulation.
//!
//! # Concept
//!
//! An "agent" is a particle with:
//! - **Sensors**: Detect nearby obstacles (repulsive walls) and goal (attractor)
//! - **Actuators**: Apply forces to move toward goals and avoid obstacles
//! - **Policy**: Decision function mapping observations to actions
//!
//! The agent navigates a 2D maze by sensing its environment and applying forces
//! to move. Unlike classical MD where forces arise from potentials, the agent
//! generates forces based on its internal decision-making process.
//!
//! # Reality Stack Layers
//!
//! 1. **Substrate**: Agent particle and obstacle particles in 2D box
//! 2. **Forces**: Soft repulsive walls + agent-generated propulsion
//! 3. **Dynamics**: Langevin dynamics (with friction for damping)
//! 4. **Agency**: Sensor-Policy-Actuator loop
//!
//! # Physics
//!
//! The agent experiences:
//! - Wall repulsion: F_wall = -k * (d - r_cut) if d < r_cut
//! - Goal attraction: F_goal = k_goal * (goal - pos) / |goal - pos|
//! - Self-propulsion: F_propel = policy(observations) * max_force
//! - Friction: F_friction = -gamma * velocity
//!
//! # Running
//!
//! ```bash
//! cargo run --example agent_maze --release
//! ```
//!
//! # Extending
//!
//! Try modifying:
//! - Maze layout (add more obstacles)
//! - Policy function (neural network instead of rule-based)
//! - Multiple agents with different goals
//! - Add learning (reward for reaching goal)

use fxnn::{
    SimulationBox,
    types::Atom,
    integrator::Langevin,
    observable,
};

/// Agent configuration
mod agent {
    pub const MASS: f32 = 1.0;
    pub const SENSOR_RANGE: f32 = 2.0;
    pub const MAX_FORCE: f32 = 5.0;
    pub const GOAL_STRENGTH: f32 = 2.0;
    pub const WALL_REPULSION: f32 = 10.0;
    pub const FRICTION: f32 = 2.0;
}

/// Represents the agent's sensory input
#[derive(Debug, Clone)]
struct Observation {
    /// Relative position to goal
    goal_direction: [f32; 2],
    /// Distance to goal
    goal_distance: f32,
    /// Nearby obstacle positions (relative)
    obstacles: Vec<([f32; 2], f32)>,
    /// Current velocity
    velocity: [f32; 2],
}

/// Agent's action output
#[derive(Debug, Clone)]
struct Action {
    /// Force to apply [fx, fy]
    force: [f32; 2],
}

/// Simple rule-based policy
struct SimplePolicy {
    /// Goal position
    goal: [f32; 2],
}

impl SimplePolicy {
    fn new(goal: [f32; 2]) -> Self {
        Self { goal }
    }

    /// Map observation to action
    fn decide(&self, obs: &Observation) -> Action {
        let mut fx = 0.0f32;
        let mut fy = 0.0f32;

        // Goal-seeking behavior: move toward goal
        if obs.goal_distance > 0.1 {
            let goal_scale = agent::GOAL_STRENGTH / obs.goal_distance.max(0.5);
            fx += obs.goal_direction[0] * goal_scale;
            fy += obs.goal_direction[1] * goal_scale;
        }

        // Obstacle avoidance: steer away from nearby obstacles
        for (dir, dist) in &obs.obstacles {
            if *dist < agent::SENSOR_RANGE {
                // Repulsive force proportional to 1/distance^2
                let repulsion = agent::WALL_REPULSION / (dist * dist).max(0.1);
                fx -= dir[0] * repulsion;
                fy -= dir[1] * repulsion;
            }
        }

        // Damping: reduce velocity to prevent oscillations
        fx -= obs.velocity[0] * 0.5;
        fy -= obs.velocity[1] * 0.5;

        // Clamp to maximum force
        let magnitude = (fx * fx + fy * fy).sqrt();
        if magnitude > agent::MAX_FORCE {
            let scale = agent::MAX_FORCE / magnitude;
            fx *= scale;
            fy *= scale;
        }

        Action { force: [fx, fy] }
    }
}

/// Agent state combining physics and decision-making
struct Agent {
    /// Physical representation
    atom: Atom,
    /// Policy for decision-making
    policy: SimplePolicy,
    /// Sensor data
    observation: Option<Observation>,
    /// Accumulated reward
    reward: f32,
    /// Steps taken
    steps: usize,
}

impl Agent {
    fn new(position: [f32; 2], goal: [f32; 2]) -> Self {
        let atom = Atom::new(0, 0, agent::MASS)
            .with_position(position[0], position[1], 0.0);

        Self {
            atom,
            policy: SimplePolicy::new(goal),
            observation: None,
            reward: 0.0,
            steps: 0,
        }
    }

    /// Sense the environment
    fn sense(&mut self, obstacles: &[Atom]) {
        let pos = [self.atom.position[0], self.atom.position[1]];
        let goal = self.policy.goal;

        // Goal direction and distance
        let dx = goal[0] - pos[0];
        let dy = goal[1] - pos[1];
        let goal_distance = (dx * dx + dy * dy).sqrt();
        let goal_direction = if goal_distance > 0.01 {
            [dx / goal_distance, dy / goal_distance]
        } else {
            [0.0, 0.0]
        };

        // Detect nearby obstacles
        let mut nearby_obstacles = Vec::new();
        for obs in obstacles {
            let ox = obs.position[0] - pos[0];
            let oy = obs.position[1] - pos[1];
            let dist = (ox * ox + oy * oy).sqrt();

            if dist < agent::SENSOR_RANGE && dist > 0.01 {
                nearby_obstacles.push(([ox / dist, oy / dist], dist));
            }
        }

        self.observation = Some(Observation {
            goal_direction,
            goal_distance,
            obstacles: nearby_obstacles,
            velocity: [self.atom.velocity[0], self.atom.velocity[1]],
        });
    }

    /// Decide and act based on observations
    fn act(&mut self) {
        if let Some(ref obs) = self.observation {
            let action = self.policy.decide(obs);

            // Apply force to atom
            self.atom.force[0] = action.force[0];
            self.atom.force[1] = action.force[1];
            self.atom.force[2] = 0.0; // Stay in 2D
        }
    }

    /// Update position using simple Euler integration with friction
    fn step(&mut self, dt: f32) {
        // Apply friction
        let friction = [
            -agent::FRICTION * self.atom.velocity[0],
            -agent::FRICTION * self.atom.velocity[1],
        ];

        // Update velocity: v += (F + friction) / m * dt
        let inv_mass = 1.0 / self.atom.mass;
        self.atom.velocity[0] += (self.atom.force[0] + friction[0]) * inv_mass * dt;
        self.atom.velocity[1] += (self.atom.force[1] + friction[1]) * inv_mass * dt;

        // Update position: x += v * dt
        self.atom.position[0] += self.atom.velocity[0] * dt;
        self.atom.position[1] += self.atom.velocity[1] * dt;

        self.steps += 1;
    }

    /// Calculate reward based on distance to goal
    fn evaluate_reward(&mut self) {
        if let Some(ref obs) = self.observation {
            // Negative reward proportional to distance (closer = better)
            let dist_reward = -obs.goal_distance * 0.1;

            // Bonus for reaching goal
            let goal_bonus = if obs.goal_distance < 0.5 { 10.0 } else { 0.0 };

            // Penalty for hitting obstacles
            let obstacle_penalty: f32 = obs.obstacles.iter()
                .filter(|(_, d)| *d < 0.3)
                .map(|_| -1.0)
                .sum();

            self.reward += dist_reward + goal_bonus + obstacle_penalty;
        }
    }

    /// Check if agent has reached the goal
    fn reached_goal(&self) -> bool {
        if let Some(ref obs) = self.observation {
            obs.goal_distance < 0.5
        } else {
            false
        }
    }
}

/// Create obstacle atoms for the maze
fn create_maze() -> Vec<Atom> {
    let mut obstacles = Vec::new();
    let mut id = 100u32;

    // Maze walls using particles
    // Outer boundary
    for i in 0..20 {
        let t = i as f32 * 0.5;
        // Top wall
        obstacles.push(Atom::new(id, 1, 100.0).with_position(t, 10.0, 0.0));
        id += 1;
        // Bottom wall
        obstacles.push(Atom::new(id, 1, 100.0).with_position(t, 0.0, 0.0));
        id += 1;
        // Left wall
        obstacles.push(Atom::new(id, 1, 100.0).with_position(0.0, t, 0.0));
        id += 1;
        // Right wall
        obstacles.push(Atom::new(id, 1, 100.0).with_position(10.0, t, 0.0));
        id += 1;
    }

    // Internal obstacles (simple maze pattern)
    // Horizontal wall 1
    for i in 0..10 {
        obstacles.push(Atom::new(id, 1, 100.0)
            .with_position(i as f32 * 0.5, 3.0, 0.0));
        id += 1;
    }

    // Horizontal wall 2
    for i in 4..14 {
        obstacles.push(Atom::new(id, 1, 100.0)
            .with_position(i as f32 * 0.5, 7.0, 0.0));
        id += 1;
    }

    // Vertical wall
    for i in 3..7 {
        obstacles.push(Atom::new(id, 1, 100.0)
            .with_position(5.0, i as f32, 0.0));
        id += 1;
    }

    obstacles
}

fn main() {
    println!("=======================================================");
    println!("  FXNN Agent Maze Navigation");
    println!("  Layer 4: Agency in the Reality Stack");
    println!("=======================================================\n");

    // =========================================================================
    // Layer 1: SUBSTRATE
    // Create agent and maze obstacles
    // =========================================================================

    println!("[Layer 1: SUBSTRATE]");

    // Create maze obstacles
    let obstacles = create_maze();
    println!("  Created maze with {} obstacle particles", obstacles.len());

    // Agent starting position and goal
    let start = [1.0, 1.5];
    let goal = [9.0, 8.5];
    let mut agent = Agent::new(start, goal);

    println!("  Agent start: ({:.1}, {:.1})", start[0], start[1]);
    println!("  Goal: ({:.1}, {:.1})", goal[0], goal[1]);

    // Simulation box (2D, but using 3D box)
    let box_ = SimulationBox::cubic(10.0);
    println!("  Arena: {:.1} x {:.1}", box_.dimensions[0], box_.dimensions[1]);

    // =========================================================================
    // Layer 2: FORCES
    // Wall repulsion + agent propulsion
    // =========================================================================

    println!("\n[Layer 2: FORCES]");
    println!("  Wall repulsion strength: {:.1}", agent::WALL_REPULSION);
    println!("  Goal attraction strength: {:.1}", agent::GOAL_STRENGTH);
    println!("  Maximum propulsion force: {:.1}", agent::MAX_FORCE);

    // =========================================================================
    // Layer 3: DYNAMICS
    // Euler integration with friction
    // =========================================================================

    println!("\n[Layer 3: DYNAMICS]");
    let dt = 0.01;
    let friction = agent::FRICTION;
    println!("  Timestep: {:.3}", dt);
    println!("  Friction coefficient: {:.1}", friction);

    // =========================================================================
    // Layer 4: AGENCY
    // Sensor-Policy-Actuator loop
    // =========================================================================

    println!("\n[Layer 4: AGENCY]");
    println!("  Sensor range: {:.1}", agent::SENSOR_RANGE);
    println!("  Policy: Rule-based (goal-seeking + obstacle avoidance)");

    // =========================================================================
    // Simulation Loop
    // =========================================================================

    println!("\n[Navigation]");
    println!("  Running agent through maze...\n");

    let max_steps = 5000;
    let report_interval = 500;

    println!("  {:>6} {:>8} {:>8} {:>8} {:>8} {:>10}",
             "Step", "X", "Y", "VX", "VY", "Goal Dist");
    println!("  {}", "-".repeat(62));

    for step in 0..max_steps {
        // Sense-Think-Act cycle
        agent.sense(&obstacles);
        agent.act();
        agent.step(dt);
        agent.evaluate_reward();

        // Report progress
        if step % report_interval == 0 || agent.reached_goal() {
            let dist = if let Some(ref obs) = agent.observation {
                obs.goal_distance
            } else {
                0.0
            };

            println!("  {:>6} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>10.2}",
                     step,
                     agent.atom.position[0],
                     agent.atom.position[1],
                     agent.atom.velocity[0],
                     agent.atom.velocity[1],
                     dist);
        }

        // Check if goal reached
        if agent.reached_goal() {
            println!("\n  *** GOAL REACHED! ***");
            break;
        }
    }

    // =========================================================================
    // Results
    // =========================================================================

    println!("\n[Results]");
    println!("  --------------------------------------------------");
    println!("  Steps taken:      {:>8}", agent.steps);
    println!("  Final position:   ({:.2}, {:.2})",
             agent.atom.position[0], agent.atom.position[1]);

    let final_dist = if let Some(ref obs) = agent.observation {
        obs.goal_distance
    } else {
        f32::MAX
    };
    println!("  Distance to goal: {:>8.2}", final_dist);
    println!("  Accumulated reward:{:>8.2}", agent.reward);
    println!("  Goal reached:     {:>8}", if agent.reached_goal() { "YES" } else { "NO" });
    println!("  --------------------------------------------------");

    // Interpretation
    println!("\n[Interpretation]");

    if agent.reached_goal() {
        let efficiency = (agent.steps as f32) / final_dist;
        println!("  Navigation: SUCCESS");
        println!("  The agent successfully navigated the maze using only");
        println!("  local sensing and a simple rule-based policy.");
        println!("  ");
        println!("  This demonstrates emergent behavior from simple rules:");
        println!("  - Move toward goal");
        println!("  - Avoid obstacles");
        println!("  - Apply friction for stability");
    } else {
        println!("  Navigation: INCOMPLETE");
        println!("  The agent did not reach the goal in {} steps.", max_steps);
        println!("  ");
        println!("  Try adjusting:");
        println!("  - Sensor range");
        println!("  - Force magnitudes");
        println!("  - Policy parameters");
        println!("  ");
        println!("  Or replace the rule-based policy with a learned one!");
    }

    println!("\n=======================================================");
    println!("  Layer 4 Demo Complete");
    println!("  Next: See multi_agent.rs for Layer 5 (Emergence)");
    println!("=======================================================\n");
}
