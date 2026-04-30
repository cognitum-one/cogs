# ADR-009: PPE compliance cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `ppe-compliance`

## Context

OSHA-regulated worksites (construction, food service, healthcare,
clean rooms) require Personal Protective Equipment (PPE) — hard hats,
high-visibility vests, masks, hairnets. Manual compliance enforcement
is unscalable; automated checks improve adherence and reduce incidents.

The seed has the ESP32 ruview WiFi-CSI stream which can detect
*presence and gross body shape* but not "is this person wearing a
hard hat". This cog accepts that limitation and instead implements a
**zone-presence + dwell** model: a ruview cog elsewhere reports
presence; this cog enforces "presence in restricted zone implies PPE
should be present" via a separate cog-store query.

## Decision

`ppe-compliance` runs at 0.5 Hz and:

1. **Subscribes to zone presence** — reads the ruview-densepose cog's
   skeleton output from the seed's RuVector store (loopback HTTP to
   `/api/v1/store/search`).
2. **Compares with PPE manifest** — when presence is detected in a
   labeled `restricted` zone, checks for an associated PPE-camera-cog
   confirmation vector within `confirmation_window_secs`.
3. **Fires non-compliance alert** if presence without confirmation.

This is a *coordination* cog — it doesn't directly read sensors; it
fuses outputs from two other cogs.

## CLI

```
cog-ppe-compliance [--once] [--interval 5]
                   [--zone restricted] [--confirmation-window 60]
```

## Output

```json
{
  "status": "compliant|warning|NON_COMPLIANT",
  "violations_session": 0,
  "presence_in_zone": false,
  "ppe_confirmed": false,
  "since_confirmation_secs": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Reuses existing `ruview-densepose` skeleton output rather than
  duplicating CSI processing.
- Cog composition pattern (subscribe to other cogs' results) is reusable.

### Negative
- Cannot directly verify "is wearing hard hat" — relies on a separate
  PPE-camera cog (vendor-supplied) feeding confirmation vectors.
- Latency window (60 s) means brief unconfirmed presence isn't flagged.

## Alternatives considered
- **Vision-only PPE classifier**. Rejected v1 — needs camera + ML model.
- **RFID-tagged PPE**. Out of scope; possible v2 add-on with a
  `ppe-rfid-cog` companion.

## RuView mode

**Required upstream.** This cog reads `ruview-densepose` output. If
that cog isn't installed/running, `ppe-compliance` reports a permanent
`status: warning` and emits a single startup error.

## Resource budget
- Binary: < 350 KB armhf.
- RAM: < 1 MB.
- CPU: < 1% at 0.5 Hz.

See ADR-001.
