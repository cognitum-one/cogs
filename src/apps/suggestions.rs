//! Smart suggestions engine
//!
//! Analyzes system state and provides intelligent optimization recommendations:
//! - Prioritizes suggestions by impact
//! - Considers user patterns and usage
//! - Provides actionable recommendations
//! - Learns from system behavior

use super::{
    browser::BrowserOptimizer,
    docker::DockerManager,
    electron::ElectronManager,
    leaks::LeakDetector,
    AppCategory, OptimizationAction,
};
use serde::{Deserialize, Serialize};
use sysinfo::{System, ProcessesToUpdate};

/// Optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub priority: SuggestionPriority,
    pub category: AppCategory,
    pub title: String,
    pub description: String,
    pub action: OptimizationAction,
    pub estimated_savings_mb: f64,
    pub app_name: Option<String>,
    pub pids: Vec<u32>,
}

/// Suggestion priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SuggestionPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl std::fmt::Display for SuggestionPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuggestionPriority::Low => write!(f, "Low"),
            SuggestionPriority::Medium => write!(f, "Medium"),
            SuggestionPriority::High => write!(f, "High"),
            SuggestionPriority::Critical => write!(f, "Critical"),
        }
    }
}

/// System memory pressure level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Low,      // <50% used
    Normal,   // 50-70% used
    High,     // 70-85% used
    Critical, // >85% used
}

/// Smart suggestions engine
pub struct SmartSuggestions {
    system: System,
    browser_optimizer: BrowserOptimizer,
    electron_manager: ElectronManager,
    docker_manager: DockerManager,
    suggestions: Vec<Suggestion>,
}

impl SmartSuggestions {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            browser_optimizer: BrowserOptimizer::new(),
            electron_manager: ElectronManager::new(),
            docker_manager: DockerManager::new(),
            suggestions: Vec::new(),
        }
    }

    /// Refresh all data and generate suggestions
    pub fn refresh(&mut self) {
        self.system.refresh_all();
        self.browser_optimizer.refresh();
        self.electron_manager.refresh();
        self.docker_manager.refresh();

        self.generate_suggestions();
    }

    /// Get current memory pressure level
    pub fn memory_pressure(&self) -> MemoryPressure {
        let total = self.system.total_memory();
        let used = self.system.used_memory();

        if total == 0 {
            return MemoryPressure::Normal;
        }

        let percent = (used as f64 / total as f64) * 100.0;

        if percent > 85.0 {
            MemoryPressure::Critical
        } else if percent > 70.0 {
            MemoryPressure::High
        } else if percent > 50.0 {
            MemoryPressure::Normal
        } else {
            MemoryPressure::Low
        }
    }

    /// Generate all suggestions
    fn generate_suggestions(&mut self) {
        self.suggestions.clear();

        let pressure = self.memory_pressure();

        // Browser suggestions
        self.add_browser_suggestions(pressure);

        // Electron app suggestions
        self.add_electron_suggestions(pressure);

        // Docker suggestions
        self.add_docker_suggestions(pressure);

        // General high-memory process suggestions
        self.add_general_suggestions(pressure);

        // Sort by priority (highest first) then by estimated savings
        self.suggestions.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(b.estimated_savings_mb.partial_cmp(&a.estimated_savings_mb).unwrap())
        });
    }

    /// Add browser-related suggestions
    fn add_browser_suggestions(&mut self, pressure: MemoryPressure) {
        for browser in self.browser_optimizer.get_browsers() {
            // Tab reduction suggestions
            if browser.estimated_tabs > 20 {
                let priority = if browser.total_memory_mb > 2000.0 || pressure == MemoryPressure::Critical {
                    SuggestionPriority::High
                } else if browser.total_memory_mb > 1000.0 || pressure == MemoryPressure::High {
                    SuggestionPriority::Medium
                } else {
                    SuggestionPriority::Low
                };

                let suggested_tabs = (browser.estimated_tabs / 2).max(10);
                let estimated_savings = browser.total_memory_mb * 0.3; // ~30% savings

                self.suggestions.push(Suggestion {
                    priority,
                    category: AppCategory::Browser,
                    title: format!("Reduce {} tabs", browser.name),
                    description: format!(
                        "{} has ~{} tabs using {:.0} MB. Consider reducing to {} tabs.",
                        browser.name, browser.estimated_tabs, browser.total_memory_mb, suggested_tabs
                    ),
                    action: OptimizationAction::ReduceTabs {
                        suggested_count: suggested_tabs,
                    },
                    estimated_savings_mb: estimated_savings,
                    app_name: Some(browser.name.clone()),
                    pids: browser.pids.clone(),
                });
            }

            // High memory browser warning
            if browser.total_memory_mb > 3000.0 {
                self.suggestions.push(Suggestion {
                    priority: SuggestionPriority::High,
                    category: AppCategory::Browser,
                    title: format!("Restart {}", browser.name),
                    description: format!(
                        "{} is using {:.0} MB - consider restarting to free memory",
                        browser.name, browser.total_memory_mb
                    ),
                    action: OptimizationAction::Restart,
                    estimated_savings_mb: browser.total_memory_mb * 0.5,
                    app_name: Some(browser.name.clone()),
                    pids: browser.pids.clone(),
                });
            }
        }
    }

    /// Add Electron app suggestions
    fn add_electron_suggestions(&mut self, pressure: MemoryPressure) {
        for app in self.electron_manager.get_apps() {
            // Bloated app warning
            if app.is_bloated() {
                let priority = if app.total_memory_mb > 1000.0 || pressure == MemoryPressure::Critical {
                    SuggestionPriority::High
                } else if pressure == MemoryPressure::High {
                    SuggestionPriority::Medium
                } else {
                    SuggestionPriority::Low
                };

                let excess = app.total_memory_mb - app.baseline_memory_mb;

                self.suggestions.push(Suggestion {
                    priority,
                    category: AppCategory::Electron,
                    title: format!("Restart {}", app.display_name),
                    description: format!(
                        "{} is using {:.0}% more memory than expected ({:.0} MB vs {:.0} MB baseline). Restart to reclaim ~{:.0} MB.",
                        app.display_name,
                        app.memory_overhead_percent - 100.0,
                        app.total_memory_mb,
                        app.baseline_memory_mb,
                        excess
                    ),
                    action: OptimizationAction::Restart,
                    estimated_savings_mb: excess,
                    app_name: Some(app.display_name.clone()),
                    pids: app.pids.clone(),
                });
            }

            // Very high memory apps
            if app.total_memory_mb > 1500.0 {
                self.suggestions.push(Suggestion {
                    priority: SuggestionPriority::High,
                    category: AppCategory::Electron,
                    title: format!("{} high memory", app.display_name),
                    description: format!(
                        "{} is using {:.0} MB across {} processes. Consider closing if not needed.",
                        app.display_name, app.total_memory_mb, app.process_count
                    ),
                    action: OptimizationAction::Close,
                    estimated_savings_mb: app.total_memory_mb,
                    app_name: Some(app.display_name.clone()),
                    pids: app.pids.clone(),
                });
            }
        }
    }

    /// Add Docker container suggestions
    fn add_docker_suggestions(&mut self, pressure: MemoryPressure) {
        if !self.docker_manager.is_available() {
            return;
        }

        // Idle containers
        for container in self.docker_manager.get_idle_containers() {
            if container.memory_mb > 200.0 {
                let priority = if pressure == MemoryPressure::Critical {
                    SuggestionPriority::High
                } else if pressure == MemoryPressure::High {
                    SuggestionPriority::Medium
                } else {
                    SuggestionPriority::Low
                };

                self.suggestions.push(Suggestion {
                    priority,
                    category: AppCategory::Container,
                    title: format!("Pause container {}", container.name),
                    description: format!(
                        "Container '{}' is idle but using {:.0} MB. Pause to save resources.",
                        container.name, container.memory_mb
                    ),
                    action: OptimizationAction::PauseContainer,
                    estimated_savings_mb: 0.0, // Pausing doesn't free memory but saves CPU
                    app_name: Some(container.name.clone()),
                    pids: Vec::new(),
                });
            }
        }

        // High memory containers
        for container in self.docker_manager.get_containers() {
            if container.memory_mb > 2000.0 {
                self.suggestions.push(Suggestion {
                    priority: SuggestionPriority::Medium,
                    category: AppCategory::Container,
                    title: format!("Container {} high memory", container.name),
                    description: format!(
                        "Container '{}' ({}) is using {:.0} MB ({:.0}% of limit).",
                        container.name, container.image, container.memory_mb, container.memory_percent
                    ),
                    action: OptimizationAction::StopContainer,
                    estimated_savings_mb: container.memory_mb,
                    app_name: Some(container.name.clone()),
                    pids: Vec::new(),
                });
            }
        }
    }

    /// Add general process suggestions
    fn add_general_suggestions(&mut self, pressure: MemoryPressure) {
        // Find high-memory processes not covered by specific optimizers
        let browser_pids: std::collections::HashSet<u32> = self
            .browser_optimizer
            .get_browsers()
            .iter()
            .flat_map(|b| b.pids.clone())
            .collect();

        let electron_pids: std::collections::HashSet<u32> = self
            .electron_manager
            .get_apps()
            .iter()
            .flat_map(|a| a.pids.clone())
            .collect();

        for (pid, process) in self.system.processes() {
            let pid_u32 = pid.as_u32();

            // Skip if already covered
            if browser_pids.contains(&pid_u32) || electron_pids.contains(&pid_u32) {
                continue;
            }

            let memory_mb = process.memory() as f64 / (1024.0 * 1024.0);
            let name = process.name().to_string_lossy().to_string();

            // High memory processes
            if memory_mb > 1000.0 {
                let priority = if memory_mb > 2000.0 || pressure == MemoryPressure::Critical {
                    SuggestionPriority::High
                } else {
                    SuggestionPriority::Medium
                };

                self.suggestions.push(Suggestion {
                    priority,
                    category: AppCategory::Other,
                    title: format!("{} high memory", name),
                    description: format!(
                        "'{}' (PID {}) is using {:.0} MB. Consider closing if not needed.",
                        name, pid_u32, memory_mb
                    ),
                    action: OptimizationAction::Close,
                    estimated_savings_mb: memory_mb,
                    app_name: Some(name),
                    pids: vec![pid_u32],
                });
            }
        }

        // Memory pressure warning
        if pressure == MemoryPressure::Critical {
            let total = self.system.total_memory() as f64 / (1024.0 * 1024.0);
            let used = self.system.used_memory() as f64 / (1024.0 * 1024.0);
            let available = total - used;

            self.suggestions.push(Suggestion {
                priority: SuggestionPriority::Critical,
                category: AppCategory::System,
                title: "Critical memory pressure".to_string(),
                description: format!(
                    "Only {:.0} MB available out of {:.0} MB total ({:.0}% used). System may become unstable.",
                    available, total, (used / total) * 100.0
                ),
                action: OptimizationAction::None,
                estimated_savings_mb: 0.0,
                app_name: None,
                pids: Vec::new(),
            });
        }
    }

    /// Get all suggestions
    pub fn get_suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }

    /// Get suggestions by category
    pub fn get_by_category(&self, category: AppCategory) -> Vec<&Suggestion> {
        self.suggestions
            .iter()
            .filter(|s| s.category == category)
            .collect()
    }

    /// Get suggestions by priority
    pub fn get_by_priority(&self, priority: SuggestionPriority) -> Vec<&Suggestion> {
        self.suggestions
            .iter()
            .filter(|s| s.priority == priority)
            .collect()
    }

    /// Get top N suggestions
    pub fn get_top(&self, n: usize) -> Vec<&Suggestion> {
        self.suggestions.iter().take(n).collect()
    }

    /// Get total potential savings
    pub fn total_potential_savings(&self) -> f64 {
        self.suggestions.iter().map(|s| s.estimated_savings_mb).sum()
    }

    /// Print suggestions summary
    pub fn print_summary(&self) {
        let pressure = self.memory_pressure();
        let pressure_icon = match pressure {
            MemoryPressure::Low => "🟢",
            MemoryPressure::Normal => "🟡",
            MemoryPressure::High => "🟠",
            MemoryPressure::Critical => "🔴",
        };

        println!("\n💡 Smart Optimization Suggestions\n");
        println!("Memory Pressure: {} {:?}", pressure_icon, pressure);

        let total = self.system.total_memory() as f64 / (1024.0 * 1024.0);
        let used = self.system.used_memory() as f64 / (1024.0 * 1024.0);
        println!(
            "Memory Usage: {:.0} MB / {:.0} MB ({:.0}%)\n",
            used,
            total,
            (used / total) * 100.0
        );

        if self.suggestions.is_empty() {
            println!("✅ No optimization suggestions at this time.");
            return;
        }

        println!("┌──────────┬──────────────────────────────┬───────────────┐");
        println!("│ Priority │ Suggestion                   │ Est. Savings  │");
        println!("├──────────┼──────────────────────────────┼───────────────┤");

        for suggestion in self.suggestions.iter().take(10) {
            let priority_icon = match suggestion.priority {
                SuggestionPriority::Critical => "🔴 Crit",
                SuggestionPriority::High => "🟠 High",
                SuggestionPriority::Medium => "🟡 Med",
                SuggestionPriority::Low => "🟢 Low",
            };

            let savings = if suggestion.estimated_savings_mb > 0.0 {
                format!("{:.0} MB", suggestion.estimated_savings_mb)
            } else {
                "-".to_string()
            };

            println!(
                "│ {:8} │ {:28} │ {:>13} │",
                priority_icon,
                truncate(&suggestion.title, 28),
                savings
            );
        }

        println!("└──────────┴──────────────────────────────┴───────────────┘");

        let total_savings = self.total_potential_savings();
        if total_savings > 0.0 {
            println!("\n💰 Total potential savings: {:.0} MB", total_savings);
        }

        println!("\n📋 Details:");
        for (i, suggestion) in self.suggestions.iter().take(5).enumerate() {
            println!("{}. {}", i + 1, suggestion.description);
        }

        if self.suggestions.len() > 5 {
            println!("   ... and {} more suggestions", self.suggestions.len() - 5);
        }
    }
}

impl Default for SmartSuggestions {
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
