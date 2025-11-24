//! Simulation results and statistics

use std::time::Duration;

/// Results from a Cognitum simulation run
#[derive(Debug, Clone)]
pub struct SimulationResults {
    /// Total cycles executed
    pub cycles: u64,

    /// Total instructions executed across all tiles
    pub instructions: u64,

    /// Wall-clock execution time
    pub execution_time: Duration,

    /// Number of RaceWay packets sent
    pub packets_sent: u64,

    /// Number of RaceWay packets received
    pub packets_received: u64,

    /// Number of active tiles at completion
    pub active_tiles: usize,

    /// Number of halted tiles
    pub halted_tiles: usize,

    /// Number of tiles with errors
    pub error_tiles: usize,

    /// Maximum stack depth observed
    pub max_stack_depth: usize,

    /// Total memory operations
    pub memory_operations: u64,
}

impl SimulationResults {
    /// Calculate instructions per cycle (IPC)
    pub fn ipc(&self) -> f64 {
        if self.cycles == 0 {
            0.0
        } else {
            self.instructions as f64 / self.cycles as f64
        }
    }

    /// Calculate cycles per second
    pub fn cycles_per_second(&self) -> f64 {
        let secs = self.execution_time.as_secs_f64();
        if secs == 0.0 {
            0.0
        } else {
            self.cycles as f64 / secs
        }
    }

    /// Calculate packet delivery ratio
    pub fn packet_delivery_ratio(&self) -> f64 {
        if self.packets_sent == 0 {
            1.0
        } else {
            self.packets_received as f64 / self.packets_sent as f64
        }
    }

    /// Check if simulation completed successfully
    pub fn is_success(&self) -> bool {
        self.error_tiles == 0
    }

    /// Get average memory operations per cycle
    pub fn memory_ops_per_cycle(&self) -> f64 {
        if self.cycles == 0 {
            0.0
        } else {
            self.memory_operations as f64 / self.cycles as f64
        }
    }
}

impl Default for SimulationResults {
    fn default() -> Self {
        Self {
            cycles: 0,
            instructions: 0,
            execution_time: Duration::ZERO,
            packets_sent: 0,
            packets_received: 0,
            active_tiles: 0,
            halted_tiles: 0,
            error_tiles: 0,
            max_stack_depth: 0,
            memory_operations: 0,
        }
    }
}

impl std::fmt::Display for SimulationResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Simulation Results:")?;
        writeln!(f, "  Cycles:           {}", self.cycles)?;
        writeln!(f, "  Instructions:     {}", self.instructions)?;
        writeln!(f, "  IPC:              {:.2}", self.ipc())?;
        writeln!(
            f,
            "  Execution Time:   {:.2}s",
            self.execution_time.as_secs_f64()
        )?;
        writeln!(f, "  Cycles/sec:       {:.2}", self.cycles_per_second())?;
        writeln!(f, "  Packets Sent:     {}", self.packets_sent)?;
        writeln!(f, "  Packets Received: {}", self.packets_received)?;
        writeln!(
            f,
            "  Delivery Ratio:   {:.2}%",
            self.packet_delivery_ratio() * 100.0
        )?;
        writeln!(f, "  Active Tiles:     {}", self.active_tiles)?;
        writeln!(f, "  Halted Tiles:     {}", self.halted_tiles)?;
        writeln!(f, "  Error Tiles:      {}", self.error_tiles)?;
        writeln!(f, "  Max Stack Depth:  {}", self.max_stack_depth)?;
        writeln!(f, "  Memory Ops:       {}", self.memory_operations)?;
        write!(f, "  Mem Ops/Cycle:    {:.2}", self.memory_ops_per_cycle())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_calculation() {
        let results = SimulationResults {
            cycles: 1000,
            instructions: 800,
            ..Default::default()
        };

        assert_eq!(results.ipc(), 0.8);
    }

    #[test]
    fn test_packet_delivery_ratio() {
        let results = SimulationResults {
            packets_sent: 100,
            packets_received: 95,
            ..Default::default()
        };

        assert_eq!(results.packet_delivery_ratio(), 0.95);
    }

    #[test]
    fn test_is_success() {
        let success = SimulationResults {
            error_tiles: 0,
            ..Default::default()
        };
        assert!(success.is_success());

        let failure = SimulationResults {
            error_tiles: 1,
            ..Default::default()
        };
        assert!(!failure.is_success());
    }
}
