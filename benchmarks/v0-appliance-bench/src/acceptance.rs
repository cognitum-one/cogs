//! Acceptance Test Runner
//!
//! Runs all three acceptance tests and produces a combined verdict.

fn main() {
    eprintln!("=== Cognitum v0 Appliance Emulator -- Acceptance Tests ===");
    eprintln!();

    // In production, these call the real harness functions.
    // Here we define the entry point structure.

    let tests: Vec<(&str, fn() -> v0_appliance_bench::harness::TestVerdict)> = vec![
        ("Test 1: 30-min endurance, zero protocol errors",
         v0_appliance_bench::harness::acceptance_test_1_endurance),
        ("Test 2: Coherence gate response",
         v0_appliance_bench::harness::acceptance_test_2_coherence_gate),
        ("Test 3: Tile failure recovery",
         v0_appliance_bench::harness::acceptance_test_3_tile_failure_recovery),
    ];

    let mut all_passed = true;

    for (desc, test_fn) in &tests {
        eprintln!("--- Running: {} ---", desc);
        let verdict = test_fn();
        eprintln!("{}", verdict.summary());
        eprintln!();
        if !verdict.passed {
            all_passed = false;
        }
    }

    if all_passed {
        eprintln!("=== ALL ACCEPTANCE TESTS PASSED ===");
        std::process::exit(0);
    } else {
        eprintln!("=== ACCEPTANCE TESTS FAILED ===");
        std::process::exit(1);
    }
}
