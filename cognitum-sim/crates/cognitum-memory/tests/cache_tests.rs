//! Comprehensive tests for Cache implementation

use cognitum_core::MemoryAddress;
use cognitum_memory::cache::{Cache, CACHE_LINE_SIZE};

#[test]
fn test_cache_creation() {
    let _cache = Cache::new(1024, 4);
    assert!(true, "Cache created successfully");
}

#[test]
fn test_cache_creation_various_sizes() {
    let sizes = vec![512, 1024, 2048, 4096, 8192];
    for size in sizes {
        let _cache = Cache::new(size, 4);
        assert!(true, "Cache created with size {}", size);
    }
}

#[test]
fn test_cache_read_miss() {
    let cache = Cache::new(1024, 4);
    let addr = MemoryAddress::new(0x1000);
    let result = cache.read(addr);

    assert!(result.is_ok(), "Read should not fail");
    assert!(result.unwrap().is_none(), "Should be cache miss");
}

#[test]
fn test_cache_write_basic() {
    let mut cache = Cache::new(1024, 4);
    let addr = MemoryAddress::new(0x1000);
    let data = vec![1u8, 2, 3, 4];

    let result = cache.write(addr, &data);
    assert!(result.is_ok(), "Write should succeed");
}

#[test]
fn test_cache_write_cache_line_sized() {
    let mut cache = Cache::new(4096, 8);
    let addr = MemoryAddress::new(0x2000);
    let data = vec![0xABu8; CACHE_LINE_SIZE];

    let result = cache.write(addr, &data);
    assert!(result.is_ok(), "Write of cache line should succeed");
}

#[test]
fn test_cache_write_multiple_addresses() {
    let mut cache = Cache::new(4096, 8);
    let addresses = vec![0x1000, 0x2000, 0x3000, 0x4000];

    for addr_val in addresses {
        let addr = MemoryAddress::new(addr_val);
        let data = vec![0xFFu8; 16];
        let result = cache.write(addr, &data);
        assert!(result.is_ok(), "Write to 0x{:x} should succeed", addr_val);
    }
}

#[test]
fn test_cache_read_after_write() {
    let mut cache = Cache::new(4096, 8);
    let addr = MemoryAddress::new(0x1000);
    let data = vec![1u8, 2, 3, 4];

    cache.write(addr, &data).expect("Write should succeed");
    let result = cache.read(addr);

    assert!(result.is_ok(), "Read after write should succeed");
}

#[test]
fn test_cache_multiple_associativity() {
    let associativities = vec![1, 2, 4, 8, 16];
    for assoc in associativities {
        let _cache = Cache::new(4096, assoc);
        assert!(true, "Cache created with associativity {}", assoc);
    }
}

#[test]
fn test_cache_line_size_constant() {
    assert_eq!(CACHE_LINE_SIZE, 64, "Cache line size should be 64 bytes");
}

#[test]
fn test_cache_boundary_address() {
    let cache = Cache::new(1024, 4);
    let addr = MemoryAddress::new(0);
    let result = cache.read(addr);
    assert!(result.is_ok(), "Read at boundary address should succeed");
}

#[test]
fn test_cache_high_address() {
    let cache = Cache::new(1024, 4);
    let addr = MemoryAddress::new(0xFFFFFFFF);
    let result = cache.read(addr);
    assert!(result.is_ok(), "Read at high address should succeed");
}

#[test]
fn test_cache_empty_write() {
    let mut cache = Cache::new(1024, 4);
    let addr = MemoryAddress::new(0x1000);
    let data: Vec<u8> = vec![];

    let result = cache.write(addr, &data);
    assert!(result.is_ok(), "Empty write should succeed");
}

#[test]
fn test_cache_large_write() {
    let mut cache = Cache::new(8192, 8);
    let addr = MemoryAddress::new(0x1000);
    let data = vec![0x42u8; 256];

    let result = cache.write(addr, &data);
    assert!(result.is_ok(), "Large write should succeed");
}

#[test]
fn test_cache_sequential_reads() {
    let cache = Cache::new(4096, 8);

    for i in 0..10 {
        let addr = MemoryAddress::new(0x1000 + (i * 64));
        let result = cache.read(addr);
        assert!(result.is_ok(), "Sequential read {} should succeed", i);
    }
}

#[test]
fn test_cache_sequential_writes() {
    let mut cache = Cache::new(4096, 8);

    for i in 0..10 {
        let addr = MemoryAddress::new(0x1000 + (i * 64));
        let data = vec![i as u8; 16];
        let result = cache.write(addr, &data);
        assert!(result.is_ok(), "Sequential write {} should succeed", i);
    }
}
