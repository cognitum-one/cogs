//! # AGENCY: Agent-Based Simulation Layer
//!
//! The AGENCY module provides an agent-based abstraction layer on top of FXNN's
//! molecular dynamics engine. It enables the simulation of embodied agents that
//! can sense, act, and learn within a physical environment.
//!
//! ## Overview
//!
//! This module implements Layer 2 of the FXNN architecture, bridging the gap between
//! low-level physics simulation and high-level intelligent behavior. Key components:
//!
//! - **[`Agent`]**: Embodied entities with physical avatars, sensors, and actuators
//! - **[`Sensor`]**: Perception modules (vision, auditory, tactile, proprioceptive)
//! - **[`Actuator`]**: Action modules (motor, communication, manipulation)
//! - **[`PolicyNetwork`]**: Decision-making interfaces (simple rules to neural networks)
//! - **[`Goal`]**: Objective functions and reward systems
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Agent Layer (AGENCY)                     │
//! ├──────────────┬──────────────┬────────────────┬──────────────┤
//! │   Sensors    │   Policy     │    Goals       │  Actuators   │
//! │  (perceive)  │  (decide)    │   (evaluate)   │    (act)     │
//! └──────────────┴──────────────┴────────────────┴──────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  Physics Layer (FXNN Core)                   │
//! │              Atoms, Forces, Integration, Collisions          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use fxnn::agency::{Agent, AgentBody, VisionSensor, MotorActuator, SimplePolicy};
//! use fxnn::agency::goal::{DistanceGoal, GoalType};
//!
//! // Create an agent body
//! let body = AgentBody::sphere(1.0, 1.0);  // radius=1.0, mass=1.0
//!
//! // Configure sensors
//! let vision = VisionSensor::new(std::f32::consts::FRAC_PI_2, 10.0);  // 90 FOV, 10 range
//!
//! // Configure actuators
//! let motor = MotorActuator::new(5.0);  // max_force=5.0
//!
//! // Create agent
//! let mut agent = Agent::new(0, body)
//!     .with_sensor(Box::new(vision))
//!     .with_actuator(Box::new(motor));
//!
//! // Add a goal
//! let target = [10.0, 0.0, 0.0];
//! agent.add_goal(Box::new(DistanceGoal::reach_position(target, 0.5)));
//! ```
//!
//! ## Sensor Types
//!
//! | Sensor | Description | Output |
//! |--------|-------------|--------|
//! | [`VisionSensor`] | Cone-based field of view with occlusion | Visible entities |
//! | [`AuditorySensor`] | Omnidirectional sound detection | Sound intensities |
//! | [`TactileSensor`] | Contact/collision detection | Contact points & forces |
//! | [`ProprioceptiveSensor`] | Internal state awareness | Position, velocity, forces |
//! | [`CommunicationSensor`] | Message reception | Incoming messages |
//!
//! ## Actuator Types
//!
//! | Actuator | Description | Effect |
//! |----------|-------------|--------|
//! | [`MotorActuator`] | Apply forces/velocities | Movement |
//! | [`CommunicationActuator`] | Send messages | Information transfer |
//! | [`ManipulationActuator`] | Interact with objects | Pick up, push, etc. |
//!
//! ## Policy Networks
//!
//! Policies map sensor observations to actuator commands:
//!
//! - **[`SimplePolicy`]**: Hardcoded rule-based behavior
//! - **[`NeuralPolicy`]**: Neural network-based decision making (FXNN brain)
//!
//! ## Goals and Rewards
//!
//! Goals provide objective functions for agent behavior:
//!
//! - **Distance goals**: Reach or avoid positions
//! - **Resource goals**: Collect or consume resources
//! - **Social goals**: Interact with other agents
//! - **Intrinsic motivation**: Curiosity-driven exploration
//!
//! ## References
//!
//! - Wooldridge, M. "An Introduction to MultiAgent Systems" (2009)
//! - Sutton & Barto "Reinforcement Learning: An Introduction" (2018)
//! - Oudeyer & Kaplan "What is Intrinsic Motivation?" (2007)

mod agent;
mod sensor;
mod actuator;
mod policy;
pub mod goal;

pub use agent::{Agent, AgentBody, AgentState, AgentId};
pub use sensor::{
    Sensor, SensorReading, SensorType,
    VisionSensor, VisionReading, VisibleEntity,
    AuditorySensor, AuditoryReading, SoundSource,
    TactileSensor, TactileReading, ContactPoint,
    ProprioceptiveSensor, ProprioceptiveReading,
    CommunicationSensor, Message,
};
pub use actuator::{
    Actuator, ActuatorCommand, ActuatorType,
    MotorActuator, MotorCommand,
    CommunicationActuator, CommunicationCommand,
    ManipulationActuator, ManipulationCommand, ManipulationTarget,
};
pub use policy::{PolicyNetwork, PolicyOutput, SimplePolicy, NeuralPolicy};
pub use goal::{Goal, GoalStatus, RewardFunction, IntrinsicMotivation};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let body = AgentBody::sphere(1.0, 1.0);
        let agent = Agent::new(0, body);
        assert_eq!(agent.id(), 0);
        assert_eq!(agent.state().position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_sensor_types() {
        let vision = VisionSensor::new(std::f32::consts::FRAC_PI_2, 10.0);
        assert_eq!(vision.sensor_type(), SensorType::Vision);
        assert_eq!(vision.range(), 10.0);
    }

    #[test]
    fn test_actuator_creation() {
        let motor = MotorActuator::new(5.0);
        assert_eq!(motor.actuator_type(), ActuatorType::Motor);
        assert_eq!(motor.max_force(), 5.0);
    }
}
