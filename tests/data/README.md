# Test Data Files

This directory contains test vectors and data files for cross-validation testing.

## Verilog Test Vectors

Test vectors compare Rust simulation against Verilog testbenches:

- `verilog_basic_ops.json` - Basic processor operations
- `verilog_isa_coverage.json` - Full ISA instruction coverage
- `verilog_memory.json` - Memory subsystem validation
- `verilog_raceway.json` - RaceWay packet routing tests
- `verilog_timing.json` - Cycle-accurate timing validation

## Format

Test vectors use JSON format:

```json
{
  "name": "test_name",
  "description": "What this tests",
  "initial_state": {
    "memory": [{"address": 0, "value": 0}],
    "registers": [0, 0, ...],
    "flags": {"zero": false, ...}
  },
  "operations": [
    {"type": "MemoryWrite", "address": 0x100, "value": 0xAA}
  ],
  "expected_state": {
    ...
  }
}
```

## Generating Test Vectors

To generate test vectors from Verilog simulations:

1. Run Verilog testbench with VCD output
2. Extract state transitions
3. Convert to JSON using `scripts/verilog_to_json.py`
4. Place in this directory

## Adding New Test Vectors

1. Create JSON file following the format above
2. Add test case in `tests/verilog_cross_validation.rs`
3. Verify test passes: `cargo test --test verilog_cross_validation`
