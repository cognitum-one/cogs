//! Basic Cognitum SDK example - Hello World simulation

use cognitum::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Cognitum SDK - Hello World Example\n");

    // Create Cognitum simulator with default configuration (256 tiles)
    let mut cognitum = CognitumSDK::new()?;

    // Simple program: ZERO, ONE, ADD, HALT
    let program = vec![
        0x30, // ZERO - push 0 onto stack
        0x31, // ONE  - push 1 onto stack
        0x28, // ADD  - add top two stack values
        0x34, // HALT - stop execution
    ];

    println!("Loading program ({} bytes) into Tile 0...", program.len());
    cognitum.load_program(TileId(0), &program)?;

    println!("Running simulation...\n");

    // Run the simulation
    let results = cognitum.run().await?;

    // Display results
    println!("{}", results);

    if results.is_success() {
        println!("\n✓ Simulation completed successfully!");
    } else {
        println!("\n✗ Simulation encountered errors");
    }

    Ok(())
}
