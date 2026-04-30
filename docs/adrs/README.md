# Architecture Decision Records — cognitum-one/cogs

ADRs for the cog ecosystem. Each ADR captures one durable decision: the
problem we faced, what we considered, what we picked, and why. Numbered
in chronological order; status reflects current adoption.

## Foundational

| #   | Title                                              | Status   |
| --- | -------------------------------------------------- | -------- |
| 001 | [Cogs as plugins](ADR-001-cogs-as-plugins-architecture.md) | Accepted |

## New practical capability cogs (2026-04 wave)

Each ADR documents one new cog: the sensors it uses, signal-processing
approach, Pi Zero 2 W resource budget, alternatives considered, and the
optional ruvnet/ruview mode (when applicable).

| #   | Title                                                                                  | Category   | RuView mode |
| --- | -------------------------------------------------------------------------------------- | ---------- | ----------- |
| 002 | [Fall detection](ADR-002-fall-detect.md)                                               | health     | optional    |
| 003 | [Cough detection](ADR-003-cough-detect.md)                                             | health     | -           |
| 004 | [Baby cry detection](ADR-004-baby-cry.md)                                              | health     | -           |
| 005 | [Snore monitor](ADR-005-snore-monitor.md)                                              | health     | -           |
| 006 | [Glass-break detection](ADR-006-glass-break.md)                                        | security   | -           |
| 007 | [Gunshot detection](ADR-007-gunshot-detect.md)                                         | security   | optional    |
| 008 | [Package arrival detection](ADR-008-package-detect.md)                                 | retail     | required    |
| 009 | [PPE compliance](ADR-009-ppe-compliance.md)                                            | industrial | required    |
| 010 | [Slip / wet-floor zone](ADR-010-slip-fall-zone.md)                                     | industrial | optional    |
| 011 | [Water-leak detection](ADR-011-water-leak.md)                                          | building   | -           |
| 012 | [Smoke / fire detection](ADR-012-smoke-fire.md)                                        | building   | optional    |
| 013 | [Frost warning](ADR-013-frost-warning.md)                                              | agriculture| -           |
| 014 | [Beehive monitor](ADR-014-beehive-monitor.md)                                          | agriculture| -           |
| 015 | [Predictive maintenance (vibration FFT)](ADR-015-predictive-maintenance.md)            | industrial | -           |
| 016 | [Parking occupancy](ADR-016-parking-occupancy.md)                                      | retail     | required    |

## ADR template

```
# ADR-NNN: <one-line title>

**Status**: Proposed | Accepted | Superseded by ADR-XXX | Deprecated
**Date**: YYYY-MM-DD
**Cog**: <cog-id> (if applicable)

## Context
What is the problem? What forces are at play?

## Decision
What did we pick?

## Consequences
- Positive
- Negative
- Neutral

## Alternatives considered
- Option A — rejected because ...
- Option B — rejected because ...

## RuView mode (optional)
How this cog optionally uses ruvnet/ruview WiFi-CSI input.
```

## Conventions

- Filename: `ADR-NNN-kebab-title.md` (zero-padded 3-digit).
- Update the table above when adding a new ADR.
- Keep ADRs immutable once Accepted. Supersede with a new ADR that links back.
- The `cogs as plugins` umbrella ADR (001) is foundational — every cog ADR
  should link back to it rather than re-explain the architecture.
