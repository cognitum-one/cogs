use crate::error::{ProcessorError, Result};

/// Stack trait for dependency injection (TDD London School)
pub trait Stack {
    fn push(&mut self, value: i32) -> Result<()>;
    fn pop(&mut self) -> Result<i32>;
    fn peek(&self) -> Result<i32>;
    fn depth(&self) -> usize;
    fn is_empty(&self) -> bool;
}

/// Concrete stack implementation
pub struct DataStack {
    data: Vec<i32>,
    max_depth: usize,
}

impl DataStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            data: Vec::with_capacity(max_depth),
            max_depth,
        }
    }

    pub fn with_default_size() -> Self {
        Self::new(256) // Default stack depth
    }
}

impl Stack for DataStack {
    fn push(&mut self, value: i32) -> Result<()> {
        if self.data.len() >= self.max_depth {
            return Err(ProcessorError::StackOverflow);
        }
        self.data.push(value);
        Ok(())
    }

    fn pop(&mut self) -> Result<i32> {
        self.data.pop().ok_or(ProcessorError::StackUnderflow)
    }

    fn peek(&self) -> Result<i32> {
        self.data
            .last()
            .copied()
            .ok_or(ProcessorError::StackUnderflow)
    }

    fn depth(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Return stack for subroutine calls
pub struct ReturnStack {
    data: Vec<u32>,
    max_depth: usize,
}

impl ReturnStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            data: Vec::with_capacity(max_depth),
            max_depth,
        }
    }

    pub fn with_default_size() -> Self {
        Self::new(64) // Smaller default for return stack
    }

    pub fn push(&mut self, addr: u32) -> Result<()> {
        if self.data.len() >= self.max_depth {
            return Err(ProcessorError::ReturnStackOverflow);
        }
        self.data.push(addr);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<u32> {
        self.data.pop().ok_or(ProcessorError::ReturnStackUnderflow)
    }

    pub fn depth(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_push_pop() {
        let mut stack = DataStack::with_default_size();
        assert!(stack.push(42).is_ok());
        assert_eq!(stack.pop().unwrap(), 42);
    }

    #[test]
    fn test_stack_underflow() {
        let mut stack = DataStack::with_default_size();
        assert_eq!(stack.pop(), Err(ProcessorError::StackUnderflow));
    }

    #[test]
    fn test_stack_overflow() {
        let mut stack = DataStack::new(2);
        assert!(stack.push(1).is_ok());
        assert!(stack.push(2).is_ok());
        assert_eq!(stack.push(3), Err(ProcessorError::StackOverflow));
    }

    #[test]
    fn test_stack_peek() {
        let mut stack = DataStack::with_default_size();
        stack.push(42).unwrap();
        assert_eq!(stack.peek().unwrap(), 42);
        assert_eq!(stack.depth(), 1); // Peek doesn't pop
    }

    #[test]
    fn test_return_stack() {
        let mut rstack = ReturnStack::with_default_size();
        rstack.push(0x1000).unwrap();
        rstack.push(0x2000).unwrap();
        assert_eq!(rstack.pop().unwrap(), 0x2000);
        assert_eq!(rstack.pop().unwrap(), 0x1000);
    }
}
