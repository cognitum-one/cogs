//! Program validation utilities

use super::errors::SdkError;

/// Validate bytecode format
pub fn validate_bytecode(bytecode: &[u8]) -> Result<(), SdkError> {
    // Check not empty
    if bytecode.is_empty() {
        return Err(SdkError::InvalidProgram("Empty bytecode".to_string()));
    }

    // Check length is multiple of 4 (32-bit instructions)
    if bytecode.len() % 4 != 0 {
        return Err(SdkError::InvalidProgram(
            "Bytecode length must be multiple of 4 bytes".to_string(),
        ));
    }

    // Check reasonable size (max 1MB)
    if bytecode.len() > 1024 * 1024 {
        return Err(SdkError::InvalidProgram(
            "Bytecode exceeds maximum size of 1MB".to_string(),
        ));
    }

    Ok(())
}

/// Validate simulator configuration
pub fn validate_config(config: &super::types::SimulatorConfig) -> Result<(), SdkError> {
    // Check worker threads
    if let Some(threads) = config.worker_threads {
        if threads == 0 {
            return Err(SdkError::ConfigError(
                "Worker threads must be at least 1".to_string(),
            ));
        }
        if threads > 256 {
            return Err(SdkError::ConfigError(
                "Worker threads cannot exceed 256".to_string(),
            ));
        }
    }

    // Check max cycles
    if let Some(cycles) = config.max_cycles {
        if cycles == 0 {
            return Err(SdkError::ConfigError(
                "Max cycles must be at least 1".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bytecode_empty() {
        assert!(validate_bytecode(&[]).is_err());
    }

    #[test]
    fn test_validate_bytecode_invalid_length() {
        assert!(validate_bytecode(&[0x01, 0x02, 0x03]).is_err());
    }

    #[test]
    fn test_validate_bytecode_valid() {
        assert!(validate_bytecode(&[0x01, 0x02, 0x03, 0x04]).is_ok());
    }

    #[test]
    fn test_validate_config_invalid_threads() {
        let mut config = super::super::types::SimulatorConfig::default();
        config.worker_threads = Some(0);
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_valid() {
        let config = super::super::types::SimulatorConfig::default();
        assert!(validate_config(&config).is_ok());
    }
}
