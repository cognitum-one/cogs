//! Request handlers

pub mod health;
pub mod program;
pub mod simulation;
pub mod websocket;

pub use health::health_check;
pub use program::{create_program, get_program};
pub use simulation::{
    create_simulation, delete_simulation, get_simulation, list_simulations,
    run_simulation, get_status, get_results,
};
pub use websocket::simulation_stream;
