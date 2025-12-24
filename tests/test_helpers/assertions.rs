//! Custom assertion helpers for tests

use std::time::Duration;

/// Assert that a result is Ok and returns the inner value
pub fn assert_ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("Expected Ok, got Err: {:?}", e),
    }
}

/// Assert that a result is Err and returns the error
pub fn assert_err<T: std::fmt::Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => panic!("Expected Err, got Ok: {:?}", value),
        Err(e) => e,
    }
}

/// Assert two floats are approximately equal
pub fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
    let diff = (a - b).abs();
    assert!(
        diff < epsilon,
        "Values not approximately equal: {} vs {} (diff: {})",
        a,
        b,
        diff
    );
}

/// Assert a duration is within expected range
pub fn assert_duration_between(actual: Duration, min: Duration, max: Duration) {
    assert!(
        actual >= min && actual <= max,
        "Duration {:?} not in range [{:?}, {:?}]",
        actual,
        min,
        max
    );
}

/// Assert a vector has expected dimensions
pub fn assert_vector_dimensions(vector: &[f32], expected: usize) {
    assert_eq!(
        vector.len(),
        expected,
        "Vector has {} dimensions, expected {}",
        vector.len(),
        expected
    );
}

/// Assert a vector is normalized (length ~= 1.0)
pub fn assert_vector_normalized(vector: &[f32]) {
    let length: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert_approx_eq(length as f64, 1.0, 0.01);
}

/// Assert collection is not empty
pub fn assert_not_empty<T>(collection: &[T]) {
    assert!(
        !collection.is_empty(),
        "Expected non-empty collection, got length 0"
    );
}

/// Assert string contains substring
pub fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "String '{}' does not contain '{}'",
        haystack,
        needle
    );
}

/// Assert string does not contain substring
pub fn assert_not_contains(haystack: &str, needle: &str) {
    assert!(
        !haystack.contains(needle),
        "String '{}' unexpectedly contains '{}'",
        haystack,
        needle
    );
}

/// Assert JSON value has expected structure
#[cfg(feature = "json-assertions")]
pub fn assert_json_shape(value: &serde_json::Value, expected_keys: &[&str]) {
    if let serde_json::Value::Object(map) = value {
        for key in expected_keys {
            assert!(
                map.contains_key(*key),
                "JSON missing expected key: {}",
                key
            );
        }
    } else {
        panic!("Expected JSON object, got: {:?}", value);
    }
}
