//! Telemetry and structured logging for energy harvester instrumentation.
//!
//! On bare metal, outputs via UART/RTT using defmt.
//! On host/std, outputs to stdout or a provided writer.
//! Designed for minimal overhead — all formatting is deferred.

use crate::duty_cycle::CycleResult;
use crate::energy_ledger::LedgerSummary;
use crate::power_manager::PowerStats;

/// Telemetry event types emitted by the harvester.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub enum TelemetryEvent {
    /// Cycle completed — full result.
    CycleComplete(CycleReport),
    /// Energy ledger summary (periodic).
    LedgerSummary(LedgerSummary),
    /// Power statistics snapshot.
    PowerStats(PowerStats),
    /// Duty period adapted.
    DutyAdapted {
        old_ms: u32,
        new_ms: u32,
        reason: AdaptReason,
    },
    /// System fault.
    Fault(FaultReport),
    /// Boot/startup event.
    Boot(BootReport),
}

/// Reason for duty period adaptation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub enum AdaptReason {
    /// Energy deficit detected — backing off.
    EnergyDeficit,
    /// Energy surplus detected — increasing frequency.
    EnergySurplus,
    /// Manual override.
    Manual,
}

/// Compact cycle report for telemetry output.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct CycleReport {
    pub cycle_id: u32,
    pub vstor_mv: u16,
    pub executed: bool,
    pub action: u8,
    pub confidence: u8,
    pub sensor_value: u16,
    pub harvested_uj: u32,
    pub consumed_uj: u32,
    pub duty_period_ms: u32,
    pub fault: bool,
}

impl From<&CycleResult> for CycleReport {
    fn from(r: &CycleResult) -> Self {
        let (action, confidence, sensor_value) = match r.action_token {
            Some(t) => (t.action as u8, t.confidence, t.sensor_value),
            None => (0, 0, 0),
        };
        CycleReport {
            cycle_id: r.action_token.map(|t| t.cycle_id).unwrap_or(0),
            vstor_mv: r.vstor_mv,
            executed: r.executed,
            action,
            confidence,
            sensor_value,
            harvested_uj: r.harvested_uj,
            consumed_uj: r.consumed_uj,
            duty_period_ms: r.duty_period_ms,
            fault: r.fault,
        }
    }
}

/// Fault report for telemetry.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct FaultReport {
    pub cycle_id: u32,
    pub fault_type: FaultType,
    pub vstor_mv: u16,
}

/// Types of faults that can occur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub enum FaultType {
    /// VSTOR dropped below critical threshold during execution.
    EmergencyCutoff,
    /// Watchdog timer expired during execution.
    WatchdogTimeout,
    /// Micro-kernel returned fault action.
    KernelFault,
    /// ADC read failure.
    AdcFault,
}

/// Boot report emitted at system startup.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct BootReport {
    pub firmware_version: u32,
    pub kernel_name: &'static str,
    pub config_th_wake_mv: u16,
    pub config_th_sleep_mv: u16,
    pub config_th_critical_mv: u16,
    pub config_duty_ms: u32,
    pub config_max_active_ms: u16,
}

/// Telemetry sink that collects events.
///
/// On host/std builds, events are collected in a Vec.
/// On bare metal, events would be written to UART/RTT via defmt.
#[cfg(feature = "std")]
pub struct TelemetrySink {
    events: Vec<TelemetryEvent>,
    max_events: usize,
}

#[cfg(feature = "std")]
impl TelemetrySink {
    /// Create a new telemetry sink with bounded event buffer.
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Vec::with_capacity(max_events.min(1024)),
            max_events,
        }
    }

    /// Record a telemetry event.
    pub fn record(&mut self, event: TelemetryEvent) {
        if self.events.len() < self.max_events {
            self.events.push(event);
        }
    }

    /// Record a cycle result as a telemetry event.
    pub fn record_cycle(&mut self, result: &CycleResult) {
        self.record(TelemetryEvent::CycleComplete(CycleReport::from(result)));
    }

    /// Get all recorded events.
    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Number of recorded events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Format a cycle report as a compact CSV line.
    pub fn format_csv(report: &CycleReport) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            report.cycle_id,
            report.vstor_mv,
            report.executed as u8,
            report.action,
            report.confidence,
            report.sensor_value,
            report.harvested_uj,
            report.consumed_uj,
            report.duty_period_ms,
            report.fault as u8,
        )
    }

    /// CSV header for cycle reports.
    pub fn csv_header() -> &'static str {
        "cycle_id,vstor_mv,executed,action,confidence,sensor_value,harvested_uj,consumed_uj,duty_period_ms,fault"
    }

    /// Generate a text summary of the current telemetry buffer.
    pub fn summary(&self) -> String {
        let total = self.events.len();
        let cycles = self
            .events
            .iter()
            .filter(|e| matches!(e, TelemetryEvent::CycleComplete(_)))
            .count();
        let faults = self
            .events
            .iter()
            .filter(|e| matches!(e, TelemetryEvent::Fault(_)))
            .count();

        format!(
            "Telemetry: {} events ({} cycles, {} faults)",
            total, cycles, faults
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duty_cycle::PowerState;

    #[test]
    fn cycle_report_from_result() {
        let result = CycleResult {
            state: PowerState::Harvest,
            vstor_mv: 3300,
            executed: true,
            action_token: Some(crate::wasm_gate::ActionToken {
                action: crate::wasm_gate::Action::Alert,
                confidence: 150,
                sensor_value: 600,
                energy_consumed_uj: 825,
                cycle_id: 42,
            }),
            harvested_uj: 76500,
            consumed_uj: 825,
            duty_period_ms: 300_000,
            fault: false,
        };

        let report = CycleReport::from(&result);
        assert_eq!(report.cycle_id, 42);
        assert_eq!(report.vstor_mv, 3300);
        assert!(report.executed);
        assert_eq!(report.action, 1); // Alert
        assert_eq!(report.confidence, 150);
    }

    #[cfg(feature = "std")]
    #[test]
    fn telemetry_sink_records_events() {
        let mut sink = TelemetrySink::new(100);

        let result = CycleResult {
            state: PowerState::Harvest,
            vstor_mv: 3300,
            executed: true,
            action_token: None,
            harvested_uj: 1000,
            consumed_uj: 500,
            duty_period_ms: 300_000,
            fault: false,
        };

        sink.record_cycle(&result);
        assert_eq!(sink.event_count(), 1);

        let summary = sink.summary();
        assert!(summary.contains("1 events"));
        assert!(summary.contains("1 cycles"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn csv_formatting() {
        let report = CycleReport {
            cycle_id: 1,
            vstor_mv: 3300,
            executed: true,
            action: 0,
            confidence: 255,
            sensor_value: 100,
            harvested_uj: 76500,
            consumed_uj: 825,
            duty_period_ms: 300000,
            fault: false,
        };

        let csv = TelemetrySink::format_csv(&report);
        assert!(csv.contains("3300"));
        assert!(csv.contains("76500"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn sink_respects_max_events() {
        let mut sink = TelemetrySink::new(2);

        for _ in 0..5 {
            sink.record(TelemetryEvent::PowerStats(PowerStats::default()));
        }

        assert_eq!(sink.event_count(), 2); // capped at max
    }
}
