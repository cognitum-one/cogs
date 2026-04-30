# ADR-005: Snore monitor cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `snore-monitor`

## Context

Snoring is a sleep-quality and apnea-risk indicator. The seed already
ships `sleep-apnea` (for clinical-style apnea detection) but lacks a
lightweight snore-frequency tracker that runs nightly to give "snore
intensity / minute" trends.

A snore is a quasi-periodic low-frequency event (60–250 Hz fundamental,
1.5–4 Hz repetition rate matching breath cycle) lasting 200–800 ms.

## Decision

`snore-monitor` runs at 2 Hz over the feature stream and tracks:

1. **Low-band energy bursts** — first-half feature channels (proxy for
   low frequencies) above adaptive baseline.
2. **Periodicity check** — autocorrelation over a 30 s ring buffer to
   confirm 1.5–4 Hz repetition.
3. **Per-minute counter** — number of snore events in last 60 s, and
   running total for the session.

This is monitoring, not alerting — it produces continuous snore-rate
data rather than firing on each event.

## CLI

```
cog-snore-monitor [--once] [--interval 1] [--burst-z 2.0]
                  [--minimum-rate-hz 1.5] [--maximum-rate-hz 4.0]
```

## Output

```json
{
  "status": "quiet|periodic|monitoring",
  "snores_per_minute": 0,
  "session_total": 0,
  "burst_z": 0.0,
  "estimated_rate_hz": 0.0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Companion to `sleep-apnea` — produces a sleep-quality summary.
- Continuous output — feeds dashboards.

### Negative
- Periodicity check requires 30 s ring; not useful for spot reads.

## Alternatives considered
- **Spectral peak detection** — needs FFT, more CPU.
- **Cross-correlation** — computationally heavier than autocorrelation.

## Resource budget
- Binary: < 380 KB armhf.
- RAM: ~ 1 MB (30 s ring at 2 Hz).
- CPU: < 3%.

See ADR-001.
