//! Proof that each declared limit actually binds.
//!
//! Every test here pairs a POSITIVE case with a NEGATIVE CONTROL, because a runner that
//! permits everything and a runner that refuses everything both produce a green suite when
//! only one direction is checked. The constraint has to be shown to hold AND to release.
//!
//! These tests are the source of `isolationEvidenceDigest`. A run that cannot demonstrate the
//! limits binding is not isolation evidence, and the release gate is right to refuse it.

use std::fs;
use std::path::PathBuf;

use cog_runner::{command_allowed, load_policy, run_under_policy, ConsolePolicy, PolicyError, Refusal};

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("cog-runner-test-{name}"));
    let _ = fs::create_dir_all(&p);
    p
}

fn write_manifest(dir: &PathBuf, body: &str) -> PathBuf {
    let p = dir.join("cog.toml");
    fs::write(&p, body).unwrap();
    p
}

/// A shell script standing in for a cog binary, so the tests exercise real spawn/kill/pipe
/// behaviour rather than a mock of it.
fn write_fake_cog(dir: &PathBuf, name: &str, script: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    p
}

fn policy(cmds: &[&str], secs: u64, bytes: usize) -> ConsolePolicy {
    ConsolePolicy {
        allowed_commands: cmds.iter().map(|s| s.to_string()).collect(),
        max_runtime_secs: secs,
        output_limit_bytes: bytes,
    }
}

// ── allowed_commands ──────────────────────────────────────────────────────────

#[test]
fn permits_an_allowlisted_command() {
    let p = policy(&["--once", "--help"], 5, 1024);
    assert!(command_allowed(&p, "--once"));
    assert!(command_allowed(&p, "--help"));
}

#[test]
fn refuses_a_command_not_on_the_list() {
    let p = policy(&["--once"], 5, 1024);
    assert!(!command_allowed(&p, "--dump-secrets"));
}

#[test]
fn matches_exactly_and_not_by_prefix() {
    // The case a prefix rule would wave through. "--once" must not authorise "--once; rm -rf /",
    // and any substring or startsWith comparison would let it past.
    let p = policy(&["--once"], 5, 1024);
    assert!(!command_allowed(&p, "--once; rm -rf /"));
    assert!(!command_allowed(&p, "--once --interval 5"));
    assert!(!command_allowed(&p, "-"));
}

#[test]
fn an_empty_allowlist_permits_nothing() {
    // The fail-open reading — "no list means unrestricted" — is what this rejects. An empty
    // allowlist is a cog that may not be invoked at all, not one that may be invoked freely.
    let p = policy(&[], 5, 1024);
    assert!(!command_allowed(&p, "--once"));
    assert!(!command_allowed(&p, ""));
}

#[test]
fn refuses_before_spawning_anything() {
    // The refusal must happen BEFORE exec. A disallowed command that runs and is then judged
    // has already had its side effects.
    let d = tmp("no-spawn");
    let marker = d.join("SPAWNED");
    let _ = fs::remove_file(&marker);
    let bin = write_fake_cog(&d, "cog-marker", &format!("touch {}", marker.display()));

    let err = run_under_policy("marker", &bin, "--not-allowed", &policy(&["--once"], 5, 1024))
        .unwrap_err();
    assert_eq!(err, Refusal::CommandNotAllowed("--not-allowed".into()));
    assert!(!marker.exists(), "binary was spawned despite the command being refused");
}

// ── max_runtime_secs ──────────────────────────────────────────────────────────

#[test]
fn kills_a_cog_that_outlives_its_deadline() {
    let d = tmp("overrun");
    let bin = write_fake_cog(&d, "cog-slow", "sleep 30");
    let ev = run_under_policy("slow", &bin, "--once", &policy(&["--once"], 1, 4096)).unwrap();

    assert!(ev.killed_at_deadline, "a 30s cog survived a 1s deadline");
    assert!(!ev.within_runtime_limit);
    assert!(!ev.all_limits_held(), "a killed run must not read as isolation evidence");
    assert!(ev.elapsed_ms < 10_000, "kill took {}ms — the deadline did not bind", ev.elapsed_ms);
}

#[test]
fn lets_a_well_behaved_cog_finish() {
    // The control. Without it, a runner that killed everything instantly would pass the test
    // above and look correct.
    let d = tmp("quick");
    let bin = write_fake_cog(&d, "cog-quick", "echo done");
    let ev = run_under_policy("quick", &bin, "--once", &policy(&["--once"], 10, 4096)).unwrap();

    assert!(!ev.killed_at_deadline, "a fast cog was killed");
    assert!(ev.within_runtime_limit);
    assert_eq!(ev.exit_code, Some(0));
}

// ── output_limit_bytes ────────────────────────────────────────────────────────

#[test]
fn truncates_output_that_exceeds_the_budget() {
    let d = tmp("loud");
    let bin = write_fake_cog(&d, "cog-loud", "head -c 100000 /dev/zero | tr '\\0' 'x'");
    let ev = run_under_policy("loud", &bin, "--once", &policy(&["--once"], 10, 256)).unwrap();

    assert!(ev.output_truncated, "100KB of output was not truncated at a 256-byte budget");
    assert_eq!(ev.output_bytes, 256, "kept {} bytes past the cap", ev.output_bytes);
    assert!(!ev.within_output_limit);
    assert!(!ev.all_limits_held());
}

#[test]
fn leaves_output_under_the_budget_intact() {
    // The control against a runner that truncates unconditionally.
    let d = tmp("quiet");
    let bin = write_fake_cog(&d, "cog-quiet", "printf 'small'");
    let ev = run_under_policy("quiet", &bin, "--once", &policy(&["--once"], 10, 4096)).unwrap();

    assert!(!ev.output_truncated);
    assert_eq!(ev.output_bytes, 5);
    assert!(ev.within_output_limit);
    assert!(ev.all_limits_held(), "a compliant run should be usable as evidence");
}

// ── policy loading fails closed ───────────────────────────────────────────────

#[test]
fn refuses_a_manifest_with_no_console_block() {
    let d = tmp("no-console");
    let m = write_manifest(&d, "[cog]\nid = \"x\"\nname = \"X\"\n");
    assert_eq!(load_policy(&m).unwrap_err(), PolicyError::NoConsoleBlock);
}

#[test]
fn refuses_limits_that_cannot_bound_anything() {
    let d = tmp("zero-limits");
    let m = write_manifest(&d, "[console]\nallowed_commands = [\"--once\"]\nmax_runtime_secs = 0\noutput_limit_bytes = 100\n");
    assert_eq!(load_policy(&m).unwrap_err(), PolicyError::InvalidLimit("max_runtime_secs"));

    let m2 = write_manifest(&tmp("zero-bytes"), "[console]\nallowed_commands = [\"--once\"]\nmax_runtime_secs = 5\noutput_limit_bytes = 0\n");
    assert_eq!(load_policy(&m2).unwrap_err(), PolicyError::InvalidLimit("output_limit_bytes"));
}

#[test]
fn loads_a_real_cog_manifest() {
    // Against the shape actually committed in this repo, so the parser cannot drift from the
    // 109 manifests it exists to read.
    let d = tmp("real");
    let m = write_manifest(&d, r#"
[cog]
id = "breathing-sync"
name = "Breathing Sync"

[console]
allowed_commands = ["--once", "--once --interval 5", "--help"]
max_runtime_secs = 15
output_limit_bytes = 65536
"#);
    let p = load_policy(&m).unwrap();
    assert_eq!(p.max_runtime_secs, 15);
    assert_eq!(p.output_limit_bytes, 65536);
    assert!(command_allowed(&p, "--once --interval 5"));
    assert!(!command_allowed(&p, "--once --interval 999"));
}
