//! WASM/Rust micro-kernel execution gate.
//!
//! The "ruvector gate" is a bounded computational kernel that:
//! 1. Reads sensor data
//! 2. Runs a computation (threshold comparison, mincut, or simple inference)
//! 3. Returns an ActionToken within the time budget
//!
//! On bare metal, this is compiled as native Rust (zero overhead).
//! For field updates, the kernel can be delivered as AoT-compiled WASM.

/// Action determined by the micro-kernel after processing sensor data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Action {
    /// No action needed — conditions nominal.
    Idle = 0,
    /// Alert condition detected — log and continue monitoring.
    Alert = 1,
    /// Critical condition — flag for next radio transmission.
    Critical = 2,
    /// Trigger actuation (e.g., open valve, sound alarm).
    Actuate = 3,
    /// Request radio transmission of accumulated data.
    Transmit = 4,
    /// Kernel execution fault — ran out of time or hit error.
    Fault = 255,
}

impl From<u8> for Action {
    fn from(v: u8) -> Self {
        match v {
            0 => Action::Idle,
            1 => Action::Alert,
            2 => Action::Critical,
            3 => Action::Actuate,
            4 => Action::Transmit,
            _ => Action::Fault,
        }
    }
}

/// Result token produced by the micro-kernel for each execution cycle.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionToken {
    /// Determined action.
    pub action: Action,
    /// Confidence level (0–255, where 255 = highest confidence).
    pub confidence: u8,
    /// Sensor reading that triggered this action (raw value, context-dependent).
    pub sensor_value: u16,
    /// Energy consumed during this execution (µJ), metered by the caller.
    pub energy_consumed_uj: u32,
    /// Cycle ID at which this token was produced.
    pub cycle_id: u32,
}

impl ActionToken {
    /// Create a fault token for when execution fails.
    pub fn fault(cycle_id: u32) -> Self {
        Self {
            action: Action::Fault,
            confidence: 0,
            sensor_value: 0,
            energy_consumed_uj: 0,
            cycle_id,
        }
    }
}

/// Trait for pluggable micro-kernel implementations.
///
/// Implement this trait to provide different computational kernels
/// that can be swapped at compile time or via WASM field updates.
pub trait MicroKernel {
    /// Execute the kernel with the given sensor reading.
    ///
    /// Must complete within `max_active_ms` milliseconds.
    /// Returns an ActionToken describing the determined response.
    fn execute(&mut self, sensor_value: u16, cycle_id: u32) -> ActionToken;

    /// Name/identifier of this kernel for telemetry.
    fn name(&self) -> &str;
}

/// Default threshold-comparison kernel.
///
/// Compares a sensor reading against configurable thresholds
/// to determine alert/critical/actuate actions.
pub struct ThresholdKernel {
    /// Below this value: Idle.
    pub alert_threshold: u16,
    /// Above this value: Critical.
    pub critical_threshold: u16,
    /// Above this value: Actuate.
    pub actuate_threshold: u16,
}

impl Default for ThresholdKernel {
    fn default() -> Self {
        Self {
            alert_threshold: 500,
            critical_threshold: 800,
            actuate_threshold: 950,
        }
    }
}

impl MicroKernel for ThresholdKernel {
    fn execute(&mut self, sensor_value: u16, cycle_id: u32) -> ActionToken {
        let (action, confidence) = if sensor_value >= self.actuate_threshold {
            (Action::Actuate, 255)
        } else if sensor_value >= self.critical_threshold {
            (Action::Critical, 200)
        } else if sensor_value >= self.alert_threshold {
            (Action::Alert, 150)
        } else {
            (Action::Idle, 255)
        };

        ActionToken {
            action,
            confidence,
            sensor_value,
            energy_consumed_uj: 0, // filled by caller
            cycle_id,
        }
    }

    fn name(&self) -> &str {
        "threshold-v1"
    }
}

/// MinCut-based anomaly detection kernel.
///
/// Maintains a sliding window of sensor values and detects when
/// the value distribution exhibits a structural break (high separability).
pub struct MinCutKernel {
    /// Sliding window of recent sensor values.
    window: [u16; 8],
    /// Current write position in the window.
    cursor: usize,
    /// Minimum number of samples before kernel is active.
    warmup: u8,
    /// Samples collected so far.
    collected: u8,
    /// Separability threshold (0–1000 fixed-point).
    separability_threshold: u16,
}

impl Default for MinCutKernel {
    fn default() -> Self {
        Self {
            window: [0; 8],
            cursor: 0,
            warmup: 4,
            collected: 0,
            separability_threshold: 500,
        }
    }
}

impl MinCutKernel {
    /// Compute a simple separability metric: max gap between sorted values.
    ///
    /// Returns a value 0–1000 where higher = more separable (anomalous).
    fn compute_separability(&self) -> u16 {
        let n = self.collected.min(8) as usize;
        if n < 2 {
            return 0;
        }

        // Copy and sort (insertion sort, tiny array)
        let mut sorted = [0u16; 8];
        sorted[..n].copy_from_slice(&self.window[..n]);
        for i in 1..n {
            let key = sorted[i];
            let mut j = i;
            while j > 0 && sorted[j - 1] > key {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key;
        }

        // Find maximum gap between consecutive sorted values
        let range = sorted[n - 1].saturating_sub(sorted[0]);
        if range == 0 {
            return 0;
        }

        let mut max_gap: u16 = 0;
        for i in 1..n {
            let gap = sorted[i].saturating_sub(sorted[i - 1]);
            if gap > max_gap {
                max_gap = gap;
            }
        }

        // Normalize gap to 0–1000 range relative to total range
        ((max_gap as u32 * 1000) / range as u32).min(1000) as u16
    }
}

impl MicroKernel for MinCutKernel {
    fn execute(&mut self, sensor_value: u16, cycle_id: u32) -> ActionToken {
        // Add to sliding window
        self.window[self.cursor] = sensor_value;
        self.cursor = (self.cursor + 1) % 8;
        self.collected = self.collected.saturating_add(1);

        if self.collected < self.warmup {
            return ActionToken {
                action: Action::Idle,
                confidence: 50, // low confidence during warmup
                sensor_value,
                energy_consumed_uj: 0,
                cycle_id,
            };
        }

        let separability = self.compute_separability();

        let (action, confidence) = if separability >= self.separability_threshold {
            (Action::Alert, (separability / 4).min(255) as u8)
        } else {
            (Action::Idle, 200)
        };

        ActionToken {
            action,
            confidence,
            sensor_value,
            energy_consumed_uj: 0,
            cycle_id,
        }
    }

    fn name(&self) -> &str {
        "mincut-v1"
    }
}

/// WASM gate that wraps a MicroKernel and enforces execution constraints.
pub struct WasmGate<K: MicroKernel> {
    /// The active micro-kernel.
    kernel: K,
    /// Total executions performed.
    execution_count: u32,
    /// Total faults observed.
    fault_count: u32,
    /// Last action token produced.
    last_token: Option<ActionToken>,
}

impl<K: MicroKernel> WasmGate<K> {
    /// Create a new WASM gate with the given kernel.
    pub fn new(kernel: K) -> Self {
        Self {
            kernel,
            execution_count: 0,
            fault_count: 0,
            last_token: None,
        }
    }

    /// Execute the micro-kernel with the given sensor value.
    ///
    /// Returns the ActionToken. The caller is responsible for metering
    /// energy consumption and filling in `energy_consumed_uj`.
    pub fn run(&mut self, sensor_value: u16, cycle_id: u32) -> ActionToken {
        let token = self.kernel.execute(sensor_value, cycle_id);
        self.execution_count = self.execution_count.saturating_add(1);

        if token.action == Action::Fault {
            self.fault_count = self.fault_count.saturating_add(1);
        }

        self.last_token = Some(token);
        token
    }

    /// Get the name of the active kernel.
    pub fn kernel_name(&self) -> &str {
        self.kernel.name()
    }

    /// Total number of executions.
    pub fn execution_count(&self) -> u32 {
        self.execution_count
    }

    /// Total number of faults.
    pub fn fault_count(&self) -> u32 {
        self.fault_count
    }

    /// Last action token produced.
    pub fn last_token(&self) -> Option<ActionToken> {
        self.last_token
    }

    /// Get mutable reference to kernel for configuration changes.
    pub fn kernel_mut(&mut self) -> &mut K {
        &mut self.kernel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_kernel_idle() {
        let mut kernel = ThresholdKernel::default();
        let token = kernel.execute(100, 0);
        assert_eq!(token.action, Action::Idle);
        assert_eq!(token.confidence, 255);
    }

    #[test]
    fn threshold_kernel_alert() {
        let mut kernel = ThresholdKernel::default();
        let token = kernel.execute(600, 1);
        assert_eq!(token.action, Action::Alert);
    }

    #[test]
    fn threshold_kernel_critical() {
        let mut kernel = ThresholdKernel::default();
        let token = kernel.execute(850, 2);
        assert_eq!(token.action, Action::Critical);
    }

    #[test]
    fn threshold_kernel_actuate() {
        let mut kernel = ThresholdKernel::default();
        let token = kernel.execute(1000, 3);
        assert_eq!(token.action, Action::Actuate);
        assert_eq!(token.confidence, 255);
    }

    #[test]
    fn mincut_kernel_warmup() {
        let mut kernel = MinCutKernel::default();
        // During warmup, should return Idle with low confidence
        let token = kernel.execute(100, 0);
        assert_eq!(token.action, Action::Idle);
        assert_eq!(token.confidence, 50);
    }

    #[test]
    fn mincut_kernel_nominal() {
        let mut kernel = MinCutKernel::default();
        // Feed identical values — no anomaly (zero range → zero separability)
        for i in 0..8 {
            kernel.execute(100, i);
        }
        let token = kernel.execute(100, 8);
        assert_eq!(token.action, Action::Idle);
    }

    #[test]
    fn mincut_kernel_anomaly() {
        let mut kernel = MinCutKernel {
            separability_threshold: 300,
            ..Default::default()
        };
        // Feed values with a clear gap: [100,100,100,100,900,900,900,900]
        for i in 0..4 {
            kernel.execute(100, i);
        }
        for i in 4..8 {
            kernel.execute(900, i);
        }
        let token = kernel.execute(900, 8);
        assert_eq!(token.action, Action::Alert);
    }

    #[test]
    fn wasm_gate_wraps_kernel() {
        let kernel = ThresholdKernel::default();
        let mut gate = WasmGate::new(kernel);

        let token = gate.run(100, 0);
        assert_eq!(token.action, Action::Idle);
        assert_eq!(gate.execution_count(), 1);
        assert_eq!(gate.fault_count(), 0);
        assert_eq!(gate.kernel_name(), "threshold-v1");
    }

    #[test]
    fn action_from_u8() {
        assert_eq!(Action::from(0), Action::Idle);
        assert_eq!(Action::from(1), Action::Alert);
        assert_eq!(Action::from(2), Action::Critical);
        assert_eq!(Action::from(3), Action::Actuate);
        assert_eq!(Action::from(4), Action::Transmit);
        assert_eq!(Action::from(99), Action::Fault);
        assert_eq!(Action::from(255), Action::Fault);
    }

    #[test]
    fn fault_token_creation() {
        let token = ActionToken::fault(42);
        assert_eq!(token.action, Action::Fault);
        assert_eq!(token.confidence, 0);
        assert_eq!(token.cycle_id, 42);
    }
}
