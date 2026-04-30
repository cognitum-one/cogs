# ADR-006: Glass-break detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `glass-break`

## Context

Glass-break is a high-confidence security signal because the acoustic
signature is well-characterized:

1. **Bang phase**: short (< 100 ms) broadband impulse with peak energy
   in the 5–7 kHz band (initial impact).
2. **Shatter phase**: 200–500 ms of energy in the 8–12 kHz band as
   pieces fall (high-frequency content).
3. **Quiet tail**: a few hundred ms of low energy.

This two-phase signature distinguishes glass-break from ordinary impulse
noise (door slam, dropped object), which doesn't have the high-frequency
shatter tail.

## Decision

`glass-break` runs at 20 Hz (50 ms frames) over the feature stream and
uses a two-phase template matcher:

1. **Bang detection**: variance spike + high-band z > `bang_z`.
2. **Shatter expectation**: within 100 ms, high-band sustained for
   200 ms with declining envelope.
3. **Fire** when both phases match within window.
4. **Cooldown** to prevent re-fires from echo / reverb.

## CLI

```
cog-glass-break [--once] [--interval 1] [--bang-z 5.0]
                [--shatter-window-ms 500] [--cooldown 30]
```

## Output

```json
{
  "status": "quiet|bang|shatter|GLASS_BREAK|cooldown",
  "glass_break_detected": false,
  "bang_z": 0.0,
  "shatter_score": 0.0,
  "total_breaks": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Two-phase logic dramatically reduces false positives vs. single-stage.
- 20 Hz frame rate captures shatter envelope.

### Negative
- 20 Hz on Pi Zero 2 W isn't free — must be careful about coexistence
  with other audio cogs.
- Doesn't work for thick safety glass (delayed shatter).

## Alternatives considered
- **CNN classifier on mel-spectrogram**. Rejected v1 — model size.
- **Single-stage spike detector**. Rejected — false positives.

## Resource budget
- Binary: < 400 KB armhf.
- RAM: < 1.5 MB.
- CPU: 5–8% at 20 Hz.

See ADR-001.
