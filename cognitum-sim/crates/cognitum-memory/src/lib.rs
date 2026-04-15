//! Memory subsystem with ML-enhanced cache prediction
//!
//! This crate implements the memory hierarchy including caches,
//! TLBs, and ML-based prefetching.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod cache;
pub mod dram;
pub mod tlb;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
