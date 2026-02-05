//! CLI command implementations
//!
//! This module contains all command handlers for the agentvm CLI.

pub mod benchmark;
pub mod evidence;
pub mod replay;
pub mod reset;
pub mod run;
pub mod snapshot;

// Re-export command handlers
pub use benchmark::handle_benchmark;
pub use evidence::handle_evidence;
pub use replay::handle_replay;
pub use reset::handle_reset;
pub use run::handle_run;
pub use snapshot::handle_snapshot;
