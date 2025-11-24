use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Cognitum ASIC Simulator"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("cognitum-cli"));
}

#[test]
fn test_load_command_help() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["load", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Load and inspect a program"));
}

#[test]
fn test_run_command_help() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["run", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Run a program on the simulator"));
}

#[test]
fn test_debug_command_help() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["debug", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Debug mode with breakpoints"));
}

#[test]
fn test_inspect_command() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["inspect", "--tiles"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Tile States"));
}

#[test]
fn test_benchmark_unknown_suite() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["benchmark", "--suite", "unknown"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Unknown suite"));
}

#[test]
fn test_load_missing_program() {
    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&["load", "--program", "nonexistent.bin", "--tile", "0"]);

    cmd.assert().failure();
}

#[test]
fn test_load_command_with_program() {
    let temp_dir = TempDir::new().unwrap();
    let program_path = temp_dir.path().join("test.bin");

    // Create a simple test program
    let program = vec![0x30, 0x31, 0x28, 0x34]; // ZERO, ONE, ADD, HALT
    fs::write(&program_path, program).unwrap();

    let mut cmd = Command::cargo_bin("cognitum").unwrap();
    cmd.args(&[
        "load",
        "--program",
        program_path.to_str().unwrap(),
        "--tile",
        "0",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Program Information"))
        .stdout(predicate::str::contains("4 bytes"));
}

#[test]
fn test_config_parsing() {
    use cognitum_cli::config::CognitumCliConfig;

    let config = CognitumCliConfig::default();
    assert_eq!(config.hardware.tiles, 256);
    assert_eq!(config.performance.worker_threads, 8);
    assert!(config.simulation.event_driven);
}
