//! Debugger and profiling tools
//!
//! This crate provides debugging and profiling capabilities for
//! the Cognitum simulator.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod debugger;
pub mod profiler;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
