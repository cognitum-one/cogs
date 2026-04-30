# ADR-010: Slip / wet-floor zone cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `slip-fall-zone`

## Context

Wet floors and spills cause more workplace falls than any other cause
in food-service, retail, and healthcare. Mechanical wet-floor signs
are reactive (placed only after staff notices). Sensor-based detection
of recent rapid motion + sustained still feet (someone standing
trying not to slip) is a useful early-warning signal.

This is **adjacent** to `fall-detect` (ADR-002): fall-detect fires when
someone has fallen; slip-fall-zone fires when conditions for falls are
elevated, before anyone falls.

## Decision

`slip-fall-zone` runs at 1 Hz over the feature stream and uses three
inputs:

1. **Motion variance shift** — sudden drop in motion variance (people
   slowing down to navigate carefully) over 30 s window.
2. **Acoustic splash signature** — short transient with broadband
   energy (water hitting floor) — may have already happened or be
   ongoing.
3. **Optional ruview pose check** — if `--ruview-mode`, monitor for
   wide stance + slow movement (cautious gait).

When all available signals exceed threshold simultaneously, raise
`SLIP_RISK_HIGH`.

## CLI

```
cog-slip-fall-zone [--once] [--interval 1] [--motion-drop-z 1.5]
                   [--splash-z 3.0] [--cooldown 600] [--ruview-mode]
```

## Output

```json
{
  "status": "normal|cautious|SPLASH|SLIP_RISK_HIGH|cooldown",
  "slip_risk_high": false,
  "motion_drop_z": 0.0,
  "splash_z": 0.0,
  "cautious_gait_score": 0.0,
  "session_alerts": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Pre-emptive — fires before falls happen.
- Multi-signal fusion reduces false positives from any single channel.

### Negative
- Long cooldown (default 10 min) means re-spills aren't double-flagged.
- Cannot localize "where in zone".

## Alternatives considered
- **Camera + computer vision**. Rejected v1 — privacy + model size.
- **Single-input motion-drop only**. Rejected — too noisy in busy
  environments.

## RuView mode

**Optional.** With ruview, the cog reads CSI features as gait
descriptors (stride length proxy, step-rate proxy) and adds a
"cautious gait" score based on slower step rate + wider stance proxy.
Without ruview, falls back to motion variance + splash audio only.

## Resource budget
- Binary: < 380 KB armhf.
- RAM: < 1.2 MB.
- CPU: < 2% at 1 Hz.

See ADR-001.
