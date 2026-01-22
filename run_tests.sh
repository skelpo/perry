#!/bin/bash
# Perry Test Runner
# Runs all test files in test-files/ directory

# Don't exit on first error - we want to run all tests

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEST_DIR="$SCRIPT_DIR/test-files"
OUTPUT_DIR="/tmp/perry_tests"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Tests to skip (known issues or special handling needed)
SKIP_TESTS=(
    # Add any tests that need special handling here
)

# Function to check if test should be skipped
should_skip() {
    local test_name=$1
    for skip in "${SKIP_TESTS[@]}"; do
        if [[ "$test_name" == "$skip" ]]; then
            return 0
        fi
    done
    return 1
}

echo "========================================"
echo "   Perry Test Runner"
echo "========================================"
echo ""

# Build the compiler first
echo "Building compiler..."
cargo build --quiet 2>/dev/null || {
    echo -e "${RED}Failed to build compiler${NC}"
    exit 1
}
echo -e "${GREEN}Compiler built successfully${NC}"
echo ""

# Track failed tests for summary
declare -a FAILED_TESTS=()

# Run each test
for test_file in "$TEST_DIR"/*.ts; do
    test_name=$(basename "$test_file" .ts)
    output_file="$OUTPUT_DIR/$test_name"

    # Check if test should be skipped
    if should_skip "$test_name"; then
        echo -e "${YELLOW}SKIP${NC}  $test_name"
        ((SKIPPED++))
        continue
    fi

    # Compile the test (suppress warnings)
    if ! cargo run --quiet -- "$test_file" -o "$output_file" 2>/dev/null; then
        # Try again to get error message
        compile_output=$(cargo run --quiet -- "$test_file" -o "$output_file" 2>&1 | grep -i "error" | head -3)
        echo -e "${RED}FAIL${NC}  $test_name (compile error)"
        if [[ -n "$compile_output" ]]; then
            echo "       $compile_output"
        fi
        ((FAILED++))
        FAILED_TESTS+=("$test_name (compile)")
        continue
    fi

    # Run the test
    run_output=$("$output_file" 2>&1)
    run_status=$?

    if [[ $run_status -ne 0 ]]; then
        echo -e "${RED}FAIL${NC}  $test_name (runtime error: $run_status)"
        echo "       Output: $run_output" | head -3
        ((FAILED++))
        FAILED_TESTS+=("$test_name (runtime)")
    else
        echo -e "${GREEN}PASS${NC}  $test_name"
        ((PASSED++))
    fi
done

# Summary
echo ""
echo "========================================"
echo "   Test Summary"
echo "========================================"
echo -e "${GREEN}Passed:${NC}  $PASSED"
echo -e "${RED}Failed:${NC}  $FAILED"
echo -e "${YELLOW}Skipped:${NC} $SKIPPED"
echo "Total:   $((PASSED + FAILED + SKIPPED))"
echo ""

# List failed tests
if [[ ${#FAILED_TESTS[@]} -gt 0 ]]; then
    echo "Failed tests:"
    for failed in "${FAILED_TESTS[@]}"; do
        echo "  - $failed"
    done
fi

# Exit with error if any tests failed
if [[ $FAILED -gt 0 ]]; then
    exit 1
fi
