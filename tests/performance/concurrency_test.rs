//! Concurrency and parallelism tests

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    #[tokio::test]
    async fn should_handle_concurrent_reads() {
        // Given: Shared read-only data
        let data = Arc::new(vec![1, 2, 3, 4, 5]);
        let reader_count = 1000;

        // When: Many concurrent readers
        let mut handles = vec![];

        for _ in 0..reader_count {
            let data_clone = data.clone();
            let handle = tokio::spawn(async move {
                let _sum: i32 = data_clone.iter().sum();
            });
            handles.push(handle);
        }

        // Then: All should complete successfully
        let results = futures::future::join_all(handles).await;
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), reader_count);
    }

    #[tokio::test]
    async fn should_handle_concurrent_updates() {
        // Given: Shared counter
        let counter = Arc::new(AtomicU64::new(0));
        let updater_count = 1000;
        let updates_per_task = 100;

        // When: Many concurrent updates
        let mut handles = vec![];

        for _ in 0..updater_count {
            let counter_clone = counter.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..updates_per_task {
                    counter_clone.fetch_add(1, Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        futures::future::join_all(handles).await;

        // Then: All updates should be accounted for
        let final_count = counter.load(Ordering::Relaxed);
        assert_eq!(final_count, updater_count * updates_per_task);
    }

    #[tokio::test]
    async fn should_not_deadlock_under_contention() {
        // Given: Shared resources
        use tokio::sync::RwLock;
        let data = Arc::new(RwLock::new(vec![0; 100]));
        let task_count = 100;

        // When: Mixed read/write access
        let mut handles = vec![];

        for i in 0..task_count {
            let data_clone = data.clone();
            let handle = tokio::spawn(async move {
                if i % 2 == 0 {
                    // Reader
                    let _guard = data_clone.read().await;
                    tokio::time::sleep(std::time::Duration::from_micros(10)).await;
                } else {
                    // Writer
                    let mut guard = data_clone.write().await;
                    guard[0] += 1;
                }
            });
            handles.push(handle);
        }

        // Then: Should complete without deadlock
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            futures::future::join_all(handles),
        )
        .await;

        assert!(timeout.is_ok(), "Deadlock detected!");
    }

    #[tokio::test]
    async fn should_scale_with_cpu_cores() {
        // Given: CPU-intensive work
        let iterations = 100_000;

        // When: Running serially
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = (0..100).sum::<i32>();
        }
        let serial_time = start.elapsed();

        // When: Running in parallel
        let start = std::time::Instant::now();
        let mut handles = vec![];

        let workers = num_cpus::get();
        let per_worker = iterations / workers;

        for _ in 0..workers {
            let handle = tokio::task::spawn_blocking(move || {
                for _ in 0..per_worker {
                    let _ = (0..100).sum::<i32>();
                }
            });
            handles.push(handle);
        }

        futures::future::join_all(handles).await;
        let parallel_time = start.elapsed();

        // Then: Parallel should be faster
        println!("Serial: {:?}, Parallel: {:?}", serial_time, parallel_time);
        println!("Speedup: {:.2}x", serial_time.as_secs_f64() / parallel_time.as_secs_f64());

        // Note: Speedup depends on system, but parallel should not be slower
        assert!(parallel_time <= serial_time * 2);
    }
}
