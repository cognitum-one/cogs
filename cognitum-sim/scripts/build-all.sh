#!/bin/bash
set -e

# Build script for both WASM and NAPI bindings
# Usage: ./scripts/build-all.sh [--release]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

RELEASE_FLAG=""
if [ "$1" = "--release" ]; then
  RELEASE_FLAG="--release"
  echo "Building in RELEASE mode"
else
  echo "Building in DEBUG mode"
fi

echo "=================================================="
echo "Building Newport WASM and NAPI Bindings"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
  echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
  echo -e "${RED}[✗]${NC} $1"
}

print_info() {
  echo -e "${YELLOW}[i]${NC} $1"
}

# Check prerequisites
print_info "Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
  print_error "Rust/Cargo not found. Please install from https://rustup.rs"
  exit 1
fi

if ! command -v node &> /dev/null; then
  print_error "Node.js not found. Please install from https://nodejs.org"
  exit 1
fi

print_status "Prerequisites OK"

# Build WASM
echo ""
print_info "Building WASM bindings..."
cd "$PROJECT_ROOT/newport-wasm"

if ! command -v wasm-pack &> /dev/null; then
  print_info "Installing wasm-pack..."
  cargo install wasm-pack
fi

print_info "Building for web target..."
wasm-pack build --target web --out-dir pkg

print_info "Building for Node.js target..."
wasm-pack build --target nodejs --out-dir pkg-node

print_info "Building for bundler target..."
wasm-pack build --target bundler --out-dir pkg-bundler

print_status "WASM bindings built successfully"

# Build NAPI
echo ""
print_info "Building NAPI bindings..."
cd "$PROJECT_ROOT/newport-napi"

if [ ! -d "node_modules" ]; then
  print_info "Installing Node.js dependencies..."
  yarn install
fi

print_info "Building native module..."
if [ "$RELEASE_FLAG" = "--release" ]; then
  yarn build
else
  yarn build:debug
fi

print_status "NAPI bindings built successfully"

# Summary
echo ""
echo "=================================================="
echo -e "${GREEN}Build Complete!${NC}"
echo "=================================================="
echo ""
echo "WASM outputs:"
echo "  - Web:     $PROJECT_ROOT/newport-wasm/pkg/"
echo "  - Node.js: $PROJECT_ROOT/newport-wasm/pkg-node/"
echo "  - Bundler: $PROJECT_ROOT/newport-wasm/pkg-bundler/"
echo ""
echo "NAPI outputs:"
echo "  - Native:  $PROJECT_ROOT/newport-napi/*.node"
echo ""
print_info "Run './scripts/test-all.sh' to run tests"
