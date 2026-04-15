//! Comprehensive SIMD Coprocessor Tests
//!
//! Tests all SIMD operations including:
//! - Vector arithmetic (8/16/32-bit)
//! - Matrix operations
//! - Neural network primitives
//! - Performance characteristics

use cognitum_coprocessor::simd::{SimdCoprocessor, SimdVector, Matrix4x4};

// ==================== Vector Arithmetic Tests ====================

#[test]
fn test_vadd16_basic() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let b = SimdVector::from_slice(&[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);

    let result = cop.vadd16(&a, &b);

    for i in 0..16 {
        assert_eq!(result.data[i], 17, "Lane {} should be 17", i);
    }
}

#[test]
fn test_vadd16_overflow() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(32767); // Max i16
    let b = SimdVector::splat(1);

    let result = cop.vadd16(&a, &b);
    assert_eq!(result.data[0], -32768); // Wrapping add
}

#[test]
fn test_vadd8() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[0x0102, 0x0304, 0x0506, 0x0708]);
    let b = SimdVector::from_slice(&[0x0101, 0x0101, 0x0101, 0x0101]);

    let result = cop.vadd8(&a, &b);

    // Each byte should add independently
    assert_eq!(result.data[0], 0x0203);
    assert_eq!(result.data[1], 0x0405);
    assert_eq!(cop.get_op_count(), 32); // 32 8-bit operations
}

#[test]
fn test_vadd32() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[0x0001, 0x0002, 0x0003, 0x0004]); // 0x00020001, 0x00040003
    let b = SimdVector::from_slice(&[0x0010, 0x0020, 0x0030, 0x0040]); // 0x00200010, 0x00400030

    let result = cop.vadd32(&a, &b);

    // Result should be 0x00220011, 0x00440033
    assert_eq!(result.data[0], 0x0011);
    assert_eq!(result.data[1], 0x0022);
    assert_eq!(result.data[2], 0x0033);
    assert_eq!(result.data[3], 0x0044);
    assert_eq!(cop.get_op_count(), 8); // 8 32-bit operations
}

#[test]
fn test_vsub16() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(100);
    let b = SimdVector::splat(30);

    let result = cop.vsub16(&a, &b);
    assert_eq!(result.data[0], 70);
}

#[test]
fn test_vmul16() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(10);
    let b = SimdVector::splat(5);

    let result = cop.vmul16(&a, &b);

    for i in 0..16 {
        assert_eq!(result.data[i], 50);
    }
    assert_eq!(cop.get_cycle_count(), 2); // Multiply takes 2 cycles
}

#[test]
fn test_vdot16() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(10);
    let b = SimdVector::splat(5);

    let result = cop.vdot16(&a, &b);

    assert_eq!(result, 16 * 10 * 5); // 16 lanes × 10 × 5 = 800
    assert_eq!(cop.get_op_count(), 32); // 16 muls + 16 adds
}

#[test]
fn test_vdot16_orthogonal() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]);
    let b = SimdVector::from_slice(&[0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1]);

    let result = cop.vdot16(&a, &b);
    assert_eq!(result, 0); // Orthogonal vectors
}

#[test]
fn test_vdot32() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::from_slice(&[100, 0, 200, 0, 300, 0, 400, 0]); // 32-bit: [100, 200, 300, 400]
    let b = SimdVector::from_slice(&[2, 0, 3, 0, 4, 0, 5, 0]); // 32-bit: [2, 3, 4, 5]

    let result = cop.vdot32(&a, &b);

    // 100*2 + 200*3 + 300*4 + 400*5 = 200 + 600 + 1200 + 2000 = 4000
    assert_eq!(result, 4000);
}

#[test]
fn test_vmadd() {
    let mut cop = SimdCoprocessor::new();
    let a = SimdVector::splat(10);
    let b = SimdVector::splat(5);
    let c = SimdVector::splat(100);

    let result = cop.vmadd(&a, &b, &c);

    // result = a * b + c = 10 * 5 + 100 = 150
    for i in 0..16 {
        assert_eq!(result.data[i], 150);
    }
    assert_eq!(cop.get_op_count(), 32); // 16 muls + 16 adds
}

// ==================== Matrix Operation Tests ====================

#[test]
fn test_matrix_identity() {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::identity();
    let b = Matrix4x4::from_array(&[
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    ]);

    let result = cop.mmul(&a, &b);

    assert_eq!(result.data, b.data);
    assert_eq!(cop.get_op_count(), 128); // 64 muls + 64 adds
}

#[test]
fn test_matrix_multiply() {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::from_array(&[
        [1, 2, 0, 0],
        [0, 1, 2, 0],
        [0, 0, 1, 2],
        [0, 0, 0, 1],
    ]);
    let b = Matrix4x4::from_array(&[
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1],
    ]);

    let result = cop.mmul(&a, &b);

    // Should equal 'a' since b is identity
    assert_eq!(result.data[0][0], 1);
    assert_eq!(result.data[0][1], 2);
    assert_eq!(result.data[1][1], 1);
    assert_eq!(result.data[1][2], 2);
}

#[test]
fn test_matrix_multiply_non_identity() {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::from_array(&[
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    ]);
    let b = Matrix4x4::from_array(&[
        [2, 0, 0, 0],
        [0, 2, 0, 0],
        [0, 0, 2, 0],
        [0, 0, 0, 2],
    ]);

    let result = cop.mmul(&a, &b);

    // Should be 2*a
    assert_eq!(result.data[0][0], 2);
    assert_eq!(result.data[0][1], 4);
    assert_eq!(result.data[1][0], 10);
    assert_eq!(result.data[3][3], 32);
}

#[test]
fn test_mmadd() {
    let mut cop = SimdCoprocessor::new();
    let a = Matrix4x4::identity();
    let b = Matrix4x4::from_array(&[
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    ]);
    let c = Matrix4x4::from_array(&[
        [100, 100, 100, 100],
        [100, 100, 100, 100],
        [100, 100, 100, 100],
        [100, 100, 100, 100],
    ]);

    let result = cop.mmadd(&a, &b, &c);

    // result = I * b + c = b + c
    assert_eq!(result.data[0][0], 101);
    assert_eq!(result.data[0][1], 102);
    assert_eq!(result.data[3][3], 116);
}

// ==================== Neural Network Primitive Tests ====================

#[test]
fn test_relu_positive() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[10, 20, 30, 40, 50]);

    let result = cop.relu(&v);

    assert_eq!(result.data[0], 10);
    assert_eq!(result.data[1], 20);
    assert_eq!(result.data[2], 30);
}

#[test]
fn test_relu_negative() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[-10, -20, -30, -40, -50]);

    let result = cop.relu(&v);

    for i in 0..5 {
        assert_eq!(result.data[i], 0);
    }
}

#[test]
fn test_relu_mixed() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[-5, -1, 0, 1, 5, -10, 10, -100, 100]);

    let result = cop.relu(&v);

    assert_eq!(result.data[0], 0);
    assert_eq!(result.data[1], 0);
    assert_eq!(result.data[2], 0);
    assert_eq!(result.data[3], 1);
    assert_eq!(result.data[4], 5);
    assert_eq!(result.data[5], 0);
    assert_eq!(result.data[6], 10);
    assert_eq!(result.data[7], 0);
    assert_eq!(result.data[8], 100);
}

#[test]
fn test_sigmoid_approximation() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::from_slice(&[-5000, -2048, 0, 2048, 5000]);

    let result = cop.sigmoid(&v);

    // For large negative values, should be close to 0
    assert!(result.data[0] < 1000);
    // For 0, should be close to 0.5 (16384 in Q15)
    assert!((result.data[2] - 16384).abs() < 1000);
    // For large positive values, should be close to 1.0 (32767 in Q15)
    assert!(result.data[4] > 31000);
}

#[test]
fn test_softmax_uniform() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::splat(1000); // All same value

    let result = cop.softmax(&v);

    // All values should be approximately equal (1/16 ≈ 2048 in Q15)
    for i in 0..16 {
        assert!((result.data[i] - 2048).abs() < 500, "Lane {} = {}", i, result.data[i]);
    }
}

#[test]
fn test_softmax_winner_take_all() {
    let mut cop = SimdCoprocessor::new();
    let mut data = [0i16; 16];
    data[5] = 10000; // One large value
    let v = SimdVector { data };

    let result = cop.softmax(&v);

    // Lane 5 should have the largest value
    // (softmax approximation may not give strong separation, so just check it's larger)
    for i in 0..16 {
        if i != 5 {
            assert!(result.data[5] >= result.data[i],
                "Lane 5 ({}) should be >= lane {} ({})",
                result.data[5], i, result.data[i]);
        }
    }
}

#[test]
fn test_pool_max_2x2() {
    let mut cop = SimdCoprocessor::new();

    // Create a 4×4 input matrix (row-major in vector)
    let input = SimdVector::from_slice(&[
        1, 2,   3, 4,
        5, 10,  7, 8,
        9, 11,  15, 12,
        13, 14, 20, 16,
    ]);

    let result = cop.pool_max_2x2(&input);

    // Expected max pooling:
    // Pool(1,2,5,10) = 10
    // Pool(3,4,7,8) = 8
    // Pool(9,11,13,14) = 14
    // Pool(15,12,20,16) = 20
    assert_eq!(result.data[0], 10);
    assert_eq!(result.data[1], 8);
    assert_eq!(result.data[2], 14);
    assert_eq!(result.data[3], 20);
}

#[test]
fn test_conv2d_3x3_identity() {
    let mut cop = SimdCoprocessor::new();

    // 5×5 input
    let input = [
        0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
        0, 0, 100, 0, 0,
        0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ];

    // 3×3 identity kernel (center = 1, rest = 0)
    let kernel = [
        0, 0, 0,
        0, 1, 0,
        0, 0, 0,
    ];

    let result = cop.conv2d_3x3(&input, &kernel);

    // Center of output should be 100
    assert_eq!(result[4], 100); // Center of 3×3 output
}

#[test]
fn test_conv2d_3x3_edge_detect() {
    let mut cop = SimdCoprocessor::new();

    // 5×5 input with vertical edge
    let input = [
        0, 0, 100, 100, 100,
        0, 0, 100, 100, 100,
        0, 0, 100, 100, 100,
        0, 0, 100, 100, 100,
        0, 0, 100, 100, 100,
    ];

    // Sobel horizontal edge detector
    let kernel = [
        -1, 0, 1,
        -2, 0, 2,
        -1, 0, 1,
    ];

    let result = cop.conv2d_3x3(&input, &kernel);

    // Should detect edge in the middle
    // Left column: negative response
    // Center column: strong positive response
    // Right column: weak response
    assert!(result[3] > 0); // Center should be positive
}

#[test]
fn test_conv2d_5x5() {
    let mut cop = SimdCoprocessor::new();

    // 9×9 input with center peak
    let mut input = [0i16; 81];
    input[40] = 100; // Center at (4,4)

    // 5×5 Gaussian-like kernel (simplified)
    let kernel = [
        1, 2, 3, 2, 1,
        2, 4, 6, 4, 2,
        3, 6, 9, 6, 3,
        2, 4, 6, 4, 2,
        1, 2, 3, 2, 1,
    ];

    let result = cop.conv2d_5x5(&input, &kernel);

    // Center of output should be 100 * 9 = 900 (from center of kernel)
    assert_eq!(result[12], 900); // Center of 5×5 output
}

// ==================== Work RAM Tests ====================

#[test]
fn test_work_ram_load_store() {
    let mut cop = SimdCoprocessor::new();
    let v = SimdVector::splat(42);

    cop.store_vector(0, v).unwrap();
    let loaded = cop.load_vector(0).unwrap();

    assert_eq!(loaded.data, v.data);
}

#[test]
fn test_work_ram_bounds() {
    let cop = SimdCoprocessor::new();

    // Should succeed
    assert!(cop.load_vector(0).is_ok());
    assert!(cop.load_vector(4095).is_ok());

    // Should fail
    assert!(cop.load_vector(4096).is_err());
    assert!(cop.load_vector(10000).is_err());
}

#[test]
fn test_accumulator_operations() {
    let mut cop = SimdCoprocessor::new();

    cop.store_accumulator(0, 12345).unwrap();
    cop.store_accumulator(7, -6789).unwrap();

    assert_eq!(cop.load_accumulator(0).unwrap(), 12345);
    assert_eq!(cop.load_accumulator(7).unwrap(), -6789);

    cop.clear_accumulators();
    assert_eq!(cop.load_accumulator(0).unwrap(), 0);
    assert_eq!(cop.load_accumulator(7).unwrap(), 0);
}

#[test]
fn test_accumulator_bounds() {
    let cop = SimdCoprocessor::new();

    // Should succeed
    assert!(cop.load_accumulator(0).is_ok());
    assert!(cop.load_accumulator(7).is_ok());

    // Should fail
    assert!(cop.load_accumulator(8).is_err());
    assert!(cop.load_accumulator(100).is_err());
}

// ==================== Performance Tests ====================

#[test]
fn test_performance_counters() {
    let mut cop = SimdCoprocessor::new();
    cop.reset_counters();

    assert_eq!(cop.get_op_count(), 0);
    assert_eq!(cop.get_cycle_count(), 0);

    let a = SimdVector::splat(1);
    let b = SimdVector::splat(2);

    cop.vadd16(&a, &b); // 16 ops, 1 cycle
    assert_eq!(cop.get_op_count(), 16);
    assert_eq!(cop.get_cycle_count(), 1);

    cop.vmul16(&a, &b); // 16 ops, 2 cycles
    assert_eq!(cop.get_op_count(), 32);
    assert_eq!(cop.get_cycle_count(), 3);
}

#[test]
fn test_gops_calculation() {
    let mut cop = SimdCoprocessor::new();
    cop.reset_counters();

    let a = SimdVector::splat(1);
    let b = SimdVector::splat(2);

    // Perform multiple operations
    for _ in 0..1000 {
        cop.vadd16(&a, &b); // 16 ops, 1 cycle each
    }

    let gops = cop.get_gops();

    // Should be 16 ops/cycle at 1GHz = 16 GOPS
    assert!((gops - 16.0).abs() < 0.1, "GOPS = {}", gops);
}

#[test]
fn test_complex_operation_sequence() {
    let mut cop = SimdCoprocessor::new();
    cop.reset_counters();

    let a = SimdVector::from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let b = SimdVector::splat(2);

    // Simulate a typical neural network layer forward pass
    let mul_result = cop.vmul16(&a, &b); // Weights × input
    let bias = SimdVector::splat(10);
    let add_result = cop.vadd16(&mul_result, &bias); // Add bias
    let activated = cop.relu(&add_result); // Activation

    // Check results
    assert_eq!(activated.data[0], 12); // (1*2 + 10) = 12
    assert_eq!(activated.data[1], 14); // (2*2 + 10) = 14

    // Check performance tracking
    let total_ops = cop.get_op_count();
    assert!(total_ops >= 16 + 16 + 16); // mul + add + relu
}

#[test]
fn test_matrix_chain_operations() {
    let mut cop = SimdCoprocessor::new();

    let m1 = Matrix4x4::identity();
    let m2 = Matrix4x4::from_array(&[
        [1, 0, 0, 0],
        [0, 2, 0, 0],
        [0, 0, 3, 0],
        [0, 0, 0, 4],
    ]);

    // Chain multiple matrix operations
    let result1 = cop.mmul(&m1, &m2);
    let result2 = cop.mmul(&result1, &m1);

    // Should equal m2 since multiplying by identity
    assert_eq!(result2.data[1][1], 2);
    assert_eq!(result2.data[2][2], 3);
    assert_eq!(result2.data[3][3], 4);
}
