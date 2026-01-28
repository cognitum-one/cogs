//! Docker container resource management
//!
//! Monitors and manages Docker containers, allowing users to:
//! - View container resource usage (memory, CPU)
//! - Pause/unpause containers
//! - Stop unused containers
//! - Identify resource-heavy containers

use super::{OptimizationAction, OptimizationResult};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Docker container info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub memory_mb: f64,
    pub memory_limit_mb: f64,
    pub memory_percent: f64,
    pub cpu_percent: f64,
    pub created: String,
    pub ports: Vec<String>,
    pub is_idle: bool,
}

/// Container status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Running,
    Paused,
    Exited,
    Created,
    Restarting,
    Dead,
    Unknown,
}

impl ContainerStatus {
    fn from_str(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.contains("running") {
            ContainerStatus::Running
        } else if s.contains("paused") {
            ContainerStatus::Paused
        } else if s.contains("exited") {
            ContainerStatus::Exited
        } else if s.contains("created") {
            ContainerStatus::Created
        } else if s.contains("restarting") {
            ContainerStatus::Restarting
        } else if s.contains("dead") {
            ContainerStatus::Dead
        } else {
            ContainerStatus::Unknown
        }
    }
}

impl ContainerInfo {
    /// Get suggested action for this container
    pub fn get_suggested_action(&self) -> OptimizationAction {
        match self.status {
            ContainerStatus::Running => {
                if self.memory_mb > 2000.0 {
                    OptimizationAction::StopContainer
                } else if self.is_idle && self.memory_mb > 500.0 {
                    OptimizationAction::PauseContainer
                } else {
                    OptimizationAction::None
                }
            }
            ContainerStatus::Paused => OptimizationAction::None,
            _ => OptimizationAction::None,
        }
    }
}

/// Docker manager
pub struct DockerManager {
    containers: Vec<ContainerInfo>,
    docker_available: bool,
    last_update: std::time::Instant,
}

impl DockerManager {
    pub fn new() -> Self {
        let mut manager = Self {
            containers: Vec::new(),
            docker_available: false,
            last_update: std::time::Instant::now(),
        };
        manager.check_docker();
        manager
    }

    /// Check if Docker is available
    fn check_docker(&mut self) {
        self.docker_available = Command::new("docker")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }

    /// Check if Docker is available
    pub fn is_available(&self) -> bool {
        self.docker_available
    }

    /// Refresh container data
    pub fn refresh(&mut self) {
        if !self.docker_available {
            return;
        }

        self.containers.clear();

        // Get container list with stats
        // Format: ID|Name|Image|Status|Memory|MemLimit|MemPerc|CPUPerc|Created|Ports
        let output = Command::new("docker")
            .args([
                "stats",
                "--no-stream",
                "--format",
                "{{.ID}}|{{.Name}}|{{.Container}}|{{.MemUsage}}|{{.MemPerc}}|{{.CPUPerc}}",
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some(container) = self.parse_stats_line(line) {
                        self.containers.push(container);
                    }
                }
            }
        }

        // Get additional container info
        self.enrich_container_info();

        self.last_update = std::time::Instant::now();
    }

    /// Parse a docker stats output line
    fn parse_stats_line(&self, line: &str) -> Option<ContainerInfo> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 6 {
            return None;
        }

        let id = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();

        // Parse memory usage (e.g., "100MiB / 2GiB")
        let mem_parts: Vec<&str> = parts[3].split('/').collect();
        let memory_mb = parse_memory_string(mem_parts.get(0).unwrap_or(&"0"));
        let memory_limit_mb = parse_memory_string(mem_parts.get(1).unwrap_or(&"0"));

        // Parse percentages
        let memory_percent = parts[4]
            .trim()
            .trim_end_matches('%')
            .parse::<f64>()
            .unwrap_or(0.0);
        let cpu_percent = parts[5]
            .trim()
            .trim_end_matches('%')
            .parse::<f64>()
            .unwrap_or(0.0);

        Some(ContainerInfo {
            id,
            name,
            image: String::new(),
            status: ContainerStatus::Running,
            memory_mb,
            memory_limit_mb,
            memory_percent,
            cpu_percent,
            created: String::new(),
            ports: Vec::new(),
            is_idle: cpu_percent < 1.0,
        })
    }

    /// Get additional container info via docker inspect
    fn enrich_container_info(&mut self) {
        for container in &mut self.containers {
            // Get image and status
            let output = Command::new("docker")
                .args([
                    "inspect",
                    "--format",
                    "{{.Config.Image}}|{{.State.Status}}|{{.Created}}",
                    &container.id,
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = stdout.trim().split('|').collect();
                    if parts.len() >= 3 {
                        container.image = parts[0].to_string();
                        container.status = ContainerStatus::from_str(parts[1]);
                        container.created = parts[2].to_string();
                    }
                }
            }

            // Get ports
            let output = Command::new("docker")
                .args(["port", &container.id])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    container.ports = stdout.lines().map(|s| s.to_string()).collect();
                }
            }
        }
    }

    /// Get all containers
    pub fn get_containers(&self) -> &[ContainerInfo] {
        &self.containers
    }

    /// Get running containers
    pub fn get_running(&self) -> Vec<&ContainerInfo> {
        self.containers
            .iter()
            .filter(|c| c.status == ContainerStatus::Running)
            .collect()
    }

    /// Get total memory usage
    pub fn total_memory_mb(&self) -> f64 {
        self.containers.iter().map(|c| c.memory_mb).sum()
    }

    /// Get idle containers (running but low CPU)
    pub fn get_idle_containers(&self) -> Vec<&ContainerInfo> {
        self.containers
            .iter()
            .filter(|c| c.status == ContainerStatus::Running && c.is_idle)
            .collect()
    }

    /// Pause a container
    pub fn pause_container(&self, id: &str) -> OptimizationResult {
        let output = Command::new("docker")
            .args(["pause", id])
            .output();

        let container_name = self.containers
            .iter()
            .find(|c| c.id == id || c.name == id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| id.to_string());

        match output {
            Ok(output) if output.status.success() => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::PauseContainer,
                success: true,
                memory_freed_mb: 0.0, // Pausing doesn't free memory but stops CPU usage
                message: "Container paused successfully".to_string(),
            },
            Ok(output) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::PauseContainer,
                success: false,
                memory_freed_mb: 0.0,
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Err(e) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::PauseContainer,
                success: false,
                memory_freed_mb: 0.0,
                message: e.to_string(),
            },
        }
    }

    /// Unpause a container
    pub fn unpause_container(&self, id: &str) -> OptimizationResult {
        let output = Command::new("docker")
            .args(["unpause", id])
            .output();

        let container_name = self.containers
            .iter()
            .find(|c| c.id == id || c.name == id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| id.to_string());

        match output {
            Ok(output) if output.status.success() => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::None,
                success: true,
                memory_freed_mb: 0.0,
                message: "Container unpaused successfully".to_string(),
            },
            Ok(output) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::None,
                success: false,
                memory_freed_mb: 0.0,
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Err(e) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::None,
                success: false,
                memory_freed_mb: 0.0,
                message: e.to_string(),
            },
        }
    }

    /// Stop a container
    pub fn stop_container(&self, id: &str) -> OptimizationResult {
        let container = self.containers
            .iter()
            .find(|c| c.id == id || c.name == id);

        let (container_name, memory_freed) = container
            .map(|c| (c.name.clone(), c.memory_mb))
            .unwrap_or_else(|| (id.to_string(), 0.0));

        let output = Command::new("docker")
            .args(["stop", id])
            .output();

        match output {
            Ok(output) if output.status.success() => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::StopContainer,
                success: true,
                memory_freed_mb: memory_freed,
                message: format!("Container stopped, freed ~{:.0} MB", memory_freed),
            },
            Ok(output) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::StopContainer,
                success: false,
                memory_freed_mb: 0.0,
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Err(e) => OptimizationResult {
                app_name: container_name,
                action: OptimizationAction::StopContainer,
                success: false,
                memory_freed_mb: 0.0,
                message: e.to_string(),
            },
        }
    }

    /// Get optimization suggestions
    pub fn get_suggestions(&self) -> Vec<(String, OptimizationAction, String)> {
        let mut suggestions = Vec::new();

        for container in &self.containers {
            let action = container.get_suggested_action();
            if action != OptimizationAction::None {
                let reason = match &action {
                    OptimizationAction::StopContainer => {
                        format!(
                            "Container '{}' is using {:.0} MB - consider stopping if not needed",
                            container.name, container.memory_mb
                        )
                    }
                    OptimizationAction::PauseContainer => {
                        format!(
                            "Container '{}' is idle but using {:.0} MB - consider pausing",
                            container.name, container.memory_mb
                        )
                    }
                    _ => continue,
                };

                suggestions.push((container.name.clone(), action, reason));
            }
        }

        suggestions
    }

    /// Print container summary
    pub fn print_summary(&self) {
        if !self.docker_available {
            println!("\n🐳 Docker: Not available or not running");
            return;
        }

        println!("\n🐳 Docker Container Resource Usage\n");

        if self.containers.is_empty() {
            println!("No running containers found.");
            return;
        }

        println!("┌──────────────────────┬───────────┬──────────┬──────────┬──────────┐");
        println!("│ Container            │ Memory    │ CPU      │ Status   │ Image    │");
        println!("├──────────────────────┼───────────┼──────────┼──────────┼──────────┤");

        let mut containers: Vec<_> = self.containers.iter().collect();
        containers.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap());

        for container in &containers {
            let status = match container.status {
                ContainerStatus::Running if container.is_idle => "💤 Idle",
                ContainerStatus::Running => "🟢 Running",
                ContainerStatus::Paused => "⏸️ Paused",
                ContainerStatus::Exited => "⏹️ Exited",
                _ => "❓ Unknown",
            };

            let image = truncate(&container.image, 8);

            println!(
                "│ {:20} │ {:>7.0} MB │ {:>6.1}%  │ {:8} │ {:8} │",
                truncate(&container.name, 20),
                container.memory_mb,
                container.cpu_percent,
                status,
                image
            );
        }

        println!("└──────────────────────┴───────────┴──────────┴──────────┴──────────┘");

        let total: f64 = containers.iter().map(|c| c.memory_mb).sum();
        let running = containers.iter().filter(|c| c.status == ContainerStatus::Running).count();
        let idle = self.get_idle_containers().len();

        println!(
            "\nTotal: {:.0} MB across {} containers ({} running, {} idle)",
            total,
            containers.len(),
            running,
            idle
        );

        // Suggestions
        let suggestions = self.get_suggestions();
        if !suggestions.is_empty() {
            println!("\n💡 Suggestions:");
            for (_, _, reason) in suggestions.iter().take(3) {
                println!("   • {}", reason);
            }
        }
    }
}

impl Default for DockerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse memory string like "100MiB" or "2GiB" to MB
fn parse_memory_string(s: &str) -> f64 {
    let s = s.trim().to_lowercase();

    // Extract number and unit
    let mut num_str = String::new();
    let mut unit = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            num_str.push(c);
        } else if c.is_alphabetic() {
            unit.push(c);
        }
    }

    let num: f64 = num_str.parse().unwrap_or(0.0);

    match unit.as_str() {
        "b" => num / (1024.0 * 1024.0),
        "kib" | "kb" | "k" => num / 1024.0,
        "mib" | "mb" | "m" => num,
        "gib" | "gb" | "g" => num * 1024.0,
        "tib" | "tb" | "t" => num * 1024.0 * 1024.0,
        _ => num,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:width$}", s, width = max)
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
