//! Comprehensive tests for DRAM implementation

use cognitum_core::MemoryAddress;
use cognitum_memory::dram::Dram;

#[test]
fn test_dram_creation() {
    let _dram = Dram::new(1024);
    assert!(true, "DRAM created successfully");
}

#[test]
fn test_dram_creation_various_sizes() {
    let sizes = vec![512, 1024, 2048, 4096, 8192, 16384];
    for size in sizes {
        let _dram = Dram::new(size);
        assert!(true, "DRAM created with size {}", size);
    }
}

#[test]
fn test_dram_read_basic() {
    let dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);

    let result = dram.read(addr, 4);
    assert!(result.is_ok(), "Read should succeed");
    assert_eq!(result.unwrap().len(), 4, "Should read 4 bytes");
}

#[test]
fn test_dram_read_initial_zeros() {
    let dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);

    let result = dram.read(addr, 8).unwrap();
    assert_eq!(result, &[0u8; 8], "Initial memory should be zeros");
}

#[test]
fn test_dram_write_basic() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);
    let data = vec![1u8, 2, 3, 4];

    let result = dram.write(addr, &data);
    assert!(result.is_ok(), "Write should succeed");
}

#[test]
fn test_dram_write_and_read_back() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(100);
    let data = vec![0xAAu8, 0xBB, 0xCC, 0xDD];

    dram.write(addr, &data).expect("Write should succeed");
    let result = dram.read(addr, 4).expect("Read should succeed");

    assert_eq!(
        result,
        data.as_slice(),
        "Read data should match written data"
    );
}

#[test]
fn test_dram_write_at_offset() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(512);
    let data = vec![0x11u8, 0x22, 0x33, 0x44];

    dram.write(addr, &data)
        .expect("Write at offset should succeed");
    let result = dram.read(addr, 4).expect("Read at offset should succeed");

    assert_eq!(result, data.as_slice(), "Data at offset should match");
}

#[test]
fn test_dram_multiple_writes() {
    let mut dram = Dram::new(1024);

    for i in 0..10 {
        let addr = MemoryAddress::new(i * 10);
        let data = vec![i as u8; 4];
        let result = dram.write(addr, &data);
        assert!(result.is_ok(), "Write {} should succeed", i);
    }
}

#[test]
fn test_dram_overwrite() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);

    let data1 = vec![1u8, 2, 3, 4];
    dram.write(addr, &data1)
        .expect("First write should succeed");

    let data2 = vec![5u8, 6, 7, 8];
    dram.write(addr, &data2).expect("Overwrite should succeed");

    let result = dram.read(addr, 4).expect("Read should succeed");
    assert_eq!(result, data2.as_slice(), "Should read overwritten data");
}

#[test]
fn test_dram_read_various_lengths() {
    let dram = Dram::new(1024);
    let lengths = vec![1, 2, 4, 8, 16, 32, 64, 128];

    for len in lengths {
        let addr = MemoryAddress::new(0);
        let result = dram.read(addr, len);
        assert!(result.is_ok(), "Read of length {} should succeed", len);
        assert_eq!(result.unwrap().len(), len, "Should read {} bytes", len);
    }
}

#[test]
fn test_dram_boundary_read() {
    let dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);

    let result = dram.read(addr, 1);
    assert!(result.is_ok(), "Boundary read should succeed");
}

#[test]
fn test_dram_boundary_write() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(0);
    let data = vec![0xFF];

    let result = dram.write(addr, &data);
    assert!(result.is_ok(), "Boundary write should succeed");
}

#[test]
fn test_dram_end_boundary_read() {
    let dram = Dram::new(1024);
    let addr = MemoryAddress::new(1020);

    let result = dram.read(addr, 4);
    assert!(result.is_ok(), "Read at end boundary should succeed");
}

#[test]
fn test_dram_end_boundary_write() {
    let mut dram = Dram::new(1024);
    let addr = MemoryAddress::new(1020);
    let data = vec![0xFF; 4];

    let result = dram.write(addr, &data);
    assert!(result.is_ok(), "Write at end boundary should succeed");
}

#[test]
fn test_dram_sequential_access_pattern() {
    let mut dram = Dram::new(4096);

    // Sequential write
    for i in 0..64 {
        let addr = MemoryAddress::new(i * 16);
        let data = vec![i as u8; 16];
        dram.write(addr, &data)
            .expect("Sequential write should succeed");
    }

    // Sequential read back
    for i in 0..64 {
        let addr = MemoryAddress::new(i * 16);
        let result = dram.read(addr, 16).expect("Sequential read should succeed");
        assert_eq!(result[0], i as u8, "Data should match at position {}", i);
    }
}

#[test]
fn test_dram_random_access_pattern() {
    let mut dram = Dram::new(4096);
    let addresses = vec![100, 500, 1000, 1500, 2000, 2500, 3000, 3500];

    // Write at random addresses
    for (idx, addr_val) in addresses.iter().enumerate() {
        let addr = MemoryAddress::new(*addr_val);
        let data = vec![idx as u8; 8];
        dram.write(addr, &data)
            .expect("Random write should succeed");
    }

    // Read back
    for (idx, addr_val) in addresses.iter().enumerate() {
        let addr = MemoryAddress::new(*addr_val);
        let result = dram.read(addr, 8).expect("Random read should succeed");
        assert_eq!(
            result[0], idx as u8,
            "Data should match at address {}",
            addr_val
        );
    }
}

#[test]
fn test_dram_large_block_transfer() {
    let mut dram = Dram::new(8192);
    let addr = MemoryAddress::new(0);
    let data = vec![0x42u8; 1024];

    dram.write(addr, &data).expect("Large write should succeed");
    let result = dram.read(addr, 1024).expect("Large read should succeed");

    assert_eq!(result.len(), 1024, "Should read 1024 bytes");
    assert_eq!(result, data.as_slice(), "Large block should match");
}

#[test]
fn test_dram_partial_overlap() {
    let mut dram = Dram::new(1024);

    // Write at address 100
    let addr1 = MemoryAddress::new(100);
    let data1 = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    dram.write(addr1, &data1)
        .expect("First write should succeed");

    // Write overlapping at address 104
    let addr2 = MemoryAddress::new(104);
    let data2 = vec![9u8, 10, 11, 12];
    dram.write(addr2, &data2)
        .expect("Overlapping write should succeed");

    // Read back original range
    let result = dram.read(addr1, 8).expect("Read should succeed");
    assert_eq!(
        &result[0..4],
        &data1[0..4],
        "First 4 bytes should be original"
    );
    assert_eq!(
        &result[4..8],
        &data2[0..4],
        "Last 4 bytes should be overwritten"
    );
}
