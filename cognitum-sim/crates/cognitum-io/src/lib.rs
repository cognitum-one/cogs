//! I/O interfaces (PCIe, Ethernet, USB)
//!
//! This crate implements various I/O interfaces for the Cognitum ASIC.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod ethernet;
pub mod pcie;
pub mod usb;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
