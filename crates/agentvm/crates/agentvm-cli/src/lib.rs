//! Agentic VM CLI Library
//!
//! This crate provides the command-line interface for Agentic VM,
//! enabling accountable agent capsule management with evidence generation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod commands;
pub mod config;
pub mod error;
pub mod output;

pub use config::Config;
pub use error::{exit_codes, CliError, Result};
pub use output::OutputFormat;
