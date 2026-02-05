//! Output formatting utilities for the CLI
//!
//! Provides consistent output formatting across all commands:
//! - Table output for lists
//! - JSON output for machine consumption
//! - Progress bars for long operations
//! - Colorized output

use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::fmt::Display;
use std::io::Write;
use tabled::{
    builder::Builder,
    settings::{object::Rows, Modify, Style, Width},
    Table,
};

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON output for machine consumption
    Json,
    /// Table output for lists
    Table,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" | "plain" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

/// Output writer for consistent formatting
pub struct OutputWriter {
    format: OutputFormat,
    color_enabled: bool,
}

impl OutputWriter {
    /// Create a new output writer
    pub fn new(format: OutputFormat, color_enabled: bool) -> Self {
        Self {
            format,
            color_enabled,
        }
    }

    /// Write a success message
    pub fn success(&self, message: &str) {
        if self.color_enabled {
            println!("{} {}", "SUCCESS".green().bold(), message);
        } else {
            println!("SUCCESS {}", message);
        }
    }

    /// Write an error message
    pub fn error(&self, message: &str) {
        if self.color_enabled {
            eprintln!("{} {}", "ERROR".red().bold(), message);
        } else {
            eprintln!("ERROR {}", message);
        }
    }

    /// Write a warning message
    pub fn warning(&self, message: &str) {
        if self.color_enabled {
            eprintln!("{} {}", "WARNING".yellow().bold(), message);
        } else {
            eprintln!("WARNING {}", message);
        }
    }

    /// Write an info message
    pub fn info(&self, message: &str) {
        if self.color_enabled {
            println!("{} {}", "INFO".blue().bold(), message);
        } else {
            println!("INFO {}", message);
        }
    }

    /// Write a key-value pair
    pub fn kv(&self, key: &str, value: impl Display) {
        if self.color_enabled {
            println!("  {}: {}", key.cyan(), value);
        } else {
            println!("  {}: {}", key, value);
        }
    }

    /// Write a header
    pub fn header(&self, text: &str) {
        if self.color_enabled {
            println!("\n{}", text.bold().underline());
        } else {
            println!("\n{}", text);
            println!("{}", "-".repeat(text.len()));
        }
    }

    /// Write a separator line
    pub fn separator(&self) {
        println!();
    }

    /// Output data in the configured format
    pub fn output<T: Serialize + TableDisplay>(&self, data: &T) -> anyhow::Result<()> {
        match self.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(data)?;
                println!("{}", json);
            }
            OutputFormat::Table => {
                let table = data.to_table();
                println!("{}", table);
            }
            OutputFormat::Text => {
                data.print_text(self);
            }
        }
        Ok(())
    }

    /// Output a list in the configured format
    pub fn output_list<T: Serialize + TableDisplay>(&self, items: &[T]) -> anyhow::Result<()> {
        match self.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(items)?;
                println!("{}", json);
            }
            OutputFormat::Table => {
                if items.is_empty() {
                    println!("No items found.");
                    return Ok(());
                }
                let headers = T::table_headers();
                let mut builder = Builder::new();
                builder.push_record(headers);
                for item in items {
                    builder.push_record(item.table_row());
                }
                let mut table = builder.build();
                table.with(Style::rounded());
                table.with(Modify::new(Rows::first()).with(Width::wrap(50)));
                println!("{}", table);
            }
            OutputFormat::Text => {
                if items.is_empty() {
                    println!("No items found.");
                    return Ok(());
                }
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.separator();
                    }
                    item.print_text(self);
                }
            }
        }
        Ok(())
    }
}

/// Trait for types that can be displayed as a table
pub trait TableDisplay {
    /// Get the table headers
    fn table_headers() -> Vec<String>;

    /// Get the row data for this item
    fn table_row(&self) -> Vec<String>;

    /// Convert to a table (single item)
    fn to_table(&self) -> Table {
        let mut builder = Builder::new();
        let headers = Self::table_headers();
        builder.push_record(headers);
        builder.push_record(self.table_row());
        let mut table = builder.build();
        table.with(Style::rounded());
        table
    }

    /// Print as human-readable text
    fn print_text(&self, writer: &OutputWriter);
}

/// Progress bar manager for long operations
pub struct ProgressManager {
    multi: MultiProgress,
}

impl ProgressManager {
    /// Create a new progress manager
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
        }
    }

    /// Create a spinner for an indeterminate operation
    pub fn spinner(&self, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&["...", "o..", ".o.", "..o", "..."]),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb
    }

    /// Create a progress bar for a known-length operation
    pub fn bar(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        pb.set_message(message.to_string());
        pb
    }

    /// Create a download-style progress bar
    pub fn download_bar(&self, total: u64, filename: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        pb.set_message(format!("Downloading {}", filename));
        pb
    }
}

impl Default for ProgressManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Print a boxed message
pub fn print_box(title: &str, lines: &[String], color: Color) {
    let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(0).max(title.len());
    let width = max_len + 4;

    let border_top = format!("+{}+", "-".repeat(width - 2));
    let border_bottom = border_top.clone();
    let title_line = format!("| {:^width$} |", title, width = width - 4);

    println!("{}", border_top.color(color));
    println!("{}", title_line.color(color).bold());
    println!("{}", format!("|{}|", "-".repeat(width - 2)).color(color));
    for line in lines {
        println!("| {:width$} |", line, width = width - 4);
    }
    println!("{}", border_bottom.color(color));
}

/// Format a byte size as human-readable
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a duration as human-readable
pub fn format_duration(nanos: u64) -> String {
    if nanos < 1_000 {
        format!("{} ns", nanos)
    } else if nanos < 1_000_000 {
        format!("{:.2} us", nanos as f64 / 1_000.0)
    } else if nanos < 1_000_000_000 {
        format!("{:.2} ms", nanos as f64 / 1_000_000.0)
    } else {
        format!("{:.2} s", nanos as f64 / 1_000_000_000.0)
    }
}

/// Format a percentage
pub fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

/// Format a hash as shortened form
pub fn format_hash(hash: &str) -> String {
    if hash.len() > 16 {
        format!("{}...{}", &hash[..8], &hash[hash.len() - 8..])
    } else {
        hash.to_string()
    }
}

/// Format a timestamp as ISO 8601
pub fn format_timestamp(timestamp_ns: u64) -> String {
    use chrono::{DateTime, Utc};
    let secs = (timestamp_ns / 1_000_000_000) as i64;
    let nanos = (timestamp_ns % 1_000_000_000) as u32;
    match DateTime::from_timestamp(secs, nanos) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => "Invalid timestamp".to_string(),
    }
}

/// Prompt user for confirmation
pub fn confirm(prompt: &str, default: bool) -> bool {
    let default_str = if default { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", prompt, default_str);
    std::io::stdout().flush().ok();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return default;
    }

    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500 ns");
        assert_eq!(format_duration(1500), "1.50 us");
        assert_eq!(format_duration(1500000), "1.50 ms");
        assert_eq!(format_duration(1500000000), "1.50 s");
    }

    #[test]
    fn test_format_hash() {
        let hash = "sha256:abc123def456789012345678901234567890123456789012345678901234";
        let short = format_hash(hash);
        assert!(short.contains("..."));
        assert!(short.len() < hash.len());
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Table
        );
    }
}
