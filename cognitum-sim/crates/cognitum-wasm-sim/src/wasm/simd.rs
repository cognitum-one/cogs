//! SIMD v128 unit simulation
//!
//! Implements 128-bit vector operations with lane-based processing

use crate::error::{Result, WasmSimError, WasmTrap};

/// SIMD v128 unit
pub struct SimdUnit {
    /// v128 value stack
    stack: Vec<[i32; 4]>,

    /// Maximum stack depth
    max_depth: usize,

    /// Operation count
    ops_count: u64,
}

impl SimdUnit {
    /// Create new SIMD unit
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(32),
            max_depth: 32,
            ops_count: 0,
        }
    }

    /// Push v128 value
    pub fn push(&mut self, value: [i32; 4]) -> Result<()> {
        if self.stack.len() >= self.max_depth {
            return Err(WasmSimError::Trap(WasmTrap::StackOverflow));
        }
        self.stack.push(value);
        Ok(())
    }

    /// Pop v128 value
    pub fn pop(&mut self) -> Result<[i32; 4]> {
        self.stack.pop()
            .ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))
    }

    /// Peek at top
    pub fn peek(&self) -> Option<&[i32; 4]> {
        self.stack.last()
    }

    /// Get stack depth
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Clear stack
    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Get operation count
    pub fn ops_count(&self) -> u64 {
        self.ops_count
    }

    // ===== i8x16 Operations (16 lanes of 8-bit) =====

    /// i8x16 add
    pub fn i8x16_add(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_bytes = a[i].to_le_bytes();
            let b_bytes = b[i].to_le_bytes();
            let mut r_bytes = [0u8; 4];
            for j in 0..4 {
                r_bytes[j] = a_bytes[j].wrapping_add(b_bytes[j]);
            }
            result[i] = i32::from_le_bytes(r_bytes);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i8x16 sub
    pub fn i8x16_sub(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_bytes = a[i].to_le_bytes();
            let b_bytes = b[i].to_le_bytes();
            let mut r_bytes = [0u8; 4];
            for j in 0..4 {
                r_bytes[j] = a_bytes[j].wrapping_sub(b_bytes[j]);
            }
            result[i] = i32::from_le_bytes(r_bytes);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i8x16 saturating add (signed)
    pub fn i8x16_add_sat_s(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_bytes = a[i].to_le_bytes();
            let b_bytes = b[i].to_le_bytes();
            let mut r_bytes = [0u8; 4];
            for j in 0..4 {
                let av = a_bytes[j] as i8;
                let bv = b_bytes[j] as i8;
                r_bytes[j] = av.saturating_add(bv) as u8;
            }
            result[i] = i32::from_le_bytes(r_bytes);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i8x16 saturating add (unsigned)
    pub fn i8x16_add_sat_u(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_bytes = a[i].to_le_bytes();
            let b_bytes = b[i].to_le_bytes();
            let mut r_bytes = [0u8; 4];
            for j in 0..4 {
                r_bytes[j] = a_bytes[j].saturating_add(b_bytes[j]);
            }
            result[i] = i32::from_le_bytes(r_bytes);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    // ===== i16x8 Operations (8 lanes of 16-bit) =====

    /// i16x8 add
    pub fn i16x8_add(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_lo = (a[i] & 0xFFFF) as i16;
            let a_hi = ((a[i] >> 16) & 0xFFFF) as i16;
            let b_lo = (b[i] & 0xFFFF) as i16;
            let b_hi = ((b[i] >> 16) & 0xFFFF) as i16;

            let r_lo = a_lo.wrapping_add(b_lo) as u16;
            let r_hi = a_hi.wrapping_add(b_hi) as u16;

            result[i] = (r_lo as i32) | ((r_hi as i32) << 16);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i16x8 mul
    pub fn i16x8_mul(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_lo = (a[i] & 0xFFFF) as i16;
            let a_hi = ((a[i] >> 16) & 0xFFFF) as i16;
            let b_lo = (b[i] & 0xFFFF) as i16;
            let b_hi = ((b[i] >> 16) & 0xFFFF) as i16;

            let r_lo = a_lo.wrapping_mul(b_lo) as u16;
            let r_hi = a_hi.wrapping_mul(b_hi) as u16;

            result[i] = (r_lo as i32) | ((r_hi as i32) << 16);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    // ===== i32x4 Operations (4 lanes of 32-bit) =====

    /// i32x4 add
    pub fn i32x4_add(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            result[i] = a[i].wrapping_add(b[i]);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i32x4 sub
    pub fn i32x4_sub(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            result[i] = a[i].wrapping_sub(b[i]);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i32x4 mul
    pub fn i32x4_mul(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            result[i] = a[i].wrapping_mul(b[i]);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// i32x4 dot product with i16x8
    pub fn i32x4_dot_i16x8_s(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            let a_lo = (a[i] & 0xFFFF) as i16 as i32;
            let a_hi = ((a[i] >> 16) & 0xFFFF) as i16 as i32;
            let b_lo = (b[i] & 0xFFFF) as i16 as i32;
            let b_hi = ((b[i] >> 16) & 0xFFFF) as i16 as i32;

            result[i] = a_lo.wrapping_mul(b_lo).wrapping_add(a_hi.wrapping_mul(b_hi));
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    // ===== v128 Bitwise =====

    /// v128 and
    pub fn v128_and(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]];
        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// v128 or
    pub fn v128_or(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = [a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]];
        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// v128 xor
    pub fn v128_xor(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]];
        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// v128 not
    pub fn v128_not(&mut self) -> Result<()> {
        let a = self.pop()?;

        let result = [!a[0], !a[1], !a[2], !a[3]];
        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// v128 andnot (a & ~b)
    pub fn v128_andnot(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = [a[0] & !b[0], a[1] & !b[1], a[2] & !b[2], a[3] & !b[3]];
        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    /// v128 bitselect ((a & c) | (b & ~c))
    pub fn v128_bitselect(&mut self) -> Result<()> {
        let c = self.pop()?;
        let b = self.pop()?;
        let a = self.pop()?;

        let mut result = [0i32; 4];
        for i in 0..4 {
            result[i] = (a[i] & c[i]) | (b[i] & !c[i]);
        }

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }

    // ===== Lane Operations =====

    /// Splat i32 value to all lanes
    pub fn i32x4_splat(&mut self, value: i32) -> Result<()> {
        self.push([value, value, value, value])?;
        self.ops_count += 1;
        Ok(())
    }

    /// Extract lane from i32x4
    pub fn i32x4_extract_lane(&mut self, lane: u8) -> Result<i32> {
        if lane >= 4 {
            return Err(WasmSimError::InvalidBytecode("Lane index out of bounds".into()));
        }
        let v = self.pop()?;
        self.ops_count += 1;
        Ok(v[lane as usize])
    }

    /// Replace lane in i32x4
    pub fn i32x4_replace_lane(&mut self, lane: u8, value: i32) -> Result<()> {
        if lane >= 4 {
            return Err(WasmSimError::InvalidBytecode("Lane index out of bounds".into()));
        }
        let mut v = self.pop()?;
        v[lane as usize] = value;
        self.push(v)?;
        self.ops_count += 1;
        Ok(())
    }

    /// Swizzle bytes using indices from second vector
    pub fn i8x16_swizzle(&mut self) -> Result<()> {
        let indices = self.pop()?;
        let values = self.pop()?;

        // Flatten both to bytes
        let v_bytes: Vec<u8> = values.iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let i_bytes: Vec<u8> = indices.iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();

        let mut r_bytes = [0u8; 16];
        for i in 0..16 {
            let idx = i_bytes[i] as usize;
            r_bytes[i] = if idx < 16 { v_bytes[idx] } else { 0 };
        }

        let result = [
            i32::from_le_bytes([r_bytes[0], r_bytes[1], r_bytes[2], r_bytes[3]]),
            i32::from_le_bytes([r_bytes[4], r_bytes[5], r_bytes[6], r_bytes[7]]),
            i32::from_le_bytes([r_bytes[8], r_bytes[9], r_bytes[10], r_bytes[11]]),
            i32::from_le_bytes([r_bytes[12], r_bytes[13], r_bytes[14], r_bytes[15]]),
        ];

        self.push(result)?;
        self.ops_count += 1;
        Ok(())
    }
}

impl Default for SimdUnit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i32x4_add() {
        let mut simd = SimdUnit::new();

        simd.push([1, 2, 3, 4]).unwrap();
        simd.push([10, 20, 30, 40]).unwrap();
        simd.i32x4_add().unwrap();

        let result = simd.pop().unwrap();
        assert_eq!(result, [11, 22, 33, 44]);
    }

    #[test]
    fn test_i32x4_mul() {
        let mut simd = SimdUnit::new();

        simd.push([1, 2, 3, 4]).unwrap();
        simd.push([2, 3, 4, 5]).unwrap();
        simd.i32x4_mul().unwrap();

        let result = simd.pop().unwrap();
        assert_eq!(result, [2, 6, 12, 20]);
    }

    #[test]
    fn test_v128_and() {
        let mut simd = SimdUnit::new();

        simd.push([0xFF, 0xF0, 0x0F, 0x00]).unwrap();
        simd.push([0xAA, 0xAA, 0xAA, 0xAA]).unwrap();
        simd.v128_and().unwrap();

        let result = simd.pop().unwrap();
        assert_eq!(result, [0xAA, 0xA0, 0x0A, 0x00]);
    }

    #[test]
    fn test_splat() {
        let mut simd = SimdUnit::new();

        simd.i32x4_splat(42).unwrap();
        let result = simd.pop().unwrap();
        assert_eq!(result, [42, 42, 42, 42]);
    }
}
