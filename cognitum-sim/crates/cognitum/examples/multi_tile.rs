//! Example demonstrating multi-tile programming

use cognitum::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Cognitum SDK - Multi-Tile Example\n");

    // Create minimal 2x2 grid (4 tiles) for this example
    let config = CognitumConfig::builder().tiles(4).build()?;

    let mut cognitum = CognitumSDK::with_config(config)?;

    // Simple program that each tile will run
    // This program pushes some values and halts
    let program = vec![
        0x30, // ZERO
        0x31, // ONE
        0x31, // ONE
        0x28, // ADD (1 + 1 = 2)
        0x34, // HALT
    ];

    // Load the same program into all 4 tiles
    println!("Loading program into 4 tiles...");
    for tile_id in 0..4 {
        cognitum.load_program(TileId(tile_id), &program)?;
        println!("  Tile {} loaded", tile_id);
    }

    println!("\nRunning simulation with 4 parallel tiles...\n");

    // Run all tiles in parallel
    let results = cognitum.run().await?;

    // Display results
    println!("{}", results);

    println!(
        "\nAll {} tiles executed the same program in parallel!",
        results.halted_tiles
    );

    Ok(())
}
