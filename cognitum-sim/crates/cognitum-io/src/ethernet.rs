//! Ethernet interface

/// Ethernet controller
pub struct EthernetController {
    /// MAC address
    _mac: [u8; 6],
}

impl EthernetController {
    /// Create a new Ethernet controller
    pub fn new(mac: [u8; 6]) -> Self {
        Self { _mac: mac }
    }
}
