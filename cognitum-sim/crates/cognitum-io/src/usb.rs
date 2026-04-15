//! USB interface

/// USB controller
pub struct UsbController {
    /// USB version
    _version: u8,
}

impl UsbController {
    /// Create a new USB controller
    pub fn new(version: u8) -> Self {
        Self { _version: version }
    }
}
