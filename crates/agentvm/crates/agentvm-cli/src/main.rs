//! Agentic VM CLI - Accountable Agent Runtime
//!
//! Command-line interface for managing agent capsules with
//! evidence generation, capability-based security, and deterministic replay.
//!
//! # Usage
//!
//! ```bash
//! # Run a command in an agent capsule
//! agentvm run claude code --evidence --workspace /path/to/repo
//!
//! # Reset capsule to a snapshot
//! agentvm reset --from-snapshot <id>
//!
//! # Manage snapshots
//! agentvm snapshot create --name "before-refactor"
//! agentvm snapshot list
//! agentvm snapshot delete <id>
//!
//! # Query and verify evidence
//! agentvm evidence get <run_id>
//! agentvm evidence verify /path/to/evidence.json
//! agentvm evidence export --format json --output audit.json
//!
//! # Replay from evidence
//! agentvm replay /path/to/evidence.json --verify-effects
//!
//! # Run benchmarks
//! agentvm benchmark run --iterations 30 --task "npm test"
//! agentvm benchmark verify report.json --p95-improvement 2.0
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod error;
mod output;

use commands::{
    benchmark::{handle_benchmark, BenchmarkAction, BenchmarkArgs},
    evidence::{handle_evidence, EvidenceAction, EvidenceArgs},
    replay::{handle_replay, ReplayArgs},
    reset::{handle_reset, ResetArgs},
    run::{handle_run, RunArgs},
    snapshot::{handle_snapshot, SnapshotAction, SnapshotArgs},
};
use config::Config;
use error::{exit_codes, CliError, Result};
use output::OutputFormat;

/// Agentic VM CLI - Accountable Agent Runtime
#[derive(Parser)]
#[command(name = "agentvm")]
#[command(author = "Cognitum Architecture Team")]
#[command(version)]
#[command(about = "Agentic VM - Accountable Agent Runtime", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Configuration file path
    #[arg(long, global = true, env = "AGENTVM_CONFIG")]
    config: Option<PathBuf>,

    /// Output format (text, json, table)
    #[arg(long, short = 'o', global = true, default_value = "text")]
    output: String,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Verbose output (-v, -vv, -vvv)
    #[arg(long, short, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Quiet mode (suppress non-essential output)
    #[arg(long, short, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a command in an agent capsule
    Run {
        /// Command to run (e.g., "claude code", "codex")
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,

        /// Enable evidence generation
        #[arg(long)]
        evidence: bool,

        /// Workspace path
        #[arg(long, short = 'w', default_value = ".")]
        workspace: PathBuf,

        /// Capsule manifest path
        #[arg(long, short = 'm')]
        manifest: Option<PathBuf>,

        /// Timeout in seconds
        #[arg(long, short = 't')]
        timeout: Option<u64>,

        /// Dry run (don't execute, show what would be done)
        #[arg(long)]
        dry_run: bool,
    },

    /// Reset capsule to a snapshot state
    Reset {
        /// Snapshot ID to restore from
        #[arg(long, required = true)]
        from_snapshot: String,

        /// Preserve workspace contents
        #[arg(long)]
        preserve_workspace: bool,

        /// Dry run (don't execute, show what would be done)
        #[arg(long)]
        dry_run: bool,

        /// Force reset without confirmation
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Manage snapshots
    Snapshot {
        #[command(subcommand)]
        action: SnapshotSubcommand,
    },

    /// Query and manage evidence bundles
    Evidence {
        #[command(subcommand)]
        action: EvidenceSubcommand,
    },

    /// Replay execution from evidence bundle
    Replay {
        /// Path to evidence bundle
        evidence: PathBuf,

        /// Verify effects match original
        #[arg(long)]
        verify_effects: bool,

        /// Workspace path for replay
        #[arg(long, short = 'w')]
        workspace: Option<PathBuf>,

        /// Dry run (don't execute, show what would be done)
        #[arg(long)]
        dry_run: bool,
    },

    /// Run benchmarks and verify criteria
    Benchmark {
        #[command(subcommand)]
        action: BenchmarkSubcommand,
    },

    /// Show configuration
    Config {
        /// Show default configuration
        #[arg(long)]
        default: bool,

        /// Initialize configuration file
        #[arg(long)]
        init: bool,
    },
}

#[derive(Subcommand)]
enum SnapshotSubcommand {
    /// Create a new snapshot
    Create {
        /// Human-readable name for the snapshot
        #[arg(long, short = 'n')]
        name: Option<String>,

        /// Capsule ID to snapshot
        #[arg(long, short = 'c')]
        capsule: Option<String>,

        /// Description
        #[arg(long, short = 'd')]
        description: Option<String>,

        /// Include memory snapshot
        #[arg(long)]
        memory: bool,
    },

    /// List all snapshots
    List {
        /// Filter by capsule ID
        #[arg(long, short = 'c')]
        capsule: Option<String>,

        /// Maximum number of results
        #[arg(long, short = 'l')]
        limit: Option<usize>,
    },

    /// Delete a snapshot
    Delete {
        /// Snapshot ID to delete
        id: String,

        /// Force deletion without confirmation
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Show snapshot details
    Show {
        /// Snapshot ID
        id: String,
    },
}

#[derive(Subcommand)]
enum EvidenceSubcommand {
    /// Get evidence by run ID
    Get {
        /// Run ID
        run_id: String,
    },

    /// Query evidence by criteria
    Query {
        /// Capsule ID filter
        #[arg(long, short = 'c')]
        capsule: Option<String>,

        /// Start time (ISO 8601)
        #[arg(long)]
        start: Option<String>,

        /// End time (ISO 8601)
        #[arg(long)]
        end: Option<String>,

        /// Maximum results
        #[arg(long, short = 'l')]
        limit: Option<usize>,
    },

    /// Verify evidence integrity
    Verify {
        /// Path to evidence bundle
        path: PathBuf,
    },

    /// Export evidence for audit
    Export {
        /// Export format (json, csv, siem)
        #[arg(long, short = 'f', default_value = "json")]
        format: String,

        /// Output path
        #[arg(long, short = 'o', required = true)]
        output: PathBuf,

        /// Run IDs to export (empty for all)
        #[arg(long)]
        run_ids: Vec<String>,

        /// Start time filter
        #[arg(long)]
        start: Option<String>,

        /// End time filter
        #[arg(long)]
        end: Option<String>,
    },
}

#[derive(Subcommand)]
enum BenchmarkSubcommand {
    /// Run benchmark suite
    Run {
        /// Number of iterations
        #[arg(long, short = 'n', default_value = "30")]
        iterations: u32,

        /// Task to benchmark
        #[arg(long, short = 't', required = true)]
        task: String,

        /// Workspace path
        #[arg(long, short = 'w')]
        workspace: Option<PathBuf>,

        /// Output path for results
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,

        /// Warmup iterations
        #[arg(long)]
        warmup: Option<u32>,
    },

    /// Analyze benchmark results
    Analyze {
        /// Evidence files to analyze
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Output path for report
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    /// Verify benchmark passes criteria
    Verify {
        /// Benchmark report path
        report: PathBuf,

        /// Required p95 improvement factor
        #[arg(long, default_value = "2.0")]
        p95_improvement: f64,

        /// Maximum coefficient of variation
        #[arg(long, default_value = "0.2")]
        cov_threshold: f64,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Setup logging
    setup_logging(cli.verbose, cli.quiet);

    // Load configuration
    let config = match load_config(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            return ExitCode::from(exit_codes::CONFIG_ERROR as u8);
        }
    };

    // Parse output format
    let output_format = match cli.output.parse::<OutputFormat>() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Invalid output format: {}", e);
            return ExitCode::from(exit_codes::ERROR as u8);
        }
    };

    // Execute command
    let result = match cli.command {
        Commands::Run {
            command,
            evidence,
            workspace,
            manifest,
            timeout,
            dry_run,
        } => {
            handle_run(
                RunArgs {
                    command,
                    evidence,
                    workspace,
                    manifest,
                    output_format,
                    timeout,
                    dry_run,
                },
                &config,
            )
            .await
        }

        Commands::Reset {
            from_snapshot,
            preserve_workspace,
            dry_run,
            force,
        } => {
            handle_reset(
                ResetArgs {
                    from_snapshot,
                    preserve_workspace,
                    output_format,
                    dry_run,
                    force,
                },
                &config,
            )
            .await
        }

        Commands::Snapshot { action } => {
            let snapshot_action = match action {
                SnapshotSubcommand::Create {
                    name,
                    capsule,
                    description,
                    memory,
                } => SnapshotAction::Create {
                    name,
                    capsule,
                    description,
                    include_memory: memory,
                },
                SnapshotSubcommand::List { capsule, limit } => {
                    SnapshotAction::List { capsule, limit }
                }
                SnapshotSubcommand::Delete { id, force } => {
                    SnapshotAction::Delete { id, force }
                }
                SnapshotSubcommand::Show { id } => SnapshotAction::Show { id },
            };

            handle_snapshot(
                SnapshotArgs {
                    action: snapshot_action,
                    output_format,
                },
                &config,
            )
            .await
        }

        Commands::Evidence { action } => {
            let evidence_action = match action {
                EvidenceSubcommand::Get { run_id } => EvidenceAction::Get { run_id },
                EvidenceSubcommand::Query {
                    capsule,
                    start,
                    end,
                    limit,
                } => EvidenceAction::Query {
                    capsule,
                    start,
                    end,
                    limit,
                },
                EvidenceSubcommand::Verify { path } => EvidenceAction::Verify { path },
                EvidenceSubcommand::Export {
                    format,
                    output,
                    run_ids,
                    start,
                    end,
                } => EvidenceAction::Export {
                    format,
                    output,
                    run_ids,
                    start,
                    end,
                },
            };

            handle_evidence(
                EvidenceArgs {
                    action: evidence_action,
                    output_format,
                },
                &config,
            )
            .await
        }

        Commands::Replay {
            evidence,
            verify_effects,
            workspace,
            dry_run,
        } => {
            handle_replay(
                ReplayArgs {
                    evidence,
                    verify_effects,
                    output_format,
                    workspace,
                    dry_run,
                },
                &config,
            )
            .await
        }

        Commands::Benchmark { action } => {
            let benchmark_action = match action {
                BenchmarkSubcommand::Run {
                    iterations,
                    task,
                    workspace,
                    output,
                    warmup,
                } => BenchmarkAction::Run {
                    iterations,
                    task,
                    workspace,
                    output,
                    warmup,
                },
                BenchmarkSubcommand::Analyze { files, output } => {
                    BenchmarkAction::Analyze { files, output }
                }
                BenchmarkSubcommand::Verify {
                    report,
                    p95_improvement,
                    cov_threshold,
                } => BenchmarkAction::Verify {
                    report,
                    p95_improvement,
                    cov_threshold,
                },
            };

            handle_benchmark(
                BenchmarkArgs {
                    action: benchmark_action,
                    output_format,
                },
                &config,
            )
            .await
        }

        Commands::Config { default, init } => handle_config(&config, default, init, &cli.no_color),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !cli.quiet {
                eprintln!("Error: {}", e);
            }
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

/// Setup logging based on verbosity level
fn setup_logging(verbose: u8, quiet: bool) {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).compact())
        .with(filter)
        .init();
}

/// Load configuration
fn load_config(cli: &Cli) -> Result<Config> {
    let mut config = if let Some(path) = &cli.config {
        Config::load_from_file(path)?
    } else {
        Config::load()?
    };

    // Apply CLI overrides
    if cli.no_color {
        config.general.color = false;
    }

    // Ensure directories exist
    config.ensure_directories()?;

    Ok(config)
}

/// Handle config command
fn handle_config(config: &Config, default: bool, init: bool, no_color: &bool) -> Result<()> {
    use colored::Colorize;

    if default {
        let default_config = Config::default();
        let content = toml::to_string_pretty(&default_config)
            .map_err(|e| CliError::Config(e.to_string()))?;
        println!("{}", content);
        return Ok(());
    }

    if init {
        let path = Config::user_config_path()
            .ok_or_else(|| CliError::Config("Could not determine config path".to_string()))?;

        if path.exists() {
            if *no_color {
                println!("WARNING Configuration file already exists at: {}", path.display());
            } else {
                println!("{} Configuration file already exists at: {}", "WARNING".yellow().bold(), path.display());
            }
            if !output::confirm("Overwrite?", false) {
                println!("Cancelled.");
                return Ok(());
            }
        }

        config.save_to_file(&path)?;
        if *no_color {
            println!("SUCCESS Configuration initialized at: {}", path.display());
        } else {
            println!("{} Configuration initialized at: {}", "SUCCESS".green().bold(), path.display());
        }
        return Ok(());
    }

    // Show current configuration
    let content = toml::to_string_pretty(config)
        .map_err(|e| CliError::Config(e.to_string()))?;

    if *no_color {
        println!("Current Configuration");
    } else {
        println!("{}", "Current Configuration".bold().underline());
    }
    println!();
    println!("{}", content);

    if let Some(path) = Config::user_config_path() {
        println!();
        if *no_color {
            println!("Config file: {}", path.display());
        } else {
            println!("{}: {}", "Config file".cyan(), path.display());
        }
    }

    Ok(())
}
