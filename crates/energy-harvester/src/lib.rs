//! # Energy Harvester Micro-Agent
//!
//! A `no_std` duty-cycled micro-agent that runs from harvested ambient energy.
//! Sleeps >99.9% of the time, wakes in <50ms bursts to execute a Rust/WASM
//! micro-kernel gated by an energy budget ledger.
//!
//! ## Architecture
//!
//! - **PowerManager**: Controls PMIC load switch, sleep modes, GPIO enable
//! - **AdcReader**: Reads VSTOR voltage, harvester current, temperature
//! - **EnergyLedger**: Tracks harvested vs consumed energy over rolling windows
//! - **DutyCycleController**: Orchestrates the HARVEST→WAKE→EXECUTE→HARVEST FSM
//! - **WasmGate**: Bounded micro-kernel execution with watchdog enforcement
//! - **Telemetry**: Structured logging for energy instrumentation

#![cfg_attr(not(feature = "std"), no_std)]

pub mod adc;
pub mod config;
pub mod duty_cycle;
pub mod energy_ledger;
pub mod power_manager;
pub mod telemetry;
pub mod wasm_gate;

pub use config::HarvesterConfig;
pub use duty_cycle::{DutyCycleController, PowerState};
pub use energy_ledger::EnergyLedger;
pub use power_manager::PowerManager;
pub use wasm_gate::{ActionToken, WasmGate};
