//! Actuator types for agent actions.
//!
//! This module provides various actuator implementations that allow agents
//! to affect the environment. Each actuator type has different capabilities
//! and effects.
//!
//! # Actuator Types
//!
//! | Actuator | Effect | Parameters |
//! |----------|--------|------------|
//! | [`MotorActuator`] | Apply forces/velocities | Force vector, max force |
//! | [`CommunicationActuator`] | Send messages | Content, range, channel |
//! | [`ManipulationActuator`] | Interact with objects | Target, action type |
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::agency::{MotorActuator, CommunicationActuator, ActuatorType, Actuator};
//!
//! // Create a motor with max force of 10
//! let motor = MotorActuator::new(10.0);
//! assert_eq!(motor.actuator_type(), ActuatorType::Motor);
//!
//! // Create a communication actuator with 15 unit range
//! let comm = CommunicationActuator::new(15.0);
//! assert_eq!(comm.actuator_type(), ActuatorType::Communication);
//! ```

use super::agent::AgentState;

/// Types of actuators available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActuatorType {
    /// Motor actuator for movement.
    Motor,
    /// Communication actuator for sending messages.
    Communication,
    /// Manipulation actuator for interacting with objects.
    Manipulation,
}

/// A command to an actuator.
#[derive(Debug, Clone)]
pub enum ActuatorCommand {
    /// Motor command.
    Motor(MotorCommand),
    /// Communication command.
    Communication(CommunicationCommand),
    /// Manipulation command.
    Manipulation(ManipulationCommand),
    /// No operation.
    None,
}

impl ActuatorCommand {
    /// Get the actuator type this command is for.
    pub fn actuator_type(&self) -> Option<ActuatorType> {
        match self {
            ActuatorCommand::Motor(_) => Some(ActuatorType::Motor),
            ActuatorCommand::Communication(_) => Some(ActuatorType::Communication),
            ActuatorCommand::Manipulation(_) => Some(ActuatorType::Manipulation),
            ActuatorCommand::None => None,
        }
    }
}

impl Default for ActuatorCommand {
    fn default() -> Self {
        ActuatorCommand::None
    }
}

/// Trait for all actuator implementations.
pub trait Actuator: Send + Sync {
    /// Get the type of this actuator.
    fn actuator_type(&self) -> ActuatorType;

    /// Execute a command on this actuator.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute
    /// * `state` - Mutable reference to the agent's state
    fn execute(&self, command: &ActuatorCommand, state: &mut AgentState);

    /// Check if this actuator can execute the given command.
    fn can_execute(&self, command: &ActuatorCommand) -> bool;

    /// Get the energy cost of executing a command.
    fn energy_cost(&self, command: &ActuatorCommand) -> f32;
}

// ============================================================================
// Motor Actuator
// ============================================================================

/// Command for a motor actuator.
#[derive(Debug, Clone)]
pub struct MotorCommand {
    /// Force to apply in world coordinates [fx, fy, fz].
    pub force: [f32; 3],
    /// Torque to apply [tx, ty, tz].
    pub torque: [f32; 3],
    /// Whether force is relative to agent orientation.
    pub relative: bool,
}

impl MotorCommand {
    /// Create a new motor command with only force.
    pub fn force(fx: f32, fy: f32, fz: f32) -> Self {
        Self {
            force: [fx, fy, fz],
            torque: [0.0; 3],
            relative: false,
        }
    }

    /// Create a new motor command with force relative to agent orientation.
    pub fn relative_force(forward: f32, right: f32, up: f32) -> Self {
        Self {
            force: [forward, right, up],
            torque: [0.0; 3],
            relative: true,
        }
    }

    /// Create a motor command with torque.
    pub fn with_torque(mut self, tx: f32, ty: f32, tz: f32) -> Self {
        self.torque = [tx, ty, tz];
        self
    }

    /// Create a command to move forward.
    pub fn forward(force: f32) -> Self {
        Self::relative_force(force, 0.0, 0.0)
    }

    /// Create a command to strafe right.
    pub fn strafe_right(force: f32) -> Self {
        Self::relative_force(0.0, force, 0.0)
    }

    /// Create a command to turn (rotate around up axis).
    pub fn turn(torque: f32) -> Self {
        Self {
            force: [0.0; 3],
            torque: [0.0, torque, 0.0],
            relative: true,
        }
    }
}

impl Default for MotorCommand {
    fn default() -> Self {
        Self {
            force: [0.0; 3],
            torque: [0.0; 3],
            relative: false,
        }
    }
}

/// Motor actuator for movement.
///
/// Applies forces and torques to the agent's body, enabling locomotion.
/// Forces can be specified in world coordinates or relative to the
/// agent's orientation.
#[derive(Debug, Clone)]
pub struct MotorActuator {
    /// Maximum force magnitude.
    max_force: f32,
    /// Maximum torque magnitude.
    max_torque: f32,
    /// Energy cost per unit force.
    energy_per_force: f32,
}

impl MotorActuator {
    /// Create a new motor actuator.
    ///
    /// # Arguments
    ///
    /// * `max_force` - Maximum force that can be applied
    ///
    /// # Returns
    ///
    /// A new `MotorActuator`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fxnn::agency::MotorActuator;
    ///
    /// let motor = MotorActuator::new(10.0);
    /// assert_eq!(motor.max_force(), 10.0);
    /// ```
    pub fn new(max_force: f32) -> Self {
        Self {
            max_force,
            max_torque: max_force * 0.5,
            energy_per_force: 0.01,
        }
    }

    /// Set the maximum torque (builder pattern).
    pub fn with_max_torque(mut self, max_torque: f32) -> Self {
        self.max_torque = max_torque;
        self
    }

    /// Set the energy cost per unit force (builder pattern).
    pub fn with_energy_cost(mut self, cost: f32) -> Self {
        self.energy_per_force = cost;
        self
    }

    /// Get the maximum force.
    pub fn max_force(&self) -> f32 {
        self.max_force
    }

    /// Get the maximum torque.
    pub fn max_torque(&self) -> f32 {
        self.max_torque
    }

    /// Clamp a vector to maximum magnitude.
    fn clamp_magnitude(v: [f32; 3], max_mag: f32) -> [f32; 3] {
        let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if mag <= max_mag || mag < 1e-6 {
            v
        } else {
            let scale = max_mag / mag;
            [v[0] * scale, v[1] * scale, v[2] * scale]
        }
    }

    /// Transform a relative vector to world coordinates.
    fn relative_to_world(v: [f32; 3], state: &AgentState) -> [f32; 3] {
        let forward = state.forward();
        let right = state.right();
        let up = state.up();
        [
            v[0] * forward[0] + v[1] * right[0] + v[2] * up[0],
            v[0] * forward[1] + v[1] * right[1] + v[2] * up[1],
            v[0] * forward[2] + v[1] * right[2] + v[2] * up[2],
        ]
    }
}

impl Actuator for MotorActuator {
    fn actuator_type(&self) -> ActuatorType {
        ActuatorType::Motor
    }

    fn execute(&self, command: &ActuatorCommand, state: &mut AgentState) {
        let motor_cmd = match command {
            ActuatorCommand::Motor(cmd) => cmd,
            _ => return,
        };

        // Transform relative forces/torques to world coordinates
        let force = if motor_cmd.relative {
            Self::relative_to_world(motor_cmd.force, state)
        } else {
            motor_cmd.force
        };

        let torque = if motor_cmd.relative {
            Self::relative_to_world(motor_cmd.torque, state)
        } else {
            motor_cmd.torque
        };

        // Clamp to maximum values
        let clamped_force = Self::clamp_magnitude(force, self.max_force);
        let clamped_torque = Self::clamp_magnitude(torque, self.max_torque);

        // Apply to state
        state.add_force(clamped_force);
        state.add_torque(clamped_torque);
    }

    fn can_execute(&self, command: &ActuatorCommand) -> bool {
        matches!(command, ActuatorCommand::Motor(_))
    }

    fn energy_cost(&self, command: &ActuatorCommand) -> f32 {
        match command {
            ActuatorCommand::Motor(cmd) => {
                let force_mag = (cmd.force[0] * cmd.force[0]
                    + cmd.force[1] * cmd.force[1]
                    + cmd.force[2] * cmd.force[2])
                .sqrt();
                force_mag * self.energy_per_force
            }
            _ => 0.0,
        }
    }
}

// ============================================================================
// Communication Actuator
// ============================================================================

/// Command for a communication actuator.
#[derive(Debug, Clone)]
pub struct CommunicationCommand {
    /// Message content (vector of floats).
    pub content: Vec<f32>,
    /// Broadcast range (0 for unlimited within actuator range).
    pub range: f32,
    /// Target agent ID (None for broadcast).
    pub target: Option<u32>,
    /// Communication channel.
    pub channel: u32,
}

impl CommunicationCommand {
    /// Create a broadcast message.
    pub fn broadcast(content: Vec<f32>) -> Self {
        Self {
            content,
            range: 0.0,
            target: None,
            channel: 0,
        }
    }

    /// Create a directed message to a specific agent.
    pub fn directed(content: Vec<f32>, target: u32) -> Self {
        Self {
            content,
            range: 0.0,
            target: Some(target),
            channel: 0,
        }
    }

    /// Set the range (builder pattern).
    pub fn with_range(mut self, range: f32) -> Self {
        self.range = range;
        self
    }

    /// Set the channel (builder pattern).
    pub fn with_channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }
}

impl Default for CommunicationCommand {
    fn default() -> Self {
        Self {
            content: Vec::new(),
            range: 0.0,
            target: None,
            channel: 0,
        }
    }
}

/// Communication actuator for sending messages.
///
/// Allows agents to broadcast or send directed messages to other agents.
/// Messages are limited by range and can be filtered by channel.
#[derive(Debug, Clone)]
pub struct CommunicationActuator {
    /// Maximum broadcast range.
    max_range: f32,
    /// Maximum message size (number of floats).
    max_message_size: usize,
    /// Default channel.
    channel: u32,
    /// Energy cost per message.
    energy_per_message: f32,
}

impl CommunicationActuator {
    /// Create a new communication actuator.
    ///
    /// # Arguments
    ///
    /// * `max_range` - Maximum broadcast range
    ///
    /// # Returns
    ///
    /// A new `CommunicationActuator`.
    pub fn new(max_range: f32) -> Self {
        Self {
            max_range,
            max_message_size: 64,
            channel: 0,
            energy_per_message: 0.1,
        }
    }

    /// Set the maximum message size (builder pattern).
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set the default channel (builder pattern).
    pub fn with_channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }

    /// Get the maximum range.
    pub fn max_range(&self) -> f32 {
        self.max_range
    }
}

impl Actuator for CommunicationActuator {
    fn actuator_type(&self) -> ActuatorType {
        ActuatorType::Communication
    }

    fn execute(&self, command: &ActuatorCommand, _state: &mut AgentState) {
        // Communication doesn't directly modify agent state.
        // The message is handled by the simulation/world layer.
        if let ActuatorCommand::Communication(cmd) = command {
            // Validate message size
            if cmd.content.len() > self.max_message_size {
                // In a real implementation, this would be logged or handled
                return;
            }
            // Message would be queued for delivery by the simulation
        }
    }

    fn can_execute(&self, command: &ActuatorCommand) -> bool {
        match command {
            ActuatorCommand::Communication(cmd) => cmd.content.len() <= self.max_message_size,
            _ => false,
        }
    }

    fn energy_cost(&self, command: &ActuatorCommand) -> f32 {
        match command {
            ActuatorCommand::Communication(cmd) => {
                // Cost scales with message size and range
                let size_factor = cmd.content.len() as f32 / self.max_message_size as f32;
                let range_factor = if cmd.range > 0.0 {
                    cmd.range / self.max_range
                } else {
                    1.0
                };
                self.energy_per_message * (1.0 + size_factor) * range_factor
            }
            _ => 0.0,
        }
    }
}

// ============================================================================
// Manipulation Actuator
// ============================================================================

/// Target for manipulation actions.
#[derive(Debug, Clone)]
pub enum ManipulationTarget {
    /// Target a specific entity by index.
    Entity(usize),
    /// Target a position in space.
    Position([f32; 3]),
    /// Target the nearest entity of a type.
    NearestOfType(u32),
}

/// Types of manipulation actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManipulationAction {
    /// Pick up an object.
    PickUp,
    /// Drop a held object.
    Drop,
    /// Push an object.
    Push,
    /// Pull an object.
    Pull,
    /// Activate/interact with an object.
    Activate,
}

/// Command for a manipulation actuator.
#[derive(Debug, Clone)]
pub struct ManipulationCommand {
    /// Target of the manipulation.
    pub target: ManipulationTarget,
    /// Type of manipulation action.
    pub action: ManipulationAction,
    /// Force to apply (for push/pull).
    pub force: f32,
}

impl ManipulationCommand {
    /// Create a pick-up command.
    pub fn pick_up(target: ManipulationTarget) -> Self {
        Self {
            target,
            action: ManipulationAction::PickUp,
            force: 0.0,
        }
    }

    /// Create a drop command.
    pub fn drop_item() -> Self {
        Self {
            target: ManipulationTarget::Position([0.0; 3]),
            action: ManipulationAction::Drop,
            force: 0.0,
        }
    }

    /// Create a push command.
    pub fn push(target: ManipulationTarget, force: f32) -> Self {
        Self {
            target,
            action: ManipulationAction::Push,
            force,
        }
    }

    /// Create a pull command.
    pub fn pull(target: ManipulationTarget, force: f32) -> Self {
        Self {
            target,
            action: ManipulationAction::Pull,
            force,
        }
    }

    /// Create an activate command.
    pub fn activate(target: ManipulationTarget) -> Self {
        Self {
            target,
            action: ManipulationAction::Activate,
            force: 0.0,
        }
    }
}

impl Default for ManipulationCommand {
    fn default() -> Self {
        Self {
            target: ManipulationTarget::Position([0.0; 3]),
            action: ManipulationAction::Activate,
            force: 0.0,
        }
    }
}

/// Manipulation actuator for interacting with objects.
///
/// Allows agents to pick up, drop, push, pull, and activate objects
/// in the environment.
#[derive(Debug, Clone)]
pub struct ManipulationActuator {
    /// Maximum reach distance.
    reach: f32,
    /// Maximum force for push/pull.
    max_force: f32,
    /// Maximum number of objects that can be held.
    max_held: usize,
    /// Currently held object indices.
    held_objects: Vec<usize>,
    /// Energy cost per action.
    energy_per_action: f32,
}

impl ManipulationActuator {
    /// Create a new manipulation actuator.
    ///
    /// # Arguments
    ///
    /// * `reach` - Maximum reach distance
    /// * `max_force` - Maximum force for push/pull actions
    ///
    /// # Returns
    ///
    /// A new `ManipulationActuator`.
    pub fn new(reach: f32, max_force: f32) -> Self {
        Self {
            reach,
            max_force,
            max_held: 1,
            held_objects: Vec::new(),
            energy_per_action: 0.05,
        }
    }

    /// Set the maximum number of held objects (builder pattern).
    pub fn with_max_held(mut self, max: usize) -> Self {
        self.max_held = max;
        self
    }

    /// Get the reach distance.
    pub fn reach(&self) -> f32 {
        self.reach
    }

    /// Get the maximum force.
    pub fn max_force(&self) -> f32 {
        self.max_force
    }

    /// Check if holding any objects.
    pub fn is_holding(&self) -> bool {
        !self.held_objects.is_empty()
    }

    /// Get the held object indices.
    pub fn held_objects(&self) -> &[usize] {
        &self.held_objects
    }
}

impl Actuator for ManipulationActuator {
    fn actuator_type(&self) -> ActuatorType {
        ActuatorType::Manipulation
    }

    fn execute(&self, command: &ActuatorCommand, _state: &mut AgentState) {
        // Manipulation effects are handled by the simulation/world layer.
        // This actuator just validates and queues the action.
        if let ActuatorCommand::Manipulation(cmd) = command {
            match cmd.action {
                ManipulationAction::PickUp => {
                    // Would check if we can pick up (reach, capacity)
                }
                ManipulationAction::Drop => {
                    // Would release held object
                }
                ManipulationAction::Push | ManipulationAction::Pull => {
                    // Would apply force to target (clamped to max_force)
                }
                ManipulationAction::Activate => {
                    // Would trigger activation on target
                }
            }
        }
    }

    fn can_execute(&self, command: &ActuatorCommand) -> bool {
        match command {
            ActuatorCommand::Manipulation(cmd) => {
                match cmd.action {
                    ManipulationAction::PickUp => self.held_objects.len() < self.max_held,
                    ManipulationAction::Drop => !self.held_objects.is_empty(),
                    ManipulationAction::Push | ManipulationAction::Pull => {
                        cmd.force.abs() <= self.max_force
                    }
                    ManipulationAction::Activate => true,
                }
            }
            _ => false,
        }
    }

    fn energy_cost(&self, command: &ActuatorCommand) -> f32 {
        match command {
            ActuatorCommand::Manipulation(cmd) => {
                match cmd.action {
                    ManipulationAction::Push | ManipulationAction::Pull => {
                        self.energy_per_action * (1.0 + cmd.force / self.max_force)
                    }
                    _ => self.energy_per_action,
                }
            }
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motor_actuator() {
        let motor = MotorActuator::new(10.0);
        assert_eq!(motor.max_force(), 10.0);
        assert_eq!(motor.actuator_type(), ActuatorType::Motor);

        let cmd = MotorCommand::force(5.0, 0.0, 0.0);
        assert!(motor.can_execute(&ActuatorCommand::Motor(cmd)));
    }

    #[test]
    fn test_motor_clamp() {
        let motor = MotorActuator::new(10.0);
        let mut state = AgentState::new();

        // Apply excessive force
        let cmd = ActuatorCommand::Motor(MotorCommand::force(20.0, 0.0, 0.0));
        motor.execute(&cmd, &mut state);

        // Force should be clamped to 10.0
        let force_mag = (state.force[0] * state.force[0]
            + state.force[1] * state.force[1]
            + state.force[2] * state.force[2])
        .sqrt();
        assert!((force_mag - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_communication_actuator() {
        let comm = CommunicationActuator::new(15.0);
        assert_eq!(comm.max_range(), 15.0);
        assert_eq!(comm.actuator_type(), ActuatorType::Communication);

        let cmd = CommunicationCommand::broadcast(vec![1.0, 2.0, 3.0]);
        assert!(comm.can_execute(&ActuatorCommand::Communication(cmd)));
    }

    #[test]
    fn test_manipulation_actuator() {
        let manip = ManipulationActuator::new(2.0, 5.0);
        assert_eq!(manip.reach(), 2.0);
        assert_eq!(manip.max_force(), 5.0);
        assert!(!manip.is_holding());

        // Can't drop when not holding
        let drop_cmd = ManipulationCommand::drop_item();
        assert!(!manip.can_execute(&ActuatorCommand::Manipulation(drop_cmd)));
    }
}
