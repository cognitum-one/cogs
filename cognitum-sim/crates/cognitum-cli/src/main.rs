use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cognitum_cli::{benchmark, debug, inspect, load, run};
use cognitum_cli::config::CognitumCliConfig;

/// Cognitum ASIC Simulator - Command Line Interface
#[derive(Parser)]
#[command(name = "cognitum")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE", global = true)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Set log level (trace, debug, info, warn, error)
    #[arg(short = 'L', long, value_name = "LEVEL", global = true)]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a program on the simulator
    Run {
        /// Program binary file to execute
        #[arg(short, long, value_name = "FILE")]
        program: PathBuf,

        /// Number of tiles to use (1-256)
        #[arg(short, long, default_value = "256")]
        tiles: u16,

        /// Maximum number of cycles to run
        #[arg(short = 'n', long)]
        cycles: Option<u64>,

        /// Enable execution trace
        #[arg(long)]
        trace: bool,

        /// Trace output file
        #[arg(long, value_name = "FILE")]
        trace_file: Option<PathBuf>,

        /// Number of worker threads
        #[arg(short = 'j', long)]
        threads: Option<usize>,
    },

    /// Load and inspect a program without running
    Load {
        /// Program binary file
        #[arg(short, long, value_name = "FILE")]
        program: PathBuf,

        /// Target tile ID (0-255)
        #[arg(short, long)]
        tile: u8,

        /// Disassemble the program
        #[arg(short, long)]
        disassemble: bool,

        /// Show memory layout
        #[arg(short, long)]
        memory: bool,
    },

    /// Debug mode with breakpoints and stepping
    Debug {
        /// Program binary file
        #[arg(short, long, value_name = "FILE")]
        program: PathBuf,

        /// Breakpoint addresses (hex format: 0x100)
        #[arg(short, long)]
        breakpoints: Vec<String>,

        /// Target tile ID (0-255)
        #[arg(short, long, default_value = "0")]
        tile: u8,

        /// Start paused
        #[arg(long)]
        pause: bool,
    },

    /// Inspect simulator state
    Inspect {
        /// Show all tile states
        #[arg(long)]
        tiles: bool,

        /// Show specific tile details
        #[arg(short, long)]
        tile: Option<u8>,

        /// Show memory regions
        #[arg(short, long)]
        memory: bool,

        /// Show RaceWay packet statistics
        #[arg(short, long)]
        raceway: bool,

        /// Show performance metrics
        #[arg(long)]
        metrics: bool,
    },

    /// Run performance benchmarks
    Benchmark {
        /// Benchmark suite to run
        #[arg(short, long, value_name = "SUITE")]
        suite: String,

        /// Number of iterations
        #[arg(short, long, default_value = "10")]
        iterations: usize,

        /// Output format (text, json, csv)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Output file for results
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose, cli.log_level.as_deref())?;

    // Load configuration
    let config = if let Some(config_path) = &cli.config {
        CognitumCliConfig::load(config_path)?
    } else {
        CognitumCliConfig::default()
    };

    // Execute command
    match cli.command {
        Command::Run {
            program,
            tiles,
            cycles,
            trace,
            trace_file,
            threads,
        } => run::execute(&config, program, tiles, cycles, trace, trace_file, threads).await?,
        Command::Load {
            program,
            tile,
            disassemble,
            memory,
        } => load::execute(&config, program, tile, disassemble, memory).await?,
        Command::Debug {
            program,
            breakpoints,
            tile,
            pause,
        } => debug::execute(&config, program, breakpoints, tile, pause).await?,
        Command::Inspect {
            tiles,
            tile,
            memory,
            raceway,
            metrics,
        } => inspect::execute(&config, tiles, tile, memory, raceway, metrics).await?,
        Command::Benchmark {
            suite,
            iterations,
            format,
            output,
        } => benchmark::execute(&config, suite, iterations, format, output).await?,
    }

    Ok(())
}

fn init_logging(verbose: bool, level: Option<&str>) -> Result<()> {
    let env_filter = match (verbose, level) {
        (true, Some(l)) => l.to_string(),
        (true, None) => "debug".to_string(),
        (false, Some(l)) => l.to_string(),
        (false, None) => "info".to_string(),
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| env_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
