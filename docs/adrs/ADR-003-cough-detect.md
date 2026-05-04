# ADR-003: Cough detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `cough-detect`

## Context

Post-COVID, cough monitoring is a meaningful early-warning signal for
respiratory illness in homes, eldercare facilities, and offices.
The seed has `respiratory-distress` (continuous breathing pattern
analysis) but no event detector for individual cough events.

A cough has a characteristic acoustic signature:

1. Sharp transient onset (< 50 ms rise time)
2. High-frequency burst followed by lower-frequency tail
3. Total duration 200–500 ms
4. Often clusters (cough bouts of 2–5 events within 10 s)

## Decision

`cough-detect` runs at 10 Hz over the feature stream and uses:

1. **Transient detector** — sliding window mean amplitude vs. previous
   window. A spike of 3+ z-scores within 50 ms triggers candidate.
2. **Spectral check** — feature channels 4-7 (high-freq band) must
   exceed channels 0-3 by 1.5x during transient.
3. **Cluster counter** — count cough events in rolling 30 s window.
   Sustained activity (3+ in 10 s) raises alert level.

### CLI

```
cog-cough-detect [--once] [--interval 1]
                 [--transient-z 3.0] [--cluster-window 30]
                 [--alert-count 3]
```

### Output

```json
{
  "status": "quiet|cough|burst|cluster_alert",
  "cough_detected": false,
  "events_30s": 0,
  "events_total": 0,
  "transient_z": 0.0,
  "spectral_ratio": 0.0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive

- Pure audio signal — works with mic-only ESP32 nodes.
- Cluster logic catches illness onset (multiple coughs in short span)
  not just background single coughs.
- 10 Hz sampling is well within Pi Zero 2 W budget.

### Negative

- Cannot distinguish cough vs. throat-clear vs. sneeze without ML.
- Falsely fires on dog barks, door slams, plastic cracks.
  Mitigated by spectral check, but not eliminated.

## Alternatives considered

- **Trained CNN classifier.** Rejected for v1 — model size on Pi Zero 2 W
  constrains us; possible v2 with quantized 4-bit model.
- **Mean-amplitude only.** Rejected — too many false positives without
  spectral check.

## Resource budget

- **Binary**: < 350 KB stripped armhf.
- **RAM**: < 1.5 MB (sliding windows + cluster ring).
- **CPU**: < 3% at 10 Hz.

See ADR-001.
