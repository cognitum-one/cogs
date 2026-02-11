//! Benchmarks for the page-aligned ANN index.
//!
//! Covers: page serialization, encoding/decoding, routing, search, and
//! build pipeline.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use cognitum::ruvector::page_index::*;
use cognitum::ruvector::types::EmbeddingId;

fn random_vectors(count: usize, dim: usize) -> Vec<(EmbeddingId, Vec<f32>)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|i| {
            (
                EmbeddingId(i as u64),
                (0..dim).map(|_| rng.gen::<f32>()).collect(),
            )
        })
        .collect()
}

fn bench_encode_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_decode");

    for dim in [64, 128, 256] {
        let vector: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let params = QuantScaleParams {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 255.0,
        };

        group.bench_with_input(
            BenchmarkId::new("encode_8bit", dim),
            &dim,
            |b, _| {
                b.iter(|| encode_vector(black_box(&vector), QuantTier::Hot, &params))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("encode_5bit", dim),
            &dim,
            |b, _| {
                b.iter(|| encode_vector(black_box(&vector), QuantTier::Warm, &params))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("encode_3bit", dim),
            &dim,
            |b, _| {
                b.iter(|| encode_vector(black_box(&vector), QuantTier::Cold, &params))
            },
        );

        let encoded_hot = encode_vector(&vector, QuantTier::Hot, &params);
        group.bench_with_input(
            BenchmarkId::new("decode_8bit", dim),
            &dim,
            |b, &d| {
                b.iter(|| decode_vector(black_box(&encoded_hot), d, QuantTier::Hot, &params))
            },
        );
    }

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_serialization");

    for vec_count in [10, 50, 100] {
        let dim = 128;
        let vectors = random_vectors(vec_count, dim);
        let config = PageIndexConfig {
            dimension: dim,
            max_vectors_per_page: vec_count,
            ..Default::default()
        };
        let mut builder = PageBuilder::new(config.clone(), 1);
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();
        builder.build(
            &vectors,
            CollectionId(1),
            TenantId(1),
            &mut routing,
            &mut store,
        );

        // Get the first page for serialization benchmark
        let page_ids = routing.centroid_hnsw.page_ids();
        if let Some(&pid) = page_ids.first() {
            let page = store.fetch(pid).unwrap();
            let serialized = serialize_page(&page);

            group.bench_with_input(
                BenchmarkId::new("serialize", vec_count),
                &vec_count,
                |b, _| {
                    b.iter(|| serialize_page(black_box(&page)))
                },
            );

            group.bench_with_input(
                BenchmarkId::new("deserialize", vec_count),
                &vec_count,
                |b, _| {
                    b.iter(|| deserialize_page(black_box(&serialized)).unwrap())
                },
            );
        }
    }

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_search");

    for (num_vectors, num_pages_label) in [(500, "~25p"), (2000, "~100p"), (5000, "~250p")] {
        let dim = 128;
        let config = PageIndexConfig {
            dimension: dim,
            max_vectors_per_page: 20,
            ..Default::default()
        };
        let mut index = PageAlignedIndex::new(config);
        let vectors = random_vectors(num_vectors, dim);
        index.build_from_vectors(&vectors, CollectionId(1), TenantId(1));

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();

        group.bench_with_input(
            BenchmarkId::new("k10_budget8", num_pages_label),
            &num_vectors,
            |b, _| {
                let budget = SearchBudget {
                    max_candidate_pages: 32,
                    max_disk_reads: 8,
                    max_duration: None,
                };
                b.iter(|| {
                    index.search_with_budget(
                        black_box(&query),
                        10,
                        &budget,
                        &SearchFilter::default(),
                    )
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("k10_budget16", num_pages_label),
            &num_vectors,
            |b, _| {
                let budget = SearchBudget {
                    max_candidate_pages: 64,
                    max_disk_reads: 16,
                    max_duration: None,
                };
                b.iter(|| {
                    index.search_with_budget(
                        black_box(&query),
                        10,
                        &budget,
                        &SearchFilter::default(),
                    )
                })
            },
        );
    }

    group.finish();
}

fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_build");

    for num_vectors in [100, 500, 1000] {
        let dim = 128;
        let vectors = random_vectors(num_vectors, dim);

        group.bench_with_input(
            BenchmarkId::new("build", num_vectors),
            &num_vectors,
            |b, _| {
                b.iter(|| {
                    let config = PageIndexConfig {
                        dimension: dim,
                        max_vectors_per_page: 20,
                        ..Default::default()
                    };
                    let mut index = PageAlignedIndex::new(config);
                    index.build_from_vectors(
                        black_box(&vectors),
                        CollectionId(1),
                        TenantId(1),
                    )
                })
            },
        );
    }

    group.finish();
}

fn bench_insert_and_compact(c: &mut Criterion) {
    let dim = 64;

    c.bench_function("insert_100_vectors", |b| {
        b.iter(|| {
            let config = PageIndexConfig {
                dimension: dim,
                max_vectors_per_page: 10,
                ..Default::default()
            };
            let mut index = PageAlignedIndex::new(config);
            for i in 0..100 {
                let v: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
                index.insert(EmbeddingId(i), &v);
            }
        })
    });
}

criterion_group!(
    benches,
    bench_encode_decode,
    bench_serialization,
    bench_search,
    bench_build,
    bench_insert_and_compact,
);
criterion_main!(benches);
