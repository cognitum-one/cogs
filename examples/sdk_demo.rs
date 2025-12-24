//! SDK Demo - Demonstrates using the Cognitum SDK bridge

use cognitum::sdk::{CognitumSimulator, SimulatorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Cognitum SDK Bridge Demo\n");

    // Create simulator with default configuration
    let config = SimulatorConfig::default();
    println!("Creating simulator with config: {:?}", config);

    let mut simulator = CognitumSimulator::new(config)?;
    println!("✓ Simulator created\n");

    // Create a simple program
    // LITERAL 42, HALT
    // LITERAL (0x08) = 0x08 << 26 = 0x20000000, with value in lower 26 bits
    // HALT (0x3F) = 0x3F << 26 = 0xFC000000
    let program: Vec<u8> = vec![
        0x2A, 0x00, 0x00, 0x20, // LITERAL 42 (little-endian)
        0x00, 0x00, 0x00, 0xFC, // HALT
    ];

    println!("Loading program ({} bytes)...", program.len());
    let program_handle = simulator.create_program(&program).await?;
    println!("✓ Program loaded: {:?}\n", program_handle);

    // Execute the program
    println!("Executing program (max 1000 cycles)...");
    let result = simulator.execute(Some(1000)).await?;
    println!("✓ Execution complete!");
    println!("  Cycles:       {}", result.cycles_executed);
    println!("  Instructions: {}", result.instructions_executed);
    println!("  Halted:       {}\n", result.halted);

    // Get simulator snapshot
    println!("Getting simulator state...");
    let snapshot = simulator.get_snapshot().await?;
    println!("✓ Snapshot:");
    println!("  Total cycles:       {}", snapshot.cycles);
    println!("  Total instructions: {}", snapshot.instructions);
    println!("  Active tiles:       {:?}\n", snapshot.active_tiles);

    // Reset and verify
    println!("Resetting simulator...");
    simulator.reset().await?;
    println!("✓ Simulator reset\n");

    println!("Demo completed successfully!");

    Ok(())
}
