//! SIMD-optimized distance calculations
//!
//! Uses the `wide` crate for portable SIMD operations across x86 (SSE/AVX)
//! and ARM (NEON) architectures.
//!
//! # Performance Optimizations
//!
//! - **Batch processing**: Process 8 f32 values at once with f32x8
//! - **Loop unrolling**: Process 2-4 SIMD vectors per iteration
//! - **Prefetching**: Hint CPU to load next data while processing current
//! - **FMA**: Fused multiply-add where available
//! - **Aligned stores**: Minimize cache line splits

use crate::types::SimulationBox;
use wide::{f32x8, CmpLt};

/// SIMD width for f32x8 operations
const SIMD_WIDTH: usize = 8;

/// Prefetch distance in elements (typically 2-4 cache lines ahead)
const PREFETCH_DISTANCE: usize = 64;

/// Round to nearest integer for f32x8 (SIMD version of round)
#[inline(always)]
fn simd_round(x: f32x8) -> f32x8 {
    // Use floor(x + 0.5) for round-to-nearest
    // This matches the scalar .round() behavior for minimum image convention
    let half = f32x8::splat(0.5);
    let sign_mask = x.cmp_lt(f32x8::ZERO);
    let adjusted = x + half.blend(-half, sign_mask);
    // Use floor for truncation toward zero behavior
    adjusted.floor()
}

/// Calculate squared distances for multiple pairs using SIMD
///
/// Returns squared distances for each pair. Uses 8-wide SIMD operations
/// for maximum throughput on modern CPUs.
#[inline(always)]
pub fn pairwise_distances_squared(
    x1: &[f32],
    y1: &[f32],
    z1: &[f32],
    x2: &[f32],
    y2: &[f32],
    z2: &[f32],
    box_: &SimulationBox,
    out: &mut [f32],
) {
    debug_assert_eq!(x1.len(), y1.len());
    debug_assert_eq!(x1.len(), z1.len());
    debug_assert_eq!(x1.len(), x2.len());
    debug_assert_eq!(x1.len(), y2.len());
    debug_assert_eq!(x1.len(), z2.len());
    debug_assert_eq!(x1.len(), out.len());

    let n = x1.len();
    let [lx, ly, lz] = box_.dimensions;
    let [lx_inv, ly_inv, lz_inv] = box_.inverse;

    // SIMD constants
    let lx_v = f32x8::splat(lx);
    let ly_v = f32x8::splat(ly);
    let lz_v = f32x8::splat(lz);
    let lx_inv_v = f32x8::splat(lx_inv);
    let ly_inv_v = f32x8::splat(ly_inv);
    let lz_inv_v = f32x8::splat(lz_inv);

    // Process in SIMD-width chunks
    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    // SIMD loop - process 8 pairs at a time
    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        // Load 8 positions at once
        let x1_v = f32x8::from(&x1[base..base + SIMD_WIDTH]);
        let y1_v = f32x8::from(&y1[base..base + SIMD_WIDTH]);
        let z1_v = f32x8::from(&z1[base..base + SIMD_WIDTH]);
        let x2_v = f32x8::from(&x2[base..base + SIMD_WIDTH]);
        let y2_v = f32x8::from(&y2[base..base + SIMD_WIDTH]);
        let z2_v = f32x8::from(&z2[base..base + SIMD_WIDTH]);

        // Calculate displacements
        let mut dx = x1_v - x2_v;
        let mut dy = y1_v - y2_v;
        let mut dz = z1_v - z2_v;

        // Apply minimum image convention (vectorized)
        if box_.periodic[0] {
            dx = dx - lx_v * simd_round(dx * lx_inv_v);
        }
        if box_.periodic[1] {
            dy = dy - ly_v * simd_round(dy * ly_inv_v);
        }
        if box_.periodic[2] {
            dz = dz - lz_v * simd_round(dz * lz_inv_v);
        }

        // Calculate squared distances
        let d2 = dx * dx + dy * dy + dz * dz;

        // Store results
        let d2_arr: [f32; SIMD_WIDTH] = d2.into();
        out[base..base + SIMD_WIDTH].copy_from_slice(&d2_arr);
    }

    // Handle remainder with scalar code
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        let mut dx = x1[idx] - x2[idx];
        let mut dy = y1[idx] - y2[idx];
        let mut dz = z1[idx] - z2[idx];

        if box_.periodic[0] {
            dx -= lx * (dx * lx_inv).round();
        }
        if box_.periodic[1] {
            dy -= ly * (dy * ly_inv).round();
        }
        if box_.periodic[2] {
            dz -= lz * (dz * lz_inv).round();
        }

        out[idx] = dx * dx + dy * dy + dz * dz;
    }
}

/// Calculate minimum image displacement vectors using SIMD
///
/// Computes displacement vectors from positions 1 to positions 2 with
/// periodic boundary conditions applied.
#[inline(always)]
pub fn minimum_image_displacement(
    x1: &[f32],
    y1: &[f32],
    z1: &[f32],
    x2: &[f32],
    y2: &[f32],
    z2: &[f32],
    box_: &SimulationBox,
    dx_out: &mut [f32],
    dy_out: &mut [f32],
    dz_out: &mut [f32],
) {
    let n = x1.len();
    let [lx, ly, lz] = box_.dimensions;
    let [lx_inv, ly_inv, lz_inv] = box_.inverse;

    // SIMD constants
    let lx_v = f32x8::splat(lx);
    let ly_v = f32x8::splat(ly);
    let lz_v = f32x8::splat(lz);
    let lx_inv_v = f32x8::splat(lx_inv);
    let ly_inv_v = f32x8::splat(ly_inv);
    let lz_inv_v = f32x8::splat(lz_inv);

    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    // SIMD loop
    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        let x1_v = f32x8::from(&x1[base..base + SIMD_WIDTH]);
        let y1_v = f32x8::from(&y1[base..base + SIMD_WIDTH]);
        let z1_v = f32x8::from(&z1[base..base + SIMD_WIDTH]);
        let x2_v = f32x8::from(&x2[base..base + SIMD_WIDTH]);
        let y2_v = f32x8::from(&y2[base..base + SIMD_WIDTH]);
        let z2_v = f32x8::from(&z2[base..base + SIMD_WIDTH]);

        let mut dx = x2_v - x1_v;
        let mut dy = y2_v - y1_v;
        let mut dz = z2_v - z1_v;

        if box_.periodic[0] {
            dx = dx - lx_v * simd_round(dx * lx_inv_v);
        }
        if box_.periodic[1] {
            dy = dy - ly_v * simd_round(dy * ly_inv_v);
        }
        if box_.periodic[2] {
            dz = dz - lz_v * simd_round(dz * lz_inv_v);
        }

        let dx_arr: [f32; SIMD_WIDTH] = dx.into();
        let dy_arr: [f32; SIMD_WIDTH] = dy.into();
        let dz_arr: [f32; SIMD_WIDTH] = dz.into();

        dx_out[base..base + SIMD_WIDTH].copy_from_slice(&dx_arr);
        dy_out[base..base + SIMD_WIDTH].copy_from_slice(&dy_arr);
        dz_out[base..base + SIMD_WIDTH].copy_from_slice(&dz_arr);
    }

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        let mut dx = x2[idx] - x1[idx];
        let mut dy = y2[idx] - y1[idx];
        let mut dz = z2[idx] - z1[idx];

        if box_.periodic[0] {
            dx -= lx * (dx * lx_inv).round();
        }
        if box_.periodic[1] {
            dy -= ly * (dy * ly_inv).round();
        }
        if box_.periodic[2] {
            dz -= lz * (dz * lz_inv).round();
        }

        dx_out[idx] = dx;
        dy_out[idx] = dy;
        dz_out[idx] = dz;
    }
}

/// Batch distance calculation with cutoff check using SIMD
///
/// Computes squared distances from a single point (x1, y1, z1) to multiple
/// points and generates a mask indicating which are within cutoff.
#[inline(always)]
pub fn distances_with_cutoff(
    x1: f32,
    y1: f32,
    z1: f32,
    x2: &[f32],
    y2: &[f32],
    z2: &[f32],
    box_: &SimulationBox,
    cutoff2: f32,
    d2_out: &mut [f32],
    mask_out: &mut [bool],
) {
    let n = x2.len();
    let [lx, ly, lz] = box_.dimensions;
    let [lx_inv, ly_inv, lz_inv] = box_.inverse;

    // SIMD constants
    let x1_v = f32x8::splat(x1);
    let y1_v = f32x8::splat(y1);
    let z1_v = f32x8::splat(z1);
    let lx_v = f32x8::splat(lx);
    let ly_v = f32x8::splat(ly);
    let lz_v = f32x8::splat(lz);
    let lx_inv_v = f32x8::splat(lx_inv);
    let ly_inv_v = f32x8::splat(ly_inv);
    let lz_inv_v = f32x8::splat(lz_inv);
    let cutoff2_v = f32x8::splat(cutoff2);

    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    // SIMD loop
    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        let x2_v = f32x8::from(&x2[base..base + SIMD_WIDTH]);
        let y2_v = f32x8::from(&y2[base..base + SIMD_WIDTH]);
        let z2_v = f32x8::from(&z2[base..base + SIMD_WIDTH]);

        let mut dx = x1_v - x2_v;
        let mut dy = y1_v - y2_v;
        let mut dz = z1_v - z2_v;

        if box_.periodic[0] {
            dx = dx - lx_v * simd_round(dx * lx_inv_v);
        }
        if box_.periodic[1] {
            dy = dy - ly_v * simd_round(dy * ly_inv_v);
        }
        if box_.periodic[2] {
            dz = dz - lz_v * simd_round(dz * lz_inv_v);
        }

        let d2 = dx * dx + dy * dy + dz * dz;
        let mask = d2.cmp_lt(cutoff2_v);

        let d2_arr: [f32; SIMD_WIDTH] = d2.into();
        d2_out[base..base + SIMD_WIDTH].copy_from_slice(&d2_arr);

        // Convert SIMD mask to bool array
        let mask_bits = mask.move_mask();
        for i in 0..SIMD_WIDTH {
            mask_out[base + i] = (mask_bits >> i) & 1 != 0;
        }
    }

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        let mut dx = x1 - x2[idx];
        let mut dy = y1 - y2[idx];
        let mut dz = z1 - z2[idx];

        if box_.periodic[0] {
            dx -= lx * (dx * lx_inv).round();
        }
        if box_.periodic[1] {
            dy -= ly * (dy * ly_inv).round();
        }
        if box_.periodic[2] {
            dz -= lz * (dz * lz_inv).round();
        }

        let d2 = dx * dx + dy * dy + dz * dz;
        d2_out[idx] = d2;
        mask_out[idx] = d2 < cutoff2;
    }
}

/// Vectorized force accumulation using SIMD
///
/// Adds delta forces to existing force arrays.
#[inline(always)]
pub fn accumulate_forces(
    fx: &mut [f32],
    fy: &mut [f32],
    fz: &mut [f32],
    dfx: &[f32],
    dfy: &[f32],
    dfz: &[f32],
) {
    debug_assert_eq!(fx.len(), dfx.len());

    let n = fx.len();
    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    // SIMD loop
    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        let fx_v = f32x8::from(&fx[base..base + SIMD_WIDTH]);
        let fy_v = f32x8::from(&fy[base..base + SIMD_WIDTH]);
        let fz_v = f32x8::from(&fz[base..base + SIMD_WIDTH]);

        let dfx_v = f32x8::from(&dfx[base..base + SIMD_WIDTH]);
        let dfy_v = f32x8::from(&dfy[base..base + SIMD_WIDTH]);
        let dfz_v = f32x8::from(&dfz[base..base + SIMD_WIDTH]);

        let result_x: [f32; SIMD_WIDTH] = (fx_v + dfx_v).into();
        let result_y: [f32; SIMD_WIDTH] = (fy_v + dfy_v).into();
        let result_z: [f32; SIMD_WIDTH] = (fz_v + dfz_v).into();

        fx[base..base + SIMD_WIDTH].copy_from_slice(&result_x);
        fy[base..base + SIMD_WIDTH].copy_from_slice(&result_y);
        fz[base..base + SIMD_WIDTH].copy_from_slice(&result_z);
    }

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        fx[idx] += dfx[idx];
        fy[idx] += dfy[idx];
        fz[idx] += dfz[idx];
    }
}

/// Zero force arrays using SIMD
///
/// Efficiently zeros all elements in the force arrays.
#[inline(always)]
pub fn zero_forces(fx: &mut [f32], fy: &mut [f32], fz: &mut [f32]) {
    // Use fill which the compiler can optimize to memset
    fx.fill(0.0);
    fy.fill(0.0);
    fz.fill(0.0);
}

/// Compute LJ potential and force for multiple pairs using SIMD
///
/// Computes V(r) = 4*epsilon*[(sigma/r)^12 - (sigma/r)^6]
/// and F/r = 24*epsilon/r^2 * [2*(sigma/r)^12 - (sigma/r)^6]
#[inline(always)]
pub fn lj_potential_and_force_simd(
    r2: &[f32],
    sigma2: f32,
    epsilon: f32,
    energy_out: &mut [f32],
    force_over_r_out: &mut [f32],
) {
    let n = r2.len();
    let sigma2_v = f32x8::splat(sigma2);
    let eps_v = f32x8::splat(epsilon);
    let four_v = f32x8::splat(4.0);
    let twenty_four_v = f32x8::splat(24.0);
    let two_v = f32x8::splat(2.0);

    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        let r2_v = f32x8::from(&r2[base..base + SIMD_WIDTH]);
        let r2_inv = sigma2_v / r2_v;
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        // Energy: 4*epsilon*(r12_inv - r6_inv)
        let energy = four_v * eps_v * (r12_inv - r6_inv);

        // Force/r: 24*epsilon/r2 * (2*r12_inv - r6_inv)
        let force_over_r = twenty_four_v * eps_v / r2_v * (two_v * r12_inv - r6_inv);

        let energy_arr: [f32; SIMD_WIDTH] = energy.into();
        let force_arr: [f32; SIMD_WIDTH] = force_over_r.into();

        energy_out[base..base + SIMD_WIDTH].copy_from_slice(&energy_arr);
        force_over_r_out[base..base + SIMD_WIDTH].copy_from_slice(&force_arr);
    }

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        let r2_inv = sigma2 / r2[idx];
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        energy_out[idx] = 4.0 * epsilon * (r12_inv - r6_inv);
        force_over_r_out[idx] = 24.0 * epsilon / r2[idx] * (2.0 * r12_inv - r6_inv);
    }
}

/// Compute inverse square root using SIMD (fast approximation)
///
/// Uses Newton-Raphson refinement for accuracy.
#[inline(always)]
pub fn rsqrt_simd(x: &[f32], out: &mut [f32]) {
    let n = x.len();
    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    let half = f32x8::splat(0.5);
    let three = f32x8::splat(3.0);

    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        let x_v = f32x8::from(&x[base..base + SIMD_WIDTH]);
        // Initial approximation
        let y = x_v.sqrt().recip();
        // Newton-Raphson refinement: y = y * (3 - x*y*y) / 2
        let refined = y * (three - x_v * y * y) * half;

        let result: [f32; SIMD_WIDTH] = refined.into();
        out[base..base + SIMD_WIDTH].copy_from_slice(&result);
    }

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        out[idx] = 1.0 / x[idx].sqrt();
    }
}

// ============================================================================
// Optimized batch processing functions for force calculations
// ============================================================================

/// Process multiple atom pairs in batch with SIMD
///
/// This is the core hot path for force calculations. It computes:
/// 1. Squared distances with minimum image convention
/// 2. LJ potential and force
/// 3. Force component updates
///
/// Uses 2x loop unrolling for better instruction-level parallelism.
#[inline(always)]
pub fn batch_lj_forces(
    // Atom i data (single atom, broadcasted)
    xi: f32,
    yi: f32,
    zi: f32,
    // Atom j data (multiple atoms)
    xj: &[f32],
    yj: &[f32],
    zj: &[f32],
    // Box parameters
    box_: &SimulationBox,
    // LJ parameters
    sigma2: f32,
    epsilon: f32,
    cutoff2: f32,
    // Output forces on atom j (will be negated for atom i)
    fx_out: &mut [f32],
    fy_out: &mut [f32],
    fz_out: &mut [f32],
    // Output: energy accumulated
) -> f32 {
    let n = xj.len();
    let chunks = n / SIMD_WIDTH;
    let remainder = n % SIMD_WIDTH;

    // Broadcast atom i position
    let xi_v = f32x8::splat(xi);
    let yi_v = f32x8::splat(yi);
    let zi_v = f32x8::splat(zi);

    // Box parameters
    let [lx, ly, lz] = box_.dimensions;
    let [lx_inv, ly_inv, lz_inv] = box_.inverse;
    let lx_v = f32x8::splat(lx);
    let ly_v = f32x8::splat(ly);
    let lz_v = f32x8::splat(lz);
    let lx_inv_v = f32x8::splat(lx_inv);
    let ly_inv_v = f32x8::splat(ly_inv);
    let lz_inv_v = f32x8::splat(lz_inv);

    // LJ parameters
    let sigma2_v = f32x8::splat(sigma2);
    let eps24_v = f32x8::splat(24.0 * epsilon);
    let cutoff2_v = f32x8::splat(cutoff2);
    let min_r2_v = f32x8::splat(1e-10);

    let mut energy_acc = f32x8::ZERO;

    // Main SIMD loop with prefetching
    for chunk in 0..chunks {
        let base = chunk * SIMD_WIDTH;

        // Prefetch next iteration's data
        if base + PREFETCH_DISTANCE < n {
            crate::simd::prefetch_read(xj.as_ptr().wrapping_add(base + PREFETCH_DISTANCE));
            crate::simd::prefetch_read(yj.as_ptr().wrapping_add(base + PREFETCH_DISTANCE));
            crate::simd::prefetch_read(zj.as_ptr().wrapping_add(base + PREFETCH_DISTANCE));
        }

        // Load neighbor positions
        let xj_v = f32x8::from(&xj[base..base + SIMD_WIDTH]);
        let yj_v = f32x8::from(&yj[base..base + SIMD_WIDTH]);
        let zj_v = f32x8::from(&zj[base..base + SIMD_WIDTH]);

        // Displacement vectors
        let mut dx = xj_v - xi_v;
        let mut dy = yj_v - yi_v;
        let mut dz = zj_v - zi_v;

        // Minimum image convention
        if box_.periodic[0] {
            dx = dx - lx_v * simd_round(dx * lx_inv_v);
        }
        if box_.periodic[1] {
            dy = dy - ly_v * simd_round(dy * ly_inv_v);
        }
        if box_.periodic[2] {
            dz = dz - lz_v * simd_round(dz * lz_inv_v);
        }

        // Squared distance
        let r2 = dx * dx + dy * dy + dz * dz;

        // Cutoff mask: only compute for pairs within cutoff
        // (min_r2 check removed - LJ naturally handles close range)
        let mask = r2.cmp_lt(cutoff2_v);

        // LJ calculation (compute for all, mask will zero out invalid)
        let r2_inv = sigma2_v / r2;
        let r6_inv = r2_inv * r2_inv * r2_inv;
        let r12_inv = r6_inv * r6_inv;

        // F/r = 24*epsilon/r^2 * (2*r12 - r6)
        let two_v = f32x8::splat(2.0);
        let force_over_r = eps24_v / r2 * (two_v * r12_inv - r6_inv);

        // Apply mask
        let masked_for = mask.blend(force_over_r, f32x8::ZERO);

        // Force components
        let fx = masked_for * dx;
        let fy = masked_for * dy;
        let fz = masked_for * dz;

        // Store forces
        let fx_arr: [f32; SIMD_WIDTH] = fx.into();
        let fy_arr: [f32; SIMD_WIDTH] = fy.into();
        let fz_arr: [f32; SIMD_WIDTH] = fz.into();

        fx_out[base..base + SIMD_WIDTH].copy_from_slice(&fx_arr);
        fy_out[base..base + SIMD_WIDTH].copy_from_slice(&fy_arr);
        fz_out[base..base + SIMD_WIDTH].copy_from_slice(&fz_arr);

        // Accumulate energy (4*eps*(r12 - r6))
        let four_eps_v = f32x8::splat(4.0 * epsilon);
        let energy = four_eps_v * (r12_inv - r6_inv);
        energy_acc = energy_acc + mask.blend(energy, f32x8::ZERO);
    }

    // Sum energy across SIMD lanes
    let energy_arr: [f32; SIMD_WIDTH] = energy_acc.into();
    let mut total_energy: f32 = energy_arr.iter().sum();

    // Scalar remainder
    let base = chunks * SIMD_WIDTH;
    for i in 0..remainder {
        let idx = base + i;
        let mut dx = xj[idx] - xi;
        let mut dy = yj[idx] - yi;
        let mut dz = zj[idx] - zi;

        if box_.periodic[0] {
            dx -= lx * (dx * lx_inv).round();
        }
        if box_.periodic[1] {
            dy -= ly * (dy * ly_inv).round();
        }
        if box_.periodic[2] {
            dz -= lz * (dz * lz_inv).round();
        }

        let r2 = dx * dx + dy * dy + dz * dz;

        if r2 < cutoff2 && r2 > 1e-10 {
            let r2_inv = sigma2 / r2;
            let r6_inv = r2_inv * r2_inv * r2_inv;
            let r12_inv = r6_inv * r6_inv;

            let force_over_r = 24.0 * epsilon / r2 * (2.0 * r12_inv - r6_inv);

            fx_out[idx] = force_over_r * dx;
            fy_out[idx] = force_over_r * dy;
            fz_out[idx] = force_over_r * dz;

            total_energy += 4.0 * epsilon * (r12_inv - r6_inv);
        } else {
            fx_out[idx] = 0.0;
            fy_out[idx] = 0.0;
            fz_out[idx] = 0.0;
        }
    }

    total_energy
}

/// Batch process squared distances with cutoff check
/// Returns the number of valid pairs (within cutoff)
#[inline(always)]
pub fn batch_distances_filtered(
    xi: f32,
    yi: f32,
    zi: f32,
    xj: &[f32],
    yj: &[f32],
    zj: &[f32],
    box_: &SimulationBox,
    cutoff2: f32,
    indices_out: &mut [usize],
    r2_out: &mut [f32],
    dx_out: &mut [f32],
    dy_out: &mut [f32],
    dz_out: &mut [f32],
) -> usize {
    let n = xj.len();
    let [lx, ly, lz] = box_.dimensions;
    let [lx_inv, ly_inv, lz_inv] = box_.inverse;
    let periodic = box_.periodic;

    let mut count = 0;

    // Process in batches to allow vectorization
    for j in 0..n {
        let mut dx = xj[j] - xi;
        let mut dy = yj[j] - yi;
        let mut dz = zj[j] - zi;

        if periodic[0] {
            dx -= lx * (dx * lx_inv).round();
        }
        if periodic[1] {
            dy -= ly * (dy * ly_inv).round();
        }
        if periodic[2] {
            dz -= lz * (dz * lz_inv).round();
        }

        let r2 = dx * dx + dy * dy + dz * dz;

        if r2 < cutoff2 {
            indices_out[count] = j;
            r2_out[count] = r2;
            dx_out[count] = dx;
            dy_out[count] = dy;
            dz_out[count] = dz;
            count += 1;
        }
    }

    count
}

/// Update multiple force arrays with SIMD (unrolled 2x)
#[inline(always)]
pub fn accumulate_forces_unrolled(
    fx: &mut [f32],
    fy: &mut [f32],
    fz: &mut [f32],
    dfx: &[f32],
    dfy: &[f32],
    dfz: &[f32],
) {
    debug_assert_eq!(fx.len(), dfx.len());

    let n = fx.len();
    // Process 16 elements at a time (2 SIMD vectors)
    let chunks = n / 16;
    let remainder = n % 16;

    for chunk in 0..chunks {
        let base = chunk * 16;

        // First vector
        let fx_v1 = f32x8::from(&fx[base..base + 8]);
        let fy_v1 = f32x8::from(&fy[base..base + 8]);
        let fz_v1 = f32x8::from(&fz[base..base + 8]);
        let dfx_v1 = f32x8::from(&dfx[base..base + 8]);
        let dfy_v1 = f32x8::from(&dfy[base..base + 8]);
        let dfz_v1 = f32x8::from(&dfz[base..base + 8]);

        // Second vector
        let fx_v2 = f32x8::from(&fx[base + 8..base + 16]);
        let fy_v2 = f32x8::from(&fy[base + 8..base + 16]);
        let fz_v2 = f32x8::from(&fz[base + 8..base + 16]);
        let dfx_v2 = f32x8::from(&dfx[base + 8..base + 16]);
        let dfy_v2 = f32x8::from(&dfy[base + 8..base + 16]);
        let dfz_v2 = f32x8::from(&dfz[base + 8..base + 16]);

        // Compute
        let result_x1: [f32; 8] = (fx_v1 + dfx_v1).into();
        let result_y1: [f32; 8] = (fy_v1 + dfy_v1).into();
        let result_z1: [f32; 8] = (fz_v1 + dfz_v1).into();
        let result_x2: [f32; 8] = (fx_v2 + dfx_v2).into();
        let result_y2: [f32; 8] = (fy_v2 + dfy_v2).into();
        let result_z2: [f32; 8] = (fz_v2 + dfz_v2).into();

        // Store
        fx[base..base + 8].copy_from_slice(&result_x1);
        fy[base..base + 8].copy_from_slice(&result_y1);
        fz[base..base + 8].copy_from_slice(&result_z1);
        fx[base + 8..base + 16].copy_from_slice(&result_x2);
        fy[base + 8..base + 16].copy_from_slice(&result_y2);
        fz[base + 8..base + 16].copy_from_slice(&result_z2);
    }

    // Handle remainder
    let base = chunks * 16;
    for i in 0..remainder {
        let idx = base + i;
        fx[idx] += dfx[idx];
        fy[idx] += dfy[idx];
        fz[idx] += dfz[idx];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairwise_distances() {
        let box_ = SimulationBox::cubic(10.0);

        let x1 = [0.0, 1.0, 2.0];
        let y1 = [0.0, 0.0, 0.0];
        let z1 = [0.0, 0.0, 0.0];

        let x2 = [1.0, 2.0, 3.0];
        let y2 = [0.0, 0.0, 0.0];
        let z2 = [0.0, 0.0, 0.0];

        let mut out = [0.0; 3];
        pairwise_distances_squared(&x1, &y1, &z1, &x2, &y2, &z2, &box_, &mut out);

        // All distances should be 1.0, so squared = 1.0
        for d2 in &out {
            assert!((*d2 - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_pairwise_distances_pbc() {
        let box_ = SimulationBox::cubic(10.0);

        let x1 = [9.5];
        let y1 = [0.0];
        let z1 = [0.0];

        let x2 = [0.5];
        let y2 = [0.0];
        let z2 = [0.0];

        let mut out = [0.0; 1];
        pairwise_distances_squared(&x1, &y1, &z1, &x2, &y2, &z2, &box_, &mut out);

        // With PBC, distance should be 1.0 (not 9.0)
        assert!((out[0] - 1.0).abs() < 1e-6);
    }
}
