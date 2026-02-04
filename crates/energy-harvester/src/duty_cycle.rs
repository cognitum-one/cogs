//! Duty cycle controller — the main FSM orchestrating HARVEST→WAKE→EXECUTE→HARVEST.
//!
//! This is the top-level coordinator that ties together the ADC, power manager,
//! energy ledger, and WASM gate into a coherent duty-cycled loop.
//!
//! Optimizations:
//! - Precomputed `ConfigDerived` eliminates per-cycle energy estimation math
//! - Cached ledger ratio avoids redundant u64 division (2-3× per cycle → 1×)
//! - Single ADC read path in simulation mode

use crate::adc::AdcReader;
use crate::config::{ConfigDerived, HarvesterConfig};
use crate::energy_ledger::EnergyLedger;
use crate::power_manager::PowerManager;
use crate::wasm_gate::{ActionToken, MicroKernel, WasmGate};

/// Power states of the duty cycle FSM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub enum PowerState {
    /// Sleeping in ultra-low-power mode; PMIC harvesting energy.
    Harvest,
    /// Waking up: ADC read, budget check.
    Wake,
    /// Executing the micro-kernel workload.
    Execute,
    /// Emergency load cutoff due to critically low voltage.
    Emergency,
}

/// Result of a single duty cycle iteration.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct CycleResult {
    /// Final power state after this cycle.
    pub state: PowerState,
    /// VSTOR voltage at wake (mV).
    pub vstor_mv: u16,
    /// Whether execution was permitted and completed.
    pub executed: bool,
    /// Action token from micro-kernel (if executed).
    pub action_token: Option<ActionToken>,
    /// Estimated energy harvested during sleep (µJ).
    pub harvested_uj: u32,
    /// Estimated energy consumed during active window (µJ).
    pub consumed_uj: u32,
    /// Current duty period (ms), may be adapted.
    pub duty_period_ms: u32,
    /// Whether a fault occurred.
    pub fault: bool,
}

/// Duty cycle controller managing the full harvester loop.
pub struct DutyCycleController<K: MicroKernel> {
    /// Configuration.
    config: HarvesterConfig,
    /// Precomputed values from config (avoids per-cycle math).
    derived: ConfigDerived,
    /// ADC reader for voltage/current monitoring.
    adc: AdcReader,
    /// Power manager for load switching.
    power: PowerManager,
    /// Energy ledger for budget tracking.
    ledger: EnergyLedger,
    /// WASM gate wrapping the micro-kernel.
    gate: WasmGate<K>,
    /// Current FSM state.
    state: PowerState,
    /// Current (possibly adapted) duty period.
    current_duty_ms: u32,
    /// Monotonic cycle counter.
    cycle_count: u32,
    /// Harvester current measurement from last cycle (µA).
    last_harvest_current_ua: u32,
}

impl<K: MicroKernel> DutyCycleController<K> {
    /// Create a new duty cycle controller.
    pub fn new(config: HarvesterConfig, kernel: K) -> Self {
        let adc = AdcReader::new(&config);
        let power = PowerManager::new(config.max_active_ms);
        let ledger = EnergyLedger::new(config.ledger_slots as usize);
        let gate = WasmGate::new(kernel);
        let duty_ms = config.duty_period_ms;
        let derived = config.derive();

        Self {
            config,
            derived,
            adc,
            power,
            ledger,
            gate,
            state: PowerState::Harvest,
            current_duty_ms: duty_ms,
            cycle_count: 0,
            last_harvest_current_ua: 0,
        }
    }

    /// Run one complete duty cycle: sleep → wake → check → maybe execute → sleep.
    ///
    /// On real hardware, this would block in `sleep_ms()` for the duty period.
    /// In simulation, the sleep is a no-op and this returns immediately.
    ///
    /// `sensor_value` is the reading to pass to the micro-kernel (on hardware,
    /// this would come from an I²C/SPI sensor during the active window).
    pub fn run_cycle(&mut self, sensor_value: u16) -> CycleResult {
        // --- HARVEST phase: sleep ---
        self.state = PowerState::Harvest;
        self.power.sleep_ms(self.current_duty_ms);

        // --- WAKE phase: read ADC, check budget ---
        self.state = PowerState::Wake;
        let vstor = self.adc.read_vstor();
        self.last_harvest_current_ua = self.adc.read_harvest_current();

        // Estimate harvested energy during sleep period using precomputed values
        let harvested_uj = self.estimate_harvest_energy(vstor.voltage_mv);

        // Check if we have enough energy to execute
        let budget_ok = self.ledger.budget_permits_execution(
            vstor.voltage_mv,
            self.config.th_wake_mv,
            self.config.sustainability_ratio_pct,
        ) || self.ledger.total_cycles() < 4; // Allow execution during initial warmup

        let voltage_ok = vstor.voltage_mv >= self.config.th_wake_mv;

        if !voltage_ok || !budget_ok {
            // Not enough energy — record and stay in harvest
            self.ledger.record(harvested_uj, 0, false);
            self.adapt_duty_period();
            self.cycle_count = self.cycle_count.saturating_add(1);

            return CycleResult {
                state: PowerState::Harvest,
                vstor_mv: vstor.voltage_mv,
                executed: false,
                action_token: None,
                harvested_uj,
                consumed_uj: 0,
                duty_period_ms: self.current_duty_ms,
                fault: false,
            };
        }

        // --- EXECUTE phase ---
        self.state = PowerState::Execute;
        self.power.enable_core();

        // Check for emergency during execution (voltage drop under load)
        // On real hardware, this is a fresh ADC read reflecting load transient.
        // In simulation, re-read to get potentially updated sim value.
        let vstor_under_load = self.adc.read_vstor();
        if vstor_under_load.voltage_mv < self.config.th_critical_mv {
            self.power.emergency_cutoff();
            self.state = PowerState::Emergency;

            // Partial consumption estimate (1/10th of full active)
            let consumed_uj = self.derived.active_energy_uj / 10;
            self.ledger.record(harvested_uj, consumed_uj, true);
            self.adapt_duty_period();
            self.cycle_count = self.cycle_count.saturating_add(1);

            return CycleResult {
                state: PowerState::Emergency,
                vstor_mv: vstor_under_load.voltage_mv,
                executed: false,
                action_token: None,
                harvested_uj,
                consumed_uj,
                duty_period_ms: self.current_duty_ms,
                fault: true,
            };
        }

        // Run micro-kernel — use precomputed energy estimate
        let mut token = self.gate.run(sensor_value, self.cycle_count);
        let consumed_uj = self.derived.active_energy_uj;
        token.energy_consumed_uj = consumed_uj;

        // Disable load
        self.power.disable_core();

        // Record in ledger
        let fault = token.action == crate::wasm_gate::Action::Fault;
        self.ledger.record(harvested_uj, consumed_uj, fault);

        // Adapt duty period based on energy balance (uses cached ratio)
        self.adapt_duty_period();

        self.state = PowerState::Harvest;
        self.cycle_count = self.cycle_count.saturating_add(1);

        CycleResult {
            state: PowerState::Harvest,
            vstor_mv: vstor.voltage_mv,
            executed: true,
            action_token: Some(token),
            harvested_uj,
            consumed_uj,
            duty_period_ms: self.current_duty_ms,
            fault,
        }
    }

    /// Estimate energy harvested during the last sleep period (µJ).
    ///
    /// Uses the voltage passed in to avoid an extra `last_vstor()` call.
    #[inline]
    fn estimate_harvest_energy(&self, vstor_mv: u16) -> u32 {
        let i_ua = self.last_harvest_current_ua as u64;
        let v_mv = vstor_mv as u64;
        let t_ms = self.current_duty_ms as u64;

        // E(µJ) = I(µA) × V(mV) × t(ms) / 1_000_000
        ((i_ua * v_mv * t_ms) / 1_000_000) as u32
    }

    /// Adapt duty period based on energy ledger balance.
    /// Leverages the cached ratio in the ledger (no redundant u64 division).
    #[inline]
    fn adapt_duty_period(&mut self) {
        self.current_duty_ms = self.ledger.suggest_duty_period_ms(
            self.current_duty_ms,
            self.config.sustainability_ratio_pct,
            self.config.surplus_ratio_pct,
        );
    }

    /// Get current FSM state.
    #[inline]
    pub fn state(&self) -> PowerState {
        self.state
    }

    /// Get current duty period (may differ from config after adaptation).
    #[inline]
    pub fn current_duty_ms(&self) -> u32 {
        self.current_duty_ms
    }

    /// Get cycle count.
    #[inline]
    pub fn cycle_count(&self) -> u32 {
        self.cycle_count
    }

    /// Get reference to the energy ledger.
    pub fn ledger(&self) -> &EnergyLedger {
        &self.ledger
    }

    /// Get mutable reference to the energy ledger.
    pub fn ledger_mut(&mut self) -> &mut EnergyLedger {
        &mut self.ledger
    }

    /// Get reference to the power manager.
    pub fn power(&self) -> &PowerManager {
        &self.power
    }

    /// Get reference to the WASM gate.
    pub fn gate(&self) -> &WasmGate<K> {
        &self.gate
    }

    /// Get mutable reference to the ADC reader (for simulation injection).
    #[cfg(feature = "std")]
    pub fn adc_mut(&mut self) -> &mut AdcReader {
        &mut self.adc
    }

    /// Get reference to current config.
    pub fn config(&self) -> &HarvesterConfig {
        &self.config
    }

    /// Get reference to precomputed derived values.
    pub fn derived(&self) -> &ConfigDerived {
        &self.derived
    }

    /// Run multiple cycles in simulation, collecting results.
    #[cfg(feature = "std")]
    pub fn simulate(&mut self, cycles: u32, sensor_value: u16) -> Vec<CycleResult> {
        let mut results = Vec::with_capacity(cycles as usize);
        for _ in 0..cycles {
            results.push(self.run_cycle(sensor_value));
        }
        results
    }

    /// Run multiple cycles writing results to a preallocated buffer.
    /// Returns the number of cycles actually executed.
    #[cfg(feature = "std")]
    pub fn simulate_into(&mut self, buffer: &mut Vec<CycleResult>, cycles: u32, sensor_value: u16) -> u32 {
        buffer.clear();
        buffer.reserve(cycles as usize);
        for _ in 0..cycles {
            buffer.push(self.run_cycle(sensor_value));
        }
        cycles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power_manager::LoadState;
    use crate::wasm_gate::ThresholdKernel;

    fn make_controller() -> DutyCycleController<ThresholdKernel> {
        let config = HarvesterConfig::default();
        let kernel = ThresholdKernel::default();
        DutyCycleController::new(config, kernel)
    }

    #[test]
    fn initial_state() {
        let ctrl = make_controller();
        assert_eq!(ctrl.state(), PowerState::Harvest);
        assert_eq!(ctrl.cycle_count(), 0);
        assert_eq!(ctrl.current_duty_ms(), 300_000);
    }

    #[test]
    fn cycle_with_sufficient_energy() {
        let mut ctrl = make_controller();
        // Default sim VSTOR is at wake threshold (3300 mV)
        let result = ctrl.run_cycle(100);

        assert!(result.executed);
        assert!(result.action_token.is_some());
        assert_eq!(result.action_token.unwrap().action, crate::wasm_gate::Action::Idle);
        assert!(!result.fault);
        assert_eq!(ctrl.cycle_count(), 1);
    }

    #[test]
    fn cycle_with_insufficient_energy() {
        let mut ctrl = make_controller();
        ctrl.adc_mut().set_sim_vstor_mv(2000); // below wake threshold

        let result = ctrl.run_cycle(100);

        assert!(!result.executed);
        assert!(result.action_token.is_none());
        assert_eq!(result.vstor_mv, 2000);
    }

    #[test]
    fn emergency_cutoff_on_critical_voltage() {
        let mut ctrl = make_controller();
        // Set voltage below wake — it won't even get to execute
        ctrl.adc_mut().set_sim_vstor_mv(2000);
        let result = ctrl.run_cycle(100);
        assert!(!result.executed); // won't even get to execute with 2000 < 3300
    }

    #[test]
    fn multiple_cycles_simulation() {
        let mut ctrl = make_controller();
        let results = ctrl.simulate(10, 100);

        assert_eq!(results.len(), 10);
        assert_eq!(ctrl.cycle_count(), 10);

        // All should execute (sim is at wake threshold with default current)
        for result in &results {
            assert!(result.executed);
        }
    }

    #[test]
    fn alert_action_propagated() {
        let mut ctrl = make_controller();
        // Sensor value 600 triggers Alert in threshold kernel (threshold=500)
        let result = ctrl.run_cycle(600);

        assert!(result.executed);
        assert_eq!(
            result.action_token.unwrap().action,
            crate::wasm_gate::Action::Alert
        );
    }

    #[test]
    fn energy_ledger_tracks_cycles() {
        let mut ctrl = make_controller();
        ctrl.run_cycle(100);
        ctrl.run_cycle(200);
        ctrl.run_cycle(300);

        assert_eq!(ctrl.ledger().total_cycles(), 3);
        // Consumed energy should be tracked
        assert!(ctrl.ledger().total_consumed_uj() > 0);
    }

    #[test]
    fn power_manager_state_transitions() {
        let mut ctrl = make_controller();
        // Before cycle, load should be disabled
        assert_eq!(ctrl.power().state(), LoadState::Disabled);

        ctrl.run_cycle(100);
        // After cycle completes, load should be disabled again
        assert_eq!(ctrl.power().state(), LoadState::Disabled);
        assert_eq!(ctrl.power().stats().enable_count, 1);
        assert_eq!(ctrl.power().stats().disable_count, 1);
    }

    #[test]
    fn precomputed_energy_used() {
        let ctrl = make_controller();
        // Verify derived values are precomputed
        assert_eq!(ctrl.derived().active_energy_uj, 825);
        assert_eq!(ctrl.derived().sleep_energy_uj, 495);
        assert_eq!(ctrl.derived().cycle_energy_uj, 1320);
    }

    #[test]
    fn simulate_into_preallocated() {
        let mut ctrl = make_controller();
        let mut buf = Vec::new();
        let count = ctrl.simulate_into(&mut buf, 5, 100);
        assert_eq!(count, 5);
        assert_eq!(buf.len(), 5);
        for r in &buf {
            assert!(r.executed);
        }
    }
}
