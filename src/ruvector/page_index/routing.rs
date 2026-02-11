//! In-memory routing index: HNSW over page centroids + page adjacency graph.
//!
//! This layer sits entirely in RAM. Its job is to narrow the search from
//! "all pages" down to a small candidate set that will be fetched from disk.

use super::types::*;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

// ============================================================================
// Centroid entry
// ============================================================================

/// Entry in the centroid HNSW: one per page.
#[derive(Debug, Clone)]
pub struct CentroidEntry {
    pub page_id: PageId,
    pub centroid: Vec<f32>,
    pub tier: QuantTier,
    pub vector_count: u32,
    pub tenant_id: TenantId,
    pub collection_id: CollectionId,
}

// ============================================================================
// Page graph (adjacency list, compressed)
// ============================================================================

/// Compressed page adjacency graph.
#[derive(Debug, Clone)]
pub struct PageGraph {
    /// Adjacency list: page_id -> sorted vec of (neighbor_id, weight).
    pub adjacency: HashMap<PageId, Vec<(PageId, f32)>>,
}

impl PageGraph {
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
        }
    }

    /// Add a directed edge.
    pub fn add_edge(&mut self, from: PageId, to: PageId, weight: f32) {
        self.adjacency
            .entry(from)
            .or_insert_with(Vec::new)
            .push((to, weight));
    }

    /// Add bidirectional edge.
    pub fn add_bidi_edge(&mut self, a: PageId, b: PageId, weight: f32) {
        self.add_edge(a, b, weight);
        self.add_edge(b, a, weight);
    }

    /// Get neighbors of a page.
    pub fn neighbors(&self, page_id: PageId) -> &[(PageId, f32)] {
        self.adjacency
            .get(&page_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Remove a page and all edges referencing it.
    pub fn remove_page(&mut self, page_id: PageId) {
        self.adjacency.remove(&page_id);
        for edges in self.adjacency.values_mut() {
            edges.retain(|(nid, _)| *nid != page_id);
        }
    }

    /// Number of pages in the graph.
    pub fn page_count(&self) -> usize {
        self.adjacency.len()
    }
}

impl Default for PageGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HNSW over page centroids (simplified, in-memory)
// ============================================================================

/// HNSW index over page centroids for fast routing.
///
/// This is intentionally a compact implementation: the number of pages is
/// orders of magnitude smaller than the number of vectors, so we can afford
/// a simpler structure.
pub struct CentroidHnsw {
    /// All centroid entries.
    entries: Vec<CentroidEntry>,
    /// Map from PageId to index in entries.
    page_to_idx: HashMap<PageId, usize>,
    /// HNSW layers: layer -> adjacency list (idx -> sorted neighbors by distance).
    layers: Vec<HashMap<usize, Vec<(usize, f32)>>>,
    /// Entry point index.
    entry_point: Option<usize>,
    /// Parameters.
    m: usize,
    ef_construction: usize,
    ef_search: usize,
    /// Maximum layer for each entry.
    max_layers: Vec<usize>,
}

impl CentroidHnsw {
    pub fn new(m: usize, ef_construction: usize, ef_search: usize) -> Self {
        Self {
            entries: Vec::new(),
            page_to_idx: HashMap::new(),
            layers: vec![HashMap::new()],
            entry_point: None,
            m,
            ef_construction,
            ef_search,
            max_layers: Vec::new(),
        }
    }

    /// Insert a centroid entry.
    pub fn insert(&mut self, entry: CentroidEntry) {
        let idx = self.entries.len();
        self.page_to_idx.insert(entry.page_id, idx);
        self.entries.push(entry);

        // Determine layer for this entry
        let level = self.random_level();
        self.max_layers.push(level);

        // Ensure we have enough layers
        while self.layers.len() <= level {
            self.layers.push(HashMap::new());
        }

        if self.entry_point.is_none() {
            self.entry_point = Some(idx);
            for l in 0..=level {
                self.layers[l].insert(idx, Vec::new());
            }
            return;
        }

        let ep = self.entry_point.unwrap();

        // Navigate from top to insertion layer
        let mut current = ep;
        let top_layer = self.layers.len() - 1;
        for l in (level + 1..=top_layer).rev() {
            current = self.greedy_search_layer(current, idx, l);
        }

        // Insert at each layer from level down to 0
        for l in (0..=level.min(top_layer)).rev() {
            let neighbors = self.search_layer(current, idx, self.ef_construction, l);
            let selected = self.select_neighbors(&neighbors, self.m);

            self.layers[l].insert(idx, selected.clone());
            for &(nidx, dist) in &selected {
                let nbrs = self.layers[l].entry(nidx).or_insert_with(Vec::new);
                nbrs.push((idx, dist));
                // Prune if too many neighbors
                if nbrs.len() > self.m * 2 {
                    nbrs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                    nbrs.truncate(self.m * 2);
                }
            }

            if !neighbors.is_empty() {
                current = neighbors[0].0;
            }
        }

        // Update entry point if this node has a higher layer
        if level > self.max_layers.get(ep).copied().unwrap_or(0) {
            self.entry_point = Some(idx);
        }
    }

    /// Search for the top-M nearest centroid pages to a query vector.
    pub fn search(&self, query: &[f32], m: usize) -> Vec<(PageId, f32)> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let ep = match self.entry_point {
            Some(ep) => ep,
            None => return Vec::new(),
        };

        // Navigate from top layer down to layer 1
        let mut current = ep;
        let top_layer = self.layers.len() - 1;
        for l in (1..=top_layer).rev() {
            current = self.greedy_search_layer_query(current, query, l);
        }

        // Search at layer 0
        let candidates = self.search_layer_query(current, query, self.ef_search.max(m), 0);

        candidates
            .into_iter()
            .take(m)
            .map(|(idx, dist)| (self.entries[idx].page_id, dist))
            .collect()
    }

    /// Get centroid entry by page id.
    pub fn get_entry(&self, page_id: PageId) -> Option<&CentroidEntry> {
        self.page_to_idx.get(&page_id).map(|&idx| &self.entries[idx])
    }

    /// Number of pages in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all page ids.
    pub fn page_ids(&self) -> Vec<PageId> {
        self.entries.iter().map(|e| e.page_id).collect()
    }

    // -- Internal helpers --

    fn random_level(&self) -> usize {
        // Geometric distribution with p = 1/m
        let p = 1.0 / (self.m as f64).ln();
        let r: f64 = rand::random();
        let level = (-r.ln() * p) as usize;
        level.min(16) // Cap at 16 layers
    }

    fn l2_distance(&self, a_idx: usize, b_idx: usize) -> f32 {
        l2_sq(&self.entries[a_idx].centroid, &self.entries[b_idx].centroid)
    }

    fn l2_distance_to_query(&self, idx: usize, query: &[f32]) -> f32 {
        l2_sq(&self.entries[idx].centroid, query)
    }

    fn greedy_search_layer(&self, start: usize, target: usize, layer: usize) -> usize {
        let mut current = start;
        let mut best_dist = self.l2_distance(current, target);

        loop {
            let mut changed = false;
            if let Some(neighbors) = self.layers.get(layer).and_then(|l| l.get(&current)) {
                for &(nidx, _) in neighbors {
                    let dist = self.l2_distance(nidx, target);
                    if dist < best_dist {
                        best_dist = dist;
                        current = nidx;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        current
    }

    fn greedy_search_layer_query(&self, start: usize, query: &[f32], layer: usize) -> usize {
        let mut current = start;
        let mut best_dist = self.l2_distance_to_query(current, query);

        loop {
            let mut changed = false;
            if let Some(neighbors) = self.layers.get(layer).and_then(|l| l.get(&current)) {
                for &(nidx, _) in neighbors {
                    let dist = self.l2_distance_to_query(nidx, query);
                    if dist < best_dist {
                        best_dist = dist;
                        current = nidx;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        current
    }

    fn search_layer(
        &self,
        start: usize,
        target: usize,
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();

        let dist = self.l2_distance(start, target);
        visited.insert(start);
        candidates.push(MinHeapEntry { dist, idx: start });
        results.push(MaxHeapEntry { dist, idx: start });

        while let Some(MinHeapEntry { dist: c_dist, idx: c_idx }) = candidates.pop() {
            let worst_dist = results.peek().map(|e| e.dist).unwrap_or(f32::INFINITY);
            if c_dist > worst_dist && results.len() >= ef {
                break;
            }

            if let Some(neighbors) = self.layers.get(layer).and_then(|l| l.get(&c_idx)) {
                for &(nidx, _) in neighbors {
                    if visited.insert(nidx) {
                        let ndist = self.l2_distance(nidx, target);
                        let worst = results.peek().map(|e| e.dist).unwrap_or(f32::INFINITY);

                        if ndist < worst || results.len() < ef {
                            candidates.push(MinHeapEntry { dist: ndist, idx: nidx });
                            results.push(MaxHeapEntry { dist: ndist, idx: nidx });
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut res: Vec<(usize, f32)> = results.into_iter().map(|e| (e.idx, e.dist)).collect();
        res.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        res
    }

    fn search_layer_query(
        &self,
        start: usize,
        query: &[f32],
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();

        let dist = self.l2_distance_to_query(start, query);
        visited.insert(start);
        candidates.push(MinHeapEntry { dist, idx: start });
        results.push(MaxHeapEntry { dist, idx: start });

        while let Some(MinHeapEntry { dist: c_dist, idx: c_idx }) = candidates.pop() {
            let worst_dist = results.peek().map(|e| e.dist).unwrap_or(f32::INFINITY);
            if c_dist > worst_dist && results.len() >= ef {
                break;
            }

            if let Some(neighbors) = self.layers.get(layer).and_then(|l| l.get(&c_idx)) {
                for &(nidx, _) in neighbors {
                    if visited.insert(nidx) {
                        let ndist = self.l2_distance_to_query(nidx, query);
                        let worst = results.peek().map(|e| e.dist).unwrap_or(f32::INFINITY);

                        if ndist < worst || results.len() < ef {
                            candidates.push(MinHeapEntry { dist: ndist, idx: nidx });
                            results.push(MaxHeapEntry { dist: ndist, idx: nidx });
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut res: Vec<(usize, f32)> = results.into_iter().map(|e| (e.idx, e.dist)).collect();
        res.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        res
    }

    fn select_neighbors(&self, candidates: &[(usize, f32)], m: usize) -> Vec<(usize, f32)> {
        candidates.iter().take(m).cloned().collect()
    }
}

// ============================================================================
// Routing index (combines CentroidHnsw + PageGraph)
// ============================================================================

/// The in-memory routing index. Holds:
/// 1. HNSW over page centroids for fast entry-point selection.
/// 2. Page adjacency graph for neighbor expansion.
/// 3. Hot page cache (LRU by page_id).
/// 4. Optional per-tenant entry points.
pub struct RoutingIndex {
    /// HNSW index on page centroids.
    pub centroid_hnsw: CentroidHnsw,
    /// Page adjacency graph.
    pub page_graph: PageGraph,
    /// Per-tenant entry point page ids.
    pub tenant_entry_points: HashMap<TenantId, Vec<PageId>>,
}

impl RoutingIndex {
    pub fn new(config: &PageIndexConfig) -> Self {
        Self {
            centroid_hnsw: CentroidHnsw::new(
                config.centroid_hnsw_m,
                config.centroid_hnsw_ef_construction,
                config.centroid_hnsw_ef_search,
            ),
            page_graph: PageGraph::new(),
            tenant_entry_points: HashMap::new(),
        }
    }

    /// Register a page in the routing index.
    pub fn add_page(&mut self, page: &PageNode) {
        let entry = CentroidEntry {
            page_id: page.header.page_id,
            centroid: page.centroid.clone(),
            tier: page.header.quant_tier,
            vector_count: page.header.vector_count,
            tenant_id: page.header.tenant_id,
            collection_id: page.header.collection_id,
        };

        self.centroid_hnsw.insert(entry);

        // Add page graph edges from the page's neighbor list
        for (i, &nid) in page.neighbor_ids.iter().enumerate() {
            let weight = page.neighbor_weights.get(i).copied().unwrap_or(1.0);
            self.page_graph.add_bidi_edge(page.header.page_id, nid, weight);
        }
    }

    /// Select candidate pages for a query, respecting the budget.
    ///
    /// Returns (page_id, routing_distance) sorted by distance ascending.
    pub fn select_candidates(
        &self,
        query: &[f32],
        budget: &SearchBudget,
        filter: &SearchFilter,
    ) -> Vec<(PageId, f32)> {
        // Phase 1: HNSW search for top-M candidates
        let initial = self.centroid_hnsw.search(query, budget.max_candidate_pages);

        // Phase 2: Expand via page graph neighbors
        let mut candidate_set: HashMap<PageId, f32> = HashMap::new();
        for (pid, dist) in &initial {
            candidate_set.insert(*pid, *dist);
        }

        // Expand one hop in page graph
        for (pid, _) in &initial {
            for &(neighbor, _weight) in self.page_graph.neighbors(*pid) {
                if !candidate_set.contains_key(&neighbor) {
                    if let Some(entry) = self.centroid_hnsw.get_entry(neighbor) {
                        let dist = l2_sq(&entry.centroid, query);
                        candidate_set.insert(neighbor, dist);
                    }
                }
            }
        }

        // Phase 3: Apply filters
        let mut candidates: Vec<(PageId, f32)> = candidate_set
            .into_iter()
            .filter(|(pid, _)| self.passes_filter(*pid, filter))
            .collect();

        // Sort by distance
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // Respect budget
        candidates.truncate(budget.max_disk_reads);

        candidates
    }

    /// Check whether a page passes the search filter.
    fn passes_filter(&self, page_id: PageId, filter: &SearchFilter) -> bool {
        if let Some(entry) = self.centroid_hnsw.get_entry(page_id) {
            if let Some(tid) = filter.tenant_id {
                if entry.tenant_id != tid {
                    return false;
                }
            }
            if let Some(cid) = filter.collection_id {
                if entry.collection_id != cid {
                    return false;
                }
            }
        }
        true
    }

    /// Estimate RAM usage in bytes.
    pub fn ram_bytes(&self) -> usize {
        let centroid_bytes: usize = self.centroid_hnsw.entries.iter()
            .map(|e| e.centroid.len() * 4 + 64) // centroid + overhead
            .sum();
        let graph_bytes: usize = self.page_graph.adjacency.values()
            .map(|v| v.len() * 12 + 32) // per edge: PageId(8) + f32(4) + overhead
            .sum();
        centroid_bytes + graph_bytes
    }
}

// ============================================================================
// Heap entries for HNSW search
// ============================================================================

#[derive(Debug, Clone)]
struct MinHeapEntry {
    dist: f32,
    idx: usize,
}

impl PartialEq for MinHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}
impl Eq for MinHeapEntry {}

impl PartialOrd for MinHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse for min-heap (BinaryHeap is max-heap by default)
        other.dist.partial_cmp(&self.dist)
    }
}
impl Ord for MinHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Clone)]
struct MaxHeapEntry {
    dist: f32,
    idx: usize,
}

impl PartialEq for MaxHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}
impl Eq for MaxHeapEntry {}

impl PartialOrd for MaxHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.dist.partial_cmp(&other.dist)
    }
}
impl Ord for MaxHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

// ============================================================================
// Distance utility
// ============================================================================

/// Squared L2 distance between two vectors.
#[inline]
pub fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruvector::types::EmbeddingId;

    fn make_centroid_entry(page_id: u64, dim: usize) -> CentroidEntry {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        CentroidEntry {
            page_id: PageId(page_id),
            centroid: (0..dim).map(|_| rng.gen::<f32>()).collect(),
            tier: QuantTier::Hot,
            vector_count: 100,
            tenant_id: TenantId(1),
            collection_id: CollectionId(1),
        }
    }

    #[test]
    fn test_page_graph_add_and_query() {
        let mut graph = PageGraph::new();
        graph.add_bidi_edge(PageId(1), PageId(2), 0.5);
        graph.add_bidi_edge(PageId(1), PageId(3), 0.8);

        let n1 = graph.neighbors(PageId(1));
        assert_eq!(n1.len(), 2);

        let n2 = graph.neighbors(PageId(2));
        assert_eq!(n2.len(), 1);
        assert_eq!(n2[0].0, PageId(1));
    }

    #[test]
    fn test_centroid_hnsw_insert_and_search() {
        let mut hnsw = CentroidHnsw::new(8, 32, 16);

        for i in 0..50 {
            hnsw.insert(make_centroid_entry(i, 64));
        }

        assert_eq!(hnsw.len(), 50);

        let query: Vec<f32> = (0..64).map(|_| rand::random::<f32>()).collect();
        let results = hnsw.search(&query, 5);
        assert_eq!(results.len(), 5);

        // Results should be sorted by distance ascending
        for i in 1..results.len() {
            assert!(results[i - 1].1 <= results[i].1);
        }
    }

    #[test]
    fn test_routing_index_candidate_selection() {
        let config = PageIndexConfig {
            dimension: 32,
            ..Default::default()
        };
        let mut routing = RoutingIndex::new(&config);

        // Create and add 20 pages
        for i in 0..20 {
            let entry = make_centroid_entry(i, 32);
            let page = PageNode {
                header: PageHeader {
                    page_id: PageId(i),
                    version: PageVersion(1),
                    checksum: 0,
                    vector_count: 50,
                    dimension: 32,
                    quant_tier: QuantTier::Hot,
                    quant_params: QuantScaleParams::default(),
                    is_delta: false,
                    collection_id: CollectionId(1),
                    tenant_id: TenantId(1),
                    created_at: 1000,
                    modified_at: 1000,
                },
                centroid: entry.centroid.clone(),
                sub_centroids: vec![],
                neighbor_ids: if i > 0 { vec![PageId(i - 1)] } else { vec![] },
                neighbor_weights: if i > 0 { vec![1.0] } else { vec![] },
                encoded_vectors: vec![],
                vector_ids: vec![],
                residuals: None,
                timestamps: vec![],
                vector_tenant_ids: vec![],
                bloom_filter: vec![],
            };
            routing.add_page(&page);
        }

        let query: Vec<f32> = (0..32).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget {
            max_candidate_pages: 10,
            max_disk_reads: 5,
            ..Default::default()
        };
        let filter = SearchFilter::default();

        let candidates = routing.select_candidates(&query, &budget, &filter);
        assert!(candidates.len() <= 5);
        assert!(!candidates.is_empty());
    }

    #[test]
    fn test_l2_sq() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((l2_sq(&a, &b) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_tenant_filtering() {
        let config = PageIndexConfig {
            dimension: 16,
            ..Default::default()
        };
        let mut routing = RoutingIndex::new(&config);

        for i in 0..10 {
            let tenant = TenantId(if i < 5 { 1 } else { 2 });
            let centroid: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
            let page = PageNode {
                header: PageHeader {
                    page_id: PageId(i),
                    version: PageVersion(1),
                    checksum: 0,
                    vector_count: 10,
                    dimension: 16,
                    quant_tier: QuantTier::Hot,
                    quant_params: QuantScaleParams::default(),
                    is_delta: false,
                    collection_id: CollectionId(1),
                    tenant_id: tenant,
                    created_at: 1000,
                    modified_at: 1000,
                },
                centroid,
                sub_centroids: vec![],
                neighbor_ids: vec![],
                neighbor_weights: vec![],
                encoded_vectors: vec![],
                vector_ids: vec![],
                residuals: None,
                timestamps: vec![],
                vector_tenant_ids: vec![],
                bloom_filter: vec![],
            };
            routing.add_page(&page);
        }

        let query: Vec<f32> = (0..16).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget {
            max_candidate_pages: 20,
            max_disk_reads: 10,
            ..Default::default()
        };
        let filter = SearchFilter {
            tenant_id: Some(TenantId(1)),
            ..Default::default()
        };

        let candidates = routing.select_candidates(&query, &budget, &filter);
        // All candidates should belong to tenant 1
        for (pid, _) in &candidates {
            let entry = routing.centroid_hnsw.get_entry(*pid).unwrap();
            assert_eq!(entry.tenant_id, TenantId(1));
        }
    }
}
