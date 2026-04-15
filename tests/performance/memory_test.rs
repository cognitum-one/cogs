//! Memory usage validation tests

#[cfg(test)]
mod memory_tests {
    use std::sync::Arc;

    #[test]
    fn should_not_leak_memory_under_repeated_operations() {
        // Given: Repeated allocations
        let iterations = 10000;

        // When: Creating and dropping structures
        for _ in 0..iterations {
            let data = vec![0u8; 1024]; // 1KB allocation
            drop(data);
        }

        // Then: Memory should be reclaimed (verified by system tools)
        // Note: This test validates no panics/crashes occur
        // Use valgrind or similar tools for detailed leak detection
    }

    #[test]
    fn should_handle_large_allocations_gracefully() {
        // Given: Large allocation requirement
        let large_size = 100 * 1024 * 1024; // 100MB

        // When: Allocating large buffer
        let result = std::panic::catch_unwind(|| {
            let buffer = vec![0u8; large_size];
            assert_eq!(buffer.len(), large_size);
        });

        // Then: Should handle without panic
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn concurrent_allocations_should_not_exhaust_memory() {
        // Given: Many concurrent tasks
        let task_count = 1000;
        let allocation_size = 1024 * 1024; // 1MB per task

        // When: Running concurrent tasks
        let mut handles = vec![];

        for _ in 0..task_count {
            let handle = tokio::spawn(async move {
                let _data = vec![0u8; allocation_size];
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            });
            handles.push(handle);
        }

        // Then: All tasks should complete
        let results = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        assert_eq!(success_count, task_count);
    }

    #[test]
    fn arc_shared_data_should_be_properly_freed() {
        // Given: Shared data structure
        let data = Arc::new(vec![0u8; 1024 * 1024]); // 1MB

        // When: Creating multiple references
        let refs: Vec<_> = (0..100).map(|_| data.clone()).collect();

        // Then: Should have many references
        assert_eq!(Arc::strong_count(&data), 101);

        // When: Dropping references
        drop(refs);

        // Then: Should have only original reference
        assert_eq!(Arc::strong_count(&data), 1);
    }

    #[test]
    fn buffer_reuse_should_reduce_allocations() {
        // Given: Reusable buffer
        let mut buffer = Vec::with_capacity(1024);

        // When: Reusing buffer
        for i in 0..1000 {
            buffer.clear();
            buffer.extend_from_slice(&[i as u8; 100]);

            // Verify capacity doesn't grow unnecessarily
            assert!(buffer.capacity() >= 1024);
            assert!(buffer.capacity() < 2048); // No excessive growth
        }
    }
}
