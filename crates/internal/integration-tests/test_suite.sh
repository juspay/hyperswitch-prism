#!/bin/bash
#
# Test Suite Runner - Run integration tests for a specific connector
#
# Usage:
#   ./test_suite.sh <connector> [suite]
#
# Examples:
#   ./test_suite.sh stripe              # Run all suites for stripe
#   ./test_suite.sh stripe authorize    # Run only authorize suite for stripe
#

set -e

CONNECTOR=$1
SUITE=$2

if [ -z "$CONNECTOR" ]; then
  echo "Usage: $0 <connector> [suite]"
  echo ""
  echo "Available connectors: stripe, adyen, checkout, paypal, etc."
  echo ""
  echo "Available suites:"
  find src/global_suites -name "suite_spec.json" -type f | \
    sed 's|.*/global_suites/||' | sed 's|/suite_spec.json||' | sed 's|_suite$||' | sort
  exit 1
fi

# Define core suites to test
CORE_SUITES=(
  "server_authentication_token"
  "create_customer"
  "authorize"
  "capture"
  "void"
  "refund"
  "get"
  "refund_sync"
  "tokenize_payment_method"
)

# If specific suite provided, test only that
if [ -n "$SUITE" ]; then
  SUITES=("$SUITE")
else
  SUITES=("${CORE_SUITES[@]}")
fi

echo "========================================"
echo "Testing Connector: $CONNECTOR"
echo "Suites: ${SUITES[*]}"
echo "========================================"
echo ""

RESULTS_DIR="test_results_${CONNECTOR}_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

PASSED=0
FAILED=0
SKIPPED=0

for suite in "${SUITES[@]}"; do
  echo "----------------------------------------"
  echo "Testing suite: $suite"
  echo "----------------------------------------"

  LOG_FILE="$RESULTS_DIR/${suite}.log"

  # Check if suite exists
  if [ ! -d "src/global_suites/${suite}_suite" ]; then
    echo "⚠️  SKIPPED - Suite directory not found"
    SKIPPED=$((SKIPPED + 1))
    echo "SKIPPED" > "$LOG_FILE"
    continue
  fi

  # Run the test
  if cargo run --bin suite_run_test -- \
    --suite "$suite" \
    --connector "$CONNECTOR" \
    > "$LOG_FILE" 2>&1; then
    echo "✅ PASSED"
    PASSED=$((PASSED + 1))
  else
    echo "❌ FAILED - See $LOG_FILE for details"
    FAILED=$((FAILED + 1))
    # Show last 20 lines of error
    echo ""
    echo "Last 20 lines of error log:"
    tail -20 "$LOG_FILE"
    echo ""
  fi
done

echo ""
echo "========================================"
echo "TEST SUMMARY"
echo "========================================"
echo "Connector: $CONNECTOR"
echo "Passed:    $PASSED"
echo "Failed:    $FAILED"
echo "Skipped:   $SKIPPED"
echo "Total:     $((PASSED + FAILED + SKIPPED))"
echo ""
echo "Logs saved to: $RESULTS_DIR/"
echo "========================================"

if [ $FAILED -gt 0 ]; then
  exit 1
fi

exit 0
