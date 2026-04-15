//! Benchmarks for Cognitum Ruvector SDK

use cognitum_sdk_ruvector::prelude::*;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_insert(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = RuvectorClient::builder()
        .embedding_dimension(256)
        .build()
        .unwrap();

    c.bench_function("insert_256d", |b| {
        let mut id = 0u64;
        b.iter(|| {
            let embedding = vec![0.1; 256];
            rt.block_on(async {
                client.insert(id, &embedding, Metadata::default()).await.unwrap();
            });
            id += 1;
        });
    });
}

fn bench_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = RuvectorClient::builder()
        .embedding_dimension(256)
        .build()
        .unwrap();

    // Populate index
    for i in 0..1000 {
        let embedding = vec![0.1; 256];
        rt.block_on(async {
            client.insert(i, &embedding, Metadata::default()).await.unwrap();
        });
    }

    let mut group = c.benchmark_group("search");
    for k in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("k", k), &k, |b, &k| {
            b.iter(|| {
                let query = vec![0.1; 256];
                rt.block_on(async {
                    client.search(&query, k).await.unwrap();
                });
            });
        });
    }
    group.finish();
}

fn bench_routing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = RuvectorClient::builder()
        .embedding_dimension(256)
        .num_tiles(16)
        .build()
        .unwrap();

    c.bench_function("predict_tile", |b| {
        b.iter(|| {
            let task = vec![0.3; 256];
            rt.block_on(async {
                client.predict_tile(&task).await.unwrap();
            });
        });
    });
}

criterion_group!(benches, bench_insert, bench_search, bench_routing);
criterion_main!(benches);
