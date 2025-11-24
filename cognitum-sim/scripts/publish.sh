#!/bin/bash
set -e

# Publish script for WASM and NAPI packages
# Usage: ./scripts/publish.sh [--dry-run]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

DRY_RUN=""
if [ "$1" = "--dry-run" ]; then
  DRY_RUN="--dry-run"
  echo "DRY RUN MODE - No packages will be published"
fi

echo "=================================================="
echo "Publishing Newport Packages"
echo "=================================================="

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_status() {
  echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
  echo -e "${RED}[✗]${NC} $1"
}

print_info() {
  echo -e "${YELLOW}[i]${NC} $1"
}

# Check if logged in to npm
if ! npm whoami &> /dev/null; then
  print_error "Not logged in to npm. Run 'npm login' first"
  exit 1
fi

print_status "Logged in to npm as $(npm whoami)"

# Publish WASM
echo ""
print_info "Publishing WASM package..."
cd "$PROJECT_ROOT/newport-wasm"

if [ ! -d "pkg" ]; then
  print_error "WASM package not built. Run './scripts/build-all.sh --release' first"
  exit 1
fi

cd pkg
if [ "$DRY_RUN" = "--dry-run" ]; then
  npm publish --dry-run --access public
  print_info "WASM package dry-run complete"
else
  npm publish --access public
  print_status "WASM package published to npm"
fi

# Publish NAPI
echo ""
print_info "Publishing NAPI package..."
cd "$PROJECT_ROOT/newport-napi"

if [ ! -f "*.node" ]; then
  print_error "NAPI package not built. Run './scripts/build-all.sh --release' first"
  exit 1
fi

if [ "$DRY_RUN" = "--dry-run" ]; then
  npm publish --dry-run --access public
  print_info "NAPI package dry-run complete"
else
  npm publish --access public
  print_status "NAPI package published to npm"
fi

# Summary
echo ""
if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "=================================================="
  echo -e "${YELLOW}Dry Run Complete - No packages published${NC}"
  echo "=================================================="
  echo ""
  print_info "Run without --dry-run to publish for real"
else
  echo "=================================================="
  echo -e "${GREEN}Packages Published Successfully!${NC}"
  echo "=================================================="
  echo ""
  echo "Published packages:"
  echo "  - @ruv/newport-wasm"
  echo "  - @ruv/newport"
  echo ""
  print_info "Packages should be available on npm shortly"
fi
