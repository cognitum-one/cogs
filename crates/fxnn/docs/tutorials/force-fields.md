# Force Fields Guide

This guide explains the force fields available in FXNN and how to configure them.

## Overview

Force fields define how atoms interact. FXNN supports:

- **Non-bonded**: Lennard-Jones, Coulomb
- **Bonded**: Bonds, angles, dihedrals
- **Composite**: Combine multiple force fields

## Lennard-Jones

The Lennard-Jones (LJ) potential models van der Waals interactions:

```
V(r) = 4ε[(σ/r)¹² - (σ/r)⁶]
```

### Parameters

| Parameter | Description | Typical Values |
|-----------|-------------|----------------|
| `epsilon` (ε) | Well depth | 1.0 (reduced), 0.996 kJ/mol (argon) |
| `sigma` (σ) | Zero-crossing distance | 1.0 (reduced), 0.34 nm (argon) |
| `cutoff` | Interaction cutoff | 2.5σ - 3.0σ |

### Usage

```rust
use fxnn::force_field::LennardJones;

// Preset for argon (reduced units)
let lj = LennardJones::argon();

// Custom parameters
let lj = LennardJones::new(1.0, 1.0, 2.5);  // epsilon, sigma, cutoff

// Real units (kJ/mol, nm)
let lj = LennardJones::new(0.996, 0.34, 1.0);
```

### Cutoff Strategies

```rust
// Sharp cutoff (default)
let lj = LennardJones::new(1.0, 1.0, 2.5);

// With tail corrections for thermodynamic properties
let lj = LennardJones::new(1.0, 1.0, 2.5)
    .with_tail_correction(true);
```

## Coulomb

Electrostatic interactions between charged atoms:

```
V(r) = q_i * q_j / (4πε₀r)
```

### Usage

```rust
use fxnn::force_field::Coulomb;

// Create Coulomb force field
let coulomb = Coulomb::new(10.0);  // 10σ cutoff

// With reaction field correction
let coulomb = Coulomb::new(10.0)
    .with_dielectric(78.0);  // water dielectric
```

### Setting Charges

```rust
// Set charges on atoms
atoms[0].charge = 0.417;   // Oxygen in TIP3P
atoms[1].charge = -0.834;  // Hydrogen
```

## Composite Force Fields

Combine multiple interactions:

```rust
use fxnn::force_field::{CompositeForceField, LennardJones, Coulomb};

let lj = LennardJones::argon();
let coulomb = Coulomb::new(10.0);

let composite = CompositeForceField::new()
    .add(lj)
    .add(coulomb);
```

## Bonded Interactions

### Harmonic Bonds

```
V(r) = k(r - r₀)²
```

```rust
use fxnn::force_field::HarmonicBond;

// Bond between atoms 0 and 1
let bond = HarmonicBond::new(
    0, 1,      // atom indices
    1000.0,    // force constant k
    0.1        // equilibrium distance r₀
);
```

### Harmonic Angles

```
V(θ) = k(θ - θ₀)²
```

```rust
use fxnn::force_field::HarmonicAngle;

// Angle between atoms 0-1-2
let angle = HarmonicAngle::new(
    0, 1, 2,           // atom indices
    500.0,             // force constant
    109.47_f32.to_radians()  // equilibrium angle
);
```

## Security Validations

FXNN validates force field parameters to prevent numerical issues:

| Parameter | Validation | Error |
|-----------|------------|-------|
| `sigma` | Must be > 1e-10 | Prevents division by zero |
| `cutoff` | Must be > 1e-10 | Prevents invalid forces |
| `epsilon` | Must be >= 0 | Negative not physical |

```rust
// This will panic
let lj = LennardJones::new(1.0, 0.0, 2.5);  // sigma = 0 invalid
```

## Force Clamping

Forces are clamped to prevent numerical instability:

```rust
const MAX_FORCE: f32 = 1e6;

// Forces exceeding MAX_FORCE are scaled symmetrically
// to preserve Newton's Third Law (f_ij = -f_ji)
```

## Overlap Handling

When atoms get too close (r < 0.5σ), soft repulsion is applied:

```rust
// Instead of divergent forces at r→0:
// - Calculate forces at r = 0.5σ
// - Apply smooth repulsion
```

## Performance Tips

1. **Use cutoffs**: Always specify reasonable cutoffs
2. **Neighbor lists**: Automatically used for large systems
3. **SIMD**: Enabled by default for vectorized calculations

## Common Force Field Sets

### Noble Gases (Reduced Units)

| Element | ε | σ |
|---------|---|---|
| Argon | 1.0 | 1.0 |
| Krypton | 1.4 | 1.06 |
| Xenon | 1.77 | 1.18 |

### TIP3P Water

| Site | Charge | σ (nm) | ε (kJ/mol) |
|------|--------|--------|------------|
| O | -0.834 | 0.315 | 0.636 |
| H | +0.417 | 0 | 0 |

## Next Steps

- [Getting Started](getting-started.md) - Basic simulation setup
- [Performance Guide](../guides/performance.md) - Optimization tips
- [ADR-001](../adr/ADR-001-five-layer-reality-stack.md) - Architecture
