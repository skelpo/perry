#!/bin/bash

# Performance Benchmark Suite: Node.js vs Bun vs Perry
# This script runs benchmarks across all three runtimes and compares results

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$SCRIPT_DIR/results"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Benchmark files
BENCHMARKS="bench_fibonacci bench_array_ops bench_string_ops bench_bitwise"

# Temporary files for results
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo -e "${BLUE}========================================"
echo "  Performance Benchmark Suite"
echo "  Node.js vs Bun vs Perry"
echo -e "========================================${NC}"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

check_command() {
  if ! command -v "$1" &> /dev/null; then
    echo -e "${RED}Error: $1 is not installed or not in PATH${NC}"
    return 1
  fi
  echo -e "  ${GREEN}✓${NC} $1 found: $($1 --version 2>&1 | head -n1)"
  return 0
}

MISSING=0
check_command "node" || MISSING=1
check_command "bun" || MISSING=1
check_command "cargo" || MISSING=1

if [ $MISSING -eq 1 ]; then
  echo -e "${RED}Please install missing prerequisites and try again.${NC}"
  exit 1
fi

echo ""

# Build Perry compiler
echo -e "${YELLOW}Building Perry compiler...${NC}"
cd "$PROJECT_DIR"
cargo build --release -p perry 2>&1 | tail -n5
cargo build --release -p perry-runtime 2>&1 | tail -n5
echo -e "${GREEN}✓${NC} Perry built successfully"
echo ""

# Pre-compile all benchmarks to native binaries
echo -e "${YELLOW}Compiling benchmarks to native binaries...${NC}"
cd "$SCRIPT_DIR"

for bench in $BENCHMARKS; do
  echo -n "  Compiling ${bench}.ts... "
  if "$PROJECT_DIR/target/release/perry" "${bench}.ts" -o "${bench}_native" 2>&1; then
    echo -e "${GREEN}OK${NC}"
  else
    echo -e "${RED}FAILED${NC}"
    echo -e "${YELLOW}Warning: Compilation failed for ${bench}.ts - will skip native binary tests${NC}"
  fi
done

echo ""

# Function to parse benchmark output
parse_output() {
  local output="$1"
  echo "$output" | grep "^TOTAL:" | cut -d: -f2
}

# Function to run a benchmark with a specific runtime
run_benchmark() {
  local runtime="$1"
  local bench="$2"
  local result=""

  case "$runtime" in
    "node")
      result=$(node --experimental-strip-types "${bench}.ts" 2>/dev/null) || true
      ;;
    "bun")
      result=$(bun run "${bench}.ts" 2>/dev/null) || true
      ;;
    "perry")
      if [ -f "${bench}_native" ]; then
        result=$("./${bench}_native" 2>/dev/null) || true
      else
        echo "SKIP"
        return
      fi
      ;;
  esac

  local total=$(parse_output "$result")
  if [ -n "$total" ]; then
    echo "$total"
  else
    echo "ERROR"
  fi
}

# Run benchmarks
echo -e "${YELLOW}Running benchmarks...${NC}"
echo ""

for bench in $BENCHMARKS; do
  echo -e "${BLUE}>>> ${bench}${NC}"

  # Node.js
  echo -n "    Node.js:   "
  node_result=$(run_benchmark "node" "$bench")
  echo "$node_result" > "$TEMP_DIR/${bench}_node"
  if [ "$node_result" = "ERROR" ]; then
    echo -e "${RED}ERROR${NC}"
  else
    echo -e "${GREEN}${node_result}ms${NC}"
  fi

  # Bun
  echo -n "    Bun:       "
  bun_result=$(run_benchmark "bun" "$bench")
  echo "$bun_result" > "$TEMP_DIR/${bench}_bun"
  if [ "$bun_result" = "ERROR" ]; then
    echo -e "${RED}ERROR${NC}"
  else
    echo -e "${GREEN}${bun_result}ms${NC}"
  fi

  # Perry
  echo -n "    Perry: "
  comp_result=$(run_benchmark "perry" "$bench")
  echo "$comp_result" > "$TEMP_DIR/${bench}_perry"
  if [ "$comp_result" = "ERROR" ] || [ "$comp_result" = "SKIP" ]; then
    echo -e "${RED}${comp_result}${NC}"
  else
    echo -e "${GREEN}${comp_result}ms${NC}"
  fi

  echo ""
done

# Print results summary
echo -e "${BLUE}========================================"
echo "   Results Summary"
echo -e "========================================${NC}"
echo ""

# Print header
printf "%-20s %12s %12s %12s\n" "Benchmark" "Node.js" "Bun" "Perry"
printf "%-20s %12s %12s %12s\n" "---------" "-------" "---" "---------"

for bench in $BENCHMARKS; do
  node_val=$(cat "$TEMP_DIR/${bench}_node")
  bun_val=$(cat "$TEMP_DIR/${bench}_bun")
  comp_val=$(cat "$TEMP_DIR/${bench}_perry")

  # Format values with "ms" suffix if numeric
  node_display="$node_val"
  bun_display="$bun_val"
  comp_display="$comp_val"

  case "$node_val" in
    ''|*[!0-9]*) ;; # not a number
    *) node_display="${node_val}ms" ;;
  esac

  case "$bun_val" in
    ''|*[!0-9]*) ;; # not a number
    *) bun_display="${bun_val}ms" ;;
  esac

  case "$comp_val" in
    ''|*[!0-9]*) ;; # not a number
    *) comp_display="${comp_val}ms" ;;
  esac

  printf "%-20s %12s %12s %12s\n" "$bench" "$node_display" "$bun_display" "$comp_display"
done

echo ""
echo -e "${BLUE}Speedup vs Node.js:${NC}"

for bench in $BENCHMARKS; do
  node_val=$(cat "$TEMP_DIR/${bench}_node")
  comp_val=$(cat "$TEMP_DIR/${bench}_perry")

  case "$node_val" in
    ''|*[!0-9]*) echo "  ${bench}: N/A"; continue ;;
  esac

  case "$comp_val" in
    ''|*[!0-9]*) echo "  ${bench}: N/A"; continue ;;
  esac

  if [ "$comp_val" -gt 0 ] 2>/dev/null; then
    speedup=$(echo "scale=2; $node_val / $comp_val" | bc)
    echo "  ${bench}: ${speedup}x"
  else
    echo "  ${bench}: N/A"
  fi
done

echo ""
echo -e "${BLUE}Speedup vs Bun:${NC}"

for bench in $BENCHMARKS; do
  bun_val=$(cat "$TEMP_DIR/${bench}_bun")
  comp_val=$(cat "$TEMP_DIR/${bench}_perry")

  case "$bun_val" in
    ''|*[!0-9]*) echo "  ${bench}: N/A"; continue ;;
  esac

  case "$comp_val" in
    ''|*[!0-9]*) echo "  ${bench}: N/A"; continue ;;
  esac

  if [ "$comp_val" -gt 0 ] 2>/dev/null; then
    speedup=$(echo "scale=2; $bun_val / $comp_val" | bc)
    echo "  ${bench}: ${speedup}x"
  else
    echo "  ${bench}: N/A"
  fi
done

# Save results to file
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="$RESULTS_DIR/benchmark_results_${TIMESTAMP}.txt"

mkdir -p "$RESULTS_DIR"

{
  echo "Performance Benchmark Results"
  echo "Date: $(date)"
  echo "Node.js: $(node --version)"
  echo "Bun: $(bun --version)"
  echo ""
  printf "%-20s %12s %12s %12s\n" "Benchmark" "Node.js" "Bun" "Perry"
  printf "%-20s %12s %12s %12s\n" "---------" "-------" "---" "---------"

  for bench in $BENCHMARKS; do
    node_val=$(cat "$TEMP_DIR/${bench}_node")
    bun_val=$(cat "$TEMP_DIR/${bench}_bun")
    comp_val=$(cat "$TEMP_DIR/${bench}_perry")

    node_display="$node_val"
    bun_display="$bun_val"
    comp_display="$comp_val"

    case "$node_val" in
      ''|*[!0-9]*) ;;
      *) node_display="${node_val}ms" ;;
    esac

    case "$bun_val" in
      ''|*[!0-9]*) ;;
      *) bun_display="${bun_val}ms" ;;
    esac

    case "$comp_val" in
      ''|*[!0-9]*) ;;
      *) comp_display="${comp_val}ms" ;;
    esac

    printf "%-20s %12s %12s %12s\n" "$bench" "$node_display" "$bun_display" "$comp_display"
  done
} > "$RESULTS_FILE"

echo ""
echo -e "${GREEN}Results saved to: ${RESULTS_FILE}${NC}"
echo ""

# Cleanup native binaries
echo -e "${YELLOW}Cleaning up...${NC}"
for bench in $BENCHMARKS; do
  rm -f "${bench}_native"
done
echo -e "${GREEN}Done!${NC}"
