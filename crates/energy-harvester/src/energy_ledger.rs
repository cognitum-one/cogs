//! Energy ledger for tracking harvested vs consumed energy over rolling windows.
//!
//! Uses fixed-point microjoule (µJ) accounting to avoid floating-point
//! operations on bare-metal targets. Maintains a circular buffer of energy
//! slots, each representing one duty cycle's energy transactions.

use heapless::Vec;

/// A single energy transaction record for one duty cycle.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct EnergySlot {
    /// Energy harvested during this cycle's sleep period (µJ).
    pub harvested_uj: u32,
    /// Energy consumed during this cycle's active window (µJ).
    pub consumed_uj: u32,
    /// Cycle sequence number (monotonic counter).
    pub cycle_id: u32,
    /// Whether this cycle had a fault (watchdog, emergency cutoff).
    pub fault: bool,
}

impl EnergySlot {
    /// Net energy for this slot (positive = surplus, may underflow if consumed > harvested).
    pub fn net_uj(&self) -> i32 {
        self.harvested_uj as i32 - self.consumed_uj as i32
    }
}

/// Rolling-window energy ledger.
///
/// Stores the most recent N slots in a circular buffer.
/// Computes aggregate metrics for energy sustainability analysis.
pub struct EnergyLedger {
    /// Circular buffer of energy slots. Capacity set at construction.
    slots: Vec<EnergySlot, 256>, // Max 256 slots for no_std; resize for std
    /// Write cursor (next slot to overwrite).
    cursor: usize,
    /// Total number of cycles recorded (may exceed capacity).
    total_cycles: u32,
    /// Running sum of harvested energy across all slots in buffer (µJ).
    sum_harvested_uj: u64,
    /// Running sum of consumed energy across all slots in buffer (µJ).
    sum_consumed_uj: u64,
    /// Number of fault events recorded.
    fault_count: u32,
    /// Maximum capacity (number of slots).
    capacity: usize,
}

impl EnergyLedger {
    /// Create a new energy ledger with the given slot capacity.
    ///
    /// Capacity is clamped to 256 for `no_std` builds using heapless::Vec.
    pub fn new(capacity: usize) -> Self {
        let capped = capacity.min(256);
        Self {
            slots: Vec::new(),
            cursor: 0,
            total_cycles: 0,
            sum_harvested_uj: 0,
            sum_consumed_uj: 0,
            fault_count: 0,
            capacity: capped,
        }
    }

    /// Record a new energy slot for a completed duty cycle.
    pub fn record(&mut self, harvested_uj: u32, consumed_uj: u32, fault: bool) {
        let slot = EnergySlot {
            harvested_uj,
            consumed_uj,
            cycle_id: self.total_cycles,
            fault,
        };

        if self.slots.len() < self.capacity {
            // Buffer not yet full — append
            let _ = self.slots.push(slot);
        } else {
            // Circular overwrite — subtract old slot's values from running sums
            let old = &self.slots[self.cursor];
            self.sum_harvested_uj = self.sum_harvested_uj.saturating_sub(old.harvested_uj as u64);
            self.sum_consumed_uj = self.sum_consumed_uj.saturating_sub(old.consumed_uj as u64);
            if old.fault {
                self.fault_count = self.fault_count.saturating_sub(1);
            }
            self.slots[self.cursor] = slot;
        }

        // Update running sums
        self.sum_harvested_uj = self.sum_harvested_uj.saturating_add(harvested_uj as u64);
        self.sum_consumed_uj = self.sum_consumed_uj.saturating_add(consumed_uj as u64);
        if fault {
            self.fault_count = self.fault_count.saturating_add(1);
        }

        self.total_cycles = self.total_cycles.saturating_add(1);
        self.cursor = (self.cursor + 1) % self.capacity;
    }

    /// Total harvested energy across the rolling window (µJ).
    pub fn total_harvested_uj(&self) -> u64 {
        self.sum_harvested_uj
    }

    /// Total consumed energy across the rolling window (µJ).
    pub fn total_consumed_uj(&self) -> u64 {
        self.sum_consumed_uj
    }

    /// Net energy balance across the window (µJ, signed).
    pub fn net_balance_uj(&self) -> i64 {
        self.sum_harvested_uj as i64 - self.sum_consumed_uj as i64
    }

    /// Energy balance ratio (×100 for fixed-point percentage).
    ///
    /// Returns `harvested * 100 / consumed`. Returns `u16::MAX` if consumed is zero.
    pub fn balance_ratio_pct(&self) -> u16 {
        if self.sum_consumed_uj == 0 {
            return u16::MAX;
        }
        let ratio = (self.sum_harvested_uj * 100) / self.sum_consumed_uj;
        ratio.min(u16::MAX as u64) as u16
    }

    /// Check if energy budget is sustainable (ratio >= threshold).
    pub fn is_sustainable(&self, threshold_pct: u16) -> bool {
        self.balance_ratio_pct() >= threshold_pct
    }

    /// Check if energy surplus permits extra activity (ratio >= surplus threshold).
    pub fn has_surplus(&self, surplus_pct: u16) -> bool {
        self.balance_ratio_pct() >= surplus_pct
    }

    /// Number of cycles recorded in the ledger (may exceed window size).
    pub fn total_cycles(&self) -> u32 {
        self.total_cycles
    }

    /// Number of active slots in the buffer.
    pub fn active_slots(&self) -> usize {
        self.slots.len()
    }

    /// Number of fault events in the current window.
    pub fn fault_count(&self) -> u32 {
        self.fault_count
    }

    /// Get a specific slot by index (0 = oldest in buffer).
    pub fn get_slot(&self, index: usize) -> Option<&EnergySlot> {
        self.slots.get(index)
    }

    /// Check if the energy budget permits one more execution cycle.
    ///
    /// Returns true if the current VSTOR voltage is above the wake threshold
    /// AND the energy ledger indicates sustainability.
    pub fn budget_permits_execution(
        &self,
        vstor_mv: u16,
        th_wake_mv: u16,
        sustainability_pct: u16,
    ) -> bool {
        vstor_mv >= th_wake_mv && self.is_sustainable(sustainability_pct)
    }

    /// Suggest adjusted duty period based on energy balance.
    ///
    /// - If ratio < sustainability: increase duty period (less frequent wakes)
    /// - If ratio > surplus: decrease duty period (more frequent wakes)
    /// - Otherwise: keep current period
    pub fn suggest_duty_period_ms(
        &self,
        current_period_ms: u32,
        sustainability_pct: u16,
        surplus_pct: u16,
    ) -> u32 {
        let ratio = self.balance_ratio_pct();

        if ratio < sustainability_pct {
            // Energy deficit — back off
            current_period_ms
                .saturating_mul(2)
                .min(600_000) // max 10 minutes
        } else if ratio > surplus_pct {
            // Energy surplus — can wake more often
            (current_period_ms / 2).max(60_000) // min 1 minute
        } else {
            current_period_ms
        }
    }

    /// Generate a summary report of the energy ledger state.
    pub fn summary(&self) -> LedgerSummary {
        LedgerSummary {
            total_cycles: self.total_cycles,
            active_slots: self.active_slots() as u16,
            total_harvested_uj: self.sum_harvested_uj,
            total_consumed_uj: self.sum_consumed_uj,
            net_balance_uj: self.net_balance_uj(),
            balance_ratio_pct: self.balance_ratio_pct(),
            fault_count: self.fault_count,
        }
    }
}

/// Summary of energy ledger state for telemetry reporting.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde-support", derive(serde::Serialize, serde::Deserialize))]
pub struct LedgerSummary {
    pub total_cycles: u32,
    pub active_slots: u16,
    pub total_harvested_uj: u64,
    pub total_consumed_uj: u64,
    pub net_balance_uj: i64,
    pub balance_ratio_pct: u16,
    pub fault_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ledger() {
        let ledger = EnergyLedger::new(16);
        assert_eq!(ledger.total_cycles(), 0);
        assert_eq!(ledger.active_slots(), 0);
        assert_eq!(ledger.total_harvested_uj(), 0);
        assert_eq!(ledger.balance_ratio_pct(), u16::MAX); // no consumption
    }

    #[test]
    fn record_and_accumulate() {
        let mut ledger = EnergyLedger::new(16);

        ledger.record(1000, 500, false);
        assert_eq!(ledger.total_harvested_uj(), 1000);
        assert_eq!(ledger.total_consumed_uj(), 500);
        assert_eq!(ledger.balance_ratio_pct(), 200); // 2.0×

        ledger.record(500, 500, false);
        assert_eq!(ledger.total_harvested_uj(), 1500);
        assert_eq!(ledger.total_consumed_uj(), 1000);
        assert_eq!(ledger.balance_ratio_pct(), 150); // 1.5×
    }

    #[test]
    fn circular_buffer_wraps() {
        let mut ledger = EnergyLedger::new(4);

        // Fill buffer
        for i in 0..4 {
            ledger.record(100 * (i + 1), 50, false);
        }
        assert_eq!(ledger.active_slots(), 4);
        // harvested: 100+200+300+400 = 1000
        assert_eq!(ledger.total_harvested_uj(), 1000);

        // Overwrite oldest (100 harvested, 50 consumed)
        ledger.record(500, 50, false);
        assert_eq!(ledger.active_slots(), 4);
        // harvested: 200+300+400+500 = 1400
        assert_eq!(ledger.total_harvested_uj(), 1400);
        assert_eq!(ledger.total_cycles(), 5);
    }

    #[test]
    fn sustainability_check() {
        let mut ledger = EnergyLedger::new(16);

        // Sustainable: harvested >> consumed
        ledger.record(1000, 100, false);
        assert!(ledger.is_sustainable(110)); // 1000% > 110%

        // Not sustainable: consumed > harvested
        let mut deficit_ledger = EnergyLedger::new(16);
        deficit_ledger.record(100, 200, false);
        assert!(!deficit_ledger.is_sustainable(110)); // 50% < 110%
    }

    #[test]
    fn duty_period_adjustment() {
        let mut ledger = EnergyLedger::new(16);

        // Deficit case — should double period
        ledger.record(50, 100, false); // ratio = 50%
        let adjusted = ledger.suggest_duty_period_ms(300_000, 110, 200);
        assert_eq!(adjusted, 600_000); // doubled, capped at max

        // Surplus case — should halve period
        let mut surplus = EnergyLedger::new(16);
        surplus.record(1000, 100, false); // ratio = 1000%
        let adjusted = surplus.suggest_duty_period_ms(300_000, 110, 200);
        assert_eq!(adjusted, 150_000); // halved
    }

    #[test]
    fn fault_tracking() {
        let mut ledger = EnergyLedger::new(16);

        ledger.record(100, 50, false);
        ledger.record(100, 50, true); // fault
        ledger.record(100, 50, true); // fault

        assert_eq!(ledger.fault_count(), 2);
    }

    #[test]
    fn budget_permits_execution() {
        let mut ledger = EnergyLedger::new(16);
        ledger.record(1000, 100, false);

        // Above wake threshold and sustainable
        assert!(ledger.budget_permits_execution(3300, 3300, 110));

        // Below wake threshold
        assert!(!ledger.budget_permits_execution(2500, 3300, 110));
    }

    #[test]
    fn summary_report() {
        let mut ledger = EnergyLedger::new(16);
        ledger.record(1000, 500, false);
        ledger.record(800, 400, true);

        let summary = ledger.summary();
        assert_eq!(summary.total_cycles, 2);
        assert_eq!(summary.total_harvested_uj, 1800);
        assert_eq!(summary.total_consumed_uj, 900);
        assert_eq!(summary.balance_ratio_pct, 200);
        assert_eq!(summary.fault_count, 1);
    }
}
