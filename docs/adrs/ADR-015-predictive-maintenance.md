# ADR-015: Predictive maintenance (vibration FFT) cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `predictive-maintenance`

## Context

Rotating machinery (pumps, motors, fans, conveyors, HVAC blowers)
exhibits well-known vibration signatures that change as bearings,
belts, or impellers degrade. The seed has `structural-vibration`
which is general-purpose; this cog is **specifically for rotating
equipment** with a baseline-learned fundamental frequency.

The signal-processing approach:

1. **Baseline phase** — learn the fundamental frequency F1 (rotation
   rate) and its harmonics' relative magnitudes.
2. **Monitoring phase** — track shift in:
   - F1 amplitude (out-of-balance)
   - 2×F1 amplitude (misalignment)
   - 1×N×F1 high-order harmonics (bearing/cage faults)
   - Sideband energy near F1 (cavitation, looseness)
3. **Severity score** combines all four into a single metric.

## Decision

`predictive-maintenance` runs at 1 Hz over the feature stream and
performs a lightweight DFT (256-point) to extract harmonic
amplitudes. No external FFT library; hand-coded radix-2 in ~80 LOC.

## CLI

```
cog-predictive-maintenance [--once] [--interval 1]
                           [--baseline-mins 5] [--severity-warn 0.4]
                           [--severity-alarm 0.7]
```

## Output

```json
{
  "status": "learning|healthy|warn|ALARM|baseline_drift",
  "alarm": false,
  "severity_score": 0.0,
  "imbalance_pct": 0.0,
  "misalignment_pct": 0.0,
  "bearing_pct": 0.0,
  "looseness_pct": 0.0,
  "baseline_complete": false,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Maps to industry-standard vibration analysis bands.
- 256-point DFT at 1 Hz is well within Pi Zero 2 W budget.
- Severity score is interpretable.

### Negative
- Requires baseline learn period (5 min default) — can't detect
  faults that exist at start.
- Very slow degradation can drift the baseline if not periodically reset.

## Alternatives considered
- **Standalone vibration sensor with on-board FFT**. Rejected — adds
  hardware cost; existing ESP32 feature stream is sufficient.
- **External FFT crate**. Rejected — bloats binary; hand-coded
  radix-2 is ~3 KB.

## Resource budget
- Binary: < 420 KB armhf.
- RAM: < 2 MB (DFT scratch + 5 min baseline).
- CPU: ~5% at 1 Hz.

See ADR-001.
