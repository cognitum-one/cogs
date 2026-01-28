//! Application-specific optimization module
//!
//! Provides intelligent memory management for common resource-heavy applications:
//! - Browsers (Chrome, Firefox, Safari, Edge, Arc, Brave)
//! - Electron apps (VS Code, Discord, Slack, Teams, etc.)
//! - Docker containers
//! - Development tools
//! - AI/ML workloads

pub mod browser;
pub mod electron;
pub mod docker;
pub mod leaks;
pub mod suggestions;

pub use browser::BrowserOptimizer;
pub use electron::ElectronManager;
pub use docker::DockerManager;
pub use leaks::LeakDetector;
pub use suggestions::SmartSuggestions;

use serde::{Deserialize, Serialize};

/// Common app categories for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppCategory {
    Browser,
    Electron,
    Development,
    Creative,
    Communication,
    Media,
    System,
    Container,
    AI,
    Other,
}

/// Process info with app categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProcess {
    pub pid: u32,
    pub name: String,
    pub category: AppCategory,
    pub memory_mb: f64,
    pub cpu_percent: f32,
    pub parent_app: Option<String>,
    pub is_main_process: bool,
}

/// Aggregated app info (groups related processes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub category: AppCategory,
    pub process_count: usize,
    pub total_memory_mb: f64,
    pub total_cpu_percent: f32,
    pub main_pid: Option<u32>,
    pub pids: Vec<u32>,
    pub is_idle: bool,
    pub idle_duration_secs: u64,
}

impl AppInfo {
    /// Check if this app is a memory hog (>500MB)
    pub fn is_memory_hog(&self) -> bool {
        self.total_memory_mb > 500.0
    }

    /// Check if this app is using significant CPU (>10%)
    pub fn is_cpu_intensive(&self) -> bool {
        self.total_cpu_percent > 10.0
    }

    /// Get optimization priority (higher = optimize first)
    pub fn optimization_priority(&self) -> f64 {
        let mut priority = 0.0;

        // Memory weight
        priority += self.total_memory_mb / 100.0;

        // CPU weight (less important than memory for optimization)
        priority += self.total_cpu_percent as f64 * 0.5;

        // Idle apps get higher priority for optimization
        if self.is_idle {
            priority *= 1.5;
        }

        // More processes = more overhead
        priority += self.process_count as f64 * 2.0;

        priority
    }
}

/// Optimization action for an app
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationAction {
    /// Suggest closing the app
    Close,
    /// Suggest suspending/pausing
    Suspend,
    /// Trim working set / release memory
    TrimMemory,
    /// Restart to clear memory leaks
    Restart,
    /// Reduce tab count (browsers)
    ReduceTabs { suggested_count: usize },
    /// Unload inactive tabs
    SuspendTabs,
    /// Stop container
    StopContainer,
    /// Pause container
    PauseContainer,
    /// Clear cache
    ClearCache,
    /// No action needed
    None,
}

/// Result of an optimization operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub app_name: String,
    pub action: OptimizationAction,
    pub success: bool,
    pub memory_freed_mb: f64,
    pub message: String,
}
