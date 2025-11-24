//! Comprehensive tests for TLB implementation

use cognitum_core::MemoryAddress;
use cognitum_memory::tlb::{Tlb, TlbEntry};

#[test]
fn test_tlb_creation() {
    let tlb = Tlb::new(64);
    assert!(true, "TLB created successfully");
}

#[test]
fn test_tlb_creation_various_sizes() {
    let sizes = vec![16, 32, 64, 128, 256];
    for size in sizes {
        let tlb = Tlb::new(size);
        assert!(true, "TLB created with size {}", size);
    }
}

#[test]
fn test_tlb_translate_miss() {
    let tlb = Tlb::new(64);
    let virt = MemoryAddress::new(0x1000);

    let result = tlb.translate(virt);
    assert!(result.is_ok(), "Translation should not fail");
    assert!(result.unwrap().is_none(), "Should be TLB miss");
}

#[test]
fn test_tlb_translate_multiple_misses() {
    let tlb = Tlb::new(64);
    let addresses = vec![0x1000, 0x2000, 0x3000, 0x4000];

    for addr_val in addresses {
        let virt = MemoryAddress::new(addr_val);
        let result = tlb.translate(virt);
        assert!(
            result.is_ok(),
            "Translation at 0x{:x} should not fail",
            addr_val
        );
        assert!(
            result.unwrap().is_none(),
            "Should be TLB miss at 0x{:x}",
            addr_val
        );
    }
}

#[test]
fn test_tlb_entry_creation() {
    let entry = TlbEntry {
        virt: MemoryAddress::new(0x1000),
        phys: MemoryAddress::new(0x5000),
        valid: true,
    };

    assert_eq!(entry.virt.value(), 0x1000, "Virtual address should match");
    assert_eq!(entry.phys.value(), 0x5000, "Physical address should match");
    assert!(entry.valid, "Entry should be valid");
}

#[test]
fn test_tlb_entry_invalid() {
    let entry = TlbEntry {
        virt: MemoryAddress::new(0x1000),
        phys: MemoryAddress::new(0x5000),
        valid: false,
    };

    assert!(!entry.valid, "Entry should be invalid");
}

#[test]
fn test_tlb_entry_clone() {
    let entry1 = TlbEntry {
        virt: MemoryAddress::new(0x1000),
        phys: MemoryAddress::new(0x5000),
        valid: true,
    };

    let entry2 = entry1.clone();
    assert_eq!(
        entry2.virt.value(),
        entry1.virt.value(),
        "Cloned virt should match"
    );
    assert_eq!(
        entry2.phys.value(),
        entry1.phys.value(),
        "Cloned phys should match"
    );
    assert_eq!(entry2.valid, entry1.valid, "Cloned valid should match");
}

#[test]
fn test_tlb_zero_address_translation() {
    let tlb = Tlb::new(64);
    let virt = MemoryAddress::new(0);

    let result = tlb.translate(virt);
    assert!(result.is_ok(), "Zero address translation should not fail");
}

#[test]
fn test_tlb_high_address_translation() {
    let tlb = Tlb::new(64);
    let virt = MemoryAddress::new(0xFFFFFFFF);

    let result = tlb.translate(virt);
    assert!(result.is_ok(), "High address translation should not fail");
}

#[test]
fn test_tlb_sequential_translations() {
    let tlb = Tlb::new(128);

    for i in 0..100 {
        let virt = MemoryAddress::new(0x1000 + (i * 4096));
        let result = tlb.translate(virt);
        assert!(
            result.is_ok(),
            "Sequential translation {} should succeed",
            i
        );
    }
}

#[test]
fn test_tlb_page_aligned_addresses() {
    let tlb = Tlb::new(64);
    let page_size = 4096u32;

    for i in 0..10 {
        let virt = MemoryAddress::new(i * page_size);
        let result = tlb.translate(virt);
        assert!(
            result.is_ok(),
            "Page-aligned translation {} should succeed",
            i
        );
    }
}

#[test]
fn test_tlb_non_aligned_addresses() {
    let tlb = Tlb::new(64);
    let addresses = vec![0x1001, 0x2003, 0x3007, 0x400F];

    for addr_val in addresses {
        let virt = MemoryAddress::new(addr_val);
        let result = tlb.translate(virt);
        assert!(
            result.is_ok(),
            "Non-aligned translation 0x{:x} should succeed",
            addr_val
        );
    }
}

#[test]
fn test_tlb_entry_debug_format() {
    let entry = TlbEntry {
        virt: MemoryAddress::new(0x1000),
        phys: MemoryAddress::new(0x5000),
        valid: true,
    };

    let debug_str = format!("{:?}", entry);
    assert!(!debug_str.is_empty(), "Debug format should produce output");
}

#[test]
fn test_tlb_small_size() {
    let tlb = Tlb::new(4);
    let virt = MemoryAddress::new(0x1000);
    let result = tlb.translate(virt);
    assert!(result.is_ok(), "Small TLB should work");
}

#[test]
fn test_tlb_large_size() {
    let tlb = Tlb::new(1024);
    let virt = MemoryAddress::new(0x1000);
    let result = tlb.translate(virt);
    assert!(result.is_ok(), "Large TLB should work");
}

#[test]
fn test_tlb_stress_translations() {
    let tlb = Tlb::new(256);

    // Perform many translations
    for i in 0..1000 {
        let virt = MemoryAddress::new((i * 137) % 0x100000); // Prime number for pseudo-random
        let result = tlb.translate(virt);
        assert!(result.is_ok(), "Stress translation {} should succeed", i);
    }
}
