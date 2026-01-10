//! Error types for ThermalBrain

use core::fmt;

/// ThermalBrain error type
#[derive(Clone, Debug, PartialEq)]
pub enum ThermalBrainError {
    // Initialization
    /// System not initialized
    NotInitialized,
    /// System already initialized
    AlreadyInitialized,
    /// Hardware initialization failed
    HardwareInitFailed,

    // Sensors
    /// Temperature sensor failed
    TempSensorFailed,
    /// Touch sensor failed
    TouchSensorFailed,
    /// WiFi scan failed
    WifiScanFailed,
    /// BLE scan failed
    BleScanFailed,

    // Processing
    /// Buffer overflow
    BufferOverflow,
    /// Encoding failed
    EncodingFailed,
    /// Matcher failed
    MatcherFailed,
    /// Neuron overload
    NeuronOverload,

    // Storage
    /// Flash write failed
    FlashWriteFailed,
    /// Flash read failed
    FlashReadFailed,
    /// Flash storage full
    FlashFull,
    /// Pattern not found
    PatternNotFound(u32),
    /// Pattern limit reached
    PatternLimitReached,
    /// Index corrupted
    IndexCorrupted,

    // Thermal
    /// System overheating
    Overheating(u32), // Temperature * 100 (to avoid f32 in enum)
    /// Thermal shock detected
    ThermalShock,

    // Config
    /// Invalid configuration
    InvalidConfig,
    /// Invalid label
    InvalidLabel,

    // System
    /// Out of memory
    OutOfMemory,
    /// Operation timeout
    Timeout,
}

impl fmt::Display for ThermalBrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "System not initialized"),
            Self::AlreadyInitialized => write!(f, "System already initialized"),
            Self::HardwareInitFailed => write!(f, "Hardware initialization failed"),
            Self::TempSensorFailed => write!(f, "Temperature sensor failed"),
            Self::TouchSensorFailed => write!(f, "Touch sensor failed"),
            Self::WifiScanFailed => write!(f, "WiFi scan failed"),
            Self::BleScanFailed => write!(f, "BLE scan failed"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
            Self::EncodingFailed => write!(f, "Encoding failed"),
            Self::MatcherFailed => write!(f, "Matcher failed"),
            Self::NeuronOverload => write!(f, "Neuron overload"),
            Self::FlashWriteFailed => write!(f, "Flash write failed"),
            Self::FlashReadFailed => write!(f, "Flash read failed"),
            Self::FlashFull => write!(f, "Flash storage full"),
            Self::PatternNotFound(id) => write!(f, "Pattern {} not found", id),
            Self::PatternLimitReached => write!(f, "Pattern limit reached"),
            Self::IndexCorrupted => write!(f, "Index corrupted"),
            Self::Overheating(temp) => write!(f, "Overheating: {}.{}°C", temp / 100, temp % 100),
            Self::ThermalShock => write!(f, "Thermal shock detected"),
            Self::InvalidConfig => write!(f, "Invalid configuration"),
            Self::InvalidLabel => write!(f, "Invalid label"),
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::Timeout => write!(f, "Operation timeout"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ThermalBrainError {}

/// Result type for ThermalBrain operations
pub type Result<T> = core::result::Result<T, ThermalBrainError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ThermalBrainError::PatternNotFound(42);
        assert_eq!(format!("{}", err), "Pattern 42 not found");

        let err = ThermalBrainError::Overheating(7500);
        assert_eq!(format!("{}", err), "Overheating: 75.0°C");
    }
}
