# ADR-011: Water-leak detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `water-leak`

## Context

Water leaks (slow drips, pinhole pipe failures, broken seals) cause an
estimated $13B annual property damage in the US alone. Most go
undetected for hours-to-days because their acoustic signature is too
quiet for human notice but is audible to a placed seed.

The signature is:

1. **Continuous low-amplitude broadband hiss** (turbulence noise from
   leak orifice).
2. **Periodic drip transient** (when free-falling water hits a
   surface — 1-3 Hz repetition).
3. **Sustained over hours** — distinguishes from one-off events.

## Decision

`water-leak` runs at 0.5 Hz over the feature stream and uses:

1. **Hiss detector**: low-energy variance baseline that's *higher* than
   nominal quiet — flags persistent broadband noise.
2. **Drip period detector**: autocorrelation peak at 1–3 Hz repetition.
3. **Persistence gate**: signature must hold for `min_persistence_secs`
   (default 5 min) to fire — distinguishes leak from passing rain.

## CLI

```
cog-water-leak [--once] [--interval 2] [--hiss-z 1.5] [--persistence 300]
```

## Output

```json
{
  "status": "dry|hiss|drip|LEAK_LIKELY|LEAK_CONFIRMED",
  "leak_likely": false,
  "leak_confirmed": false,
  "hiss_z": 0.0,
  "drip_rate_hz": 0.0,
  "persistence_secs": 0.0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Persistence gate keeps false alerts very low.
- Two-stage (likely → confirmed) gives early warning + confirmation.
- Audio-only — no plumbing instrumentation needed.

### Negative
- Mounted seed must be near plumbing (under-sink, utility room).
- Cannot localize within room.

## Alternatives considered
- **Conductive water sensor strips**. Rejected — separate hardware.
- **PIR + audio combined**. Out of scope — leaks rarely correlate with
  motion.

## Resource budget
- Binary: < 360 KB armhf.
- RAM: < 1.5 MB (autocorrelation ring).
- CPU: < 2% at 0.5 Hz.

See ADR-001.
