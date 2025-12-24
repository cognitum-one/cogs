//! Configuration unit tests

#[cfg(test)]
mod config_tests {
    use std::time::Duration;

    #[derive(Debug, Clone, PartialEq)]
    pub struct SdkConfig {
        pub max_cycles: u64,
        pub tile_count: u32,
        pub timeout: Duration,
        pub enable_profiling: bool,
    }

    impl Default for SdkConfig {
        fn default() -> Self {
            Self {
                max_cycles: 1_000_000,
                tile_count: 256,
                timeout: Duration::from_secs(300),
                enable_profiling: false,
            }
        }
    }

    impl SdkConfig {
        pub fn validate(&self) -> Result<(), ConfigError> {
            if self.max_cycles == 0 {
                return Err(ConfigError::InvalidMaxCycles);
            }
            if self.tile_count == 0 || self.tile_count > 4096 {
                return Err(ConfigError::InvalidTileCount);
            }
            if self.timeout.as_secs() == 0 {
                return Err(ConfigError::InvalidTimeout);
            }
            Ok(())
        }
    }

    #[derive(Debug, thiserror::Error, PartialEq)]
    pub enum ConfigError {
        #[error("Invalid max cycles")]
        InvalidMaxCycles,
        #[error("Invalid tile count")]
        InvalidTileCount,
        #[error("Invalid timeout")]
        InvalidTimeout,
    }

    #[test]
    fn should_create_default_config() {
        let config = SdkConfig::default();

        assert_eq!(config.max_cycles, 1_000_000);
        assert_eq!(config.tile_count, 256);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(!config.enable_profiling);
    }

    #[test]
    fn should_validate_valid_config() {
        let config = SdkConfig {
            max_cycles: 100000,
            tile_count: 128,
            timeout: Duration::from_secs(60),
            enable_profiling: true,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn should_reject_zero_max_cycles() {
        let config = SdkConfig {
            max_cycles: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert_eq!(result, Err(ConfigError::InvalidMaxCycles));
    }

    #[test]
    fn should_reject_zero_tile_count() {
        let config = SdkConfig {
            tile_count: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert_eq!(result, Err(ConfigError::InvalidTileCount));
    }

    #[test]
    fn should_reject_excessive_tile_count() {
        let config = SdkConfig {
            tile_count: 5000,
            ..Default::default()
        };

        let result = config.validate();
        assert_eq!(result, Err(ConfigError::InvalidTileCount));
    }

    #[test]
    fn should_reject_zero_timeout() {
        let config = SdkConfig {
            timeout: Duration::from_secs(0),
            ..Default::default()
        };

        let result = config.validate();
        assert_eq!(result, Err(ConfigError::InvalidTimeout));
    }

    #[test]
    fn should_accept_max_tile_count() {
        let config = SdkConfig {
            tile_count: 4096,
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }
}
