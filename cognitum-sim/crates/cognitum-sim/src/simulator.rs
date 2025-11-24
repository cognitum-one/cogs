//! Main simulator implementation

use cognitum_core::{Result, types::SimTime};
use cognitum_processor::A2SCpu;

/// Cognitum ASIC simulator
pub struct Simulator {
    /// CPU instance
    cpu: A2SCpu,
    /// Current simulation time
    time: SimTime,
}

impl Simulator {
    /// Create a new simulator
    pub fn new() -> Self {
        Self {
            cpu: A2SCpu::new(),
            time: 0,
        }
    }

    /// Run simulation for specified number of cycles
    pub fn run(&mut self, cycles: u64) -> Result<()> {
        for _ in 0..cycles {
            self.cpu.step()?;
            self.time += 1;
        }
        Ok(())
    }

    /// Get current simulation time
    pub fn time(&self) -> SimTime {
        self.time
    }
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}
