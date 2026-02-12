//! Core types for the page-aligned ANN index.
//!
//! Defines the fundamental data structures: page nodes, headers, quantization
//! tiers, search budgets, and query trace records.

use crate::ruvector::types::EmbeddingId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ============================================================================
// Identifiers
// ============================================================================

/// Unique identifier for a page node on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageId(pub u64);

/// Monotonically increasing page version for atomic swaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PageVersion(pub u64);

/// Collection identifier (multi-tenant isolation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectionId(pub u64);

/// Tenant identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub u64);

// ============================================================================
// Quantization tiers
// ============================================================================

/// Quantization tier controlling compression and fidelity per page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantTier {
    /// Hot pages: 8-bit scalar quantization (4x compression).
    Hot,
    /// Warm pages: 5-bit quantization with calibration (~6x compression).
    Warm,
    /// Cold pages: 3-bit quantization with error bounds (~10x compression).
    Cold,
}

impl QuantTier {
    /// Bits per component for this tier.
    pub fn bits_per_component(&self) -> u8 {
        match self {
            QuantTier::Hot => 8,
            QuantTier::Warm => 5,
            QuantTier::Cold => 3,
        }
    }

    /// Approximate compression ratio vs f32.
    pub fn compression_ratio(&self) -> f32 {
        32.0 / self.bits_per_component() as f32
    }
}

/// Scale parameters for dequantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantScaleParams {
    pub min_val: f32,
    pub max_val: f32,
    pub scale: f32,
}

impl Default for QuantScaleParams {
    fn default() -> Self {
        Self {
            min_val: 0.0,
            max_val: 1.0,
            scale: 1.0 / 255.0,
        }
    }
}

// ============================================================================
// Page header
// ============================================================================

/// Fixed-size header at the start of every page on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageHeader {
    /// Unique page identifier.
    pub page_id: PageId,
    /// Version for atomic swap during compaction.
    pub version: PageVersion,
    /// CRC32 checksum of the entire page (excluding this field).
    pub checksum: u32,
    /// Number of vectors stored in this page.
    pub vector_count: u32,
    /// Vector dimension.
    pub dimension: u16,
    /// Quantization tier.
    pub quant_tier: QuantTier,
    /// Scale parameters for the quantization used.
    pub quant_params: QuantScaleParams,
    /// Whether this is a delta page (recent writes, not yet compacted).
    pub is_delta: bool,
    /// Collection this page belongs to.
    pub collection_id: CollectionId,
    /// Tenant this page belongs to.
    pub tenant_id: TenantId,
    /// Unix timestamp of page creation.
    pub created_at: u64,
    /// Unix timestamp of last modification.
    pub modified_at: u64,
}

// ============================================================================
// Page node (the on-disk unit)
// ============================================================================

/// A page node: the atomic unit of disk I/O and graph traversal.
///
/// Each page packs a cluster of vectors into one storage page, along with
/// a centroid for routing, neighbor links for the page graph, and lightweight
/// metadata for filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageNode {
    /// Page header with id, version, checksum, counts.
    pub header: PageHeader,

    // -- Routing payload --
    /// Representative centroid vector for this page (f32, used for routing).
    pub centroid: Vec<f32>,
    /// Optional secondary centroids for sub-clusters within the page.
    pub sub_centroids: Vec<Vec<f32>>,

    // -- Neighbor payload --
    /// Neighbor page ids in the page graph.
    pub neighbor_ids: Vec<PageId>,
    /// Optional edge weights to neighbors.
    pub neighbor_weights: Vec<f32>,

    // -- Vector payload --
    /// Encoded vectors (quantized bytes packed contiguously).
    pub encoded_vectors: Vec<u8>,
    /// Vector IDs corresponding to each encoded vector.
    pub vector_ids: Vec<EmbeddingId>,
    /// Optional residual data for reranking.
    pub residuals: Option<Vec<u8>>,

    // -- Lightweight metadata --
    /// Per-vector timestamps (parallel to vector_ids).
    pub timestamps: Vec<u64>,
    /// Per-vector tenant ids (parallel to vector_ids, for cross-tenant pages).
    pub vector_tenant_ids: Vec<TenantId>,
    /// Bloom filter bytes for quick tag/label filtering.
    pub bloom_filter: Vec<u8>,
}

impl PageNode {
    /// Compute the byte size of this page node (approximate).
    pub fn byte_size(&self) -> usize {
        let centroid_bytes = self.centroid.len() * 4;
        let sub_centroid_bytes: usize = self.sub_centroids.iter().map(|c| c.len() * 4).sum();
        let neighbor_bytes = self.neighbor_ids.len() * 8 + self.neighbor_weights.len() * 4;
        let vector_bytes = self.encoded_vectors.len();
        let id_bytes = self.vector_ids.len() * 8;
        let residual_bytes = self.residuals.as_ref().map_or(0, |r| r.len());
        let meta_bytes = self.timestamps.len() * 8
            + self.vector_tenant_ids.len() * 8
            + self.bloom_filter.len();

        // Header is roughly 128 bytes serialized
        128 + centroid_bytes
            + sub_centroid_bytes
            + neighbor_bytes
            + vector_bytes
            + id_bytes
            + residual_bytes
            + meta_bytes
    }

    /// Number of vectors in this page.
    pub fn vector_count(&self) -> usize {
        self.vector_ids.len()
    }

    /// Whether this is a delta (write-buffer) page.
    pub fn is_delta(&self) -> bool {
        self.header.is_delta
    }
}

// ============================================================================
// Page manifest
// ============================================================================

/// Manifest tracking all pages, their locations, and versions.
/// Swapped atomically during compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageManifest {
    /// Manifest version (incremented on every swap).
    pub version: u64,
    /// Map from PageId to file offset and size on disk.
    pub page_locations: HashMap<PageId, PageLocation>,
    /// Currently active delta page ids.
    pub delta_page_ids: Vec<PageId>,
    /// Checksum of the manifest itself.
    pub checksum: u32,
    /// Unix timestamp.
    pub created_at: u64,
}

/// Location of a page on disk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageLocation {
    /// Byte offset in the data file.
    pub offset: u64,
    /// Byte length of the serialized page.
    pub length: u32,
    /// Page version at this location.
    pub version: PageVersion,
}

// ============================================================================
// Search budget and configuration
// ============================================================================

/// Budget constraints for a single search query.
#[derive(Debug, Clone)]
pub struct SearchBudget {
    /// Maximum number of candidate pages to consider (routing phase).
    pub max_candidate_pages: usize,
    /// Maximum number of pages to actually fetch from disk.
    pub max_disk_reads: usize,
    /// Maximum wall-clock time for the query.
    pub max_duration: Option<Duration>,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            max_candidate_pages: 64,
            max_disk_reads: 16,
            max_duration: None,
        }
    }
}

/// Configuration for the page-aligned index.
#[derive(Debug, Clone)]
pub struct PageIndexConfig {
    /// Vector dimension.
    pub dimension: usize,
    /// Target page size in bytes (e.g., 4096, 65536).
    pub target_page_size: usize,
    /// Default quantization tier for new pages.
    pub default_quant_tier: QuantTier,
    /// Number of neighbors per page in the page graph.
    pub page_graph_neighbors: usize,
    /// HNSW M parameter for centroid index.
    pub centroid_hnsw_m: usize,
    /// HNSW ef_construction for centroid index.
    pub centroid_hnsw_ef_construction: usize,
    /// HNSW ef_search for centroid index.
    pub centroid_hnsw_ef_search: usize,
    /// Maximum vectors per page (derived from page size and quant tier).
    pub max_vectors_per_page: usize,
    /// Number of delta pages before triggering compaction.
    pub compaction_threshold: usize,
    /// Default search budget.
    pub default_budget: SearchBudget,
}

impl PageIndexConfig {
    /// Compute max vectors per page given dimension and quant tier.
    pub fn compute_max_vectors(target_page_size: usize, dimension: usize, tier: QuantTier) -> usize {
        // Header ~128 bytes, centroid = dim*4, neighbors ~256 bytes
        let overhead = 128 + dimension * 4 + 256;
        let available = target_page_size.saturating_sub(overhead);
        let bytes_per_vector = match tier {
            QuantTier::Hot => dimension, // 8-bit = 1 byte per dim
            QuantTier::Warm => (dimension * 5 + 7) / 8, // 5-bit packed
            QuantTier::Cold => (dimension * 3 + 7) / 8, // 3-bit packed
        };
        // Add 8 bytes per vector for id, 8 for timestamp
        let total_per_vector = bytes_per_vector + 16;
        if total_per_vector == 0 {
            return 0;
        }
        available / total_per_vector
    }
}

impl Default for PageIndexConfig {
    fn default() -> Self {
        let dimension = 256;
        let target_page_size = 65536; // 64KB pages
        let tier = QuantTier::Hot;
        let max_vecs = Self::compute_max_vectors(target_page_size, dimension, tier);

        Self {
            dimension,
            target_page_size,
            default_quant_tier: tier,
            page_graph_neighbors: 16,
            centroid_hnsw_m: 16,
            centroid_hnsw_ef_construction: 200,
            centroid_hnsw_ef_search: 50,
            max_vectors_per_page: max_vecs,
            compaction_threshold: 32,
            default_budget: SearchBudget::default(),
        }
    }
}

// ============================================================================
// Query trace
// ============================================================================

/// Trace record emitted for every query, supporting audit and tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTrace {
    /// Pages considered by the routing phase.
    pub pages_considered: Vec<PageId>,
    /// Pages actually fetched from disk.
    pub pages_fetched: Vec<PageId>,
    /// Number of vectors decoded.
    pub vectors_decoded: usize,
    /// Number of distance computations performed.
    pub distance_computations: usize,
    /// Progression of best score during search (score, page_id).
    pub score_progression: Vec<(f32, PageId)>,
    /// Wall-clock duration of the routing phase.
    pub routing_duration_us: u64,
    /// Wall-clock duration of the disk + decode + score phase.
    pub disk_phase_duration_us: u64,
    /// Total query duration.
    pub total_duration_us: u64,
    /// Whether the search budget was exhausted.
    pub budget_exhausted: bool,
}

impl QueryTrace {
    pub fn new() -> Self {
        Self {
            pages_considered: Vec::new(),
            pages_fetched: Vec::new(),
            vectors_decoded: 0,
            distance_computations: 0,
            score_progression: Vec::new(),
            routing_duration_us: 0,
            disk_phase_duration_us: 0,
            total_duration_us: 0,
            budget_exhausted: false,
        }
    }
}

impl Default for QueryTrace {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Search result with trace
// ============================================================================

/// Result of a page-aligned search including the trace.
#[derive(Debug, Clone)]
pub struct PageSearchResult {
    /// Vector id.
    pub id: EmbeddingId,
    /// Distance to query (lower is better).
    pub distance: f32,
    /// Which page this result came from.
    pub source_page: PageId,
}

/// Full search response.
#[derive(Debug, Clone)]
pub struct PageSearchResponse {
    /// Top-k results sorted by distance ascending.
    pub results: Vec<PageSearchResult>,
    /// Query trace for audit and tuning.
    pub trace: QueryTrace,
}

// ============================================================================
// Filter constraints
// ============================================================================

/// Filter constraints applied during search.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Only return results from this tenant.
    pub tenant_id: Option<TenantId>,
    /// Only return results from this collection.
    pub collection_id: Option<CollectionId>,
    /// Only return results newer than this timestamp.
    pub min_timestamp: Option<u64>,
    /// Only return results older than this timestamp.
    pub max_timestamp: Option<u64>,
    /// Tag filter (matched against bloom filter).
    pub tags: Vec<String>,
}

// ============================================================================
// Compaction policy
// ============================================================================

/// Policy controlling when and how compaction runs.
#[derive(Debug, Clone)]
pub struct CompactionPolicy {
    /// Trigger compaction when delta page count exceeds this.
    pub delta_count_trigger: usize,
    /// Trigger compaction when total delta bytes exceed this.
    pub delta_bytes_trigger: usize,
    /// Trigger compaction when min-cut coherence drops below this.
    pub coherence_threshold: f64,
    /// Maximum pages to recluster in one compaction pass.
    pub max_recluster_pages: usize,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self {
            delta_count_trigger: 32,
            delta_bytes_trigger: 64 * 1024 * 1024, // 64 MB
            coherence_threshold: 0.5,
            max_recluster_pages: 128,
        }
    }
}

// ============================================================================
// Index statistics
// ============================================================================

/// Statistics about the page-aligned index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageIndexStats {
    /// Total number of base pages.
    pub base_page_count: usize,
    /// Total number of delta pages.
    pub delta_page_count: usize,
    /// Total vectors across all pages.
    pub total_vectors: usize,
    /// Total bytes on disk.
    pub total_disk_bytes: usize,
    /// RAM bytes for centroids and routing.
    pub ram_bytes: usize,
    /// Mean vectors per page.
    pub mean_vectors_per_page: f32,
    /// Mean reads per query (rolling average).
    pub mean_reads_per_query: f32,
    /// Number of compactions performed.
    pub compaction_count: usize,
}
