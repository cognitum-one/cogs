//! Simulation box with periodic boundary conditions

use serde::{Deserialize, Serialize};

/// Simulation box defining the simulation domain
///
/// Supports cubic, orthorhombic, and triclinic boxes with periodic boundary conditions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimulationBox {
    /// Box dimensions [Lx, Ly, Lz] in nm
    pub dimensions: [f32; 3],

    /// Periodic boundary conditions in each dimension
    pub periodic: [bool; 3],

    /// Precomputed inverse dimensions for fast PBC wrapping
    pub inverse: [f32; 3],
}

impl SimulationBox {
    /// Create a cubic box with side length L
    pub fn cubic(length: f32) -> Self {
        Self {
            dimensions: [length, length, length],
            periodic: [true, true, true],
            inverse: [1.0 / length, 1.0 / length, 1.0 / length],
        }
    }

    /// Create an orthorhombic box
    pub fn orthorhombic(lx: f32, ly: f32, lz: f32) -> Self {
        Self {
            dimensions: [lx, ly, lz],
            periodic: [true, true, true],
            inverse: [1.0 / lx, 1.0 / ly, 1.0 / lz],
        }
    }

    /// Create a non-periodic box (for isolated systems)
    pub fn non_periodic(lx: f32, ly: f32, lz: f32) -> Self {
        Self {
            dimensions: [lx, ly, lz],
            periodic: [false, false, false],
            inverse: [1.0 / lx, 1.0 / ly, 1.0 / lz],
        }
    }

    /// Set periodicity for each dimension
    pub fn with_periodic(mut self, px: bool, py: bool, pz: bool) -> Self {
        self.periodic = [px, py, pz];
        self
    }

    /// Calculate volume of the box
    #[inline]
    pub fn volume(&self) -> f32 {
        self.dimensions[0] * self.dimensions[1] * self.dimensions[2]
    }

    /// Apply minimum image convention to a displacement vector
    ///
    /// Returns the shortest displacement vector considering periodic boundaries.
    /// This is a hot path - always inline for maximum performance.
    #[inline(always)]
    pub fn minimum_image(&self, dx: f32, dy: f32, dz: f32) -> [f32; 3] {
        let mut result = [dx, dy, dz];

        if self.periodic[0] {
            result[0] -= self.dimensions[0] * (dx * self.inverse[0]).round();
        }
        if self.periodic[1] {
            result[1] -= self.dimensions[1] * (dy * self.inverse[1]).round();
        }
        if self.periodic[2] {
            result[2] -= self.dimensions[2] * (dz * self.inverse[2]).round();
        }

        result
    }

    /// Wrap a position into the primary box
    ///
    /// Hot path for integrator - always inline.
    #[inline(always)]
    pub fn wrap_position(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let mut result = [x, y, z];

        if self.periodic[0] {
            result[0] -= self.dimensions[0] * (x * self.inverse[0]).floor();
        }
        if self.periodic[1] {
            result[1] -= self.dimensions[1] * (y * self.inverse[1]).floor();
        }
        if self.periodic[2] {
            result[2] -= self.dimensions[2] * (z * self.inverse[2]).floor();
        }

        result
    }

    /// Calculate distance squared with minimum image convention
    ///
    /// Hot path for neighbor list and force calculations.
    #[inline(always)]
    pub fn distance_squared(&self, pos1: &[f32; 3], pos2: &[f32; 3]) -> f32 {
        let dx = pos1[0] - pos2[0];
        let dy = pos1[1] - pos2[1];
        let dz = pos1[2] - pos2[2];

        let [dx, dy, dz] = self.minimum_image(dx, dy, dz);

        dx * dx + dy * dy + dz * dz
    }

    /// Calculate distance with minimum image convention
    #[inline(always)]
    pub fn distance(&self, pos1: &[f32; 3], pos2: &[f32; 3]) -> f32 {
        self.distance_squared(pos1, pos2).sqrt()
    }

    /// Calculate displacement vector with minimum image convention
    ///
    /// Hot path for force calculations.
    #[inline(always)]
    pub fn displacement(&self, from: &[f32; 3], to: &[f32; 3]) -> [f32; 3] {
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let dz = to[2] - from[2];
        self.minimum_image(dx, dy, dz)
    }

    /// Check if a position is inside the box
    #[inline]
    pub fn contains(&self, pos: &[f32; 3]) -> bool {
        pos[0] >= 0.0
            && pos[0] < self.dimensions[0]
            && pos[1] >= 0.0
            && pos[1] < self.dimensions[1]
            && pos[2] >= 0.0
            && pos[2] < self.dimensions[2]
    }

    /// Get the center of the box
    #[inline]
    pub fn center(&self) -> [f32; 3] {
        [
            self.dimensions[0] * 0.5,
            self.dimensions[1] * 0.5,
            self.dimensions[2] * 0.5,
        ]
    }
}

impl Default for SimulationBox {
    fn default() -> Self {
        Self::cubic(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_box() {
        let box_ = SimulationBox::cubic(10.0);
        assert_eq!(box_.dimensions, [10.0, 10.0, 10.0]);
        assert_eq!(box_.periodic, [true, true, true]);
        assert!((box_.volume() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn test_minimum_image() {
        let box_ = SimulationBox::cubic(10.0);

        // No wrapping needed
        let [dx, dy, dz] = box_.minimum_image(1.0, 2.0, 3.0);
        assert!((dx - 1.0).abs() < 1e-6);
        assert!((dy - 2.0).abs() < 1e-6);
        assert!((dz - 3.0).abs() < 1e-6);

        // Wrapping needed
        let [dx, dy, dz] = box_.minimum_image(8.0, -7.0, 12.0);
        assert!((dx - (-2.0)).abs() < 1e-6);
        assert!((dy - 3.0).abs() < 1e-6);
        assert!((dz - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_wrap_position() {
        let box_ = SimulationBox::cubic(10.0);

        let [x, y, z] = box_.wrap_position(12.0, -3.0, 25.0);
        assert!((x - 2.0).abs() < 1e-6);
        assert!((y - 7.0).abs() < 1e-6);
        assert!((z - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_pbc() {
        let box_ = SimulationBox::cubic(10.0);

        let pos1 = [1.0, 1.0, 1.0];
        let pos2 = [9.0, 9.0, 9.0];

        // Without PBC: sqrt(3*64) = 13.86
        // With PBC: sqrt(3*4) = 3.46
        let d = box_.distance(&pos1, &pos2);
        assert!((d - (12.0_f32).sqrt()).abs() < 1e-5);
    }
}
