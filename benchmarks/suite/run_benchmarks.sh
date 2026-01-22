#!/usr/bin/env bash
# Comprehensive benchmark runner for Perry vs Node.js vs Bun

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPILETS="${SCRIPT_DIR}/../../target/release/perry"
RESULTS_DIR="${SCRIPT_DIR}/results"
mkdir -p "$RESULTS_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Check if compilers exist
check_runtime() {
    if command -v "$1" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 found: $(command -v $1)"
        return 0
    else
        echo -e "${YELLOW}✗${NC} $1 not found"
        return 1
    fi
}

echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}${CYAN}     Perry Comprehensive Benchmark Suite${NC}"
echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

echo -e "${BOLD}Checking runtimes...${NC}"
HAS_NODE=0
HAS_BUN=0
HAS_COMPILETS=0

check_runtime "node" && HAS_NODE=1
check_runtime "bun" && HAS_BUN=1
if [ -f "$COMPILETS" ]; then
    echo -e "${GREEN}✓${NC} perry found: $COMPILETS"
    HAS_COMPILETS=1
else
    echo -e "${RED}✗${NC} perry not found at $COMPILETS"
    echo "   Run: cd ${SCRIPT_DIR}/../.. && cargo build --release"
    exit 1
fi

echo ""

# Get runtime versions
echo -e "${BOLD}Runtime versions:${NC}"
[ $HAS_NODE -eq 1 ] && echo "  Node.js: $(node --version)"
[ $HAS_BUN -eq 1 ] && echo "  Bun: $(bun --version)"
echo "  Perry: native binary"
echo ""

# Benchmark files
BENCHMARKS="02_loop_overhead.ts
03_array_write.ts
04_array_read.ts
05_fibonacci.ts
06_math_intensive.ts
07_object_create.ts
08_string_concat.ts
09_method_calls.ts
10_nested_loops.ts
11_prime_sieve.ts
12_binary_trees.ts
13_factorial.ts
14_closure.ts
15_mandelbrot.ts"

# Compile all benchmarks first
echo -e "${BOLD}Compiling benchmarks with Perry...${NC}"
cd "$SCRIPT_DIR"
for bench in $BENCHMARKS; do
    name="${bench%.ts}"
    echo -n "  Compiling $bench... "
    if "$COMPILETS" "$bench" -o "$name" 2>/dev/null; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${RED}FAILED${NC}"
    fi
done
echo ""

# Function to extract timing from output
extract_time() {
    echo "$1" | grep -E "^[a-z_]+:[0-9]+" | head -1 | cut -d: -f2
}

# Run benchmarks
echo -e "${BOLD}Running benchmarks...${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
printf "${BOLD}%-20s %12s %12s %12s${NC}\n" "Benchmark" "Perry" "Node.js" "Bun"
echo -e "───────────────────────────────────────────────────────────────"

# Track wins/losses
WINS_NODE=0
LOSSES_NODE=0
TIES_NODE=0
WINS_BUN=0
LOSSES_BUN=0
TIES_BUN=0

for bench in $BENCHMARKS; do
    name="${bench%.ts}"
    display_name=$(echo "$name" | sed 's/^[0-9]*_//')

    # Run Perry
    if [ -f "./$name" ]; then
        output=$("./$name" 2>&1)
        perry_time=$(extract_time "$output")
    else
        perry_time="ERR"
    fi

    # Run Node.js
    if [ $HAS_NODE -eq 1 ]; then
        output=$(node "$bench" 2>&1)
        node_time=$(extract_time "$output")
    else
        node_time="-"
    fi

    # Run Bun
    if [ $HAS_BUN -eq 1 ]; then
        output=$(bun run "$bench" 2>&1)
        bun_time=$(extract_time "$output")
    else
        bun_time="-"
    fi

    # Track wins/losses vs Node
    if [ -n "$perry_time" ] && [ "$perry_time" != "ERR" ] && [ -n "$node_time" ] && [ "$node_time" != "-" ]; then
        if [ "$perry_time" -lt "$node_time" ]; then
            WINS_NODE=$((WINS_NODE + 1))
        elif [ "$perry_time" -gt "$node_time" ]; then
            LOSSES_NODE=$((LOSSES_NODE + 1))
        else
            TIES_NODE=$((TIES_NODE + 1))
        fi
    fi

    # Track wins/losses vs Bun
    if [ -n "$perry_time" ] && [ "$perry_time" != "ERR" ] && [ -n "$bun_time" ] && [ "$bun_time" != "-" ]; then
        if [ "$perry_time" -lt "$bun_time" ]; then
            WINS_BUN=$((WINS_BUN + 1))
        elif [ "$perry_time" -gt "$bun_time" ]; then
            LOSSES_BUN=$((LOSSES_BUN + 1))
        else
            TIES_BUN=$((TIES_BUN + 1))
        fi
    fi

    # Print results with colors based on comparison to Node
    if [ -n "$perry_time" ] && [ "$perry_time" != "ERR" ]; then
        if [ -n "$node_time" ] && [ "$node_time" != "-" ] && [ "$perry_time" -lt "$node_time" ]; then
            perry_display="${GREEN}${perry_time}ms${NC}"
        elif [ -n "$node_time" ] && [ "$node_time" != "-" ] && [ "$perry_time" -gt "$node_time" ]; then
            perry_display="${RED}${perry_time}ms${NC}"
        else
            perry_display="${perry_time}ms"
        fi
    else
        perry_display="ERR"
    fi

    printf "%-20s " "$display_name"
    echo -e "${perry_display}\t\t${node_time:-"-"}ms\t\t${bun_time:-"-"}ms"
done

echo -e "═══════════════════════════════════════════════════════════════"
echo ""

# Measure startup time
echo -e "${BOLD}Startup time (average of 5 runs):${NC}"
echo -e "───────────────────────────────────────────────────────────────"

# Compile startup benchmark
"$COMPILETS" "01_startup.ts" -o "01_startup" 2>/dev/null

# Measure startup times
measure_startup() {
    local cmd="$1"
    local total=0
    for i in 1 2 3 4 5; do
        start=$(python3 -c "import time; print(int(time.time() * 1000))")
        eval "$cmd" > /dev/null 2>&1
        end=$(python3 -c "import time; print(int(time.time() * 1000))")
        elapsed=$((end - start))
        total=$((total + elapsed))
    done
    echo $((total / 5))
}

perry_startup=$(measure_startup "./01_startup")
[ $HAS_NODE -eq 1 ] && node_startup=$(measure_startup "node 01_startup.ts") || node_startup="-"
[ $HAS_BUN -eq 1 ] && bun_startup=$(measure_startup "bun run 01_startup.ts") || bun_startup="-"

printf "%-20s %12s %12s %12s\n" "cold start" "${perry_startup}ms" "${node_startup}ms" "${bun_startup}ms"
echo ""

# Measure executable size
echo -e "${BOLD}Executable/binary size:${NC}"
echo -e "───────────────────────────────────────────────────────────────"

perry_size=$(ls -lh 05_fibonacci 2>/dev/null | awk '{print $5}')
if [ $HAS_NODE -eq 1 ]; then
    node_bin=$(which node)
    node_size=$(ls -lh "$node_bin" 2>/dev/null | awk '{print $5}')
else
    node_size="-"
fi
if [ $HAS_BUN -eq 1 ]; then
    bun_bin=$(which bun)
    bun_size=$(ls -lh "$bun_bin" 2>/dev/null | awk '{print $5}')
else
    bun_size="-"
fi

printf "%-20s %12s %12s %12s\n" "binary size" "$perry_size" "$node_size" "$bun_size"

# Show perry compiled binary sizes
echo ""
echo -e "${BOLD}Compiled binary sizes (Perry):${NC}"
for bench in $BENCHMARKS; do
    name="${bench%.ts}"
    if [ -f "./$name" ]; then
        size=$(ls -lh "./$name" | awk '{print $5}')
        display_name=$(echo "$name" | sed 's/^[0-9]*_//')
        printf "  %-20s %s\n" "$display_name" "$size"
    fi
done
echo ""

# Memory usage (RSS peak)
echo -e "${BOLD}Peak memory usage (RSS) for binary_trees:${NC}"
echo -e "───────────────────────────────────────────────────────────────"

if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    if [ -f "./12_binary_trees" ]; then
        result=$(/usr/bin/time -l ./12_binary_trees 2>&1 | grep "maximum resident set size" | awk '{print $1}')
        if [ -n "$result" ]; then
            perry_mem="$((result / 1024 / 1024))MB"
        else
            perry_mem="-"
        fi
    fi

    if [ $HAS_NODE -eq 1 ]; then
        result=$(/usr/bin/time -l node 12_binary_trees.ts 2>&1 | grep "maximum resident set size" | awk '{print $1}')
        if [ -n "$result" ]; then
            node_mem="$((result / 1024 / 1024))MB"
        else
            node_mem="-"
        fi
    else
        node_mem="-"
    fi

    if [ $HAS_BUN -eq 1 ]; then
        result=$(/usr/bin/time -l bun run 12_binary_trees.ts 2>&1 | grep "maximum resident set size" | awk '{print $1}')
        if [ -n "$result" ]; then
            bun_mem="$((result / 1024 / 1024))MB"
        else
            bun_mem="-"
        fi
    else
        bun_mem="-"
    fi
else
    perry_mem="-"
    node_mem="-"
    bun_mem="-"
fi

printf "%-20s %12s %12s %12s\n" "peak RSS" "$perry_mem" "$node_mem" "$bun_mem"
echo ""

# Summary
echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}Summary:${NC}"
echo ""
echo -e "  vs Node.js: ${GREEN}$WINS_NODE faster${NC}, ${RED}$LOSSES_NODE slower${NC}, $TIES_NODE tied"
[ $HAS_BUN -eq 1 ] && echo -e "  vs Bun:     ${GREEN}$WINS_BUN faster${NC}, ${RED}$LOSSES_BUN slower${NC}, $TIES_BUN tied"
echo ""
echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════════${NC}"

# Cleanup compiled binaries
echo ""
echo "Cleaning up compiled binaries..."
for bench in $BENCHMARKS; do
    name="${bench%.ts}"
    rm -f "$name" "${name}.o"
done
rm -f "01_startup" "01_startup.o"

echo -e "${GREEN}Done!${NC}"
