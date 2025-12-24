//! Cognitum CLI library
//!
//! Exposes configuration and command modules for testing

pub mod config;
pub mod commands;

pub use commands::{benchmark, debug, inspect, load, run};
