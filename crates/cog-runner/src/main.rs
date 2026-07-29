//! CLI: run a cog under its declared [console] limits and emit the evidence.
//!
//!   cog-runner --cog-toml src/cogs/<id>/cog.toml --binary <path> --command "--once"
//!
//! Exit 0 only when every declared limit held. A non-zero exit means the run is NOT usable as
//! isolation evidence, which is the outcome the release gate needs to be able to see.

use std::path::PathBuf;
use std::process::exit;

use cog_runner::{load_policy, run_under_policy};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let manifest = match arg("--cog-toml") { Some(v) => PathBuf::from(v), None => { eprintln!("--cog-toml required"); exit(2); } };
    let binary = match arg("--binary") { Some(v) => PathBuf::from(v), None => { eprintln!("--binary required"); exit(2); } };
    let command = arg("--command").unwrap_or_else(|| "--once".to_string());
    let cog_id = arg("--cog-id").unwrap_or_else(|| "unknown".to_string());

    let policy = match load_policy(&manifest) {
        Ok(p) => p,
        Err(e) => { eprintln!("policy refused: {e}"); exit(3); }
    };

    match run_under_policy(&cog_id, &binary, &command, &policy) {
        Err(refusal) => { eprintln!("refused: {refusal}"); exit(4); }
        Ok(ev) => {
            println!("{}", serde_json::to_string_pretty(&ev).unwrap());
            // Exit non-zero when a limit was breached. The evidence still prints — an operator
            // needs to see WHICH limit failed — but the process reports failure so a workflow
            // cannot mistake a breached run for a clean one.
            if !ev.all_limits_held() { exit(1); }
        }
    }
}
