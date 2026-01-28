//! Browser memory optimization
//!
//! Detects and manages memory usage for major browsers:
//! - Google Chrome / Chromium
//! - Mozilla Firefox
//! - Apple Safari
//! - Microsoft Edge
//! - Arc Browser
//! - Brave Browser
//! - Opera / Opera GX
//! - Vivaldi

use super::{AppCategory, AppInfo, AppProcess, OptimizationAction, OptimizationResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, ProcessesToUpdate, Pid};

/// Browser identification patterns
#[derive(Debug, Clone)]
pub struct BrowserPattern {
    pub name: &'static str,
    pub display_name: &'static str,
    /// Main process patterns (case-insensitive)
    pub main_patterns: &'static [&'static str],
    /// Helper/renderer process patterns
    pub helper_patterns: &'static [&'static str],
    /// GPU process patterns
    pub gpu_patterns: &'static [&'static str],
    /// Extension/plugin patterns
    pub extension_patterns: &'static [&'static str],
}

/// Known browser patterns
pub const BROWSERS: &[BrowserPattern] = &[
    BrowserPattern {
        name: "chrome",
        display_name: "Google Chrome",
        main_patterns: &["google chrome", "chrome.exe", "chrome"],
        helper_patterns: &["chrome helper", "google chrome helper", "chromedriver"],
        gpu_patterns: &["chrome helper (gpu)", "chrome gpu"],
        extension_patterns: &["chrome helper (renderer)", "chrome helper (plugin)"],
    },
    BrowserPattern {
        name: "firefox",
        display_name: "Mozilla Firefox",
        main_patterns: &["firefox", "firefox.exe"],
        helper_patterns: &["firefox helper", "plugin-container", "firefox-bin"],
        gpu_patterns: &["firefox gpu"],
        extension_patterns: &["web content", "webextensions"],
    },
    BrowserPattern {
        name: "safari",
        display_name: "Apple Safari",
        main_patterns: &["safari", "safari.app"],
        helper_patterns: &["safari web content", "webkit networking", "safari networking"],
        gpu_patterns: &["safari graphics"],
        extension_patterns: &["safari extension"],
    },
    BrowserPattern {
        name: "edge",
        display_name: "Microsoft Edge",
        main_patterns: &["microsoft edge", "msedge", "msedge.exe"],
        helper_patterns: &["microsoft edge helper", "msedge helper"],
        gpu_patterns: &["msedge helper (gpu)"],
        extension_patterns: &["msedge helper (renderer)"],
    },
    BrowserPattern {
        name: "arc",
        display_name: "Arc Browser",
        main_patterns: &["arc", "arc.app"],
        helper_patterns: &["arc helper", "arc helper (renderer)"],
        gpu_patterns: &["arc helper (gpu)"],
        extension_patterns: &["arc helper (plugin)"],
    },
    BrowserPattern {
        name: "brave",
        display_name: "Brave Browser",
        main_patterns: &["brave browser", "brave.exe", "brave"],
        helper_patterns: &["brave browser helper", "brave helper"],
        gpu_patterns: &["brave browser helper (gpu)"],
        extension_patterns: &["brave browser helper (renderer)"],
    },
    BrowserPattern {
        name: "opera",
        display_name: "Opera",
        main_patterns: &["opera", "opera.exe", "opera gx"],
        helper_patterns: &["opera helper", "opera gx helper"],
        gpu_patterns: &["opera helper (gpu)"],
        extension_patterns: &["opera helper (renderer)"],
    },
    BrowserPattern {
        name: "vivaldi",
        display_name: "Vivaldi",
        main_patterns: &["vivaldi", "vivaldi.exe"],
        helper_patterns: &["vivaldi helper"],
        gpu_patterns: &["vivaldi helper (gpu)"],
        extension_patterns: &["vivaldi helper (renderer)"],
    },
];

/// Detailed browser process info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProcess {
    pub pid: u32,
    pub name: String,
    pub process_type: BrowserProcessType,
    pub memory_mb: f64,
    pub cpu_percent: f32,
}

/// Type of browser process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserProcessType {
    Main,
    Renderer,
    GPU,
    Extension,
    Plugin,
    Utility,
    Network,
    Unknown,
}

/// Browser instance info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub name: String,
    pub display_name: String,
    pub total_memory_mb: f64,
    pub total_cpu_percent: f32,
    pub process_count: usize,
    pub estimated_tabs: usize,
    pub main_pid: Option<u32>,
    pub pids: Vec<u32>,
    pub processes: Vec<BrowserProcess>,
    pub gpu_memory_mb: f64,
    pub renderer_memory_mb: f64,
    pub extension_memory_mb: f64,
}

impl BrowserInfo {
    /// Get suggested action based on resource usage
    pub fn get_suggested_action(&self) -> OptimizationAction {
        if self.total_memory_mb > 4000.0 {
            OptimizationAction::Restart
        } else if self.total_memory_mb > 2000.0 {
            OptimizationAction::ReduceTabs {
                suggested_count: self.estimated_tabs / 2,
            }
        } else if self.total_memory_mb > 1000.0 && self.estimated_tabs > 20 {
            OptimizationAction::SuspendTabs
        } else if self.total_memory_mb > 500.0 {
            OptimizationAction::TrimMemory
        } else {
            OptimizationAction::None
        }
    }

    /// Memory per estimated tab
    pub fn memory_per_tab(&self) -> f64 {
        if self.estimated_tabs > 0 {
            self.total_memory_mb / self.estimated_tabs as f64
        } else {
            0.0
        }
    }
}

/// Browser memory optimizer
pub struct BrowserOptimizer {
    system: System,
    browsers: HashMap<String, BrowserInfo>,
    last_update: std::time::Instant,
}

impl BrowserOptimizer {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);

        Self {
            system,
            browsers: HashMap::new(),
            last_update: std::time::Instant::now(),
        }
    }

    /// Refresh process data
    pub fn refresh(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.detect_browsers();
        self.last_update = std::time::Instant::now();
    }

    /// Detect all running browsers
    fn detect_browsers(&mut self) {
        self.browsers.clear();

        for pattern in BROWSERS {
            let mut browser_info = BrowserInfo {
                name: pattern.name.to_string(),
                display_name: pattern.display_name.to_string(),
                total_memory_mb: 0.0,
                total_cpu_percent: 0.0,
                process_count: 0,
                estimated_tabs: 0,
                main_pid: None,
                pids: Vec::new(),
                processes: Vec::new(),
                gpu_memory_mb: 0.0,
                renderer_memory_mb: 0.0,
                extension_memory_mb: 0.0,
            };

            for (pid, process) in self.system.processes() {
                let name = process.name().to_string_lossy().to_lowercase();
                let memory_mb = process.memory() as f64 / (1024.0 * 1024.0);
                let cpu_percent = process.cpu_usage();

                let process_type = self.classify_process(&name, pattern);

                if process_type != BrowserProcessType::Unknown {
                    let pid_u32 = pid.as_u32();
                    let browser_proc = BrowserProcess {
                        pid: pid_u32,
                        name: process.name().to_string_lossy().to_string(),
                        process_type,
                        memory_mb,
                        cpu_percent,
                    };

                    browser_info.total_memory_mb += memory_mb;
                    browser_info.total_cpu_percent += cpu_percent;
                    browser_info.process_count += 1;
                    browser_info.pids.push(pid_u32);

                    match process_type {
                        BrowserProcessType::Main => {
                            browser_info.main_pid = Some(pid.as_u32());
                        }
                        BrowserProcessType::GPU => {
                            browser_info.gpu_memory_mb += memory_mb;
                        }
                        BrowserProcessType::Renderer => {
                            browser_info.renderer_memory_mb += memory_mb;
                            browser_info.estimated_tabs += 1;
                        }
                        BrowserProcessType::Extension => {
                            browser_info.extension_memory_mb += memory_mb;
                        }
                        _ => {}
                    }

                    browser_info.processes.push(browser_proc);
                }
            }

            // Only include if we found any processes
            if browser_info.process_count > 0 {
                // Estimate tabs from renderer processes (each tab ~= 1 renderer)
                // But there's usually at least 1 renderer even with no tabs
                if browser_info.estimated_tabs > 0 {
                    browser_info.estimated_tabs = browser_info.estimated_tabs.saturating_sub(1).max(1);
                }

                self.browsers.insert(pattern.name.to_string(), browser_info);
            }
        }
    }

    /// Classify a process based on browser pattern
    fn classify_process(&self, name: &str, pattern: &BrowserPattern) -> BrowserProcessType {
        // Check main process first
        for p in pattern.main_patterns {
            if name.contains(p) && !name.contains("helper") {
                return BrowserProcessType::Main;
            }
        }

        // Check GPU process
        for p in pattern.gpu_patterns {
            if name.contains(p) {
                return BrowserProcessType::GPU;
            }
        }

        // Check extension process
        for p in pattern.extension_patterns {
            if name.contains(p) {
                return BrowserProcessType::Extension;
            }
        }

        // Check helper/renderer process
        for p in pattern.helper_patterns {
            if name.contains(p) {
                return BrowserProcessType::Renderer;
            }
        }

        // Check if it matches any main pattern (catch-all for related processes)
        for p in pattern.main_patterns {
            if name.contains(p) {
                return BrowserProcessType::Utility;
            }
        }

        BrowserProcessType::Unknown
    }

    /// Get all detected browsers
    pub fn get_browsers(&self) -> Vec<&BrowserInfo> {
        self.browsers.values().collect()
    }

    /// Get browser by name
    pub fn get_browser(&self, name: &str) -> Option<&BrowserInfo> {
        self.browsers.get(name)
    }

    /// Get total browser memory usage
    pub fn total_memory_mb(&self) -> f64 {
        self.browsers.values().map(|b| b.total_memory_mb).sum()
    }

    /// Get total browser CPU usage
    pub fn total_cpu_percent(&self) -> f32 {
        self.browsers.values().map(|b| b.total_cpu_percent).sum()
    }

    /// Get browser with highest memory usage
    pub fn highest_memory_browser(&self) -> Option<&BrowserInfo> {
        self.browsers
            .values()
            .max_by(|a, b| a.total_memory_mb.partial_cmp(&b.total_memory_mb).unwrap())
    }

    /// Get optimization suggestions for all browsers
    pub fn get_suggestions(&self) -> Vec<(String, OptimizationAction, String)> {
        let mut suggestions = Vec::new();

        for browser in self.browsers.values() {
            let action = browser.get_suggested_action();
            if action != OptimizationAction::None {
                let reason = match &action {
                    OptimizationAction::Restart => {
                        format!(
                            "{} is using {:.0} MB - consider restarting to free memory",
                            browser.display_name, browser.total_memory_mb
                        )
                    }
                    OptimizationAction::ReduceTabs { suggested_count } => {
                        format!(
                            "{} has ~{} tabs using {:.0} MB - consider closing some tabs (suggest {})",
                            browser.display_name,
                            browser.estimated_tabs,
                            browser.total_memory_mb,
                            suggested_count
                        )
                    }
                    OptimizationAction::SuspendTabs => {
                        format!(
                            "{} has ~{} tabs - consider using a tab suspender extension",
                            browser.display_name, browser.estimated_tabs
                        )
                    }
                    OptimizationAction::TrimMemory => {
                        format!(
                            "{} is using {:.0} MB - memory can be trimmed",
                            browser.display_name, browser.total_memory_mb
                        )
                    }
                    _ => continue,
                };

                suggestions.push((browser.display_name.clone(), action, reason));
            }
        }

        // Sort by memory (highest first)
        suggestions.sort_by(|a, b| {
            let mem_a = self.browsers.values().find(|browser| browser.display_name == a.0).map(|browser| browser.total_memory_mb).unwrap_or(0.0);
            let mem_b = self.browsers.values().find(|browser| browser.display_name == b.0).map(|browser| browser.total_memory_mb).unwrap_or(0.0);
            mem_b.partial_cmp(&mem_a).unwrap()
        });

        suggestions
    }

    /// Attempt to trim browser memory (platform-specific)
    #[cfg(target_os = "windows")]
    pub fn trim_browser_memory(&self, browser_name: &str) -> OptimizationResult {
        use std::os::raw::c_void;

        let browser = match self.browsers.get(browser_name) {
            Some(b) => b,
            None => {
                return OptimizationResult {
                    app_name: browser_name.to_string(),
                    action: OptimizationAction::TrimMemory,
                    success: false,
                    memory_freed_mb: 0.0,
                    message: "Browser not found".to_string(),
                }
            }
        };

        let mut total_freed = 0.0;
        let mut trimmed = 0;

        for proc in &browser.processes {
            unsafe {
                use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_QUERY_INFORMATION};
                use windows::Win32::System::ProcessStatus::EmptyWorkingSet;

                if let Ok(handle) = OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION,
                    false,
                    proc.pid,
                ) {
                    let before = proc.memory_mb;
                    if EmptyWorkingSet(handle).is_ok() {
                        // Estimate ~30% reduction
                        total_freed += before * 0.3;
                        trimmed += 1;
                    }
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                }
            }
        }

        OptimizationResult {
            app_name: browser.display_name.clone(),
            action: OptimizationAction::TrimMemory,
            success: trimmed > 0,
            memory_freed_mb: total_freed,
            message: format!("Trimmed {} processes, estimated {:.0} MB freed", trimmed, total_freed),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn trim_browser_memory(&self, browser_name: &str) -> OptimizationResult {
        let browser = match self.browsers.get(browser_name) {
            Some(b) => b,
            None => {
                return OptimizationResult {
                    app_name: browser_name.to_string(),
                    action: OptimizationAction::TrimMemory,
                    success: false,
                    memory_freed_mb: 0.0,
                    message: "Browser not found".to_string(),
                }
            }
        };

        // On macOS, we can use memory_pressure notification or purge
        // For individual apps, we can send SIGURG or use madvise hints
        // But direct memory trimming isn't as straightforward as Windows

        OptimizationResult {
            app_name: browser.display_name.clone(),
            action: OptimizationAction::TrimMemory,
            success: false,
            memory_freed_mb: 0.0,
            message: format!(
                "{} using {:.0} MB. On macOS, use 'Optimize Now' for system-wide cleanup or restart the browser.",
                browser.display_name, browser.total_memory_mb
            ),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn trim_browser_memory(&self, browser_name: &str) -> OptimizationResult {
        OptimizationResult {
            app_name: browser_name.to_string(),
            action: OptimizationAction::TrimMemory,
            success: false,
            memory_freed_mb: 0.0,
            message: "Memory trimming not supported on this platform".to_string(),
        }
    }

    /// Print browser summary
    pub fn print_summary(&self) {
        println!("\n🌐 Browser Memory Usage\n");
        println!("┌──────────────────────┬───────────┬──────────┬───────┬─────────────┐");
        println!("│ Browser              │ Memory    │ CPU      │ Tabs  │ Processes   │");
        println!("├──────────────────────┼───────────┼──────────┼───────┼─────────────┤");

        let mut browsers: Vec<_> = self.browsers.values().collect();
        browsers.sort_by(|a, b| b.total_memory_mb.partial_cmp(&a.total_memory_mb).unwrap());

        for browser in &browsers {
            let mem_indicator = if browser.total_memory_mb > 2000.0 {
                "🔴"
            } else if browser.total_memory_mb > 1000.0 {
                "🟡"
            } else {
                "🟢"
            };

            println!(
                "│ {} {:18} │ {:>7.0} MB │ {:>6.1}%  │ {:>5} │ {:>11} │",
                mem_indicator,
                truncate(&browser.display_name, 18),
                browser.total_memory_mb,
                browser.total_cpu_percent,
                browser.estimated_tabs,
                browser.process_count
            );
        }

        println!("└──────────────────────┴───────────┴──────────┴───────┴─────────────┘");

        let total_mem: f64 = browsers.iter().map(|b| b.total_memory_mb).sum();
        let total_cpu: f32 = browsers.iter().map(|b| b.total_cpu_percent).sum();
        println!(
            "\nTotal: {:.0} MB memory, {:.1}% CPU across {} browsers",
            total_mem,
            total_cpu,
            browsers.len()
        );

        // Print suggestions
        let suggestions = self.get_suggestions();
        if !suggestions.is_empty() {
            println!("\n💡 Suggestions:");
            for (_, _, reason) in suggestions.iter().take(3) {
                println!("   • {}", reason);
            }
        }
    }
}

impl Default for BrowserOptimizer {
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
