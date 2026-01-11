//! SIMD-optimized distance calculations
//!
//! Uses the `wide` crate for portable SIMD operations across x86 (SSE/AVX)
//! and ARM (NEON) architectures.

use crate::types::SimulationBox;
use wide::{f32x8, CmpLt};

/// SIMD width for f32x8 operations
const SIMD_WIDTH: usize = 8;

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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
