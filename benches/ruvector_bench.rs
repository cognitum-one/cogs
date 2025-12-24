//! Performance benchmarks for Ruvector SDK
//!
//! Target performance:
//! - Embedding generation: < 1μs per tile
//! - HNSW search (1M vectors): < 10ms
//! - Routing prediction: < 100μs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use cognitum::ruvector::{
    CognitumRuvector, RuvectorConfig, EmbeddingGenerator, DefaultEmbeddingGenerator,
    VectorIndex, HnswVectorIndex, TaskRouter, TinyDancerRouter,
};
use cognitum::ruvector::types::{TileState, Embedding, TaskEmbedding, EmbeddingId, Metadata};

fn benchmark_embedding_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_generation");

    let generator = DefaultEmbeddingGenerator::new(256);

    // Single tile embedding
    group.bench_function("single_tile", |b| {
        let state = TileState::random();
        b.iter(|| {
            black_box(generator.from_tile_state(black_box(&state)))
        });
    });

    // Batch embedding (64 tiles)
    group.bench_function("batch_64_tiles", |b| {
        let states: Vec<TileState> = (0..64).map(|_| TileState::random()).collect();
        b.iter(|| {
            black_box(generator.batch_generate(black_box(&states)))
        });
    });

    group.finish();
}

fn benchmark_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search");

    // Small index (1K vectors)
    let mut index_1k = HnswVectorIndex::new(256);
    for i in 0..1_000 {
        let embedding = Embedding::random(256);
        let metadata = Metadata::default();
        index_1k.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
    }

    group.bench_function("search_1k_vectors", |b| {
        let query = Embedding::random(256);
        b.iter(|| {
            black_box(index_1k.search(black_box(&query), 10))
        });
    });

    // Medium index (10K vectors)
    let mut index_10k = HnswVectorIndex::new(256);
    for i in 0..10_000 {
        let embedding = Embedding::random(256);
        let metadata = Metadata::default();
        index_10k.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
    }

    group.bench_function("search_10k_vectors", |b| {
        let query = Embedding::random(256);
        b.iter(|| {
            black_box(index_10k.search(black_box(&query), 10))
        });
    });

    // Large index (100K vectors) - note: 1M would be too slow for CI
    let mut index_100k = HnswVectorIndex::new(256);
    for i in 0..100_000 {
        let embedding = Embedding::random(256);
        let metadata = Metadata::default();
        index_100k.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
    }

    group.bench_function("search_100k_vectors", |b| {
        let query = Embedding::random(256);
        b.iter(|| {
            black_box(index_100k.search(black_box(&query), 10))
        });
    });

    group.finish();
}

fn benchmark_task_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_routing");

    for num_tiles in [4, 8, 16, 32] {
        let router = TinyDancerRouter::new(num_tiles, 256);

        group.bench_with_input(
            BenchmarkId::new("predict_tile", num_tiles),
            &num_tiles,
            |b, _| {
                let task = TaskEmbedding::random();
                b.iter(|| {
                    black_box(router.predict_tile(black_box(&task)))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("confidence", num_tiles),
            &num_tiles,
            |b, _| {
                let task = TaskEmbedding::random();
                b.iter(|| {
                    black_box(router.confidence(black_box(&task)))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    let config = RuvectorConfig::default();
    let ruvector = CognitumRuvector::new(config);

    // Capture state and store
    group.bench_function("capture_and_store", |b| {
        let states: Vec<TileState> = (0..16).map(|_| TileState::random()).collect();
        b.iter(|| {
            let embeddings = ruvector.capture_state(black_box(&states));
            black_box(ruvector.store_embeddings(&embeddings))
        });
    });

    // Populate index for search benchmark
    for _ in 0..1000 {
        let states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();
        let embeddings = ruvector.capture_state(&states);
        ruvector.store_embeddings(&embeddings).unwrap();
    }

    // Search benchmark
    group.bench_function("capture_and_search", |b| {
        let state = TileState::random();
        b.iter(|| {
            let embeddings = ruvector.capture_state(black_box(&[state]));
            black_box(ruvector.search_similar(&embeddings[0], 10))
        });
    });

    // Route task benchmark
    group.bench_function("route_task", |b| {
        let task = TaskEmbedding::random();
        b.iter(|| {
            black_box(ruvector.route_task(black_box(&task)))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_embedding_generation,
    benchmark_vector_search,
    benchmark_task_routing,
    benchmark_end_to_end,
);
criterion_main!(benches);
