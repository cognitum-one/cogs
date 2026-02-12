//! Page-Aligned ANN Index for SSD-Resident Vectors
//!
//! Billion-scale vector search where most vectors live on SSD, with
//! predictable p95 latency by bounding random reads per query.
//!
//! # Architecture (3 layers)
//!
//! 1. **Storage layer** (`storage`): Page binary format, quantization
//!    encoding/decoding (8/5/3-bit tiers), checksums, manifests.
//!
//! 2. **Routing layer** (`routing`): In-memory HNSW over page centroids
//!    plus a compact page adjacency graph. Narrows the search to a bounded
//!    set of candidate pages before any disk I/O.
//!
//! 3. **Execution layer** (`search`): Fetches candidate pages, decodes
//!    vectors, scores against the query with SIMD, applies early exit,
//!    and returns top-k results with a full audit trace.
//!
//! # Write path
//!
//! New vectors land in delta pages (`delta`) which are periodically
//! compacted into base pages. Min-cut coherence can trigger local
//! reclustering of page boundaries.
//!
//! # Build pipeline
//!
//! The offline `builder` clusters vectors into page-sized groups,
//! computes centroids, builds the page graph, and registers pages
//! in the routing index and store.
//!
//! # Design reference
//!
//! See ADR-009: Page-Aligned ANN Index for SSD-Resident Vectors.

pub mod types;
pub mod storage;
pub mod routing;
pub mod search;
pub mod delta;
pub mod builder;

// Re-export primary types
pub use types::{
    PageId, PageVersion, CollectionId, TenantId,
    QuantTier, QuantScaleParams,
    PageHeader, PageNode, PageManifest, PageLocation,
    SearchBudget, PageIndexConfig,
    QueryTrace, PageSearchResult, PageSearchResponse,
    SearchFilter, CompactionPolicy, PageIndexStats,
};

pub use storage::{
    serialize_page, deserialize_page,
    encode_vector, decode_vector, encoded_vector_size,
    fit_quant_params, crc32,
    PageStorageError,
};

pub use routing::{
    CentroidEntry, PageGraph, CentroidHnsw, RoutingIndex,
    l2_sq,
};

pub use search::{
    PageStore, InMemoryPageStore,
    execute_search,
};

pub use delta::{
    DeltaPageManager, CompactionResult,
    compact_deltas, compute_centroid,
};

pub use builder::{
    PageBuilder, BuildResult,
};

// ============================================================================
// Convenience: PageAlignedIndex (combines all layers)
// ============================================================================

/// High-level page-aligned ANN index combining routing, storage, and deltas.
///
/// This is the main entry point for using the page-aligned index.
pub struct PageAlignedIndex {
    /// Configuration.
    pub config: PageIndexConfig,
    /// In-memory routing index (centroids + page graph).
    pub routing: RoutingIndex,
    /// Page store (in-memory or backed by SSD).
    pub store: InMemoryPageStore,
    /// Delta page manager for writes.
    pub delta_manager: DeltaPageManager,
    /// Compaction policy.
    pub compaction_policy: CompactionPolicy,
    /// Next page id counter.
    next_page_id: u64,
    /// Statistics.
    stats: PageIndexStats,
}

impl PageAlignedIndex {
    /// Create a new page-aligned index.
    pub fn new(config: PageIndexConfig) -> Self {
        let routing = RoutingIndex::new(&config);
        let store = InMemoryPageStore::new();
        let delta_manager = DeltaPageManager::new(
            config.clone(),
            1_000_000, // delta page ids start high
            CollectionId(0),
            TenantId(0),
        );

        Self {
            config,
            routing,
            store,
            delta_manager,
            compaction_policy: CompactionPolicy::default(),
            next_page_id: 1,
            stats: PageIndexStats {
                base_page_count: 0,
                delta_page_count: 0,
                total_vectors: 0,
                total_disk_bytes: 0,
                ram_bytes: 0,
                mean_vectors_per_page: 0.0,
                mean_reads_per_query: 0.0,
                compaction_count: 0,
            },
        }
    }

    /// Build the index from a batch of vectors (offline).
    pub fn build_from_vectors(
        &mut self,
        vectors: &[(crate::ruvector::types::EmbeddingId, Vec<f32>)],
        collection_id: CollectionId,
        tenant_id: TenantId,
    ) -> BuildResult {
        let mut builder = PageBuilder::new(self.config.clone(), self.next_page_id);
        let result = builder.build(
            vectors,
            collection_id,
            tenant_id,
            &mut self.routing,
            &mut self.store,
        );
        self.next_page_id += result.page_count as u64 + 1;
        self.stats.base_page_count = result.page_count;
        self.stats.total_vectors = result.vector_count;
        self.stats.mean_vectors_per_page = result.mean_vectors_per_page;
        self.stats.ram_bytes = self.routing.ram_bytes();
        result
    }

    /// Insert a single vector (goes to delta pages).
    pub fn insert(
        &mut self,
        id: crate::ruvector::types::EmbeddingId,
        vector: &[f32],
    ) {
        self.delta_manager.insert(id, vector);
        self.stats.total_vectors += 1;

        // Register delta page in routing if sealed
        if let Some(page) = self.delta_manager.current_page() {
            // The current page centroid is used for routing
            // (it gets re-registered on each insert, which is fine for small deltas)
            self.routing.add_page(&page);
            self.store.insert(page);
        }

        // Auto-compact if needed
        if self.delta_manager.should_compact(&self.compaction_policy) {
            self.compact();
        }
    }

    /// Search for the top-k nearest neighbors.
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
    ) -> PageSearchResponse {
        self.search_with_filter(query, k, &SearchFilter::default())
    }

    /// Search with filters.
    pub fn search_with_filter(
        &self,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
    ) -> PageSearchResponse {
        execute_search(
            query,
            k,
            &self.config.default_budget,
            filter,
            &self.routing,
            &self.store,
        )
    }

    /// Search with a custom budget.
    pub fn search_with_budget(
        &self,
        query: &[f32],
        k: usize,
        budget: &SearchBudget,
        filter: &SearchFilter,
    ) -> PageSearchResponse {
        execute_search(query, k, budget, filter, &self.routing, &self.store)
    }

    /// Trigger compaction of delta pages into base pages.
    pub fn compact(&mut self) {
        self.delta_manager.seal_current();
        let deltas = self.delta_manager.drain_sealed();

        if deltas.is_empty() {
            return;
        }

        let mut next_id = self.next_page_id;
        let result = compact_deltas(
            &deltas,
            &[],
            &self.config,
            CollectionId(0),
            TenantId(0),
            &mut next_id,
        );
        self.next_page_id = next_id;

        // Register new base pages
        for page in result.new_pages {
            self.routing.add_page(&page);
            self.store.insert(page);
            self.stats.base_page_count += 1;
        }

        self.stats.delta_page_count = 0;
        self.stats.compaction_count += 1;
        self.stats.ram_bytes = self.routing.ram_bytes();
    }

    /// Get current index statistics.
    pub fn stats(&self) -> &PageIndexStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruvector::types::EmbeddingId;

    #[test]
    fn test_page_aligned_index_build_and_search() {
        let config = PageIndexConfig {
            dimension: 32,
            max_vectors_per_page: 20,
            ..Default::default()
        };
        let mut index = PageAlignedIndex::new(config);

        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..100)
            .map(|i| {
                (
                    EmbeddingId(i),
                    (0..32).map(|_| rand::random::<f32>()).collect(),
                )
            })
            .collect();

        let build_result = index.build_from_vectors(&vectors, CollectionId(1), TenantId(1));
        assert_eq!(build_result.vector_count, 100);
        assert!(build_result.page_count >= 5);

        // Search
        let query: Vec<f32> = (0..32).map(|_| rand::random::<f32>()).collect();
        let response = index.search(&query, 10);
        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 10);

        // Trace populated
        assert!(!response.trace.pages_fetched.is_empty());
    }

    #[test]
    fn test_page_aligned_index_insert_and_search() {
        let config = PageIndexConfig {
            dimension: 16,
            max_vectors_per_page: 5,
            ..Default::default()
        };
        let mut index = PageAlignedIndex::new(config);

        // Insert vectors one by one
        for i in 0..20 {
            let v: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
            index.insert(EmbeddingId(i), &v);
        }

        let query: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
        let response = index.search(&query, 5);
        assert!(!response.results.is_empty());
    }

    #[test]
    fn test_page_aligned_index_compaction() {
        let config = PageIndexConfig {
            dimension: 8,
            max_vectors_per_page: 5,
            compaction_threshold: 3,
            ..Default::default()
        };
        let mut index = PageAlignedIndex::new(config);
        index.compaction_policy.delta_count_trigger = 3;

        for i in 0..20 {
            let v: Vec<f32> = (0..8).map(|_| rand::random::<f32>()).collect();
            index.insert(EmbeddingId(i), &v);
        }

        // Compaction should have triggered
        assert!(index.stats().compaction_count > 0);
    }

    #[test]
    fn test_reads_per_query_bounded() {
        let dim = 32;
        let config = PageIndexConfig {
            dimension: dim,
            max_vectors_per_page: 20,
            ..Default::default()
        };
        let mut index = PageAlignedIndex::new(config);

        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..200)
            .map(|i| {
                (
                    EmbeddingId(i),
                    (0..dim).map(|_| rand::random::<f32>()).collect(),
                )
            })
            .collect();

        index.build_from_vectors(&vectors, CollectionId(1), TenantId(1));

        let budget = SearchBudget {
            max_candidate_pages: 20,
            max_disk_reads: 4,
            max_duration: None,
        };

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let response = index.search_with_budget(&query, 10, &budget, &SearchFilter::default());

        // Disk reads must be bounded
        assert!(response.trace.pages_fetched.len() <= 4);
    }
}
