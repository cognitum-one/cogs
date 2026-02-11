//! Query execution pipeline: disk fetch, decode, score, early exit, trace.
//!
//! Given a query vector and a set of candidate pages from the routing index,
//! this module fetches pages, decodes vectors, scores them against the query,
//! applies early exit, and returns top-k results with a full trace.

use super::routing::{l2_sq, RoutingIndex};
use super::storage::{decode_vector, encoded_vector_size, PageStorageError};
use super::types::*;
use crate::ruvector::types::EmbeddingId;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;

// ============================================================================
// Page store trait (abstracts disk / in-memory storage)
// ============================================================================

/// Trait for fetching pages. Implementations can wrap SSD I/O, mmap, or
/// in-memory buffers.
pub trait PageStore: Send + Sync {
    /// Fetch a page by id. Returns the deserialized PageNode.
    fn fetch(&self, page_id: PageId) -> Result<PageNode, PageStorageError>;
}

/// Simple in-memory page store for testing and small datasets.
#[derive(Debug, Clone)]
pub struct InMemoryPageStore {
    pages: HashMap<PageId, PageNode>,
}

impl InMemoryPageStore {
    pub fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }

    pub fn insert(&mut self, page: PageNode) {
        self.pages.insert(page.header.page_id, page);
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

impl Default for InMemoryPageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PageStore for InMemoryPageStore {
    fn fetch(&self, page_id: PageId) -> Result<PageNode, PageStorageError> {
        self.pages
            .get(&page_id)
            .cloned()
            .ok_or(PageStorageError::PageNotFound(page_id))
    }
}

// ============================================================================
// SIMD-friendly distance scoring
// ============================================================================

/// Score a decoded vector against the query (L2 squared distance).
#[inline]
fn score_vector(query: &[f32], vector: &[f32]) -> f32 {
    l2_sq(query, vector)
}

/// Score all vectors in a page against the query.
///
/// Returns vec of (vector_id, distance).
fn score_page_vectors(
    query: &[f32],
    page: &PageNode,
) -> Vec<(EmbeddingId, f32)> {
    let dim = page.header.dimension as usize;
    let tier = page.header.quant_tier;
    let params = &page.header.quant_params;
    let vec_size = encoded_vector_size(dim, tier);

    let mut results = Vec::with_capacity(page.vector_ids.len());

    for (i, &vid) in page.vector_ids.iter().enumerate() {
        let start = i * vec_size;
        let end = start + vec_size;

        if end > page.encoded_vectors.len() {
            break;
        }

        let decoded = decode_vector(&page.encoded_vectors[start..end], dim, tier, params);
        let dist = score_vector(query, &decoded);
        results.push((vid, dist));
    }

    results
}

// ============================================================================
// Top-K collector
// ============================================================================

/// Entry in the max-heap for top-k collection (we keep the worst at the top
/// so we can quickly decide whether a new candidate beats it).
#[derive(Debug, Clone)]
struct TopKEntry {
    dist: f32,
    id: EmbeddingId,
    source_page: PageId,
}

impl PartialEq for TopKEntry {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}
impl Eq for TopKEntry {}
impl PartialOrd for TopKEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Max-heap: largest distance on top
        self.dist.partial_cmp(&other.dist)
    }
}
impl Ord for TopKEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

struct TopKCollector {
    heap: BinaryHeap<TopKEntry>,
    k: usize,
}

impl TopKCollector {
    fn new(k: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(k + 1),
            k,
        }
    }

    /// Try to insert a candidate. Returns true if it was accepted.
    fn push(&mut self, id: EmbeddingId, dist: f32, source_page: PageId) -> bool {
        if self.heap.len() < self.k {
            self.heap.push(TopKEntry { dist, id, source_page });
            return true;
        }

        if let Some(worst) = self.heap.peek() {
            if dist < worst.dist {
                self.heap.pop();
                self.heap.push(TopKEntry { dist, id, source_page });
                return true;
            }
        }
        false
    }

    /// Current worst distance in the top-k set.
    fn worst_distance(&self) -> f32 {
        self.heap.peek().map(|e| e.dist).unwrap_or(f32::INFINITY)
    }

    /// Drain into sorted results.
    fn into_sorted(self) -> Vec<PageSearchResult> {
        let mut results: Vec<PageSearchResult> = self
            .heap
            .into_iter()
            .map(|e| PageSearchResult {
                id: e.id,
                distance: e.dist,
                source_page: e.source_page,
            })
            .collect();
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        results
    }
}

// ============================================================================
// Search executor
// ============================================================================

/// Execute a page-aligned ANN search.
///
/// Algorithm:
/// 1. Route in RAM: centroid HNSW + page graph expansion -> candidate pages.
/// 2. Disk phase: fetch pages in priority order, decode, score.
/// 3. Early exit: stop when top-k stable and budget used.
/// 4. Return results + trace.
pub fn execute_search(
    query: &[f32],
    k: usize,
    budget: &SearchBudget,
    filter: &SearchFilter,
    routing: &RoutingIndex,
    store: &dyn PageStore,
) -> PageSearchResponse {
    let total_start = Instant::now();
    let mut trace = QueryTrace::new();

    // Phase 1: Route in RAM
    let routing_start = Instant::now();
    let candidates = routing.select_candidates(query, budget, filter);
    trace.routing_duration_us = routing_start.elapsed().as_micros() as u64;
    trace.pages_considered = candidates.iter().map(|(pid, _)| *pid).collect();

    // Phase 2: Fetch and score
    let disk_start = Instant::now();
    let mut collector = TopKCollector::new(k);
    let mut pages_fetched = 0;
    let mut stable_count = 0;

    for (page_id, _routing_dist) in &candidates {
        if pages_fetched >= budget.max_disk_reads {
            trace.budget_exhausted = true;
            break;
        }

        // Check time budget
        if let Some(max_dur) = budget.max_duration {
            if total_start.elapsed() > max_dur {
                trace.budget_exhausted = true;
                break;
            }
        }

        // Fetch page
        let page = match store.fetch(*page_id) {
            Ok(p) => p,
            Err(_) => continue,
        };
        pages_fetched += 1;
        trace.pages_fetched.push(*page_id);

        // Apply timestamp filters at vector level
        let scores = score_page_vectors(query, &page);
        trace.vectors_decoded += scores.len();
        trace.distance_computations += scores.len();

        let prev_worst = collector.worst_distance();
        let mut any_inserted = false;

        for (vid, dist) in scores {
            // Apply vector-level filters
            if let Some(min_ts) = filter.min_timestamp {
                if let Some(idx) = page.vector_ids.iter().position(|&id| id == vid) {
                    if page.timestamps.get(idx).copied().unwrap_or(0) < min_ts {
                        continue;
                    }
                }
            }
            if let Some(max_ts) = filter.max_timestamp {
                if let Some(idx) = page.vector_ids.iter().position(|&id| id == vid) {
                    if page.timestamps.get(idx).copied().unwrap_or(u64::MAX) > max_ts {
                        continue;
                    }
                }
            }

            if collector.push(vid, dist, *page_id) {
                any_inserted = true;
            }
        }

        // Track score progression
        let current_worst = collector.worst_distance();
        trace.score_progression.push((current_worst, *page_id));

        // Early exit: if top-k didn't change, increment stable counter
        if !any_inserted || (current_worst - prev_worst).abs() < 1e-6 {
            stable_count += 1;
        } else {
            stable_count = 0;
        }

        // If top-k is stable for 3 consecutive pages, exit early
        if stable_count >= 3 && collector.heap.len() >= k {
            break;
        }
    }

    trace.disk_phase_duration_us = disk_start.elapsed().as_micros() as u64;
    trace.total_duration_us = total_start.elapsed().as_micros() as u64;

    PageSearchResponse {
        results: collector.into_sorted(),
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::storage::encode_vector;

    fn make_test_page_with_vectors(
        page_id: u64,
        dim: usize,
        num_vectors: usize,
        centroid: Vec<f32>,
    ) -> PageNode {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let params = QuantScaleParams {
            min_val: -1.0,
            max_val: 1.0,
            scale: 2.0 / 255.0,
        };

        let mut vector_ids = Vec::with_capacity(num_vectors);
        let mut encoded = Vec::new();
        let mut timestamps = Vec::with_capacity(num_vectors);

        for i in 0..num_vectors {
            let v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            encoded.extend_from_slice(&encode_vector(&v, QuantTier::Hot, &params));
            vector_ids.push(EmbeddingId(page_id * 1000 + i as u64));
            timestamps.push(1000 + i as u64);
        }

        PageNode {
            header: PageHeader {
                page_id: PageId(page_id),
                version: PageVersion(1),
                checksum: 0,
                vector_count: num_vectors as u32,
                dimension: dim as u16,
                quant_tier: QuantTier::Hot,
                quant_params: params,
                is_delta: false,
                collection_id: CollectionId(1),
                tenant_id: TenantId(1),
                created_at: 1000,
                modified_at: 1000,
            },
            centroid,
            sub_centroids: vec![],
            neighbor_ids: vec![],
            neighbor_weights: vec![],
            encoded_vectors: encoded,
            vector_ids,
            residuals: None,
            timestamps,
            vector_tenant_ids: vec![TenantId(1); num_vectors],
            bloom_filter: vec![],
        }
    }

    #[test]
    fn test_score_page_vectors() {
        let dim = 32;
        let centroid: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let page = make_test_page_with_vectors(1, dim, 20, centroid);

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let scores = score_page_vectors(&query, &page);

        assert_eq!(scores.len(), 20);
        for (_, dist) in &scores {
            assert!(dist.is_finite());
            assert!(*dist >= 0.0);
        }
    }

    #[test]
    fn test_topk_collector() {
        let mut collector = TopKCollector::new(3);

        collector.push(EmbeddingId(1), 5.0, PageId(0));
        collector.push(EmbeddingId(2), 3.0, PageId(0));
        collector.push(EmbeddingId(3), 7.0, PageId(0));
        collector.push(EmbeddingId(4), 1.0, PageId(0));

        let results = collector.into_sorted();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, EmbeddingId(4)); // dist 1.0
        assert_eq!(results[1].id, EmbeddingId(2)); // dist 3.0
        assert_eq!(results[2].id, EmbeddingId(1)); // dist 5.0
    }

    #[test]
    fn test_execute_search_basic() {
        let dim = 32;
        let config = PageIndexConfig {
            dimension: dim,
            ..Default::default()
        };
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        // Build 10 pages, each with 20 vectors
        for i in 0..10 {
            let centroid: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
            let page = make_test_page_with_vectors(i, dim, 20, centroid.clone());
            routing.add_page(&page);
            store.insert(page);
        }

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget {
            max_candidate_pages: 10,
            max_disk_reads: 5,
            max_duration: None,
        };
        let filter = SearchFilter::default();

        let response = execute_search(&query, 10, &budget, &filter, &routing, &store);

        assert!(response.results.len() <= 10);
        assert!(!response.results.is_empty());
        // Results sorted by distance
        for i in 1..response.results.len() {
            assert!(response.results[i - 1].distance <= response.results[i].distance);
        }

        // Trace must be populated
        assert!(!response.trace.pages_fetched.is_empty());
        assert!(response.trace.pages_fetched.len() <= 5);
        assert!(response.trace.vectors_decoded > 0);
        assert!(response.trace.total_duration_us > 0);
    }

    #[test]
    fn test_max_pages_respected() {
        let dim = 16;
        let config = PageIndexConfig {
            dimension: dim,
            ..Default::default()
        };
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        for i in 0..20 {
            let centroid: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
            let page = make_test_page_with_vectors(i, dim, 10, centroid.clone());
            routing.add_page(&page);
            store.insert(page);
        }

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget {
            max_candidate_pages: 20,
            max_disk_reads: 3,
            max_duration: None,
        };

        let response = execute_search(&query, 5, &budget, &SearchFilter::default(), &routing, &store);

        // Must not exceed disk read budget
        assert!(response.trace.pages_fetched.len() <= 3);
    }

    #[test]
    fn test_trace_emitted() {
        let dim = 16;
        let config = PageIndexConfig {
            dimension: dim,
            ..Default::default()
        };
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        for i in 0..5 {
            let centroid: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
            let page = make_test_page_with_vectors(i, dim, 5, centroid.clone());
            routing.add_page(&page);
            store.insert(page);
        }

        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget::default();

        let response = execute_search(&query, 3, &budget, &SearchFilter::default(), &routing, &store);

        assert!(!response.trace.pages_considered.is_empty());
        assert!(!response.trace.pages_fetched.is_empty());
        assert!(response.trace.vectors_decoded > 0);
        assert!(response.trace.distance_computations > 0);
        assert!(response.trace.routing_duration_us > 0 || response.trace.total_duration_us > 0);
    }
}
