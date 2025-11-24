//! Time management for deterministic simulation

use std::time::{Duration, Instant};

/// Manages virtual time and clock cycles for the simulation
pub struct TimeManager {
    /// Current virtual time (nanoseconds since simulation start)
    current_time: u64,

    /// Clock frequency (Hz)
    clock_frequency: u64,

    /// Time resolution (nanoseconds)
    time_resolution: u64,

    /// Deterministic mode (pause real time)
    deterministic: bool,

    /// Real-time start (for wall-clock correlation)
    real_start: Instant,
}

impl TimeManager {
    pub fn new(clock_frequency_hz: u64) -> Self {
        Self {
            current_time: 0,
            clock_frequency: clock_frequency_hz,
            time_resolution: 1, // 1ns
            deterministic: false,
            real_start: Instant::now(),
        }
    }

    pub fn current_time(&self) -> u64 {
        self.current_time
    }

    pub fn current_cycle(&self) -> u64 {
        self.current_time * self.clock_frequency / 1_000_000_000
    }

    pub fn advance(&mut self, duration: Duration) {
        self.current_time += duration.as_nanos() as u64;
    }

    pub fn advance_cycles(&mut self, cycles: u64) {
        let ns_per_cycle = 1_000_000_000 / self.clock_frequency;
        self.current_time += cycles * ns_per_cycle;
    }

    pub fn set_deterministic(&mut self, enabled: bool) {
        self.deterministic = enabled;
        // Note: tokio::time::pause() and resume() require test-util feature
        // These are commented out for now to avoid compilation errors
        // if enabled {
        //     tokio::time::pause();
        // } else {
        //     tokio::time::resume();
        // }
    }

    pub fn wall_time_elapsed(&self) -> Duration {
        self.real_start.elapsed()
    }

    pub fn speedup(&self) -> f64 {
        let virtual_time = Duration::from_nanos(self.current_time);
        let real_time = self.wall_time_elapsed();

        if real_time.as_secs_f64() == 0.0 {
            return 0.0;
        }

        virtual_time.as_secs_f64() / real_time.as_secs_f64()
    }

    pub fn reset(&mut self) {
        self.current_time = 0;
        self.real_start = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_advancement() {
        let mut time = TimeManager::new(1_000_000_000); // 1 GHz
        assert_eq!(time.current_time(), 0);
        assert_eq!(time.current_cycle(), 0);

        time.advance_cycles(10);
        assert_eq!(time.current_cycle(), 10);

        time.advance(Duration::from_nanos(100));
        assert_eq!(time.current_time(), 110);
    }

    #[test]
    fn test_cycle_conversion() {
        let mut time = TimeManager::new(1_000_000_000); // 1 GHz

        // At 1 GHz, 1 cycle = 1 ns
        time.advance_cycles(1000);
        assert_eq!(time.current_time(), 1000);
        assert_eq!(time.current_cycle(), 1000);
    }
}
