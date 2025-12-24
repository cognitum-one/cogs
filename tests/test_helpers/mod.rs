//! Test helper utilities and mock factories

pub mod mocks;
pub mod fixtures;
pub mod assertions;

/// Create a test UUID for consistent testing
pub fn test_uuid() -> String {
    "00000000-0000-0000-0000-000000000000".to_string()
}

/// Create a test timestamp
pub fn test_timestamp() -> i64 {
    1609459200 // 2021-01-01 00:00:00 UTC
}

/// Create test binary data
pub fn test_program_binary() -> Vec<u8> {
    vec![0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01, 0x01, 0x00] // ELF header
}

/// Create test API key
pub fn test_api_key() -> String {
    "sk_test_0123456789abcdef0123456789abcdef".to_string()
}

/// Create test JWT token
pub fn test_jwt_token() -> String {
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0In0.test".to_string()
}

/// Assert duration is within expected range
#[macro_export]
macro_rules! assert_duration_within {
    ($duration:expr, $expected:expr, $tolerance:expr) => {
        let diff = if $duration > $expected {
            $duration - $expected
        } else {
            $expected - $duration
        };
        assert!(
            diff <= $tolerance,
            "Duration {:?} not within {:?} ± {:?}",
            $duration,
            $expected,
            $tolerance
        );
    };
}

/// Assert vector has expected properties
#[macro_export]
macro_rules! assert_vector_valid {
    ($vector:expr, $dimensions:expr) => {
        assert_eq!($vector.len(), $dimensions, "Invalid vector dimensions");
        assert!($vector.iter().all(|&x| x.is_finite()), "Vector contains non-finite values");
    };
}
