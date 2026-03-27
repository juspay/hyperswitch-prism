#!/bin/bash
# Pre-Push Validation Script
# Run this before pushing code to ensure everything works

set -e

WITH_TESTS=false
FIX_FORMAT=false
VERBOSE=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --with-tests)
      WITH_TESTS=true
      shift
      ;;
    --fix)
      FIX_FORMAT=true
      shift
      ;;
    --verbose|-v)
      VERBOSE=true
      shift
      ;;
    --help|-h)
      echo "Usage: pre-push.sh [--with-tests] [--fix] [--verbose]"
      echo ""
      echo "Options:"
      echo "  --with-tests    Run cargo test (slower but more thorough)"
      echo "  --fix          Auto-fix formatting issues"
      echo "  --verbose, -v  Show detailed output"
      echo "  --help, -h     Show this help message"
      exit 0
      ;;
    *)
      echo -e "${RED}Unknown option: $1${NC}"
      echo "Usage: pre-push.sh [--with-tests] [--fix] [--verbose]"
      exit 1
      ;;
  esac
done

# Helper function for verbose output
v_echo() {
    if [ "$VERBOSE" = true ]; then
        echo "$@"
    fi
}

echo -e "${BLUE}🔍 Running pre-push validation...${NC}"
echo ""

FAILED=0
TOTAL=6
CURRENT=0

# Function to print step header
print_step() {
    CURRENT=$((CURRENT + 1))
    echo -e "${BLUE}[$CURRENT/$TOTAL]${NC} $1"
}

# Function to print success
print_success() {
    echo -e "   ${GREEN}✓${NC} $1"
}

# Function to print error
print_error() {
    echo -e "   ${RED}✗${NC} $1"
}

# Step 1: Check formatting
print_step "Checking code formatting..."
if [ "$FIX_FORMAT" = true ]; then
    v_echo "Running: cargo +nightly fmt --all"
    if cargo +nightly fmt --all 2>&1 | ( [ "$VERBOSE" = true ] && cat || grep -v "^$" ); then
        print_success "Formatting fixed"
    else
        print_success "Formatting fixed (no changes needed)"
    fi
else
    v_echo "Running: cargo +nightly fmt --all --check"
    if cargo +nightly fmt --all --check 2>&1 | grep -q "Diff"; then
        print_error "Formatting issues found"
        echo ""
        echo -e "${YELLOW}💡 Tip:${NC} Run with --fix to auto-fix formatting issues"
        echo ""
        cargo +nightly fmt --all --check 2>&1 | head -20
        exit 3
    else
        print_success "Formatting check passed"
    fi
fi
echo ""

# Step 2: Cargo check
print_step "Running cargo check..."
v_echo "Running: cargo check --all-features --all-targets"
if cargo check --all-features --all-targets 2>&1 | tee /tmp/cargo-check.log | tail -5 | grep -q "Finished"; then
    print_success "Compilation successful"
else
    print_error "Compilation failed"
    echo ""
    grep -E "^error" /tmp/cargo-check.log | head -10
    exit 1
fi
echo ""

# Step 3: Clippy
print_step "Running clippy..."
v_echo "Running: cargo clippy --all-features --all-targets -- -D warnings"
if cargo clippy --all-features --all-targets -- -D warnings 2>&1 | tee /tmp/clippy.log | tail -5 | grep -q "Finished"; then
    print_success "Clippy check passed"
else
    print_error "Clippy found issues"
    echo ""
    grep -E "^error|^warning:" /tmp/clippy.log | head -15
    exit 2
fi
echo ""

# Step 4: Proto generation
print_step "Generating SDK bindings from proto files..."
v_echo "Running: make generate"
if make generate > /tmp/generate.log 2>&1; then
    print_success "SDK bindings generated"
    v_echo ""
    v_echo "Generated files:"
    v_echo "  - Rust FFI flow registrations"
    v_echo "  - Python SDK flows"
    v_echo "  - JavaScript/TypeScript SDK flows"
    v_echo "  - Kotlin SDK flows"
else
    print_error "SDK generation failed"
    echo ""
    tail -30 /tmp/generate.log
    exit 4
fi
echo ""

# Step 5: Documentation
print_step "Generating connector documentation..."
v_echo "Running: make docs"
if make docs > /tmp/docs.log 2>&1; then
    print_success "Documentation generated"
    v_echo ""
    v_echo "Generated documentation for all connectors"
else
    print_error "Documentation generation failed"
    echo ""
    tail -30 /tmp/docs.log
    exit 5
fi
echo ""

# Step 6: Tests (optional)
if [ "$WITH_TESTS" = true ]; then
    print_step "Running tests..."
    v_echo "Running: cargo test --all-features"
    if cargo test --all-features 2>&1 | tee /tmp/test.log | tail -20 | grep -q "test result: ok"; then
        print_success "All tests passed"
    else
        print_error "Tests failed"
        echo ""
        grep -E "^test.*FAILED|^failures:" /tmp/test.log | head -20
        exit 6
    fi
else
    print_step "Skipping tests (use --with-tests to run)"
    echo -e "   ${YELLOW}⏭️${NC} Tests skipped"
fi
echo ""

# Summary
echo -e "${GREEN}🎉 All validation checks passed!${NC}"
echo ""
echo -e "${BLUE}Summary:${NC}"
echo "  ✓ Code formatting"
echo "  ✓ Compilation"
echo "  ✓ Clippy linting"
echo "  ✓ SDK bindings generation"
echo "  ✓ Documentation generation"
if [ "$WITH_TESTS" = true ]; then
    echo "  ✓ Tests"
fi
echo ""
echo -e "${GREEN}You're ready to push!${NC}"
