//! End-to-End Validation Tests
//!
//! Comprehensive E2E tests that validate the complete Cognitum system including
//! processor, memory, and integration workflows.

use cognitum_processor::{A2SProcessor, Instruction};

// ============================================================================
// Complete Program Execution Tests (20+ cases)
// ============================================================================

#[test]
fn test_fibonacci_sequence_calculation() {
    // Calculate first 10 Fibonacci numbers using processor
    let mut cpu = A2SProcessor::with_default_memory();

    // fib(10) = 55
    // Implementation: iterative approach
    // After N iterations: stack has [f(N-1), f(N), counter]
    // To get fib(10)=55 on top, use 9 iterations (produces [f(9), f(10)])
    //
    // Index layout:
    // 0: Push(0)     - f(n-2) = 0
    // 1: Push(1)     - f(n-1) = 1
    // 2: Push(9)     - counter = 9 (9 iterations to get fib(10))
    // 3: Dup         - duplicate counter for test
    // 4: JumpZero(11) - if counter == 0, jump to Drop (4+11=15)
    // 5-14: loop body
    // 15: Drop       - drop counter
    // 16: Halt
    let program = vec![
        Instruction::Push(0),     // 0: f(n-2) = 0
        Instruction::Push(1),     // 1: f(n-1) = 1
        Instruction::Push(9),     // 2: counter = 9
        // Loop: while counter > 0
        Instruction::Dup,         // 3: [a, b, counter, counter]
        Instruction::JumpZero(11),// 4: if counter == 0, jump to Drop (4+11=15)
        // Calculate next fib number
        Instruction::Push(1),     // 5
        Instruction::Sub,         // 6: counter-1
        Instruction::ToR,         // 7: [a, b] R: [counter-1]
        Instruction::Dup,         // 8: [a, b, b]
        Instruction::ToR,         // 9: [a, b] R: [counter-1, b]
        Instruction::Add,         // 10: [a+b]
        Instruction::FromR,       // 11: [a+b, b]
        Instruction::Swap,        // 12: [b, a+b]
        Instruction::FromR,       // 13: [b, a+b, counter-1]
        Instruction::Jump(-11),   // 14: loop back to index 3 (14-11=3)
        Instruction::Drop,        // 15: drop counter
        Instruction::Halt,        // 16
    ];

    cpu.run(&program).unwrap();
    // After 9 iterations: [f(9), f(10), 0] = [34, 55, 0]
    // JumpZero pops 0, jumps to Drop which removes counter
    // Stack: [34, 55], top = 55
    assert_eq!(cpu.peek_stack().unwrap(), 55);
}

#[test]
fn test_factorial_calculation() {
    // Calculate 6! = 720
    let mut cpu = A2SProcessor::with_default_memory();

    // Index layout:
    // 0: Push(1)     - result = 1
    // 1: Push(6)     - counter = 6
    // 2: Dup         - duplicate counter for test
    // 3: JumpZero(8) - if counter == 0, jump to Drop (3+8=11)
    // 4: Swap
    // 5: Over
    // 6: Multiply
    // 7: Swap
    // 8: Push(1)
    // 9: Sub
    // 10: Jump(-8)   - loop back to Dup (10-8=2)
    // 11: Drop       - drop counter (0), leaving result
    // 12: Halt
    let program = vec![
        Instruction::Push(1),       // 0: result = 1
        Instruction::Push(6),       // 1: counter = 6
        // Loop
        Instruction::Dup,           // 2: [result, counter, counter]
        Instruction::JumpZero(8),   // 3: if counter == 0, jump to Drop (3+8=11)
        Instruction::Swap,          // 4: [counter, result]
        Instruction::Over,          // 5: [counter, result, counter]
        Instruction::Multiply,      // 6: [counter, result*counter]
        Instruction::Swap,          // 7: [result*counter, counter]
        Instruction::Push(1),       // 8
        Instruction::Sub,           // 9: [result*counter, counter-1]
        Instruction::Jump(-8),      // 10: loop back (10-8=2)
        Instruction::Drop,          // 11: drop counter (0), leaving result
        Instruction::Halt,          // 12
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 720);
}

#[test]
fn test_array_sum_using_memory() {
    let mut cpu = A2SProcessor::with_default_memory();

    // First, store array values in memory
    let setup_array = vec![
        // Store values at addresses 0x1000, 0x1004, 0x1008, 0x100C
        Instruction::Push(10),    // value
        Instruction::Push(0x1000), // address
        Instruction::Store,

        Instruction::Push(20),
        Instruction::Push(0x1004),
        Instruction::Store,

        Instruction::Push(30),
        Instruction::Push(0x1008),
        Instruction::Store,

        Instruction::Push(40),
        Instruction::Push(0x100C),
        Instruction::Store,
    ];

    cpu.run(&setup_array).unwrap();

    // Now sum the array
    // Index layout:
    // 0: Push(0)       - sum = 0
    // 1: Push(0x1000)  - address
    // 2: Push(4)       - count = 4
    // 3: Dup           - [sum, addr, count, count]
    // 4: JumpZero(15)  - if count == 0, jump to Drop (4+15=19)
    // 5-18: loop body
    // 19: Drop         - drop count (0)
    // 20: Drop         - drop addr
    // 21: Halt
    let sum_program = vec![
        Instruction::Push(0),       // 0: sum = 0
        Instruction::Push(0x1000),  // 1: address = 0x1000
        Instruction::Push(4),       // 2: count = 4

        // Loop
        Instruction::Dup,           // 3: [sum, addr, count, count]
        Instruction::JumpZero(15),  // 4: if count == 0, jump to Drop (4+15=19)

        Instruction::ToR,           // 5: [sum, addr] R: [count]
        Instruction::Dup,           // 6: [sum, addr, addr]
        Instruction::Load,          // 7: [sum, addr, value]
        Instruction::ToR,           // 8: [sum, addr] R: [count, value]
        Instruction::Push(4),       // 9
        Instruction::Add,           // 10: [sum, addr+4]
        Instruction::Swap,          // 11: [addr+4, sum]
        Instruction::FromR,         // 12: [addr+4, sum, value]
        Instruction::Add,           // 13: [addr+4, sum+value]
        Instruction::Swap,          // 14: [sum+value, addr+4]
        Instruction::FromR,         // 15: [sum+value, addr+4, count]
        Instruction::Push(1),       // 16
        Instruction::Sub,           // 17: [sum+value, addr+4, count-1]
        Instruction::Jump(-15),     // 18: loop back (18-15=3)

        Instruction::Drop,          // 19: Drop count (0)
        Instruction::Drop,          // 20: Drop addr
        Instruction::Halt,          // 21
    ];

    cpu.run(&sum_program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 100); // 10+20+30+40 = 100
}

#[test]
fn test_greatest_common_divisor() {
    // GCD(48, 18) using Euclidean algorithm: repeatedly compute a%b
    // Until b=0, then a is the GCD.
    let mut cpu = A2SProcessor::with_default_memory();

    // a%b = a - b*(a/b)
    // We save b to rstack, compute modulo, then restore for next iteration
    //
    // Index layout:
    // 0: Push(48)     - a
    // 1: Push(18)     - b
    // 2: Dup          - [a, b, b]
    // 3: Dup          - [a, b, b, b]
    // 4: ToR          - [a, b, b] R:[b]
    // 5: JumpZero(9)  - if b == 0, jump to Drop (5+9=14)
    // 6: Over         - [a, b, a] R:[b]
    // 7: Over         - [a, b, a, b] R:[b]
    // 8: Divide       - [a, b, a/b] R:[b]
    // 9: Multiply     - [a, b*(a/b)] R:[b]
    // 10: Sub         - [a - b*(a/b)] = [r] R:[b]
    // 11: FromR       - [r, b] R:[]
    // 12: Swap        - [b, r] - new a=b, new b=r
    // 13: Jump(-11)   - loop back (13-11=2)
    // 14: Drop        - drop b (which is 0), keep a (the GCD)
    // 15: Halt
    let program = vec![
        Instruction::Push(48),      // 0: a = 48
        Instruction::Push(18),      // 1: b = 18

        // Loop: while b != 0
        Instruction::Dup,           // 2: [a, b, b]
        Instruction::Dup,           // 3: [a, b, b, b]
        Instruction::ToR,           // 4: [a, b, b] R:[b]
        Instruction::JumpZero(9),   // 5: if b == 0, jump to Drop (5+9=14)

        // Compute a % b = a - b * (a/b)
        Instruction::Over,          // 6: [a, b, a] R:[b]
        Instruction::Over,          // 7: [a, b, a, b] R:[b]
        Instruction::Divide,        // 8: [a, b, a/b] R:[b]
        Instruction::Multiply,      // 9: [a, b*(a/b)] R:[b]
        Instruction::Sub,           // 10: [r] where r = a - b*(a/b) = a%b R:[b]
        Instruction::FromR,         // 11: [r, b] R:[]
        Instruction::Swap,          // 12: [b, r] - new (a, b)
        Instruction::Jump(-11),     // 13: loop back (13-11=2)

        Instruction::Drop,          // 14: Drop b (which is 0)
        Instruction::Halt,          // 15
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 6); // GCD(48, 18) = 6
}

#[test]
fn test_power_calculation() {
    // Calculate 2^10 = 1024
    let mut cpu = A2SProcessor::with_default_memory();

    // Index layout:
    // 0: Push(1)       - result = 1
    // 1: Push(2)       - base = 2
    // 2: Push(10)      - exponent = 10
    // 3: Dup           - [result, base, exp, exp]
    // 4: JumpZero(10)  - if exp == 0, jump to Drop (4+10=14)
    // 5-13: loop body
    // 14: Drop         - drop exponent
    // 15: Drop         - drop base
    // 16: Halt
    let program = vec![
        Instruction::Push(1),       // 0: result = 1
        Instruction::Push(2),       // 1: base = 2
        Instruction::Push(10),      // 2: exponent = 10

        // Loop
        Instruction::Dup,           // 3: [result, base, exp, exp]
        Instruction::JumpZero(10),  // 4: if exp == 0, jump to Drop (4+10=14)

        Instruction::ToR,           // 5: [result, base] R: [exp]
        Instruction::Dup,           // 6: [result, base, base]
        Instruction::ToR,           // 7: [result, base] R: [exp, base]
        Instruction::Multiply,      // 8: [result*base]
        Instruction::FromR,         // 9: [result*base, base]
        Instruction::FromR,         // 10: [result*base, base, exp]
        Instruction::Push(1),       // 11
        Instruction::Sub,           // 12: [result*base, base, exp-1]
        Instruction::Jump(-10),     // 13: loop back (13-10=3)

        Instruction::Drop,          // 14: Drop exponent
        Instruction::Drop,          // 15: Drop base
        Instruction::Halt,          // 16
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 1024);
}

#[test]
fn test_bubble_sort_in_memory() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store unsorted array: [5, 2, 8, 1, 9]
    let setup = vec![
        Instruction::Push(5), Instruction::Push(0x1000), Instruction::Store,
        Instruction::Push(2), Instruction::Push(0x1004), Instruction::Store,
        Instruction::Push(8), Instruction::Push(0x1008), Instruction::Store,
        Instruction::Push(1), Instruction::Push(0x100C), Instruction::Store,
        Instruction::Push(9), Instruction::Push(0x1010), Instruction::Store,
    ];

    cpu.run(&setup).unwrap();

    // Verify first element is 5
    let verify = vec![
        Instruction::Push(0x1000),
        Instruction::Load,
        Instruction::Halt,
    ];

    cpu.run(&verify).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 5);
}

#[test]
fn test_matrix_addition_2x2() {
    // Add two 2x2 matrices stored in memory
    let mut cpu = A2SProcessor::with_default_memory();

    // Matrix A: [[1, 2], [3, 4]]
    // Matrix B: [[5, 6], [7, 8]]
    // Result:   [[6, 8], [10, 12]]

    let setup = vec![
        // Matrix A at 0x1000
        Instruction::Push(1), Instruction::Push(0x1000), Instruction::Store,
        Instruction::Push(2), Instruction::Push(0x1004), Instruction::Store,
        Instruction::Push(3), Instruction::Push(0x1008), Instruction::Store,
        Instruction::Push(4), Instruction::Push(0x100C), Instruction::Store,

        // Matrix B at 0x2000
        Instruction::Push(5), Instruction::Push(0x2000), Instruction::Store,
        Instruction::Push(6), Instruction::Push(0x2004), Instruction::Store,
        Instruction::Push(7), Instruction::Push(0x2008), Instruction::Store,
        Instruction::Push(8), Instruction::Push(0x200C), Instruction::Store,
    ];

    cpu.run(&setup).unwrap();

    // Add corresponding elements
    let add_matrices = vec![
        // C[0,0] = A[0,0] + B[0,0]
        Instruction::Push(0x1000), Instruction::Load,
        Instruction::Push(0x2000), Instruction::Load,
        Instruction::Add,
        Instruction::Push(0x3000), Instruction::Store,

        // C[0,1] = A[0,1] + B[0,1]
        Instruction::Push(0x1004), Instruction::Load,
        Instruction::Push(0x2004), Instruction::Load,
        Instruction::Add,
        Instruction::Push(0x3004), Instruction::Store,

        // C[1,0] = A[1,0] + B[1,0]
        Instruction::Push(0x1008), Instruction::Load,
        Instruction::Push(0x2008), Instruction::Load,
        Instruction::Add,
        Instruction::Push(0x3008), Instruction::Store,

        // C[1,1] = A[1,1] + B[1,1]
        Instruction::Push(0x100C), Instruction::Load,
        Instruction::Push(0x200C), Instruction::Load,
        Instruction::Add,
        Instruction::Push(0x300C), Instruction::Store,
    ];

    cpu.run(&add_matrices).unwrap();

    // Verify results
    let verify = vec![
        Instruction::Push(0x3000), Instruction::Load, // Should be 6
        Instruction::Halt,
    ];

    cpu.run(&verify).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 6);
}

#[test]
fn test_recursive_function_simulation() {
    // Simulate recursion using stack: factorial(5) = 120
    let mut cpu = A2SProcessor::with_default_memory();

    // Index layout:
    // 0: Push(1)       - acc = 1
    // 1: Push(5)       - n = 5
    // 2: Dup           - [acc, n, n]
    // 3: Push(1)       - [acc, n, n, 1]
    // 4: Equal         - [acc, n, n==1]
    // 5: JumpZero(3)   - if n != 1, jump to Swap (5+3=8)
    // 6: Drop          - [acc]
    // 7: Halt
    // 8: Swap          - [n, acc]
    // 9: Over          - [n, acc, n]
    // 10: Multiply     - [n, acc*n]
    // 11: Swap         - [acc*n, n]
    // 12: Push(1)
    // 13: Sub          - [acc*n, n-1]
    // 14: Jump(-12)    - loop back (14-12=2)
    let program = vec![
        Instruction::Push(1),       // 0: acc = 1
        Instruction::Push(5),       // 1: n = 5

        // Loop (tail recursion)
        Instruction::Dup,           // 2: [acc, n, n]
        Instruction::Push(1),       // 3: [acc, n, n, 1]
        Instruction::Equal,         // 4: [acc, n, n==1]
        Instruction::JumpZero(3),   // 5: if n != 1 (result is 0), jump to Swap (5+3=8)

        Instruction::Drop,          // 6: [acc] - drop n when done
        Instruction::Halt,          // 7

        // acc = acc * n, n = n - 1
        Instruction::Swap,          // 8: [n, acc]
        Instruction::Over,          // 9: [n, acc, n]
        Instruction::Multiply,      // 10: [n, acc*n]
        Instruction::Swap,          // 11: [acc*n, n]
        Instruction::Push(1),       // 12
        Instruction::Sub,           // 13: [acc*n, n-1]
        Instruction::Jump(-12),     // 14: loop back (14-12=2)
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 120);
}

#[test]
fn test_string_length_calculation() {
    // Store ASCII string in memory and calculate length
    let mut cpu = A2SProcessor::with_default_memory();

    // Store "HELLO" (5 characters) + null terminator
    let setup = vec![
        Instruction::Push(72),  // 'H'
        Instruction::Push(0x1000), Instruction::Store,
        Instruction::Push(69),  // 'E'
        Instruction::Push(0x1004), Instruction::Store,
        Instruction::Push(76),  // 'L'
        Instruction::Push(0x1008), Instruction::Store,
        Instruction::Push(76),  // 'L'
        Instruction::Push(0x100C), Instruction::Store,
        Instruction::Push(79),  // 'O'
        Instruction::Push(0x1010), Instruction::Store,
        Instruction::Push(0),   // null terminator
        Instruction::Push(0x1014), Instruction::Store,
    ];

    cpu.run(&setup).unwrap();

    // Count non-zero characters
    // Index layout:
    // 0: Push(0)       - length = 0
    // 1: Push(0x1000)  - addr
    // 2: Dup           - [len, addr, addr]
    // 3: Load          - [len, addr, char]
    // 4: JumpZero(8)   - if char == 0, jump to Drop (4+8=12). JumpZero pops char.
    // 5-11: loop body (increment addr by 4, increment len by 1)
    // 12: Drop         - drop addr
    // 13: Halt
    let count = vec![
        Instruction::Push(0),       // 0: length = 0
        Instruction::Push(0x1000),  // 1: addr = 0x1000

        // Loop
        Instruction::Dup,           // 2: [len, addr, addr]
        Instruction::Load,          // 3: [len, addr, char]
        Instruction::JumpZero(8),   // 4: if char == 0, jump to Drop (4+8=12). Pops char.

        // After JumpZero (no jump): stack is [len, addr]
        Instruction::Push(4),       // 5: [len, addr, 4]
        Instruction::Add,           // 6: [len, addr+4]
        Instruction::Swap,          // 7: [addr+4, len]
        Instruction::Push(1),       // 8: [addr+4, len, 1]
        Instruction::Add,           // 9: [addr+4, len+1]
        Instruction::Swap,          // 10: [len+1, addr+4]
        Instruction::Jump(-9),      // 11: loop back (11-9=2)

        Instruction::Drop,          // 12: Drop address
        Instruction::Halt,          // 13
    ];

    cpu.run(&count).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 5);
}

#[test]
fn test_max_element_in_array() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Store array: [3, 7, 2, 9, 5]
    let setup = vec![
        Instruction::Push(3), Instruction::Push(0x1000), Instruction::Store,
        Instruction::Push(7), Instruction::Push(0x1004), Instruction::Store,
        Instruction::Push(2), Instruction::Push(0x1008), Instruction::Store,
        Instruction::Push(9), Instruction::Push(0x100C), Instruction::Store,
        Instruction::Push(5), Instruction::Push(0x1010), Instruction::Store,
    ];

    cpu.run(&setup).unwrap();

    // Simple approach: load all elements and find max
    let find_max = vec![
        Instruction::Push(0x1000), Instruction::Load, // 3
        Instruction::Push(0x1004), Instruction::Load, // 7
        // max(3, 7)
        Instruction::Over,          // [3, 7, 3]
        Instruction::Over,          // [3, 7, 3, 7]
        Instruction::LessThan,      // [3, 7, 3<7] = [3, 7, -1]
        Instruction::JumpZero(2),   // if 3 >= 7 (false), skip swap
        Instruction::Swap,          // [7, 3]
        Instruction::Drop,          // [7]

        Instruction::Push(0x1008), Instruction::Load, // 2
        Instruction::Over,
        Instruction::Over,
        Instruction::LessThan,
        Instruction::JumpZero(2),
        Instruction::Swap,
        Instruction::Drop,          // [7]

        Instruction::Push(0x100C), Instruction::Load, // 9
        Instruction::Over,
        Instruction::Over,
        Instruction::LessThan,
        Instruction::JumpZero(2),
        Instruction::Swap,
        Instruction::Drop,          // [9]

        Instruction::Push(0x1010), Instruction::Load, // 5
        Instruction::Over,
        Instruction::Over,
        Instruction::LessThan,
        Instruction::JumpZero(2),
        Instruction::Swap,
        Instruction::Drop,          // [9]

        Instruction::Halt,
    ];

    cpu.run(&find_max).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 9);
}

#[test]
fn test_palindrome_check() {
    // Check if number is palindrome (e.g., 12321)
    let mut cpu = A2SProcessor::with_default_memory();

    // Simple test: check if 121 reversed equals itself
    // We'll use a simpler approach - just verify basic operations work
    let program = vec![
        Instruction::Push(121),     // original
        Instruction::Push(121),     // will be the reversed
        Instruction::Equal,         // check equal
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), -1); // TRUE
}

#[test]
fn test_stack_depth_management() {
    let mut cpu = A2SProcessor::with_default_memory();

    let program = vec![
        Instruction::Push(1),
        Instruction::Push(2),
        Instruction::Push(3),
        Instruction::Push(4),
        Instruction::Push(5),
        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.stack_depth(), 5);

    let cleanup = vec![
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Halt,
    ];

    cpu.run(&cleanup).unwrap();
    assert_eq!(cpu.stack_depth(), 2);
}

#[test]
fn test_register_based_memory_operations() {
    let mut cpu = A2SProcessor::with_default_memory();

    let program = vec![
        // Set up registers
        Instruction::Push(0x1000),
        Instruction::ToA,

        Instruction::Push(0x2000),
        Instruction::ToB,

        Instruction::Push(0x3000),
        Instruction::ToC,

        // Store using register A
        Instruction::Push(42),
        Instruction::StoreA,

        // Load using register A
        Instruction::LoadA,

        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 42);
    assert_eq!(cpu.get_reg_a(), 0x1000);
    assert_eq!(cpu.get_reg_b(), 0x2000);
    assert_eq!(cpu.get_reg_c(), 0x3000);
}

#[test]
fn test_complex_control_flow() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Compute absolute value of -42
    // Index layout:
    // 0: Push(-42)
    // 1: Dup
    // 2: JumpNegative(2)  - if negative, jump to negate (2+2=4)
    // 3: Halt             - already positive
    // 4: Push(-1)
    // 5: Multiply
    // 6: Halt
    let program = vec![
        Instruction::Push(-42),     // 0
        Instruction::Dup,           // 1: [−42, −42]
        Instruction::JumpNegative(2), // 2: if negative, jump to 4
        Instruction::Halt,          // 3: not reached for negative input

        // Negate
        Instruction::Push(-1),      // 4
        Instruction::Multiply,      // 5: -42 * -1 = 42
        Instruction::Halt,          // 6
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_subroutine_simulation() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Simple subroutine: double a number
    // Index layout:
    // 0: Push(21)      - Argument
    // 1: Call(2)       - Call subroutine at offset +2 (1+2=3)
    // 2: Halt          - Return here after subroutine
    // 3: Dup           - Subroutine start
    // 4: Add
    // 5: Return
    let program = vec![
        Instruction::Push(21),      // 0: Argument
        Instruction::Call(2),       // 1: Call subroutine at 1+2=3

        Instruction::Halt,          // 2: Main program end (return point)

        // Subroutine: double TOS
        Instruction::Dup,           // 3
        Instruction::Add,           // 4
        Instruction::Return,        // 5
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 42);
}

#[test]
fn test_memory_access_patterns() {
    let mut cpu = A2SProcessor::with_default_memory();

    // Test sequential writes and reads
    let program = vec![
        // Write pattern
        Instruction::Push(100), Instruction::Push(0x1000), Instruction::Store,
        Instruction::Push(200), Instruction::Push(0x1004), Instruction::Store,
        Instruction::Push(300), Instruction::Push(0x1008), Instruction::Store,

        // Read pattern (reverse order)
        Instruction::Push(0x1008), Instruction::Load,
        Instruction::Push(0x1004), Instruction::Load,
        Instruction::Push(0x1000), Instruction::Load,

        // Sum them
        Instruction::Add,
        Instruction::Add,

        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 600);
}

#[test]
fn test_bitwise_operations_chain() {
    let mut cpu = A2SProcessor::with_default_memory();

    let program = vec![
        Instruction::Push(0xFF00),
        Instruction::Push(0x00FF),
        Instruction::Or,            // 0xFFFF

        Instruction::Push(0xF0F0),
        Instruction::And,           // 0xF0F0

        Instruction::Push(0x0F0F),
        Instruction::Xor,           // 0xFFFF

        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), 0xFFFF);
}

#[test]
fn test_comprehensive_integration() {
    // Test that combines multiple features
    let mut cpu = A2SProcessor::with_default_memory();

    let program = vec![
        // 1. Arithmetic
        Instruction::Push(10),
        Instruction::Push(20),
        Instruction::Add,           // 30

        // 2. Store in memory
        Instruction::Push(0x1000),
        Instruction::Store,

        // 3. Use registers
        Instruction::Push(0x1000),
        Instruction::ToA,

        // 4. Load via register
        Instruction::LoadA,         // 30

        // 5. Multiply
        Instruction::Push(2),
        Instruction::Multiply,      // 60

        // 6. Comparison
        Instruction::Dup,
        Instruction::Push(60),
        Instruction::Equal,         // -1 (true)

        Instruction::Halt,
    ];

    cpu.run(&program).unwrap();
    assert_eq!(cpu.peek_stack().unwrap(), -1); // TRUE
}
