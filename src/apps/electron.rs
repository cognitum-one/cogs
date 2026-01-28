//! Electron app detection and memory management
//!
//! Electron apps are essentially embedded Chromium browsers and often
//! consume significant memory. This module detects common Electron apps:
//! - VS Code
//! - Discord
//! - Slack
//! - Microsoft Teams
//! - Notion
//! - Figma
//! - Spotify
//! - Obsidian
//! - 1Password
//! - Postman
//! - And many more...

use super::{AppCategory, AppInfo, OptimizationAction, OptimizationResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, ProcessesToUpdate, Pid};

/// Known Electron app patterns
#[derive(Debug, Clone)]
pub struct ElectronAppPattern {
    pub name: &'static str,
    pub display_name: &'static str,
    pub patterns: &'static [&'static str],
    pub category: AppCategory,
    /// Expected baseline memory (MB) - for leak detection
    pub baseline_memory_mb: f64,
}

/// Known Electron applications
pub const ELECTRON_APPS: &[ElectronAppPattern] = &[
    ElectronAppPattern {
        name: "vscode",
        display_name: "Visual Studio Code",
        patterns: &["code", "code.exe", "code helper", "code - insiders"],
        category: AppCategory::Development,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "discord",
        display_name: "Discord",
        patterns: &["discord", "discord.exe", "discord helper"],
        category: AppCategory::Communication,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "slack",
        display_name: "Slack",
        patterns: &["slack", "slack.exe", "slack helper"],
        category: AppCategory::Communication,
        baseline_memory_mb: 250.0,
    },
    ElectronAppPattern {
        name: "teams",
        display_name: "Microsoft Teams",
        patterns: &["teams", "ms-teams", "microsoft teams", "teams.exe"],
        category: AppCategory::Communication,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "notion",
        display_name: "Notion",
        patterns: &["notion", "notion.exe", "notion helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "figma",
        display_name: "Figma",
        patterns: &["figma", "figma.exe", "figma helper", "figma agent"],
        category: AppCategory::Creative,
        baseline_memory_mb: 400.0,
    },
    ElectronAppPattern {
        name: "spotify",
        display_name: "Spotify",
        patterns: &["spotify", "spotify.exe", "spotify helper"],
        category: AppCategory::Media,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "obsidian",
        display_name: "Obsidian",
        patterns: &["obsidian", "obsidian.exe", "obsidian helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "1password",
        display_name: "1Password",
        patterns: &["1password", "1password.exe", "1password helper"],
        category: AppCategory::System,
        baseline_memory_mb: 100.0,
    },
    ElectronAppPattern {
        name: "postman",
        display_name: "Postman",
        patterns: &["postman", "postman.exe", "postman helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "whatsapp",
        display_name: "WhatsApp",
        patterns: &["whatsapp", "whatsapp.exe", "whatsapp helper"],
        category: AppCategory::Communication,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "signal",
        display_name: "Signal",
        patterns: &["signal", "signal.exe", "signal helper"],
        category: AppCategory::Communication,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "telegram",
        display_name: "Telegram Desktop",
        patterns: &["telegram", "telegram.exe", "telegram desktop"],
        category: AppCategory::Communication,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "cursor",
        display_name: "Cursor",
        patterns: &["cursor", "cursor.exe", "cursor helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "windsurf",
        display_name: "Windsurf",
        patterns: &["windsurf", "windsurf.exe", "windsurf helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "zed",
        display_name: "Zed",
        patterns: &["zed", "zed.exe", "zed helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "linear",
        display_name: "Linear",
        patterns: &["linear", "linear.exe", "linear helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "hyper",
        display_name: "Hyper Terminal",
        patterns: &["hyper", "hyper.exe", "hyper helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 150.0,
    },
    ElectronAppPattern {
        name: "atom",
        display_name: "Atom",
        patterns: &["atom", "atom.exe", "atom helper"],
        category: AppCategory::Development,
        baseline_memory_mb: 250.0,
    },
    ElectronAppPattern {
        name: "bitwarden",
        display_name: "Bitwarden",
        patterns: &["bitwarden", "bitwarden.exe", "bitwarden helper"],
        category: AppCategory::System,
        baseline_memory_mb: 100.0,
    },
    ElectronAppPattern {
        name: "mongodb-compass",
        display_name: "MongoDB Compass",
        patterns: &["mongodb compass", "mongodb-compass", "compass"],
        category: AppCategory::Development,
        baseline_memory_mb: 300.0,
    },
    ElectronAppPattern {
        name: "insomnia",
        display_name: "Insomnia",
        patterns: &["insomnia", "insomnia.exe"],
        category: AppCategory::Development,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "loom",
        display_name: "Loom",
        patterns: &["loom", "loom.exe", "loom helper"],
        category: AppCategory::Media,
        baseline_memory_mb: 200.0,
    },
    ElectronAppPattern {
        name: "gitkraken",
        display_name: "GitKraken",
        patterns: &["gitkraken", "gitkraken.exe"],
        category: AppCategory::Development,
        baseline_memory_mb: 250.0,
    },
];

/// Electron app instance info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectronAppInfo {
    pub name: String,
    pub display_name: String,
    pub category: AppCategory,
    pub total_memory_mb: f64,
    pub total_cpu_percent: f32,
    pub process_count: usize,
    pub main_pid: Option<u32>,
    pub pids: Vec<u32>,
    pub baseline_memory_mb: f64,
    pub memory_overhead_percent: f64,
    pub is_running: bool,
}

impl ElectronAppInfo {
    /// Check if app is using more memory than expected
    pub fn is_bloated(&self) -> bool {
        self.total_memory_mb > self.baseline_memory_mb * 2.0
    }

    /// Check if likely has a memory leak
    pub fn likely_memory_leak(&self) -> bool {
        self.memory_overhead_percent > 200.0
    }

    /// Get suggested action
    pub fn get_suggested_action(&self) -> OptimizationAction {
        if self.total_memory_mb > 1500.0 {
            OptimizationAction::Restart
        } else if self.is_bloated() {
            OptimizationAction::TrimMemory
        } else {
            OptimizationAction::None
        }
    }
}

/// Electron app manager
pub struct ElectronManager {
    system: System,
    apps: HashMap<String, ElectronAppInfo>,
    last_update: std::time::Instant,
}

impl ElectronManager {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);

        Self {
            system,
            apps: HashMap::new(),
            last_update: std::time::Instant::now(),
        }
    }

    /// Refresh process data
    pub fn refresh(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.detect_electron_apps();
        self.last_update = std::time::Instant::now();
    }

    /// Detect all running Electron apps
    fn detect_electron_apps(&mut self) {
        self.apps.clear();

        for pattern in ELECTRON_APPS {
            let mut app_info = ElectronAppInfo {
                name: pattern.name.to_string(),
                display_name: pattern.display_name.to_string(),
                category: pattern.category,
                total_memory_mb: 0.0,
                total_cpu_percent: 0.0,
                process_count: 0,
                main_pid: None,
                pids: Vec::new(),
                baseline_memory_mb: pattern.baseline_memory_mb,
                memory_overhead_percent: 0.0,
                is_running: false,
            };

            for (pid, process) in self.system.processes() {
                let name = process.name().to_string_lossy().to_lowercase();

                // Check if process matches any pattern
                let matches = pattern.patterns.iter().any(|p| name.contains(p));

                if matches {
                    let memory_mb = process.memory() as f64 / (1024.0 * 1024.0);
                    let cpu_percent = process.cpu_usage();

                    app_info.total_memory_mb += memory_mb;
                    app_info.total_cpu_percent += cpu_percent;
                    app_info.process_count += 1;
                    app_info.pids.push(pid.as_u32());
                    app_info.is_running = true;

                    // First matching process is usually the main process
                    if app_info.main_pid.is_none() && !name.contains("helper") {
                        app_info.main_pid = Some(pid.as_u32());
                    }
                }
            }

            if app_info.is_running {
                // Calculate memory overhead
                app_info.memory_overhead_percent =
                    (app_info.total_memory_mb / app_info.baseline_memory_mb) * 100.0;

                self.apps.insert(pattern.name.to_string(), app_info);
            }
        }
    }

    /// Get all detected Electron apps
    pub fn get_apps(&self) -> Vec<&ElectronAppInfo> {
        self.apps.values().collect()
    }

    /// Get app by name
    pub fn get_app(&self, name: &str) -> Option<&ElectronAppInfo> {
        self.apps.get(name)
    }

    /// Get total Electron app memory
    pub fn total_memory_mb(&self) -> f64 {
        self.apps.values().map(|a| a.total_memory_mb).sum()
    }

    /// Get apps that are bloated
    pub fn get_bloated_apps(&self) -> Vec<&ElectronAppInfo> {
        self.apps.values().filter(|a| a.is_bloated()).collect()
    }

    /// Get optimization suggestions
    pub fn get_suggestions(&self) -> Vec<(String, OptimizationAction, String)> {
        let mut suggestions = Vec::new();

        for app in self.apps.values() {
            let action = app.get_suggested_action();
            if action != OptimizationAction::None {
                let reason = match &action {
                    OptimizationAction::Restart => {
                        format!(
                            "{} is using {:.0} MB ({:.0}% of baseline) - restart recommended",
                            app.display_name,
                            app.total_memory_mb,
                            app.memory_overhead_percent
                        )
                    }
                    OptimizationAction::TrimMemory => {
                        format!(
                            "{} is using {:.0} MB ({:.0}% of expected {:.0} MB)",
                            app.display_name,
                            app.total_memory_mb,
                            app.memory_overhead_percent,
                            app.baseline_memory_mb
                        )
                    }
                    _ => continue,
                };

                suggestions.push((app.display_name.clone(), action, reason));
            }
        }

        suggestions.sort_by(|a, b| {
            let mem_a = self.apps.values().find(|app| app.display_name == a.0)
                .map(|a| a.total_memory_mb).unwrap_or(0.0);
            let mem_b = self.apps.values().find(|app| app.display_name == b.0)
                .map(|a| a.total_memory_mb).unwrap_or(0.0);
            mem_b.partial_cmp(&mem_a).unwrap()
        });

        suggestions
    }

    /// Print Electron apps summary
    pub fn print_summary(&self) {
        println!("\n⚡ Electron Apps Memory Usage\n");
        println!("┌──────────────────────┬───────────┬──────────┬───────────┬──────────┐");
        println!("│ Application          │ Memory    │ Baseline │ Overhead  │ Status   │");
        println!("├──────────────────────┼───────────┼──────────┼───────────┼──────────┤");

        let mut apps: Vec<_> = self.apps.values().collect();
        apps.sort_by(|a, b| b.total_memory_mb.partial_cmp(&a.total_memory_mb).unwrap());

        for app in &apps {
            let status = if app.total_memory_mb > 1000.0 {
                "🔴 Heavy"
            } else if app.is_bloated() {
                "🟡 Bloated"
            } else {
                "🟢 Normal"
            };

            println!(
                "│ {:20} │ {:>7.0} MB │ {:>6.0} MB │ {:>8.0}% │ {:8} │",
                truncate(&app.display_name, 20),
                app.total_memory_mb,
                app.baseline_memory_mb,
                app.memory_overhead_percent,
                status
            );
        }

        println!("└──────────────────────┴───────────┴──────────┴───────────┴──────────┘");

        let total: f64 = apps.iter().map(|a| a.total_memory_mb).sum();
        println!(
            "\nTotal: {:.0} MB across {} Electron apps ({} processes)",
            total,
            apps.len(),
            apps.iter().map(|a| a.process_count).sum::<usize>()
        );

        let bloated = self.get_bloated_apps();
        if !bloated.is_empty() {
            println!("\n💡 Suggestions:");
            for app in bloated.iter().take(3) {
                println!(
                    "   • {} is using {:.0}% more memory than expected - consider restarting",
                    app.display_name,
                    app.memory_overhead_percent - 100.0
                );
            }
        }
    }
}

impl Default for ElectronManager {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:width$}", s, width = max)
    } else {
        format!("{}...", &s[..max - 3])
    }
}
