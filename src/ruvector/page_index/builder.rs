//! Offline page builder: clustering, page formation, graph construction.
//!
//! Step 1 of the build pipeline. Takes a set of training vectors, clusters
//! them into page-sized groups, computes centroids, builds the page graph,
//! and serializes pages to a store.

use super::delta::compute_centroid;
use super::routing::{l2_sq, RoutingIndex};
use super::search::InMemoryPageStore;
use super::storage::{encode_vector, fit_quant_params};
use super::types::*;
use crate::ruvector::types::EmbeddingId;
use std::collections::HashMap;

// ============================================================================
// Builder
// ============================================================================

/// Build result from offline page formation.
#[derive(Debug)]
pub struct BuildResult {
    /// Number of pages created.
    pub page_count: usize,
    /// Total vectors packed.
    pub vector_count: usize,
    /// Mean vectors per page.
    pub mean_vectors_per_page: f32,
}

/// Offline page builder.
///
/// 1. Coarse clustering to create page candidates.
/// 2. Pack vectors into pages by locality.
/// 3. Compute centroid and optional subcentroids.
/// 4. Build page graph using centroid similarity.
/// 5. Register pages in routing index and store.
pub struct PageBuilder {
    config: PageIndexConfig,
    next_page_id: u64,
}

impl PageBuilder {
    pub fn new(config: PageIndexConfig, start_page_id: u64) -> Self {
        Self {
            config,
            next_page_id: start_page_id,
        }
    }

    /// Build page index from a set of vectors.
    ///
    /// # Arguments
    /// - `vectors`: (EmbeddingId, vector_data) pairs.
    /// - `collection_id`: Collection to assign pages to.
    /// - `tenant_id`: Tenant to assign pages to.
    /// - `routing`: Routing index to populate.
    /// - `store`: Page store to populate.
    pub fn build(
        &mut self,
        vectors: &[(EmbeddingId, Vec<f32>)],
        collection_id: CollectionId,
        tenant_id: TenantId,
        routing: &mut RoutingIndex,
        store: &mut InMemoryPageStore,
    ) -> BuildResult {
        if vectors.is_empty() {
            return BuildResult {
                page_count: 0,
                vector_count: 0,
                mean_vectors_per_page: 0.0,
            };
        }

        let dim = self.config.dimension;
        let max_per_page = self.config.max_vectors_per_page.max(1);
        let tier = self.config.default_quant_tier;

        // Step 1: Coarse clustering (k-means)
        let num_pages = (vectors.len() + max_per_page - 1) / max_per_page;
        let assignments = self.kmeans_cluster(vectors, num_pages);

        // Step 2: Group vectors by cluster assignment
        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, &cluster) in assignments.iter().enumerate() {
            clusters.entry(cluster).or_insert_with(Vec::new).push(i);
        }

        // Step 3: Build pages from clusters
        let mut pages: Vec<PageNode> = Vec::with_capacity(num_pages);

        for (_cluster_id, indices) in &clusters {
            // Split cluster into page-sized chunks if needed
            for chunk in indices.chunks(max_per_page) {
                let page_id = PageId(self.next_page_id);
                self.next_page_id += 1;

                let chunk_vectors: Vec<Vec<f32>> =
                    chunk.iter().map(|&i| vectors[i].1.clone()).collect();
                let chunk_ids: Vec<EmbeddingId> =
                    chunk.iter().map(|&i| vectors[i].0).collect();

                let centroid = compute_centroid(&chunk_vectors, dim);
                let params = fit_quant_params(&chunk_vectors);

                let mut encoded = Vec::new();
                for v in &chunk_vectors {
                    encoded.extend_from_slice(&encode_vector(v, tier, &params));
                }

                let count = chunk_ids.len();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                pages.push(PageNode {
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
                        created_at: now,
                        modified_at: now,
                    },
                    centroid,
                    sub_centroids: vec![],
                    neighbor_ids: vec![],
                    neighbor_weights: vec![],
                    encoded_vectors: encoded,
                    vector_ids: chunk_ids,
                    residuals: None,
                    timestamps: vec![now; count],
                    vector_tenant_ids: vec![tenant_id; count],
                    bloom_filter: vec![],
                });
            }
        }

        // Step 4: Build page graph (connect nearest centroids)
        self.build_page_graph(&mut pages);

        // Step 5: Register in routing index and store
        let page_count = pages.len();
        let vector_count = vectors.len();

        for page in pages {
            routing.add_page(&page);
            store.insert(page);
        }

        let mean_vpp = if page_count > 0 {
            vector_count as f32 / page_count as f32
        } else {
            0.0
        };

        BuildResult {
            page_count,
            vector_count,
            mean_vectors_per_page: mean_vpp,
        }
    }

    /// Simple k-means clustering for page assignment.
    fn kmeans_cluster(
        &self,
        vectors: &[(EmbeddingId, Vec<f32>)],
        k: usize,
    ) -> Vec<usize> {
        let n = vectors.len();
        let dim = self.config.dimension;
        let k = k.min(n).max(1);
        let iterations = 20;

        // Initialize centroids from evenly-spaced samples
        let mut centroids: Vec<Vec<f32>> = (0..k)
            .map(|i| {
                let idx = (i * n) / k;
                vectors[idx].1.clone()
            })
            .collect();

        let mut assignments = vec![0usize; n];

        for _ in 0..iterations {
            // Assignment step
            for (i, (_, vec)) in vectors.iter().enumerate() {
                let mut best_cluster = 0;
                let mut best_dist = f32::INFINITY;
                for (c, centroid) in centroids.iter().enumerate() {
                    let dist = l2_sq(vec, centroid);
                    if dist < best_dist {
                        best_dist = dist;
                        best_cluster = c;
                    }
                }
                assignments[i] = best_cluster;
            }

            // Update step
            let mut new_centroids = vec![vec![0.0f64; dim]; k];
            let mut counts = vec![0usize; k];

            for (i, (_, vec)) in vectors.iter().enumerate() {
                let c = assignments[i];
                counts[c] += 1;
                for (j, &val) in vec.iter().enumerate().take(dim) {
                    new_centroids[c][j] += val as f64;
                }
            }

            for c in 0..k {
                if counts[c] > 0 {
                    centroids[c] = new_centroids[c]
                        .iter()
                        .map(|&v| (v / counts[c] as f64) as f32)
                        .collect();
                }
            }
        }

        assignments
    }

    /// Build page-graph edges based on centroid similarity.
    fn build_page_graph(&self, pages: &mut [PageNode]) {
        let k = self.config.page_graph_neighbors;
        // Clone centroids and page ids to avoid borrow conflict
        let centroids: Vec<Vec<f32>> = pages.iter().map(|p| p.centroid.clone()).collect();
        let page_ids: Vec<PageId> = pages.iter().map(|p| p.header.page_id).collect();

        let mut assignments: Vec<(Vec<PageId>, Vec<f32>)> = Vec::with_capacity(pages.len());
        for i in 0..pages.len() {
            let mut dists: Vec<(usize, f32)> = (0..pages.len())
                .filter(|&j| j != i)
                .map(|j| (j, l2_sq(&centroids[i], &centroids[j])))
                .collect();
            dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let num_neighbors = k.min(dists.len());
            let nids: Vec<PageId> = dists[..num_neighbors]
                .iter()
                .map(|(j, _)| page_ids[*j])
                .collect();
            let nweights: Vec<f32> = dists[..num_neighbors]
                .iter()
                .map(|(_, d)| *d)
                .collect();
            assignments.push((nids, nweights));
        }

        for (i, (nids, nweights)) in assignments.into_iter().enumerate() {
            pages[i].neighbor_ids = nids;
            pages[i].neighbor_weights = nweights;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let config = PageIndexConfig {
            dimension: 16,
            max_vectors_per_page: 10,
            ..Default::default()
        };
        let mut builder = PageBuilder::new(config.clone(), 1);
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..50)
            .map(|i| {
                (
                    EmbeddingId(i),
                    (0..16).map(|_| rand::random::<f32>()).collect(),
                )
            })
            .collect();

        let result = builder.build(
            &vectors,
            CollectionId(1),
            TenantId(1),
            &mut routing,
            &mut store,
        );

        assert_eq!(result.vector_count, 50);
        assert!(result.page_count >= 5); // 50 vectors / 10 per page
        assert!(result.mean_vectors_per_page > 0.0);
        assert!(result.mean_vectors_per_page <= 10.0);

        // Routing index should have all pages
        assert_eq!(routing.centroid_hnsw.len(), result.page_count);

        // Store should have all pages
        assert_eq!(store.len(), result.page_count);
    }

    #[test]
    fn test_builder_search_after_build() {
        let dim = 32;
        let config = PageIndexConfig {
            dimension: dim,
            max_vectors_per_page: 20,
            ..Default::default()
        };
        let mut builder = PageBuilder::new(config.clone(), 1);
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..100)
            .map(|i| {
                (
                    EmbeddingId(i),
                    (0..dim).map(|_| rand::random::<f32>()).collect(),
                )
            })
            .collect();

        builder.build(
            &vectors,
            CollectionId(1),
            TenantId(1),
            &mut routing,
            &mut store,
        );

        // Now search
        let query: Vec<f32> = (0..dim).map(|_| rand::random::<f32>()).collect();
        let budget = SearchBudget {
            max_candidate_pages: 10,
            max_disk_reads: 5,
            ..Default::default()
        };
        let filter = SearchFilter::default();

        let response = super::super::search::execute_search(
            &query,
            10,
            &budget,
            &filter,
            &routing,
            &store,
        );

        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 10);
    }

    #[test]
    fn test_builder_page_graph_connectivity() {
        let dim = 8;
        let config = PageIndexConfig {
            dimension: dim,
            max_vectors_per_page: 5,
            page_graph_neighbors: 4,
            ..Default::default()
        };
        let mut builder = PageBuilder::new(config.clone(), 1);
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..30)
            .map(|i| {
                (
                    EmbeddingId(i),
                    (0..dim).map(|_| rand::random::<f32>()).collect(),
                )
            })
            .collect();

        builder.build(
            &vectors,
            CollectionId(1),
            TenantId(1),
            &mut routing,
            &mut store,
        );

        // Check that page graph has edges
        assert!(routing.page_graph.page_count() > 0);
    }

    #[test]
    fn test_builder_empty_input() {
        let config = PageIndexConfig::default();
        let mut builder = PageBuilder::new(config.clone(), 1);
        let mut routing = RoutingIndex::new(&config);
        let mut store = InMemoryPageStore::new();

        let result = builder.build(
            &[],
            CollectionId(1),
            TenantId(1),
            &mut routing,
            &mut store,
        );

        assert_eq!(result.page_count, 0);
        assert_eq!(result.vector_count, 0);
    }
}
