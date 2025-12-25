//! API Performance Benchmarks
//!
//! Target performance:
//! - Request latency p99: < 50ms
//! - Throughput: > 10K req/sec
//! - Connection pooling efficiency

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

// Mock rate limiter for benchmarking
struct MockRateLimiter {
    counters: parking_lot::RwLock<HashMap<String, (u64, SystemTime)>>,
    limit: u64,
}

impl MockRateLimiter {
    fn new(limit: u64) -> Self {
        Self {
            counters: parking_lot::RwLock::new(HashMap::new()),
            limit,
        }
    }

    fn check(&self, key: &str) -> bool {
        let mut counters = self.counters.write();
        let now = SystemTime::now();

        let entry = counters.entry(key.to_string()).or_insert((0, now));

        // Reset if window expired
        if now.duration_since(entry.1).unwrap_or_default() > Duration::from_secs(60) {
            *entry = (0, now);
        }

        if entry.0 < self.limit {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

// Mock connection pool
struct ConnectionPool {
    connections: parking_lot::Mutex<Vec<Connection>>,
    max_size: usize,
}

struct Connection {
    id: usize,
    active: bool,
}

impl ConnectionPool {
    fn new(max_size: usize) -> Self {
        let connections = (0..max_size)
            .map(|id| Connection { id, active: false })
            .collect();

        Self {
            connections: parking_lot::Mutex::new(connections),
            max_size,
        }
    }

    fn acquire(&self) -> Option<usize> {
        let mut conns = self.connections.lock();
        conns.iter_mut()
            .find(|c| !c.active)
            .map(|c| {
                c.active = true;
                c.id
            })
    }

    fn release(&self, id: usize) {
        let mut conns = self.connections.lock();
        if let Some(conn) = conns.get_mut(id) {
            conn.active = false;
        }
    }
}

fn bench_rate_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiting");
    group.throughput(Throughput::Elements(1));

    // Single key rate limiting
    group.bench_function("single_key_check", |b| {
        let limiter = MockRateLimiter::new(1000);
        b.iter(|| {
            let allowed = limiter.check(black_box("sk_test_123"));
            black_box(allowed);
        });
    });

    // Multiple keys (simulating concurrent API keys)
    for num_keys in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("multi_key_check", num_keys),
            &num_keys,
            |b, &count| {
                let limiter = MockRateLimiter::new(1000);
                let keys: Vec<String> = (0..count)
                    .map(|i| format!("sk_test_{}", i))
                    .collect();

                b.iter(|| {
                    for key in &keys {
                        let allowed = limiter.check(black_box(key));
                        black_box(allowed);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_connection_pooling(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pooling");

    // Connection acquire/release
    for pool_size in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("acquire_release", pool_size),
            &pool_size,
            |b, &size| {
                let pool = ConnectionPool::new(size);
                b.iter(|| {
                    if let Some(conn_id) = pool.acquire() {
                        // Simulate work
                        black_box(conn_id);
                        pool.release(conn_id);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_request_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_latency");
    group.throughput(Throughput::Elements(1));

    // Simulate minimal request processing
    group.bench_function("minimal_request", |b| {
        let limiter = Arc::new(MockRateLimiter::new(10000));
        let pool = Arc::new(ConnectionPool::new(100));

        b.iter(|| {
            // Rate limit check
            let key = "sk_test_123";
            if limiter.check(black_box(key)) {
                // Acquire connection
                if let Some(conn_id) = pool.acquire() {
                    // Simulate processing
                    black_box(conn_id);
                    // Release connection
                    pool.release(conn_id);
                }
            }
        });
    });

    // Simulate full request pipeline
    group.bench_function("full_request_pipeline", |b| {
        let limiter = Arc::new(MockRateLimiter::new(10000));
        let pool = Arc::new(ConnectionPool::new(100));

        b.iter(|| {
            // 1. Rate limit check
            let key = "sk_test_123";
            if !limiter.check(black_box(key)) {
                return;
            }

            // 2. Acquire connection
            let conn_id = match pool.acquire() {
                Some(id) => id,
                None => return,
            };

            // 3. Simulate query execution
            let mut result = 0u64;
            for i in 0..100 {
                result = result.wrapping_add(i);
            }
            black_box(result);

            // 4. Release connection
            pool.release(conn_id);
        });
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Sequential requests
    for num_requests in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(num_requests));
        group.bench_with_input(
            BenchmarkId::new("sequential_requests", num_requests),
            &num_requests,
            |b, &count| {
                let limiter = Arc::new(MockRateLimiter::new(1_000_000));
                let pool = Arc::new(ConnectionPool::new(100));

                b.iter(|| {
                    for i in 0..count {
                        let key = format!("sk_test_{}", i % 100);
                        if limiter.check(black_box(&key)) {
                            if let Some(conn_id) = pool.acquire() {
                                black_box(conn_id);
                                pool.release(conn_id);
                            }
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    // Simulated concurrent access patterns
    group.bench_function("contended_limiter", |b| {
        let limiter = Arc::new(MockRateLimiter::new(10000));

        b.iter(|| {
            // Simulate 10 concurrent requests to same key
            for _ in 0..10 {
                let allowed = limiter.check(black_box("sk_test_shared"));
                black_box(allowed);
            }
        });
    });

    group.bench_function("distributed_keys", |b| {
        let limiter = Arc::new(MockRateLimiter::new(10000));

        b.iter(|| {
            // Simulate 10 concurrent requests to different keys
            for i in 0..10 {
                let key = format!("sk_test_{}", i);
                let allowed = limiter.check(black_box(&key));
                black_box(allowed);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_rate_limiting,
    bench_connection_pooling,
    bench_request_latency,
    bench_throughput,
    bench_concurrent_access,
);
criterion_main!(benches);
