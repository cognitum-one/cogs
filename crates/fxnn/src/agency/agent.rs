//! Core Agent structure for embodied simulation.
//!
//! This module provides the [`Agent`] struct, which represents an autonomous
//! entity in the simulation with a physical body, sensors, actuators, and
//! a policy for decision-making.
//!
//! # Overview
//!
//! An agent is composed of:
//!
//! - **Physical avatar**: Position, velocity, orientation, and body shape
//! - **Sensors**: Perception of the environment (vision, hearing, touch, etc.)
//! - **Actuators**: Means of affecting the environment (movement, communication)
//! - **Policy**: Decision-making mechanism (rules or neural network)
//! - **Goals**: Objectives and reward functions
//! - **Learning state**: Experience buffer and learning parameters
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::agency::{Agent, AgentBody, VisionSensor, MotorActuator};
//!
//! // Create agent with spherical body
//! let body = AgentBody::sphere(0.5, 1.0);  // radius=0.5, mass=1.0
//! let mut agent = Agent::new(0, body);
//!
//! // Add sensors and actuators
//! agent.add_sensor(Box::new(VisionSensor::new(1.57, 10.0)));
//! agent.add_actuator(Box::new(MotorActuator::new(5.0)));
//!
//! // Set initial position
//! agent.set_position([1.0, 2.0, 0.0]);
//! ```

use serde::{Deserialize, Serialize};

use super::sensor::{Sensor, SensorReading};
use super::actuator::{Actuator, ActuatorCommand};
use super::policy::PolicyNetwork;
use super::goal::Goal;

/// Unique identifier for an agent.
pub type AgentId = u32;

/// Physical body of an agent.
///
/// Defines the shape, size, and mass distribution of the agent's
/// physical presence in the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBody {
    /// Body shape type.
    pub shape: BodyShape,
    /// Total mass of the body in simulation units.
    pub mass: f32,
    /// Moment of inertia tensor (diagonal elements for principal axes).
    pub inertia: [f32; 3],
    /// Coefficient of restitution (bounciness, 0-1).
    pub restitution: f32,
    /// Coefficient of friction.
    pub friction: f32,
}

/// Shape types for agent bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BodyShape {
    /// Spherical body with uniform radius.
    Sphere {
        /// Radius of the sphere.
        radius: f32,
    },
    /// Capsule (cylinder with hemispherical caps).
    Capsule {
        /// Radius of the capsule.
        radius: f32,
        /// Length of the cylindrical section.
        length: f32,
    },
    /// Axis-aligned box.
    Box {
        /// Half-extents in each dimension [x, y, z].
        half_extents: [f32; 3],
    },
    /// Ellipsoid with different radii along each axis.
    Ellipsoid {
        /// Radii along each axis [rx, ry, rz].
        radii: [f32; 3],
    },
}

impl AgentBody {
    /// Create a spherical body.
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the sphere
    /// * `mass` - Total mass of the body
    ///
    /// # Returns
    ///
    /// A new spherical `AgentBody`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fxnn::agency::AgentBody;
    ///
    /// let body = AgentBody::sphere(1.0, 2.0);
    /// assert_eq!(body.mass, 2.0);
    /// ```
    pub fn sphere(radius: f32, mass: f32) -> Self {
        // Moment of inertia for solid sphere: I = (2/5) * m * r^2
        let i = 0.4 * mass * radius * radius;
        Self {
            shape: BodyShape::Sphere { radius },
            mass,
            inertia: [i, i, i],
            restitution: 0.5,
            friction: 0.3,
        }
    }

    /// Create a capsule body (cylinder with hemispherical caps).
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the capsule
    /// * `length` - Length of the cylindrical section
    /// * `mass` - Total mass of the body
    ///
    /// # Returns
    ///
    /// A new capsule `AgentBody`.
    pub fn capsule(radius: f32, length: f32, mass: f32) -> Self {
        // Approximate moment of inertia for capsule
        let cylinder_mass = mass * length / (length + 4.0 * radius / 3.0);
        let sphere_mass = mass - cylinder_mass;

        // Cylinder: I_axial = (1/2)*m*r^2, I_transverse = (1/12)*m*(3r^2 + h^2)
        // Sphere: I = (2/5)*m*r^2
        let i_axial = 0.5 * cylinder_mass * radius * radius + 0.4 * sphere_mass * radius * radius;
        let i_transverse = cylinder_mass * (0.25 * radius * radius + length * length / 12.0)
            + sphere_mass * 0.4 * radius * radius;

        Self {
            shape: BodyShape::Capsule { radius, length },
            mass,
            inertia: [i_transverse, i_axial, i_transverse],
            restitution: 0.5,
            friction: 0.3,
        }
    }

    /// Create a box-shaped body.
    ///
    /// # Arguments
    ///
    /// * `half_extents` - Half-extents in each dimension [x, y, z]
    /// * `mass` - Total mass of the body
    ///
    /// # Returns
    ///
    /// A new box `AgentBody`.
    pub fn box_shape(half_extents: [f32; 3], mass: f32) -> Self {
        let [hx, hy, hz] = half_extents;
        // Moment of inertia for solid box: I_x = (1/12)*m*(y^2 + z^2)
        let factor = mass / 3.0;
        Self {
            shape: BodyShape::Box { half_extents },
            mass,
            inertia: [
                factor * (hy * hy + hz * hz),
                factor * (hx * hx + hz * hz),
                factor * (hx * hx + hy * hy),
            ],
            restitution: 0.5,
            friction: 0.3,
        }
    }

    /// Create an ellipsoid body.
    ///
    /// # Arguments
    ///
    /// * `radii` - Radii along each axis [rx, ry, rz]
    /// * `mass` - Total mass of the body
    ///
    /// # Returns
    ///
    /// A new ellipsoid `AgentBody`.
    pub fn ellipsoid(radii: [f32; 3], mass: f32) -> Self {
        let [rx, ry, rz] = radii;
        // Moment of inertia for solid ellipsoid: I_x = (1/5)*m*(y^2 + z^2)
        let factor = mass / 5.0;
        Self {
            shape: BodyShape::Ellipsoid { radii },
            mass,
            inertia: [
                factor * (ry * ry + rz * rz),
                factor * (rx * rx + rz * rz),
                factor * (rx * rx + ry * ry),
            ],
            restitution: 0.5,
            friction: 0.3,
        }
    }

    /// Set the coefficient of restitution (builder pattern).
    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution.clamp(0.0, 1.0);
        self
    }

    /// Set the coefficient of friction (builder pattern).
    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction.max(0.0);
        self
    }

    /// Get the bounding radius of the body.
    ///
    /// Returns the radius of the smallest sphere that contains the body.
    pub fn bounding_radius(&self) -> f32 {
        match &self.shape {
            BodyShape::Sphere { radius } => *radius,
            BodyShape::Capsule { radius, length } => radius + length * 0.5,
            BodyShape::Box { half_extents } => {
                let [hx, hy, hz] = half_extents;
                (hx * hx + hy * hy + hz * hz).sqrt()
            }
            BodyShape::Ellipsoid { radii } => {
                radii[0].max(radii[1]).max(radii[2])
            }
        }
    }
}

/// Dynamic state of an agent.
///
/// Contains all time-varying properties of the agent that change
/// during simulation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentState {
    /// Position in 3D space [x, y, z].
    pub position: [f32; 3],
    /// Linear velocity [vx, vy, vz].
    pub velocity: [f32; 3],
    /// Orientation as a quaternion [w, x, y, z].
    pub orientation: [f32; 4],
    /// Angular velocity [wx, wy, wz].
    pub angular_velocity: [f32; 3],
    /// Net force acting on the agent [fx, fy, fz].
    pub force: [f32; 3],
    /// Net torque acting on the agent [tx, ty, tz].
    pub torque: [f32; 3],
    /// Current energy level (for energy-based systems).
    pub energy: f32,
    /// Current health/integrity (0-1).
    pub health: f32,
}

impl AgentState {
    /// Create a new agent state at the origin.
    pub fn new() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0], // Identity quaternion
            angular_velocity: [0.0; 3],
            force: [0.0; 3],
            torque: [0.0; 3],
            energy: 1.0,
            health: 1.0,
        }
    }

    /// Get the forward direction vector based on current orientation.
    ///
    /// Returns the unit vector pointing in the agent's forward direction.
    pub fn forward(&self) -> [f32; 3] {
        let [w, x, y, z] = self.orientation;
        // Rotate the forward vector (0, 0, 1) by the quaternion
        [
            2.0 * (x * z + w * y),
            2.0 * (y * z - w * x),
            1.0 - 2.0 * (x * x + y * y),
        ]
    }

    /// Get the right direction vector based on current orientation.
    pub fn right(&self) -> [f32; 3] {
        let [w, x, y, z] = self.orientation;
        // Rotate the right vector (1, 0, 0) by the quaternion
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + w * z),
            2.0 * (x * z - w * y),
        ]
    }

    /// Get the up direction vector based on current orientation.
    pub fn up(&self) -> [f32; 3] {
        let [w, x, y, z] = self.orientation;
        // Rotate the up vector (0, 1, 0) by the quaternion
        [
            2.0 * (x * y - w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + w * x),
        ]
    }

    /// Calculate the speed (magnitude of velocity).
    pub fn speed(&self) -> f32 {
        let [vx, vy, vz] = self.velocity;
        (vx * vx + vy * vy + vz * vz).sqrt()
    }

    /// Calculate the kinetic energy given the body mass.
    pub fn kinetic_energy(&self, mass: f32) -> f32 {
        let v2 = self.velocity[0] * self.velocity[0]
            + self.velocity[1] * self.velocity[1]
            + self.velocity[2] * self.velocity[2];
        0.5 * mass * v2
    }

    /// Zero all forces and torques.
    pub fn zero_forces(&mut self) {
        self.force = [0.0; 3];
        self.torque = [0.0; 3];
    }

    /// Add a force to the accumulator.
    pub fn add_force(&mut self, force: [f32; 3]) {
        self.force[0] += force[0];
        self.force[1] += force[1];
        self.force[2] += force[2];
    }

    /// Add a torque to the accumulator.
    pub fn add_torque(&mut self, torque: [f32; 3]) {
        self.torque[0] += torque[0];
        self.torque[1] += torque[1];
        self.torque[2] += torque[2];
    }
}

/// Learning state for reinforcement learning agents.
#[derive(Debug, Clone, Default)]
pub struct LearningState {
    /// Total accumulated reward.
    pub total_reward: f32,
    /// Reward from the last step.
    pub last_reward: f32,
    /// Number of episodes completed.
    pub episodes: u32,
    /// Number of steps in current episode.
    pub episode_steps: u32,
    /// Experience buffer capacity.
    pub buffer_capacity: usize,
    /// Learning rate.
    pub learning_rate: f32,
    /// Discount factor (gamma).
    pub discount: f32,
    /// Exploration rate (epsilon).
    pub exploration_rate: f32,
}

impl LearningState {
    /// Create a new learning state with default parameters.
    pub fn new() -> Self {
        Self {
            total_reward: 0.0,
            last_reward: 0.0,
            episodes: 0,
            episode_steps: 0,
            buffer_capacity: 10000,
            learning_rate: 0.001,
            discount: 0.99,
            exploration_rate: 0.1,
        }
    }

    /// Update with a new reward.
    pub fn add_reward(&mut self, reward: f32) {
        self.last_reward = reward;
        self.total_reward += reward;
        self.episode_steps += 1;
    }

    /// Reset for a new episode.
    pub fn new_episode(&mut self) {
        self.episodes += 1;
        self.episode_steps = 0;
    }

    /// Decay the exploration rate.
    pub fn decay_exploration(&mut self, decay_rate: f32, min_exploration: f32) {
        self.exploration_rate = (self.exploration_rate * decay_rate).max(min_exploration);
    }
}

/// An autonomous agent in the simulation.
///
/// Agents are embodied entities that can perceive their environment through
/// sensors, make decisions via a policy, and act through actuators.
pub struct Agent {
    /// Unique identifier.
    id: AgentId,
    /// Physical body properties.
    body: AgentBody,
    /// Dynamic state.
    state: AgentState,
    /// Sensors for perception.
    sensors: Vec<Box<dyn Sensor>>,
    /// Actuators for action.
    actuators: Vec<Box<dyn Actuator>>,
    /// Policy network for decision making.
    policy: Option<Box<dyn PolicyNetwork>>,
    /// Goals and objectives.
    goals: Vec<Box<dyn Goal>>,
    /// Learning state.
    learning: LearningState,
    /// Whether the agent is active.
    active: bool,
    /// Custom tag/label.
    tag: String,
}

impl Agent {
    /// Create a new agent with the given ID and body.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this agent
    /// * `body` - Physical body properties
    ///
    /// # Returns
    ///
    /// A new `Agent` at the origin with no sensors or actuators.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fxnn::agency::{Agent, AgentBody};
    ///
    /// let body = AgentBody::sphere(1.0, 1.0);
    /// let agent = Agent::new(0, body);
    /// assert_eq!(agent.id(), 0);
    /// ```
    pub fn new(id: AgentId, body: AgentBody) -> Self {
        Self {
            id,
            body,
            state: AgentState::new(),
            sensors: Vec::new(),
            actuators: Vec::new(),
            policy: None,
            goals: Vec::new(),
            learning: LearningState::new(),
            active: true,
            tag: String::new(),
        }
    }

    /// Get the agent's unique identifier.
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Get a reference to the agent's body.
    pub fn body(&self) -> &AgentBody {
        &self.body
    }

    /// Get a mutable reference to the agent's body.
    pub fn body_mut(&mut self) -> &mut AgentBody {
        &mut self.body
    }

    /// Get a reference to the agent's state.
    pub fn state(&self) -> &AgentState {
        &self.state
    }

    /// Get a mutable reference to the agent's state.
    pub fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    /// Get a reference to the learning state.
    pub fn learning(&self) -> &LearningState {
        &self.learning
    }

    /// Get a mutable reference to the learning state.
    pub fn learning_mut(&mut self) -> &mut LearningState {
        &mut self.learning
    }

    /// Check if the agent is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Set the agent's active state.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Get the agent's tag.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Set the agent's tag (builder pattern).
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Set the agent's position.
    pub fn set_position(&mut self, position: [f32; 3]) {
        self.state.position = position;
    }

    /// Set the agent's velocity.
    pub fn set_velocity(&mut self, velocity: [f32; 3]) {
        self.state.velocity = velocity;
    }

    /// Set the agent's orientation (quaternion [w, x, y, z]).
    pub fn set_orientation(&mut self, orientation: [f32; 4]) {
        self.state.orientation = orientation;
    }

    /// Add a sensor to the agent.
    pub fn add_sensor(&mut self, sensor: Box<dyn Sensor>) {
        self.sensors.push(sensor);
    }

    /// Add a sensor (builder pattern).
    pub fn with_sensor(mut self, sensor: Box<dyn Sensor>) -> Self {
        self.sensors.push(sensor);
        self
    }

    /// Get the sensors.
    pub fn sensors(&self) -> &[Box<dyn Sensor>] {
        &self.sensors
    }

    /// Get mutable access to sensors.
    pub fn sensors_mut(&mut self) -> &mut [Box<dyn Sensor>] {
        &mut self.sensors
    }

    /// Add an actuator to the agent.
    pub fn add_actuator(&mut self, actuator: Box<dyn Actuator>) {
        self.actuators.push(actuator);
    }

    /// Add an actuator (builder pattern).
    pub fn with_actuator(mut self, actuator: Box<dyn Actuator>) -> Self {
        self.actuators.push(actuator);
        self
    }

    /// Get the actuators.
    pub fn actuators(&self) -> &[Box<dyn Actuator>] {
        &self.actuators
    }

    /// Get mutable access to actuators.
    pub fn actuators_mut(&mut self) -> &mut [Box<dyn Actuator>] {
        &mut self.actuators
    }

    /// Set the policy network.
    pub fn set_policy(&mut self, policy: Box<dyn PolicyNetwork>) {
        self.policy = Some(policy);
    }

    /// Set the policy network (builder pattern).
    pub fn with_policy(mut self, policy: Box<dyn PolicyNetwork>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Get a reference to the policy.
    pub fn policy(&self) -> Option<&dyn PolicyNetwork> {
        self.policy.as_ref().map(|p| p.as_ref())
    }

    /// Get a mutable reference to the policy.
    pub fn policy_mut(&mut self) -> Option<&mut Box<dyn PolicyNetwork>> {
        self.policy.as_mut()
    }

    /// Add a goal to the agent.
    pub fn add_goal(&mut self, goal: Box<dyn Goal>) {
        self.goals.push(goal);
    }

    /// Add a goal (builder pattern).
    pub fn with_goal(mut self, goal: Box<dyn Goal>) -> Self {
        self.goals.push(goal);
        self
    }

    /// Get the goals.
    pub fn goals(&self) -> &[Box<dyn Goal>] {
        &self.goals
    }

    /// Get mutable access to goals.
    pub fn goals_mut(&mut self) -> &mut [Box<dyn Goal>] {
        &mut self.goals
    }

    /// Clear all goals.
    pub fn clear_goals(&mut self) {
        self.goals.clear();
    }

    /// Collect sensor readings.
    ///
    /// Queries all sensors and returns their readings.
    ///
    /// # Arguments
    ///
    /// * `world_state` - Current state of the world (positions, etc.)
    ///
    /// # Returns
    ///
    /// Vector of sensor readings from all sensors.
    pub fn sense(&self, world_state: &WorldState) -> Vec<SensorReading> {
        self.sensors
            .iter()
            .map(|sensor| sensor.sense(&self.state, world_state))
            .collect()
    }

    /// Execute actuator commands.
    ///
    /// Applies the given commands to the corresponding actuators.
    ///
    /// # Arguments
    ///
    /// * `commands` - Commands for each actuator
    pub fn act(&mut self, commands: &[ActuatorCommand]) {
        for (actuator, command) in self.actuators.iter().zip(commands.iter()) {
            actuator.execute(command, &mut self.state);
        }
    }

    /// Evaluate all goals and return total reward.
    ///
    /// # Arguments
    ///
    /// * `world_state` - Current state of the world
    ///
    /// # Returns
    ///
    /// Total reward from all goals.
    pub fn evaluate_goals(&self, world_state: &WorldState) -> f32 {
        self.goals
            .iter()
            .map(|goal| goal.evaluate(&self.state, world_state))
            .sum()
    }

    /// Check if all goals are achieved.
    pub fn all_goals_achieved(&self, world_state: &WorldState) -> bool {
        self.goals
            .iter()
            .all(|goal| goal.is_achieved(&self.state, world_state))
    }

    /// Perform one step of the agent's decision loop.
    ///
    /// 1. Sense the environment
    /// 2. Decide on actions using policy
    /// 3. Execute actions through actuators
    /// 4. Evaluate goals and update learning state
    ///
    /// # Arguments
    ///
    /// * `world_state` - Current state of the world
    ///
    /// # Returns
    ///
    /// The reward obtained this step.
    pub fn step(&mut self, world_state: &WorldState) -> f32 {
        if !self.active {
            return 0.0;
        }

        // 1. Sense
        let observations = self.sense(world_state);

        // 2. Decide (if policy exists)
        if let Some(ref policy) = self.policy {
            let output = policy.forward(&observations);

            // 3. Act
            self.act(&output.commands);
        }

        // 4. Evaluate
        let reward = self.evaluate_goals(world_state);
        self.learning.add_reward(reward);

        reward
    }
}

/// World state for sensor queries and goal evaluation.
///
/// Contains information about the environment that agents can perceive.
#[derive(Debug, Clone, Default)]
pub struct WorldState {
    /// Positions of all entities in the world.
    pub positions: Vec<[f32; 3]>,
    /// Velocities of all entities.
    pub velocities: Vec<[f32; 3]>,
    /// Types/categories of entities.
    pub entity_types: Vec<u32>,
    /// Active sound sources.
    pub sound_sources: Vec<SoundSourceData>,
    /// Messages in transit.
    pub messages: Vec<MessageData>,
    /// Current simulation time.
    pub time: f32,
}

/// Data for a sound source in the world.
#[derive(Debug, Clone)]
pub struct SoundSourceData {
    /// Position of the sound source.
    pub position: [f32; 3],
    /// Intensity of the sound.
    pub intensity: f32,
    /// Frequency of the sound.
    pub frequency: f32,
}

/// Data for a message in transit.
#[derive(Debug, Clone)]
pub struct MessageData {
    /// Sender agent ID.
    pub sender: AgentId,
    /// Intended recipient (None for broadcast).
    pub recipient: Option<AgentId>,
    /// Message content.
    pub content: Vec<f32>,
    /// Position where message was sent.
    pub origin: [f32; 3],
    /// Range of the message.
    pub range: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_body_sphere() {
        let body = AgentBody::sphere(1.0, 2.0);
        assert_eq!(body.mass, 2.0);
        assert_eq!(body.bounding_radius(), 1.0);
        // Moment of inertia: I = 0.4 * 2.0 * 1.0^2 = 0.8
        assert!((body.inertia[0] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_agent_state_directions() {
        let state = AgentState::new();
        // Identity quaternion should give standard basis
        let forward = state.forward();
        assert!((forward[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_agent_creation_and_config() {
        let body = AgentBody::sphere(1.0, 1.0);
        let agent = Agent::new(42, body)
            .with_tag("test_agent");

        assert_eq!(agent.id(), 42);
        assert_eq!(agent.tag(), "test_agent");
        assert!(agent.is_active());
    }

    #[test]
    fn test_learning_state() {
        let mut learning = LearningState::new();
        learning.add_reward(1.0);
        learning.add_reward(0.5);

        assert_eq!(learning.total_reward, 1.5);
        assert_eq!(learning.last_reward, 0.5);
        assert_eq!(learning.episode_steps, 2);

        learning.new_episode();
        assert_eq!(learning.episodes, 1);
        assert_eq!(learning.episode_steps, 0);
    }
}
