#!/bin/bash
set -e

# Test script for both WASM and NAPI bindings
# Usage: ./scripts/test-all.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=================================================="
echo "Testing Newport WASM and NAPI Bindings"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_status() {
  echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
  echo -e "${RED}[✗]${NC} $1"
}

print_info() {
  echo -e "${YELLOW}[i]${NC} $1"
}

# Test WASM
echo ""
print_info "Testing WASM bindings..."
cd "$PROJECT_ROOT/newport-wasm"

if command -v wasm-pack &> /dev/null; then
  if command -v chrome &> /dev/null || command -v google-chrome &> /dev/null; then
    print_info "Running tests in Chrome..."
    wasm-pack test --headless --chrome
    print_status "WASM Chrome tests passed"
  else
    print_error "Chrome not found, skipping browser tests"
  fi

  if command -v firefox &> /dev/null; then
    print_info "Running tests in Firefox..."
    wasm-pack test --headless --firefox
    print_status "WASM Firefox tests passed"
  else
    print_info "Firefox not found, skipping Firefox tests"
  fi
else
  print_error "wasm-pack not found. Run './scripts/build-all.sh' first"
  exit 1
fi

# Test NAPI
echo ""
print_info "Testing NAPI bindings..."
cd "$PROJECT_ROOT/newport-napi"

if [ ! -f "*.node" ]; then
  print_error "Native module not found. Run './scripts/build-all.sh' first"
  exit 1
fi

print_info "Running Node.js tests..."
yarn test
print_status "NAPI tests passed"

# Summary
echo ""
echo "=================================================="
echo -e "${GREEN}All Tests Passed!${NC}"
echo "=================================================="
