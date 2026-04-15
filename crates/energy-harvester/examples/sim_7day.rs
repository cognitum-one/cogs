//! 7-day energy harvester simulation with realistic lighting profiles.
//!
//! Simulates a 10 cm² indoor PV cell under varying office lighting conditions:
//!   - Dawn ramp: 6:00–8:00 (0→500 lux)
//!   - Office day: 8:00–18:00 (300–500 lux)
//!   - Dusk ramp: 18:00–20:00 (500→0 lux)
//!   - Night: 20:00–6:00 (0–10 lux ambient)
//!
//! Outputs CSV to stdout. Pipe to a file for analysis.
//!
//! Usage:
//!   cargo run --example sim_7day > sim_output.csv

use energy_harvester::config::HarvesterConfig;
use energy_harvester::duty_cycle::DutyCycleController;
use energy_harvester::telemetry::{CycleReport, TelemetrySink};
use energy_harvester::wasm_gate::ThresholdKernel;

/// Simulated harvester current (µA) as a function of lux level.
/// 10 cm² indoor PV at ~15% conversion: ~0.15 mW at 500 lux → ~45 µA at 3.3V.
fn lux_to_harvest_current_ua(lux: u32) -> u32 {
    // Approximate: I(µA) ≈ lux × 0.09 (linear model for indoor PV)
    (lux as u64 * 9 / 100) as u32
}

/// Simulated VSTOR voltage (mV) based on net energy flow.
/// Simple RC model: voltage rises when harvesting, drops when consuming.
fn update_vstor(vstor_mv: u16, harvest_ua: u32, consumed: bool, duty_ms: u32) -> u16 {
    // Supercap model: 0.1F at 3.3V → Q = C×V = 330 mC
    // dV = I × t / C; I in µA, t in ms, C in F → dV(mV) = I(µA) × t(ms) / C(µF)
    // C = 100_000 µF (0.1 F)
    let c_uf: u64 = 100_000;
    let charge_mv = (harvest_ua as u64 * duty_ms as u64) / c_uf;

    let drain_mv = if consumed {
        // 5 mA for 50 ms = 250 µA·s; dV = 250_000 µA·ms / 100_000 µF = 2.5 mV
        (5000u64 * 50) / c_uf
    } else {
        0
    };

    let new_mv = vstor_mv as i32 + charge_mv as i32 - drain_mv as i32;
    new_mv.clamp(0, 5500) as u16
}

/// Get lux for a given minute-of-day (0–1439).
fn minute_to_lux(minute: u32) -> u32 {
    let hour = minute / 60;
    let frac = (minute % 60) as f64 / 60.0;

    match hour {
        0..=5 => 5,                                         // Night: ~5 lux ambient
        6..=7 => (5.0 + frac * 247.5 + (hour - 6) as f64 * 247.5) as u32, // Dawn ramp
        8..=17 => 400 + ((minute as f64 * 0.1).sin() * 100.0) as u32, // Office with variance
        18..=19 => {
            let decay = (hour - 18) as f64 + frac;
            (500.0 * (1.0 - decay / 2.0)).max(5.0) as u32
        }
        _ => 5, // Night
    }
}

fn main() {
    let config = HarvesterConfig::default();
    let kernel = ThresholdKernel::default();
    let mut ctrl = DutyCycleController::new(config, kernel);
    let mut sink = TelemetrySink::new(100_000);

    // Print CSV header
    println!("{},day,hour,minute,lux,harvest_ua,vstor_before_mv", TelemetrySink::csv_header());

    let duty_min = 5u32; // 5-minute duty cycle
    let cycles_per_day = 24 * 60 / duty_min;
    let total_days = 7u32;
    let mut vstor_mv: u16 = 2800; // Start partially charged

    let mut total_harvested_mj: u64 = 0;
    let mut total_consumed_mj: u64 = 0;
    let mut total_executed: u32 = 0;
    let mut total_skipped: u32 = 0;

    for day in 0..total_days {
        for cycle_in_day in 0..cycles_per_day {
            let minute = cycle_in_day * duty_min;
            let hour = minute / 60;
            let lux = minute_to_lux(minute);
            let harvest_ua = lux_to_harvest_current_ua(lux);

            // Inject simulated values
            ctrl.adc_mut().set_sim_vstor_mv(vstor_mv);
            ctrl.adc_mut().set_sim_current_ua(harvest_ua);

            // Generate a sensor value (simulated temperature: 20–30°C mapped to 200–300)
            let sensor_value = 200 + ((minute as f64 * 0.05).sin() * 50.0).abs() as u16;

            let result = ctrl.run_cycle(sensor_value);

            // Update VSTOR model
            vstor_mv = update_vstor(vstor_mv, harvest_ua, result.executed, ctrl.current_duty_ms());

            // Track totals
            total_harvested_mj += result.harvested_uj as u64;
            total_consumed_mj += result.consumed_uj as u64;
            if result.executed {
                total_executed += 1;
            } else {
                total_skipped += 1;
            }

            // Output CSV row
            let report = CycleReport::from(&result);
            println!(
                "{},{},{},{},{},{},{}",
                TelemetrySink::format_csv(&report),
                day,
                hour,
                minute % 60,
                lux,
                harvest_ua,
                vstor_mv
            );

            sink.record_cycle(&result);
        }
    }

    // Print summary to stderr
    let total_cycles = total_executed + total_skipped;
    eprintln!("=== 7-Day Simulation Summary ===");
    eprintln!("Total cycles:     {}", total_cycles);
    eprintln!("Executed:         {} ({:.1}%)", total_executed, total_executed as f64 / total_cycles as f64 * 100.0);
    eprintln!("Skipped:          {} ({:.1}%)", total_skipped, total_skipped as f64 / total_cycles as f64 * 100.0);
    eprintln!("Total harvested:  {:.3} mJ", total_harvested_mj as f64 / 1000.0);
    eprintln!("Total consumed:   {:.3} mJ", total_consumed_mj as f64 / 1000.0);
    eprintln!(
        "Net balance:      {:.3} mJ",
        (total_harvested_mj as i64 - total_consumed_mj as i64) as f64 / 1000.0
    );
    eprintln!("Final VSTOR:      {} mV", vstor_mv);

    let ratio = if total_consumed_mj > 0 {
        total_harvested_mj as f64 / total_consumed_mj as f64
    } else {
        f64::INFINITY
    };
    eprintln!("Harvest/consume:  {:.2}×", ratio);
    eprintln!(
        "Sustainability:   {}",
        if ratio >= 1.1 { "PASS" } else { "FAIL" }
    );
}
