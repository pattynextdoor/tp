#!/usr/bin/env bash
#
# tp vs zoxide benchmark
#
# Prerequisites:
#   - hyperfine: https://github.com/sharkdp/hyperfine
#   - zoxide:    https://github.com/ajeetdsouza/zoxide
#   - tp:        cargo build --release
#
# Usage:
#   ./bench/bench.sh                    # uses ./target/release/tp
#   ./bench/bench.sh /path/to/tp        # custom tp binary
#   BENCH_ENTRIES=1000 ./bench/bench.sh # custom entry count

set -euo pipefail

TP="${1:-./target/release/tp}"
ENTRIES="${BENCH_ENTRIES:-500}"
RUNS="${BENCH_RUNS:-200}"
WARMUP="${BENCH_WARMUP:-10}"
BENCH_DIR=$(mktemp -d)
RESULTS_DIR="bench/results"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

mkdir -p "$RESULTS_DIR"

cleanup() {
    # Clean up zoxide test entries
    for i in $(seq 1 "$ENTRIES"); do
        zoxide remove "$BENCH_DIR/project-$i/src" 2>/dev/null || true
    done
    rm -rf "$BENCH_DIR"
}
trap cleanup EXIT

# --- Validate tools ---
for cmd in hyperfine zoxide; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd not found. Please install it first."
        exit 1
    fi
done

if [[ ! -x "$TP" ]]; then
    echo "Error: tp binary not found at $TP"
    echo "Run: cargo build --release"
    exit 1
fi

echo -e "${GREEN}tp benchmark${NC}"
echo "  tp binary:  $TP"
echo "  zoxide:     $(zoxide --version)"
echo "  entries:    $ENTRIES"
echo "  runs:       $RUNS"
echo "  warmup:     $WARMUP"
echo ""

# --- Create test directories ---
echo -e "${YELLOW}Seeding $ENTRIES directories...${NC}"
for i in $(seq 1 "$ENTRIES"); do
    mkdir -p "$BENCH_DIR/project-$i/src"
done

# Seed zoxide
for i in $(seq 1 "$ENTRIES"); do
    zoxide add "$BENCH_DIR/project-$i/src"
done

# Seed tp
for i in $(seq 1 "$ENTRIES"); do
    "$TP" add "$BENCH_DIR/project-$i/src" 2>/dev/null
done

echo "Done."
echo ""

# --- Benchmark: Exact query ---
echo -e "${GREEN}=== Exact query: 'project-42' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/exact.json" \
    "zoxide query project-42" \
    "$TP project-42"
echo ""

# --- Benchmark: Broad fuzzy query ---
echo -e "${GREEN}=== Broad fuzzy query: 'project' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/fuzzy.json" \
    "zoxide query project" \
    "$TP project"
echo ""

# --- Benchmark: Multi-token query ---
echo -e "${GREEN}=== Multi-token query: 'project 250' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/multi.json" \
    "zoxide query project 250" \
    "$TP project 250"
echo ""

# --- Benchmark: Add (write) ---
echo -e "${GREEN}=== Add (write operation) ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N \
    --export-json "$RESULTS_DIR/add.json" \
    "zoxide add $BENCH_DIR/project-1/src" \
    "$TP add $BENCH_DIR/project-1/src"
echo ""

# --- Summary ---
echo -e "${GREEN}=== Summary ===${NC}"
echo ""
echo "JSON results saved to $RESULTS_DIR/*.json"
echo ""
if command -v jq &>/dev/null; then
    for f in "$RESULTS_DIR"/*.json; do
        name=$(basename "$f" .json)
        echo -e "${YELLOW}$name:${NC}"
        jq -r '.results[] | "  \(.command | split(" ") | .[0] | split("/") | .[-1])  \(.mean * 1000 | . * 100 | round / 100)ms ± \(.stddev * 1000 | . * 100 | round / 100)ms"' "$f"
        echo ""
    done
else
    echo "Install jq for formatted results, or explore manually:"
    echo "  cat $RESULTS_DIR/exact.json | jq '.results[] | {command, mean, stddev}'"
fi
