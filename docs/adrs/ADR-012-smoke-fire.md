# ADR-012: Smoke / fire detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `smoke-fire`

## Context

Conventional smoke alarms (ionization, photoelectric) are reactive — they
fire when smoke has already filled enough of the chamber to trigger.
A seed-based detector can fuse three orthogonal evidence streams:

1. **Acoustic crackle** — fire produces a characteristic crackle
   (200 Hz–4 kHz transients).
2. **Thermal drift proxy** — sustained CSI-variance reduction
   (warm air rising disrupts WiFi propagation predictably).
3. **Optional ruview plume** — visible smoke disrupts CSI subcarriers
   in a specific pattern.

Multi-signal fusion catches fires earlier *and* with fewer false
alarms than any single source.

## Decision

`smoke-fire` runs at 1 Hz over the feature stream and tracks each
signal independently. It fires when **at least 2 of 3** signals are
above threshold simultaneously.

State machine:

- `quiet` — < 2 signals
- `monitoring` — 2 signals weakly elevated
- `FIRE_LIKELY` — 2 signals above firm threshold
- `FIRE_CONFIRMED` — 3 signals above firm threshold

## CLI

```
cog-smoke-fire [--once] [--interval 1] [--crackle-z 2.5]
               [--thermal-drift-z 1.5] [--cooldown 60] [--ruview-mode]
```

## Output

```json
{
  "status": "quiet|monitoring|FIRE_LIKELY|FIRE_CONFIRMED|cooldown",
  "fire_likely": false,
  "fire_confirmed": false,
  "crackle_z": 0.0,
  "thermal_drift_z": 0.0,
  "ruview_plume_score": 0.0,
  "signals_active": 0,
  "total_alerts": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Earlier detection than ionization-only.
- Three independent signals reduce false-positive correlation.

### Negative
- Cannot replace UL-listed smoke alarms (regulatory).
- Thermal drift proxy is indirect — works best in enclosed rooms.

## Alternatives considered
- **CO sensor**. Out of scope — separate hardware.
- **MOS gas sensor**. Out of scope — separate hardware.

## RuView mode

**Optional.** With ruview, the cog reads CSI subcarrier variance
patterns that change in predictable ways with smoke aerosols. Without
ruview, falls back to acoustic crackle + thermal drift only (still
catches 2-of-3 fires but with shorter response time).

## Resource budget
- Binary: < 400 KB armhf.
- RAM: < 1.5 MB.
- CPU: < 3% at 1 Hz.

See ADR-001.
