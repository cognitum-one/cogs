//! PCIe interface

/// PCIe controller
pub struct PcieController {
    /// Number of lanes
    _lanes: u8,
}

impl PcieController {
    /// Create a new PCIe controller
    pub fn new(lanes: u8) -> Self {
        Self { _lanes: lanes }
    }
}
