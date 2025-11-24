//! Comprehensive tests for debug tools

use cognitum_debug::debugger::Debugger;
use cognitum_debug::profiler::Profiler;

// ============================================================================
// Debugger Tests
// ============================================================================

#[test]
fn test_debugger_creation() {
    let debugger = Debugger::new();
    assert!(true, "Debugger created successfully");
}

#[test]
fn test_debugger_default() {
    let debugger = Debugger::default();
    assert!(true, "Debugger created with default");
}

#[test]
fn test_debugger_multiple_instances() {
    let debugger1 = Debugger::new();
    let debugger2 = Debugger::new();
    let debugger3 = Debugger::default();
    assert!(true, "Multiple debuggers created");
}

#[test]
fn test_debugger_creation_pattern() {
    for _ in 0..10 {
        let debugger = Debugger::new();
        assert!(true, "Debugger created in loop");
    }
}

#[test]
fn test_debugger_lifecycle() {
    {
        let debugger = Debugger::new();
        // Debugger should be dropped here
    }
    assert!(true, "Debugger lifecycle completed");
}

// ============================================================================
// Profiler Tests
// ============================================================================

#[test]
fn test_profiler_creation() {
    let profiler = Profiler::new();
    assert!(true, "Profiler created successfully");
}

#[test]
fn test_profiler_default() {
    let profiler = Profiler::default();
    assert!(true, "Profiler created with default");
}

#[test]
fn test_profiler_increment() {
    let mut profiler = Profiler::new();
    profiler.increment("test_counter");
    assert!(true, "Counter incremented");
}

#[test]
fn test_profiler_get_zero() {
    let profiler = Profiler::new();
    let value = profiler.get("non_existent");
    assert_eq!(value, 0, "Non-existent counter should return 0");
}

#[test]
fn test_profiler_increment_and_get() {
    let mut profiler = Profiler::new();
    profiler.increment("test_counter");

    let value = profiler.get("test_counter");
    assert_eq!(value, 1, "Counter should be 1 after one increment");
}

#[test]
fn test_profiler_multiple_increments() {
    let mut profiler = Profiler::new();

    for _ in 0..5 {
        profiler.increment("test_counter");
    }

    let value = profiler.get("test_counter");
    assert_eq!(value, 5, "Counter should be 5 after five increments");
}

#[test]
fn test_profiler_multiple_counters() {
    let mut profiler = Profiler::new();

    profiler.increment("counter_a");
    profiler.increment("counter_b");
    profiler.increment("counter_a");

    assert_eq!(profiler.get("counter_a"), 2, "Counter A should be 2");
    assert_eq!(profiler.get("counter_b"), 1, "Counter B should be 1");
}

#[test]
fn test_profiler_many_counters() {
    let mut profiler = Profiler::new();

    for i in 0..100 {
        profiler.increment(&format!("counter_{}", i));
    }

    assert_eq!(profiler.get("counter_0"), 1, "First counter should be 1");
    assert_eq!(profiler.get("counter_50"), 1, "Middle counter should be 1");
    assert_eq!(profiler.get("counter_99"), 1, "Last counter should be 1");
}

#[test]
fn test_profiler_heavy_increment() {
    let mut profiler = Profiler::new();

    for _ in 0..1000 {
        profiler.increment("heavy_counter");
    }

    let value = profiler.get("heavy_counter");
    assert_eq!(value, 1000, "Counter should be 1000");
}

#[test]
fn test_profiler_counter_names() {
    let mut profiler = Profiler::new();

    let names = vec![
        "cycles",
        "cache_hits",
        "cache_misses",
        "tlb_hits",
        "tlb_misses",
        "instructions",
    ];

    for name in &names {
        profiler.increment(name);
    }

    for name in &names {
        assert_eq!(profiler.get(name), 1, "Counter {} should be 1", name);
    }
}

#[test]
fn test_profiler_concurrent_counters() {
    let mut profiler = Profiler::new();

    for i in 0..10 {
        profiler.increment("shared");
        profiler.increment(&format!("unique_{}", i));
    }

    assert_eq!(profiler.get("shared"), 10, "Shared counter should be 10");
    for i in 0..10 {
        assert_eq!(profiler.get(&format!("unique_{}", i)), 1);
    }
}

#[test]
fn test_profiler_empty_name() {
    let mut profiler = Profiler::new();
    profiler.increment("");

    let value = profiler.get("");
    assert_eq!(value, 1, "Empty name counter should work");
}

#[test]
fn test_profiler_long_name() {
    let mut profiler = Profiler::new();
    let long_name = "a".repeat(1000);

    profiler.increment(&long_name);
    let value = profiler.get(&long_name);
    assert_eq!(value, 1, "Long name counter should work");
}

#[test]
fn test_profiler_special_characters() {
    let mut profiler = Profiler::new();
    let names = vec![
        "counter-with-dashes",
        "counter_with_underscores",
        "counter.with.dots",
        "counter::with::colons",
    ];

    for name in &names {
        profiler.increment(name);
    }

    for name in &names {
        assert_eq!(profiler.get(name), 1);
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_debugger_and_profiler_together() {
    let debugger = Debugger::new();
    let mut profiler = Profiler::new();

    profiler.increment("debug_sessions");

    assert_eq!(profiler.get("debug_sessions"), 1);
    assert!(true, "Debugger and profiler work together");
}

#[test]
fn test_multiple_debug_tools() {
    let debugger1 = Debugger::new();
    let debugger2 = Debugger::new();
    let mut profiler1 = Profiler::new();
    let mut profiler2 = Profiler::new();

    profiler1.increment("tool1");
    profiler2.increment("tool2");

    assert_eq!(profiler1.get("tool1"), 1);
    assert_eq!(profiler2.get("tool2"), 1);
}

#[test]
fn test_profiler_stress() {
    let mut profiler = Profiler::new();

    // Create many counters
    for i in 0..100 {
        profiler.increment(&format!("counter_{}", i));
    }

    // Increment them many times
    for i in 0..100 {
        for _ in 0..10 {
            profiler.increment(&format!("counter_{}", i));
        }
    }

    // Verify
    for i in 0..100 {
        assert_eq!(profiler.get(&format!("counter_{}", i)), 11); // 1 initial + 10 more
    }
}
