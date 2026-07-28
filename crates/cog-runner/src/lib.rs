//! Enforces each cog's declared `[console]` limits, and produces the evidence that it did.
//!
//! # Why this exists
//!
//! All 109 `cog.toml` files declare a `[console]` block — `allowed_commands`,
//! `max_runtime_secs`, `output_limit_bytes` — and before this crate **nothing in the repository
//! enforced any of it** (109 declarations, 0 enforcement sites, measured with a control). The
//! limits were documentation.
//!
//! `cognitum-one/website`'s release gate (ADR-113 §12) refuses to mark a cog `available`
//! without an `isolationEvidenceDigest`. For an edge cog the tenancy boundary is the DEVICE the
//! customer owns, so "isolated" means exactly these three constraints holding at runtime. With
//! nothing enforcing them there was no evidence to produce, which is why that gate has never
//! been satisfiable.
//!
//! # Why enforcement lives in the RUNNER and not in the cog
//!
//! Two of the three constraints cannot be enforced by the cog at all:
//!
//! - `allowed_commands` — a process cannot restrict how it was invoked. By the time `main`
//!   runs, the argv it was given has already been chosen. Only the spawner can refuse.
//! - `output_limit_bytes` — the writer cannot bound what the reader accepts. Only the process
//!   holding the pipe can truncate.
//!
//! And the third should not be: a cog enforcing its own `max_runtime_secs` is self-attestation.
//! A cog that ignores its limit — through a bug or otherwise — is precisely the case the limit
//! exists to catch, and it would be the component reporting compliance. Evidence has to come
//! from something the cog does not control.
//!
//! # Fail closed
//!
//! Every ambiguity refuses. A `cog.toml` with no `[console]` block is refused rather than
//! treated as unconstrained; an empty `allowed_commands` permits nothing rather than
//! everything; an unparseable manifest refuses. Absent evidence never means permitted.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The `[console]` block, as declared in `cog.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ConsolePolicy {
    /// Exact command strings the console may invoke. An allowlist, never a pattern: a pattern
    /// that is wrong is permissive, and this is the boundary that decides what runs at all.
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub max_runtime_secs: u64,
    #[serde(default)]
    pub output_limit_bytes: usize,
}

#[derive(Debug, Deserialize)]
struct CogManifest {
    console: Option<ConsolePolicy>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PolicyError {
    Unreadable(String),
    Unparseable(String),
    /// No `[console]` block. Refused rather than defaulted — a cog with no declared limits is
    /// not a cog with no limits, it is a cog whose limits nobody stated.
    NoConsoleBlock,
    /// A limit present but meaningless. Zero seconds cannot bound anything, and a zero output
    /// budget would truncate every run to nothing.
    InvalidLimit(&'static str),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::Unreadable(p) => write!(f, "cannot read manifest: {p}"),
            PolicyError::Unparseable(e) => write!(f, "cannot parse manifest: {e}"),
            PolicyError::NoConsoleBlock => write!(
                f,
                "cog.toml declares no [console] block; refusing to run a cog whose limits nobody stated"
            ),
            PolicyError::InvalidLimit(w) => write!(f, "[console].{w} is not a usable limit"),
        }
    }
}

/// Read and validate a cog's console policy.
pub fn load_policy(manifest_path: &Path) -> Result<ConsolePolicy, PolicyError> {
    let raw = std::fs::read_to_string(manifest_path)
        .map_err(|_| PolicyError::Unreadable(manifest_path.display().to_string()))?;
    let manifest: CogManifest =
        toml::from_str(&raw).map_err(|e| PolicyError::Unparseable(e.to_string()))?;
    let policy = manifest.console.ok_or(PolicyError::NoConsoleBlock)?;

    if policy.max_runtime_secs == 0 {
        return Err(PolicyError::InvalidLimit("max_runtime_secs"));
    }
    if policy.output_limit_bytes == 0 {
        return Err(PolicyError::InvalidLimit("output_limit_bytes"));
    }
    Ok(policy)
}

/// Why a run was refused before the binary was ever spawned.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The requested argv is not in the allowlist. Includes the empty-allowlist case: an empty
    /// list permits NOTHING. Reading it as "unrestricted" is the fail-open reading, and this
    /// is the gate that decides whether arbitrary argv reaches a customer's device.
    CommandNotAllowed(String),
    Policy(PolicyError),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::CommandNotAllowed(c) => write!(
                f,
                "command {c:?} is not in [console].allowed_commands; refused before spawn"
            ),
            Refusal::Policy(e) => write!(f, "{e}"),
        }
    }
}

/// What actually happened, and what was enforced doing it.
#[derive(Debug, Serialize)]
pub struct RunEvidence {
    pub cog_id: String,
    pub command: String,
    pub exit_code: Option<i32>,
    /// True when the runner killed the process at the deadline rather than it exiting.
    pub killed_at_deadline: bool,
    pub elapsed_ms: u128,
    pub max_runtime_secs: u64,
    pub output_bytes: usize,
    pub output_limit_bytes: usize,
    /// True when output reached the cap and was cut. Recorded rather than inferred from
    /// `output_bytes == output_limit_bytes`, which is ambiguous at exactly the boundary.
    pub output_truncated: bool,
    pub within_runtime_limit: bool,
    pub within_output_limit: bool,
}

impl RunEvidence {
    /// Whether every declared constraint held. A run is only isolation evidence if it is.
    pub fn all_limits_held(&self) -> bool {
        self.within_runtime_limit && self.within_output_limit
    }
}

/// Whether `command` is permitted, matched EXACTLY against the allowlist.
///
/// Exact match, not prefix or substring: `--once` must not authorise `--once; rm -rf /`, and a
/// prefix rule would. The allowlist entries are whole invocations.
pub fn command_allowed(policy: &ConsolePolicy, command: &str) -> bool {
    policy.allowed_commands.iter().any(|c| c == command)
}

/// Spawn a cog under its declared limits.
///
/// The deadline is enforced by the runner: the child is killed when it passes, and the evidence
/// records that it was killed rather than that it exited. Output is read to the cap and the
/// remainder discarded, so a runaway writer cannot exhaust the console's memory.
pub fn run_under_policy(
    cog_id: &str,
    binary: &Path,
    command: &str,
    policy: &ConsolePolicy,
) -> Result<RunEvidence, Refusal> {
    if !command_allowed(policy, command) {
        return Err(Refusal::CommandNotAllowed(command.to_string()));
    }

    let args: Vec<&str> = command.split_whitespace().collect();
    let started = Instant::now();
    let deadline = Duration::from_secs(policy.max_runtime_secs);

    let mut child = Command::new(binary)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Refusal::Policy(PolicyError::Unreadable(format!("{}: {e}", binary.display()))))?;

    let mut killed = false;
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {
                if started.elapsed() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    killed = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break None,
        }
    };
    let elapsed_ms = started.elapsed().as_millis();

    // Read to the cap PLUS ONE byte. Reading exactly the cap cannot distinguish "produced
    // exactly the limit" from "produced more and was cut" — the extra byte is what makes
    // `output_truncated` a fact rather than a guess.
    let mut buf = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.by_ref().take(policy.output_limit_bytes as u64 + 1).read_to_end(&mut buf);
    }
    if buf.len() <= policy.output_limit_bytes {
        if let Some(mut err) = child.stderr.take() {
            let remaining = policy.output_limit_bytes + 1 - buf.len();
            let _ = err.by_ref().take(remaining as u64).read_to_end(&mut buf);
        }
    }
    let output_truncated = buf.len() > policy.output_limit_bytes;
    if output_truncated {
        buf.truncate(policy.output_limit_bytes);
    }

    Ok(RunEvidence {
        cog_id: cog_id.to_string(),
        command: command.to_string(),
        exit_code,
        killed_at_deadline: killed,
        elapsed_ms,
        max_runtime_secs: policy.max_runtime_secs,
        output_bytes: buf.len(),
        output_limit_bytes: policy.output_limit_bytes,
        output_truncated,
        // The limit HELD if the process finished on its own. Being killed means the cog
        // exceeded its declared runtime — the limit did its job, but the cog did not honour it,
        // and that distinction is what an operator needs.
        within_runtime_limit: !killed,
        within_output_limit: !output_truncated,
    })
}
