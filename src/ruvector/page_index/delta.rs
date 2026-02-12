//! Delta page manager and compaction.
//!
//! New inserts and updates land in delta pages (write buffers) per collection
//! and tenant. Delta pages have their own centroids and participate in routing.
//! Periodic compaction merges delta pages into base pages, reclustering
//! boundaries as needed.

use super::storage::{encode_vector, fit_quant_params};
use super::types::*;
use crate::ruvector::types::EmbeddingId;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Delta page manager
// ============================================================================

/// Manages delta (write-buffer) pages for a collection.
///
/// Inserts accumulate in a current delta page. When the page is full
/// (vector count hits max), it is sealed and a new delta page starts.
/// Sealed deltas are searchable and will be merged during compaction.
pub struct DeltaPageManager {
    /// Configuration.
    config: PageIndexConfig,
    /// Current open delta page (accepting inserts).
    current_delta: Option<DeltaPageBuilder>,
    /// Sealed delta pages awaiting compaction.
    sealed_deltas: Vec<PageNode>,
    /// Next page id to assign.
    next_page_id: u64,
    /// Collection these deltas belong to.
    collection_id: CollectionId,
    /// Tenant these deltas belong to.
    tenant_id: TenantId,
}

impl DeltaPageManager {
    pub fn new(
        config: PageIndexConfig,
        start_page_id: u64,
        collection_id: CollectionId,
        tenant_id: TenantId,
    ) -> Self {
        Self {
            config,
            current_delta: None,
            sealed_deltas: Vec::new(),
            next_page_id: start_page_id,
            collection_id,
            tenant_id,
        }
    }

    /// Insert a vector into the current delta page.
    ///
    /// If the current page is full, it is sealed and a new one is created.
    pub fn insert(&mut self, id: EmbeddingId, vector: &[f32]) {
        if self.current_delta.is_none() {
            self.current_delta = Some(self.new_delta_builder());
        }

        let builder = self.current_delta.as_mut().unwrap();

        if builder.is_full() {
            // Seal current and start new
            let sealed = builder.build();
            self.sealed_deltas.push(sealed);
            self.current_delta = Some(self.new_delta_builder());
        }

        let builder = self.current_delta.as_mut().unwrap();
        builder.add_vector(id, vector);
    }

    /// Seal the current delta page (if any) so it becomes searchable.
    pub fn seal_current(&mut self) {
        if let Some(builder) = self.current_delta.take() {
            if builder.vector_count > 0 {
                self.sealed_deltas.push(builder.build());
            }
        }
    }

    /// Get all sealed delta pages.
    pub fn sealed_pages(&self) -> &[PageNode] {
        &self.sealed_deltas
    }

    /// Get the current (unsealed) delta page if it has vectors.
    pub fn current_page(&self) -> Option<PageNode> {
        self.current_delta
            .as_ref()
            .filter(|b| b.vector_count > 0)
            .map(|b| b.build_snapshot())
    }

    /// Number of sealed delta pages.
    pub fn sealed_count(&self) -> usize {
        self.sealed_deltas.len()
    }

    /// Total vectors across all delta pages (sealed + current).
    pub fn total_vectors(&self) -> usize {
        let sealed: usize = self.sealed_deltas.iter().map(|p| p.vector_count()).sum();
        let current = self
            .current_delta
            .as_ref()
            .map(|b| b.vector_count)
            .unwrap_or(0);
        sealed + current
    }

    /// Whether compaction should trigger based on the policy.
    pub fn should_compact(&self, policy: &CompactionPolicy) -> bool {
        if self.sealed_deltas.len() >= policy.delta_count_trigger {
            return true;
        }
        let total_bytes: usize = self.sealed_deltas.iter().map(|p| p.byte_size()).sum();
        if total_bytes >= policy.delta_bytes_trigger {
            return true;
        }
        false
    }

    /// Drain all sealed deltas for compaction. Returns them and clears the list.
    pub fn drain_sealed(&mut self) -> Vec<PageNode> {
        std::mem::take(&mut self.sealed_deltas)
    }

    fn new_delta_builder(&mut self) -> DeltaPageBuilder {
        let page_id = PageId(self.next_page_id);
        self.next_page_id += 1;

        DeltaPageBuilder {
            page_id,
            dimension: self.config.dimension,
            quant_tier: self.config.default_quant_tier,
            max_vectors: self.config.max_vectors_per_page,
            collection_id: self.collection_id,
            tenant_id: self.tenant_id,
            vectors: Vec::new(),
            vector_ids: Vec::new(),
            timestamps: Vec::new(),
            vector_count: 0,
        }
    }
}

// ============================================================================
// Delta page builder
// ============================================================================

/// Accumulates vectors for a single delta page.
struct DeltaPageBuilder {
    page_id: PageId,
    dimension: usize,
    quant_tier: QuantTier,
    max_vectors: usize,
    collection_id: CollectionId,
    tenant_id: TenantId,
    vectors: Vec<Vec<f32>>,
    vector_ids: Vec<EmbeddingId>,
    timestamps: Vec<u64>,
    vector_count: usize,
}

impl DeltaPageBuilder {
    fn is_full(&self) -> bool {
        self.vector_count >= self.max_vectors
    }

    fn add_vector(&mut self, id: EmbeddingId, vector: &[f32]) {
        self.vectors.push(vector.to_vec());
        self.vector_ids.push(id);
        self.timestamps.push(now_unix());
        self.vector_count += 1;
    }

    /// Build the final delta PageNode.
    fn build(&self) -> PageNode {
        self.build_internal()
    }

    /// Build a snapshot without consuming (for peeking at current).
    fn build_snapshot(&self) -> PageNode {
        self.build_internal()
    }

    fn build_internal(&self) -> PageNode {
        // Compute centroid as mean of all vectors
        let centroid = compute_centroid(&self.vectors, self.dimension);

        // Fit quantization params
        let params = fit_quant_params(&self.vectors);

        // Encode vectors
        let mut encoded = Vec::new();
        for v in &self.vectors {
            encoded.extend_from_slice(&encode_vector(v, self.quant_tier, &params));
        }

        PageNode {
            header: PageHeader {
                page_id: self.page_id,
                version: PageVersion(1),
                checksum: 0,
                vector_count: self.vector_count as u32,
                dimension: self.dimension as u16,
                quant_tier: self.quant_tier,
                quant_params: params,
                is_delta: true,
                collection_id: self.collection_id,
                tenant_id: self.tenant_id,
                created_at: now_unix(),
                modified_at: now_unix(),
            },
            centroid,
            sub_centroids: vec![],
            neighbor_ids: vec![],
            neighbor_weights: vec![],
            encoded_vectors: encoded,
            vector_ids: self.vector_ids.clone(),
            residuals: None,
            timestamps: self.timestamps.clone(),
            vector_tenant_ids: vec![self.tenant_id; self.vector_count],
            bloom_filter: vec![],
        }
    }
}

// ============================================================================
// Compaction
// ============================================================================

/// Result of a compaction pass.
#[derive(Debug)]
pub struct CompactionResult {
    /// Newly formed base pages.
    pub new_pages: Vec<PageNode>,
    /// Page ids that were removed (old deltas merged).
    pub removed_page_ids: Vec<PageId>,
    /// Number of vectors processed.
    pub vectors_processed: usize,
    /// Whether any reclustering was performed.
    pub reclustered: bool,
}

/// Compact a set of delta pages and optionally merge with existing base pages.
///
/// This is the core compaction routine:
/// 1. Decode all vectors from delta pages.
/// 2. Optionally merge with vectors from affected base pages.
/// 3. Recluster into new pages.
/// 4. Produce new base pages and a list of removed page ids.
pub fn compact_deltas(
    deltas: &[PageNode],
    base_pages: &[PageNode],
    config: &PageIndexConfig,
    collection_id: CollectionId,
    tenant_id: TenantId,
    next_page_id: &mut u64,
) -> CompactionResult {
    let dim = config.dimension;
    let tier = config.default_quant_tier;
    let max_per_page = config.max_vectors_per_page;

    // Step 1: Extract all vectors
    let mut all_vectors: Vec<(EmbeddingId, Vec<f32>, u64)> = Vec::new();

    for page in deltas.iter().chain(base_pages.iter()) {
        let vec_size = super::storage::encoded_vector_size(dim, page.header.quant_tier);
        for (i, &vid) in page.vector_ids.iter().enumerate() {
            let start = i * vec_size;
            let end = start + vec_size;
            if end <= page.encoded_vectors.len() {
                let decoded = super::storage::decode_vector(
                    &page.encoded_vectors[start..end],
                    dim,
                    page.header.quant_tier,
                    &page.header.quant_params,
                );
                let ts = page.timestamps.get(i).copied().unwrap_or(0);
                all_vectors.push((vid, decoded, ts));
            }
        }
    }

    let vectors_processed = all_vectors.len();

    // Step 2: Cluster into pages (simple sequential packing for now;
    // a production implementation would use k-means or locality-sensitive packing)
    let mut new_pages = Vec::new();
    let chunks: Vec<_> = all_vectors.chunks(max_per_page.max(1)).collect();

    for chunk in chunks {
        let page_id = PageId(*next_page_id);
        *next_page_id += 1;

        let raw_vectors: Vec<Vec<f32>> = chunk.iter().map(|(_, v, _)| v.clone()).collect();
        let centroid = compute_centroid(&raw_vectors, dim);
        let params = fit_quant_params(&raw_vectors);

        let mut encoded = Vec::new();
        for (_, v, _) in chunk {
            encoded.extend_from_slice(&encode_vector(v, tier, &params));
        }

        let vector_ids: Vec<EmbeddingId> = chunk.iter().map(|(id, _, _)| *id).collect();
        let timestamps: Vec<u64> = chunk.iter().map(|(_, _, ts)| *ts).collect();
        let count = vector_ids.len();

        new_pages.push(PageNode {
            header: PageHeader {
                page_id,
                version: PageVersion(1),
                checksum: 0,
                vector_count: count as u32,
                dimension: dim as u16,
                quant_tier: tier,
                quant_params: params,
                is_delta: false,
                collection_id,
                tenant_id,
                created_at: now_unix(),
                modified_at: now_unix(),
            },
            centroid,
            sub_centroids: vec![],
            neighbor_ids: vec![],
            neighbor_weights: vec![],
            encoded_vectors: encoded,
            vector_ids,
            residuals: None,
            timestamps,
            vector_tenant_ids: vec![tenant_id; count],
            bloom_filter: vec![],
        });
    }

    // Step 3: Build neighbor edges between new pages (based on centroid similarity)
    if new_pages.len() > 1 {
        // Clone centroids and page ids so we can mutate pages afterward
        let centroids: Vec<Vec<f32>> = new_pages.iter().map(|p| p.centroid.clone()).collect();
        let page_ids: Vec<PageId> = new_pages.iter().map(|p| p.header.page_id).collect();

        let mut assignments: Vec<(Vec<PageId>, Vec<f32>)> = Vec::with_capacity(new_pages.len());
        for i in 0..new_pages.len() {
            let mut dists: Vec<(usize, f32)> = (0..new_pages.len())
                .filter(|&j| j != i)
                .map(|j| (j, super::routing::l2_sq(&centroids[i], &centroids[j])))
                .collect();
            dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let k = config.page_graph_neighbors.min(dists.len());
            let nids: Vec<PageId> = dists[..k].iter().map(|(j, _)| page_ids[*j]).collect();
            let nweights: Vec<f32> = dists[..k].iter().map(|(_, d)| *d).collect();
            assignments.push((nids, nweights));
        }

        for (i, (nids, nweights)) in assignments.into_iter().enumerate() {
            new_pages[i].neighbor_ids = nids;
            new_pages[i].neighbor_weights = nweights;
        }
    }

    let removed_ids: Vec<PageId> = deltas
        .iter()
        .chain(base_pages.iter())
        .map(|p| p.header.page_id)
        .collect();

    CompactionResult {
        new_pages,
        removed_page_ids: removed_ids,
        vectors_processed,
        reclustered: !base_pages.is_empty(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Compute centroid (mean) of a set of vectors.
pub fn compute_centroid(vectors: &[Vec<f32>], dimension: usize) -> Vec<f32> {
    if vectors.is_empty() {
        return vec![0.0; dimension];
    }
    let mut centroid = vec![0.0f64; dimension];
    for v in vectors {
        for (i, &val) in v.iter().enumerate().take(dimension) {
            centroid[i] += val as f64;
        }
    }
    let n = vectors.len() as f64;
    centroid.iter().map(|&v| (v / n) as f32).collect()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_manager_insert_and_seal() {
        let config = PageIndexConfig {
            dimension: 32,
            max_vectors_per_page: 10,
            ..Default::default()
        };
        let mut mgr = DeltaPageManager::new(config, 1000, CollectionId(1), TenantId(1));

        // Insert 25 vectors → should create 2 sealed pages + 1 with 5 vectors
        for i in 0..25 {
            let v: Vec<f32> = (0..32).map(|j| (i * 32 + j) as f32).collect();
            mgr.insert(EmbeddingId(i as u64), &v);
        }

        // 2 sealed (after filling 10 each), 5 in current
        assert_eq!(mgr.sealed_count(), 2);
        assert_eq!(mgr.total_vectors(), 25);

        mgr.seal_current();
        assert_eq!(mgr.sealed_count(), 3);
    }

    #[test]
    fn test_delta_manager_should_compact() {
        let config = PageIndexConfig {
            dimension: 16,
            max_vectors_per_page: 5,
            ..Default::default()
        };
        let mut mgr = DeltaPageManager::new(config, 2000, CollectionId(1), TenantId(1));
        let policy = CompactionPolicy {
            delta_count_trigger: 3,
            ..Default::default()
        };

        // Insert enough to create 3+ sealed pages
        for i in 0..20 {
            let v: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
            mgr.insert(EmbeddingId(i), &v);
        }

        assert!(mgr.should_compact(&policy));
    }

    #[test]
    fn test_compact_deltas_basic() {
        let config = PageIndexConfig {
            dimension: 16,
            max_vectors_per_page: 10,
            ..Default::default()
        };
        let mut mgr = DeltaPageManager::new(config.clone(), 3000, CollectionId(1), TenantId(1));

        for i in 0..15 {
            let v: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
            mgr.insert(EmbeddingId(i), &v);
        }
        mgr.seal_current();

        let deltas = mgr.drain_sealed();
        assert!(!deltas.is_empty());

        let mut next_id = 5000;
        let result = compact_deltas(
            &deltas,
            &[],
            &config,
            CollectionId(1),
            TenantId(1),
            &mut next_id,
        );

        assert_eq!(result.vectors_processed, 15);
        assert!(!result.new_pages.is_empty());
        // All new pages should be base pages (not delta)
        for page in &result.new_pages {
            assert!(!page.is_delta());
        }
    }

    #[test]
    fn test_compact_with_base_pages() {
        let config = PageIndexConfig {
            dimension: 8,
            max_vectors_per_page: 5,
            ..Default::default()
        };

        // Create a base page
        let base_vectors: Vec<Vec<f32>> = (0..5)
            .map(|_| (0..8).map(|_| rand::random::<f32>()).collect())
            .collect();
        let params = fit_quant_params(&base_vectors);
        let mut encoded = Vec::new();
        for v in &base_vectors {
            encoded.extend_from_slice(&encode_vector(v, QuantTier::Hot, &params));
        }
        let base_page = PageNode {
            header: PageHeader {
                page_id: PageId(100),
                version: PageVersion(1),
                checksum: 0,
                vector_count: 5,
                dimension: 8,
                quant_tier: QuantTier::Hot,
                quant_params: params,
                is_delta: false,
                collection_id: CollectionId(1),
                tenant_id: TenantId(1),
                created_at: 1000,
                modified_at: 1000,
            },
            centroid: compute_centroid(&base_vectors, 8),
            sub_centroids: vec![],
            neighbor_ids: vec![],
            neighbor_weights: vec![],
            encoded_vectors: encoded,
            vector_ids: (0..5).map(|i| EmbeddingId(i)).collect(),
            residuals: None,
            timestamps: vec![1000; 5],
            vector_tenant_ids: vec![TenantId(1); 5],
            bloom_filter: vec![],
        };

        // Create delta pages
        let mut mgr = DeltaPageManager::new(config.clone(), 200, CollectionId(1), TenantId(1));
        for i in 10..18 {
            let v: Vec<f32> = (0..8).map(|_| rand::random::<f32>()).collect();
            mgr.insert(EmbeddingId(i), &v);
        }
        mgr.seal_current();
        let deltas = mgr.drain_sealed();

        let mut next_id = 300;
        let result = compact_deltas(
            &deltas,
            &[base_page],
            &config,
            CollectionId(1),
            TenantId(1),
            &mut next_id,
        );

        // Should have merged 5 base + 8 delta = 13 vectors
        assert_eq!(result.vectors_processed, 13);
        assert!(result.reclustered);
    }

    #[test]
    fn test_compute_centroid() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0],
            vec![3.0, 4.0, 5.0],
        ];
        let centroid = compute_centroid(&vectors, 3);
        assert!((centroid[0] - 2.0).abs() < 1e-5);
        assert!((centroid[1] - 3.0).abs() < 1e-5);
        assert!((centroid[2] - 4.0).abs() < 1e-5);
    }
}
