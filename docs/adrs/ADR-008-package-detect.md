# ADR-008: Package arrival detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `package-detect`

## Context

Porch piracy and missed deliveries are persistent problems. A simple
"new object appeared and stayed" detector is high-value for residential
seeds at front doors and dock seeds at loading bays.

This is fundamentally a *vision* cog — without a camera or CSI input
that resolves stationary-object presence, you cannot detect a package
on a porch. The seed has the ESP32 ruview WiFi-CSI stream as a
camera-substitute.

## Decision

`package-detect` is a **ruview-required** cog. It runs at 0.5 Hz over
the CSI feature stream and tracks:

1. **Static-object signature** — a sustained CSI-variance shift in
   specific subcarrier bands (proxy for new physical mass in scene).
2. **Persistence gate** — shift must hold for `min_persistence_secs`
   to discriminate from someone walking past.
3. **Departure detection** — the shift relaxes back when the package
   is removed, generating a `package_taken` event.

## CLI

```
cog-package-detect [--once] [--interval 2] [--persistence 30]
                   [--shift-z 2.5]
```

## Output

```json
{
  "status": "empty|transient|PACKAGE_PRESENT|PACKAGE_TAKEN",
  "package_present": false,
  "persistence_secs": 0.0,
  "shift_z": 0.0,
  "session_arrivals": 0,
  "session_departures": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- No camera privacy concerns.
- Works through window glass (CSI penetration).

### Negative
- Cannot identify package vs. dropped object.
- Requires CSI input — cog refuses to run without ruview mode.

## Alternatives considered
- **PIR motion + 30 s timeout heuristic**. Rejected — too noisy.
- **Camera + computer vision**. Out of scope for v1 cog (model size on Pi
  Zero 2 W; privacy concerns). Possible v2 with vendor-specific cog.

## RuView mode

**Required.** This cog refuses to run without ESP32 CSI feature stream.
The 8-feature frame is interpreted as subcarrier amplitudes; sustained
variance shifts in specific subcarrier groups indicate static-object
presence.

## Resource budget
- Binary: < 380 KB armhf.
- RAM: < 1.5 MB.
- CPU: < 1% at 0.5 Hz.

See ADR-001.
