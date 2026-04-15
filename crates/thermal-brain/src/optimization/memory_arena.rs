//! Memory Arena Allocator
//!
//! Pool-based memory allocation for no_std environments.
//! Reduces allocation overhead and fragmentation.
//!
//! Features:
//! - Fixed-size block pools
//! - O(1) allocation and deallocation
//! - No external allocator dependency
//! - Suitable for embedded systems

use core::cell::Cell;
use heapless::Vec as HVec;

/// Maximum blocks per pool
const MAX_BLOCKS: usize = 64;

/// Maximum pools
const MAX_POOLS: usize = 8;

/// Block sizes available (bytes)
pub const BLOCK_SIZES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];

/// Block header (stored at start of each block)
#[derive(Clone, Copy, Debug)]
struct BlockHeader {
    /// Pool index this block belongs to
    pool_idx: u8,
    /// Block index within pool
    block_idx: u8,
    /// Is block allocated
    allocated: bool,
    /// Reserved for alignment
    _reserved: u8,
}

impl BlockHeader {
    const SIZE: usize = 4;

    fn new(pool_idx: u8, block_idx: u8) -> Self {
        Self {
            pool_idx,
            block_idx,
            allocated: false,
            _reserved: 0,
        }
    }
}

/// Memory pool for fixed-size blocks
#[derive(Debug)]
pub struct MemoryPool {
    /// Block size (including header)
    block_size: usize,
    /// Number of blocks
    num_blocks: usize,
    /// Free block bitmap (1 = free, 0 = allocated)
    free_bitmap: u64,
    /// Pool index
    pool_idx: u8,
    /// Allocation count
    alloc_count: u32,
    /// Peak allocation count
    peak_alloc: u32,
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new(block_size: usize, num_blocks: usize, pool_idx: u8) -> Self {
        // All blocks start free
        let free_bitmap = if num_blocks >= 64 {
            u64::MAX
        } else {
            (1u64 << num_blocks) - 1
        };

        Self {
            block_size,
            num_blocks: num_blocks.min(64),
            free_bitmap,
            pool_idx,
            alloc_count: 0,
            peak_alloc: 0,
        }
    }

    /// Allocate a block from this pool
    ///
    /// Returns block index if successful
    pub fn allocate(&mut self) -> Option<usize> {
        if self.free_bitmap == 0 {
            return None;
        }

        // Find first free block (trailing zeros = first set bit position)
        let block_idx = self.free_bitmap.trailing_zeros() as usize;

        if block_idx >= self.num_blocks {
            return None;
        }

        // Mark as allocated
        self.free_bitmap &= !(1u64 << block_idx);
        self.alloc_count += 1;
        self.peak_alloc = self.peak_alloc.max(self.alloc_count);

        Some(block_idx)
    }

    /// Deallocate a block
    pub fn deallocate(&mut self, block_idx: usize) -> bool {
        if block_idx >= self.num_blocks {
            return false;
        }

        let mask = 1u64 << block_idx;

        // Check if already free
        if self.free_bitmap & mask != 0 {
            return false; // Double free
        }

        // Mark as free
        self.free_bitmap |= mask;
        self.alloc_count = self.alloc_count.saturating_sub(1);

        true
    }

    /// Get number of free blocks
    pub fn free_count(&self) -> usize {
        self.free_bitmap.count_ones() as usize
    }

    /// Get number of allocated blocks
    pub fn allocated_count(&self) -> usize {
        self.num_blocks - self.free_count()
    }

    /// Check if pool is full
    pub fn is_full(&self) -> bool {
        self.free_bitmap == 0
    }

    /// Check if pool is empty (all blocks free)
    pub fn is_empty(&self) -> bool {
        self.free_count() == self.num_blocks
    }

    /// Get block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get usable size (block size minus header)
    pub fn usable_size(&self) -> usize {
        self.block_size.saturating_sub(BlockHeader::SIZE)
    }

    /// Get utilization (0.0 to 1.0)
    pub fn utilization(&self) -> f32 {
        self.allocated_count() as f32 / self.num_blocks as f32
    }

    /// Get peak utilization
    pub fn peak_utilization(&self) -> f32 {
        self.peak_alloc as f32 / self.num_blocks as f32
    }

    /// Reset pool (free all blocks)
    pub fn reset(&mut self) {
        self.free_bitmap = if self.num_blocks >= 64 {
            u64::MAX
        } else {
            (1u64 << self.num_blocks) - 1
        };
        self.alloc_count = 0;
    }
}

/// Arena statistics
#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaStats {
    /// Total capacity (bytes)
    pub total_capacity: usize,
    /// Used bytes
    pub used_bytes: usize,
    /// Free bytes
    pub free_bytes: usize,
    /// Total allocations performed
    pub total_allocations: u64,
    /// Total deallocations performed
    pub total_deallocations: u64,
    /// Allocation failures
    pub allocation_failures: u64,
    /// Fragmentation estimate (0.0 to 1.0)
    pub fragmentation: f32,
}

/// Memory arena with multiple pool sizes
///
/// Provides efficient memory allocation for embedded systems without
/// requiring a global allocator.
pub struct MemoryArena {
    /// Memory pools for different sizes
    pools: HVec<MemoryPool, MAX_POOLS>,
    /// Arena statistics
    stats: ArenaStats,
}

impl MemoryArena {
    /// Create a new memory arena with default pool configuration
    pub fn new() -> Self {
        let mut pools = HVec::new();

        // Create pools for each block size
        for (i, &size) in BLOCK_SIZES.iter().enumerate() {
            // More small blocks, fewer large blocks
            let num_blocks = match i {
                0..=2 => 32, // 16, 32, 64 byte blocks
                3..=4 => 16, // 128, 256 byte blocks
                _ => 8,      // 512+ byte blocks
            };

            let _ = pools.push(MemoryPool::new(size, num_blocks, i as u8));
        }

        let total_capacity: usize = pools.iter()
            .map(|p| p.block_size * p.num_blocks)
            .sum();

        Self {
            pools,
            stats: ArenaStats {
                total_capacity,
                free_bytes: total_capacity,
                ..Default::default()
            },
        }
    }

    /// Create arena with custom pool configuration
    pub fn with_config(configs: &[(usize, usize)]) -> Self {
        let mut pools = HVec::new();

        for (i, &(block_size, num_blocks)) in configs.iter().take(MAX_POOLS).enumerate() {
            let _ = pools.push(MemoryPool::new(block_size, num_blocks, i as u8));
        }

        let total_capacity: usize = pools.iter()
            .map(|p| p.block_size * p.num_blocks)
            .sum();

        Self {
            pools,
            stats: ArenaStats {
                total_capacity,
                free_bytes: total_capacity,
                ..Default::default()
            },
        }
    }

    /// Allocate memory of given size
    ///
    /// Returns (pool_idx, block_idx) if successful
    pub fn allocate(&mut self, size: usize) -> Option<(u8, u8)> {
        // Find smallest pool that fits
        let required_size = size + BlockHeader::SIZE;

        let mut result: Option<(u8, u8, usize)> = None;

        for pool in self.pools.iter_mut() {
            if pool.block_size >= required_size {
                if let Some(block_idx) = pool.allocate() {
                    result = Some((pool.pool_idx, block_idx as u8, pool.block_size));
                    break;
                }
            }
        }

        if let Some((pool_idx, block_idx, block_size)) = result {
            self.stats.total_allocations += 1;
            self.stats.used_bytes += block_size;
            self.stats.free_bytes = self.stats.free_bytes.saturating_sub(block_size);
            self.update_fragmentation();
            return Some((pool_idx, block_idx));
        }

        // No suitable block found
        self.stats.allocation_failures += 1;
        None
    }

    /// Deallocate memory
    pub fn deallocate(&mut self, pool_idx: u8, block_idx: u8) -> bool {
        if let Some(pool) = self.pools.get_mut(pool_idx as usize) {
            if pool.deallocate(block_idx as usize) {
                self.stats.total_deallocations += 1;
                self.stats.used_bytes = self.stats.used_bytes.saturating_sub(pool.block_size);
                self.stats.free_bytes += pool.block_size;
                self.update_fragmentation();
                return true;
            }
        }
        false
    }

    /// Update fragmentation estimate
    fn update_fragmentation(&mut self) {
        if self.pools.is_empty() {
            return;
        }

        // Fragmentation = 1 - (largest_contiguous_free / total_free)
        // Simplified: use average pool utilization variance
        let utilizations: HVec<f32, MAX_POOLS> = self.pools.iter()
            .map(|p| p.utilization())
            .collect();

        if utilizations.is_empty() {
            return;
        }

        let avg: f32 = utilizations.iter().sum::<f32>() / utilizations.len() as f32;
        let variance: f32 = utilizations.iter()
            .map(|&u| (u - avg).powi(2))
            .sum::<f32>() / utilizations.len() as f32;

        self.stats.fragmentation = variance.sqrt().min(1.0);
    }

    /// Get arena statistics
    pub fn stats(&self) -> ArenaStats {
        self.stats
    }

    /// Get pool by index
    pub fn get_pool(&self, idx: usize) -> Option<&MemoryPool> {
        self.pools.get(idx)
    }

    /// Get number of pools
    pub fn num_pools(&self) -> usize {
        self.pools.len()
    }

    /// Get total capacity
    pub fn capacity(&self) -> usize {
        self.stats.total_capacity
    }

    /// Get used bytes
    pub fn used(&self) -> usize {
        self.stats.used_bytes
    }

    /// Get free bytes
    pub fn free(&self) -> usize {
        self.stats.free_bytes
    }

    /// Get utilization (0.0 to 1.0)
    pub fn utilization(&self) -> f32 {
        if self.stats.total_capacity == 0 {
            0.0
        } else {
            self.stats.used_bytes as f32 / self.stats.total_capacity as f32
        }
    }

    /// Find best fit pool for size
    pub fn best_fit_pool(&self, size: usize) -> Option<usize> {
        let required = size + BlockHeader::SIZE;
        self.pools.iter()
            .enumerate()
            .filter(|(_, p)| p.block_size >= required && !p.is_full())
            .min_by_key(|(_, p)| p.block_size)
            .map(|(i, _)| i)
    }

    /// Reset all pools
    pub fn reset(&mut self) {
        for pool in self.pools.iter_mut() {
            pool.reset();
        }

        self.stats = ArenaStats {
            total_capacity: self.stats.total_capacity,
            free_bytes: self.stats.total_capacity,
            ..Default::default()
        };
    }

    /// Compact arena (defragment)
    ///
    /// Note: In a real implementation, this would move data.
    /// Here we just reset statistics.
    pub fn compact(&mut self) {
        self.stats.fragmentation = 0.0;
    }
}

impl Default for MemoryArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Scoped allocation guard
///
/// Automatically deallocates when dropped.
pub struct ScopedAlloc<'a> {
    arena: &'a mut MemoryArena,
    pool_idx: u8,
    block_idx: u8,
    valid: bool,
}

impl<'a> ScopedAlloc<'a> {
    /// Create a new scoped allocation
    pub fn new(arena: &'a mut MemoryArena, size: usize) -> Option<Self> {
        arena.allocate(size).map(|(pool_idx, block_idx)| Self {
            arena,
            pool_idx,
            block_idx,
            valid: true,
        })
    }

    /// Get allocation info
    pub fn info(&self) -> (u8, u8) {
        (self.pool_idx, self.block_idx)
    }

    /// Release without deallocation (transfer ownership)
    pub fn release(mut self) -> (u8, u8) {
        self.valid = false;
        (self.pool_idx, self.block_idx)
    }
}

impl<'a> Drop for ScopedAlloc<'a> {
    fn drop(&mut self) {
        if self.valid {
            self.arena.deallocate(self.pool_idx, self.block_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool() {
        let mut pool = MemoryPool::new(64, 8, 0);

        assert_eq!(pool.free_count(), 8);
        assert_eq!(pool.allocated_count(), 0);

        // Allocate
        let block = pool.allocate();
        assert!(block.is_some());
        assert_eq!(pool.allocated_count(), 1);

        // Deallocate
        assert!(pool.deallocate(block.unwrap()));
        assert_eq!(pool.free_count(), 8);
    }

    #[test]
    fn test_pool_full() {
        let mut pool = MemoryPool::new(32, 4, 0);

        // Allocate all
        for _ in 0..4 {
            assert!(pool.allocate().is_some());
        }

        assert!(pool.is_full());
        assert!(pool.allocate().is_none());
    }

    #[test]
    fn test_arena_allocation() {
        let mut arena = MemoryArena::new();

        // Allocate small
        let small = arena.allocate(10);
        assert!(small.is_some());

        // Allocate medium
        let medium = arena.allocate(100);
        assert!(medium.is_some());

        // Allocate large
        let large = arena.allocate(1000);
        assert!(large.is_some());

        assert!(arena.used() > 0);
    }

    #[test]
    fn test_arena_deallocation() {
        let mut arena = MemoryArena::new();

        let (pool_idx, block_idx) = arena.allocate(50).unwrap();
        let used_before = arena.used();

        assert!(arena.deallocate(pool_idx, block_idx));
        assert!(arena.used() < used_before);
    }

    #[test]
    fn test_best_fit() {
        let arena = MemoryArena::new();

        // 50 bytes should fit in 64-byte pool (index 2)
        let pool_idx = arena.best_fit_pool(50);
        assert!(pool_idx.is_some());

        let pool = arena.get_pool(pool_idx.unwrap()).unwrap();
        assert!(pool.block_size >= 50 + BlockHeader::SIZE);
    }

    #[test]
    fn test_arena_stats() {
        let mut arena = MemoryArena::new();

        arena.allocate(32);
        arena.allocate(64);
        arena.allocate(128);

        let stats = arena.stats();
        assert_eq!(stats.total_allocations, 3);
        assert!(stats.used_bytes > 0);
    }

    #[test]
    fn test_arena_reset() {
        let mut arena = MemoryArena::new();

        arena.allocate(100);
        arena.allocate(200);
        arena.allocate(300);

        arena.reset();

        assert_eq!(arena.used(), 0);
        assert_eq!(arena.stats().total_allocations, 0);
    }

    #[test]
    fn test_custom_config() {
        let configs = [(32, 16), (128, 8), (512, 4)];
        let arena = MemoryArena::with_config(&configs);

        assert_eq!(arena.num_pools(), 3);
    }
}
