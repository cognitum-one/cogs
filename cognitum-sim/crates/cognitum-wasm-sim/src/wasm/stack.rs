//! WASM value stack with register mapping and spill/fill
//!
//! Implements a hybrid stack that maps top entries to hardware registers
//! and spills deeper entries to memory. Uses circular buffer for O(1) operations.

use crate::error::{Result, WasmSimError, WasmTrap};

/// WASM value stack with hardware register mapping
///
/// Optimized with:
/// - Circular buffer for O(1) push/pop (no shifting)
/// - Cache-aligned register file
/// - Pre-allocated spill buffer
#[repr(C)]  // Cache-friendly layout
pub struct WasmStack {
    /// Register file (fast access for top N entries)
    /// Using circular buffer with head/tail pointers
    registers: Vec<i32>,

    /// Spill memory (for entries beyond register file)
    spill: Vec<i32>,

    /// Maximum register file size
    register_depth: usize,

    /// Circular buffer head (oldest entry)
    head: usize,

    /// Circular buffer tail (newest entry)
    tail: usize,

    /// Number of entries in register file
    reg_count: usize,

    /// Shadow stack for call return addresses
    shadow_stack: Vec<u32>,

    /// Maximum shadow stack depth
    shadow_depth: usize,

    /// Frame pointer (for local variables)
    fp: usize,

    /// Local variable storage
    locals: Vec<i32>,
}

impl WasmStack {
    /// Create new stack with specified depths
    #[inline]
    pub fn new(register_depth: usize, shadow_depth: usize) -> Self {
        Self {
            registers: vec![0; register_depth],
            spill: Vec::with_capacity(256),
            register_depth,
            head: 0,
            tail: 0,
            reg_count: 0,
            shadow_stack: Vec::with_capacity(shadow_depth),
            shadow_depth,
            fp: 0,
            locals: Vec::with_capacity(64),
        }
    }

    /// Push value onto stack - O(1) using circular buffer
    #[inline(always)]
    pub fn push(&mut self, value: i32) -> Result<()> {
        if self.reg_count >= self.register_depth {
            // Spill oldest entry (at head)
            let spill_value = self.registers[self.head];
            self.spill.push(spill_value);
            self.head = (self.head + 1) % self.register_depth;
            self.reg_count -= 1;
        }

        // Add new value at tail
        self.registers[self.tail] = value;
        self.tail = (self.tail + 1) % self.register_depth;
        self.reg_count += 1;

        Ok(())
    }

    /// Pop value from stack - O(1) using circular buffer
    #[inline(always)]
    pub fn pop(&mut self) -> Result<i32> {
        if self.reg_count == 0 && self.spill.is_empty() {
            return Err(WasmSimError::Trap(WasmTrap::StackUnderflow));
        }

        if self.reg_count > 0 {
            // Pop from register file (from tail, newest entry)
            self.tail = if self.tail == 0 { self.register_depth - 1 } else { self.tail - 1 };
            self.reg_count -= 1;
            Ok(self.registers[self.tail])
        } else {
            // When registers are exhausted, pop directly from spill
            self.spill.pop()
                .ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))
        }
    }

    /// Peek at top of stack without popping - O(1)
    #[inline(always)]
    pub fn peek(&self) -> Option<i32> {
        if self.reg_count > 0 {
            let idx = if self.tail == 0 { self.register_depth - 1 } else { self.tail - 1 };
            Some(self.registers[idx])
        } else if !self.spill.is_empty() {
            self.spill.last().copied()
        } else {
            None
        }
    }

    /// Peek at Nth item from top (0 = top)
    #[inline]
    pub fn peek_n(&self, n: usize) -> Option<i32> {
        let total_depth = self.depth();
        if n >= total_depth {
            return None;
        }

        if n < self.reg_count {
            // Within register file - index from tail backwards
            let idx = (self.tail + self.register_depth - 1 - n) % self.register_depth;
            Some(self.registers[idx])
        } else {
            // In spill memory
            let spill_idx = self.spill.len() - 1 - (n - self.reg_count);
            self.spill.get(spill_idx).copied()
        }
    }

    /// Get stack depth - O(1)
    #[inline(always)]
    pub fn depth(&self) -> usize {
        self.reg_count + self.spill.len()
    }

    /// Check if stack is empty - O(1)
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.reg_count == 0 && self.spill.is_empty()
    }

    /// Clear stack
    #[inline]
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.reg_count = 0;
        self.spill.clear();
        self.shadow_stack.clear();
        self.locals.clear();
    }

    /// Duplicate top of stack
    pub fn dup(&mut self) -> Result<()> {
        let value = self.peek().ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))?;
        self.push(value)
    }

    /// Swap top two elements
    pub fn swap(&mut self) -> Result<()> {
        if self.depth() < 2 {
            return Err(WasmSimError::Trap(WasmTrap::StackUnderflow));
        }

        let a = self.pop()?;
        let b = self.pop()?;
        self.push(a)?;
        self.push(b)?;

        Ok(())
    }

    /// Push call return address to shadow stack
    pub fn push_return(&mut self, addr: u32) -> Result<()> {
        if self.shadow_stack.len() >= self.shadow_depth {
            return Err(WasmSimError::Trap(WasmTrap::CallStackOverflow));
        }
        self.shadow_stack.push(addr);
        Ok(())
    }

    /// Pop return address from shadow stack
    pub fn pop_return(&mut self) -> Result<u32> {
        self.shadow_stack.pop()
            .ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))
    }

    /// Get call depth
    pub fn call_depth(&self) -> usize {
        self.shadow_stack.len()
    }

    /// Initialize locals for a function call
    pub fn init_locals(&mut self, count: usize) {
        self.fp = self.locals.len();
        self.locals.resize(self.fp + count, 0);
    }

    /// Get local variable
    pub fn get_local(&self, index: u32) -> Result<i32> {
        let idx = self.fp + index as usize;
        self.locals.get(idx)
            .copied()
            .ok_or(WasmSimError::MemoryOutOfBounds {
                address: index,
                size: 4,
            })
    }

    /// Set local variable
    pub fn set_local(&mut self, index: u32, value: i32) -> Result<()> {
        let idx = self.fp + index as usize;
        if idx >= self.locals.len() {
            return Err(WasmSimError::MemoryOutOfBounds {
                address: index,
                size: 4,
            });
        }
        self.locals[idx] = value;
        Ok(())
    }

    /// Pop locals frame (on function return)
    pub fn pop_frame(&mut self, prev_fp: usize) {
        self.locals.truncate(self.fp);
        self.fp = prev_fp;
    }

    /// Get register file state (for debugging)
    /// Returns entries in logical order (oldest to newest)
    pub fn registers(&self) -> Vec<i32> {
        let mut result = Vec::with_capacity(self.reg_count);
        for i in 0..self.reg_count {
            let idx = (self.head + i) % self.register_depth;
            result.push(self.registers[idx]);
        }
        result
    }

    /// Get spill memory state (for debugging)
    pub fn spill_memory(&self) -> &[i32] {
        &self.spill
    }
}

/// V128 stack for SIMD operations
pub struct V128Stack {
    /// 128-bit values stored as 4 x i32
    values: Vec<[i32; 4]>,

    /// Maximum depth
    max_depth: usize,
}

impl V128Stack {
    /// Create new v128 stack
    pub fn new(max_depth: usize) -> Self {
        Self {
            values: Vec::with_capacity(max_depth),
            max_depth,
        }
    }

    /// Push v128 value
    pub fn push(&mut self, value: [i32; 4]) -> Result<()> {
        if self.values.len() >= self.max_depth {
            return Err(WasmSimError::Trap(WasmTrap::StackOverflow));
        }
        self.values.push(value);
        Ok(())
    }

    /// Pop v128 value
    pub fn pop(&mut self) -> Result<[i32; 4]> {
        self.values.pop()
            .ok_or(WasmSimError::Trap(WasmTrap::StackUnderflow))
    }

    /// Peek at top
    pub fn peek(&self) -> Option<&[i32; 4]> {
        self.values.last()
    }

    /// Get depth
    pub fn depth(&self) -> usize {
        self.values.len()
    }

    /// Clear stack
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_push_pop() {
        let mut stack = WasmStack::new(16, 32);

        stack.push(42).unwrap();
        stack.push(100).unwrap();

        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.pop().unwrap(), 100);
        assert_eq!(stack.pop().unwrap(), 42);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_spill() {
        let mut stack = WasmStack::new(4, 32); // Small register file

        // Push more than register depth
        for i in 0..10i32 {
            stack.push(i).unwrap();
        }

        assert_eq!(stack.depth(), 10);

        // Pop all and verify order (LIFO)
        for i in (0..10i32).rev() {
            let val = stack.pop().unwrap();
            assert_eq!(val, i, "Expected {} but got {} when popping", i, val);
        }
    }

    #[test]
    fn test_shadow_stack() {
        let mut stack = WasmStack::new(16, 32);

        stack.push_return(0x1000).unwrap();
        stack.push_return(0x2000).unwrap();

        assert_eq!(stack.call_depth(), 2);
        assert_eq!(stack.pop_return().unwrap(), 0x2000);
        assert_eq!(stack.pop_return().unwrap(), 0x1000);
    }

    #[test]
    fn test_locals() {
        let mut stack = WasmStack::new(16, 32);

        stack.init_locals(3);
        stack.set_local(0, 10).unwrap();
        stack.set_local(1, 20).unwrap();
        stack.set_local(2, 30).unwrap();

        assert_eq!(stack.get_local(0).unwrap(), 10);
        assert_eq!(stack.get_local(1).unwrap(), 20);
        assert_eq!(stack.get_local(2).unwrap(), 30);
    }
}
