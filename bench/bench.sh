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
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m'

mkdir -p "$RESULTS_DIR"

cleanup() {
    # Clean up zoxide test entries (suppress all output)
    for i in $(seq 1 "$ENTRIES"); do
        zoxide remove "$BENCH_DIR/project-$i/src" 2>/dev/null || true
    done
    # Also clean up the varied-visit and scale entries
    for i in $(seq 1 "$((ENTRIES * 2))"); do
        zoxide remove "$BENCH_DIR/varied-$i/src" 2>/dev/null || true
        zoxide remove "$BENCH_DIR/scale-$i/src" 2>/dev/null || true
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

# ================================================================
# PART 1: Core benchmarks (flat seeding)
# ================================================================
echo -e "${CYAN}━━━ Part 1: Core queries (flat seeding, $ENTRIES entries) ━━━${NC}"
echo -e "${DIM}Each path visited once — tests raw query speed${NC}"
echo ""

# --- Create and seed test directories ---
echo -e "${YELLOW}Seeding $ENTRIES directories...${NC}"
for i in $(seq 1 "$ENTRIES"); do
    mkdir -p "$BENCH_DIR/project-$i/src"
done

for i in $(seq 1 "$ENTRIES"); do
    zoxide add "$BENCH_DIR/project-$i/src"
done

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

# ================================================================
# PART 2: Realistic visit patterns
# ================================================================
echo -e "${CYAN}━━━ Part 2: Realistic visit patterns ━━━${NC}"
echo -e "${DIM}Varied visit counts — tests frecency ranking under real-world conditions${NC}"
echo ""

# Create 300 directories with varied visit frequencies:
#   - 50 "hot" paths (20 visits each)
#   - 100 "warm" paths (5 visits each)
#   - 150 "cold" paths (1 visit each)
VARIED_HOT=50
VARIED_WARM=100
VARIED_COLD=150
VARIED_TOTAL=$((VARIED_HOT + VARIED_WARM + VARIED_COLD))

echo -e "${YELLOW}Seeding $VARIED_TOTAL directories with varied visit patterns...${NC}"
for i in $(seq 1 "$VARIED_TOTAL"); do
    mkdir -p "$BENCH_DIR/varied-$i/src"
done

# Hot paths: 20 visits each
echo -e "  ${DIM}Hot paths (1-$VARIED_HOT): 20 visits each${NC}"
for i in $(seq 1 "$VARIED_HOT"); do
    for _ in $(seq 1 20); do
        zoxide add "$BENCH_DIR/varied-$i/src"
        "$TP" add "$BENCH_DIR/varied-$i/src" 2>/dev/null
    done
done

# Warm paths: 5 visits each
echo -e "  ${DIM}Warm paths ($((VARIED_HOT+1))-$((VARIED_HOT+VARIED_WARM))): 5 visits each${NC}"
for i in $(seq $((VARIED_HOT + 1)) $((VARIED_HOT + VARIED_WARM))); do
    for _ in $(seq 1 5); do
        zoxide add "$BENCH_DIR/varied-$i/src"
        "$TP" add "$BENCH_DIR/varied-$i/src" 2>/dev/null
    done
done

# Cold paths: 1 visit each
echo -e "  ${DIM}Cold paths ($((VARIED_HOT+VARIED_WARM+1))-$VARIED_TOTAL): 1 visit each${NC}"
for i in $(seq $((VARIED_HOT + VARIED_WARM + 1)) "$VARIED_TOTAL"); do
    zoxide add "$BENCH_DIR/varied-$i/src"
    "$TP" add "$BENCH_DIR/varied-$i/src" 2>/dev/null
done

echo "Done."
echo ""

# Query a hot path (should be fast — both tools rank it high)
echo -e "${GREEN}=== Query hot path: 'varied-25' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/varied-hot.json" \
    "zoxide query varied-25" \
    "$TP varied-25"
echo ""

# Query a cold path (frecency ranking has to work harder)
echo -e "${GREEN}=== Query cold path: 'varied-250' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/varied-cold.json" \
    "zoxide query varied-250" \
    "$TP varied-250"
echo ""

# Ambiguous query — multiple hits with different visit counts
echo -e "${GREEN}=== Ambiguous query: 'varied' (all 300 match) ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/varied-ambiguous.json" \
    "zoxide query varied" \
    "$TP varied"
echo ""

# ================================================================
# PART 3: Stale paths (self-healing cost)
# ================================================================
echo -e "${CYAN}━━━ Part 3: Stale paths (self-healing cost) ━━━${NC}"
echo -e "${DIM}Delete 40% of paths, then query — shows tp's Path::exists() overhead${NC}"
echo ""

STALE_TOTAL=200
STALE_DELETE=80

echo -e "${YELLOW}Seeding $STALE_TOTAL directories, will delete $STALE_DELETE...${NC}"
for i in $(seq 1 "$STALE_TOTAL"); do
    mkdir -p "$BENCH_DIR/stale-$i/src"
    zoxide add "$BENCH_DIR/stale-$i/src"
    "$TP" add "$BENCH_DIR/stale-$i/src" 2>/dev/null
done

# Delete 40% of the directories (but they remain in both databases)
for i in $(seq 1 "$STALE_DELETE"); do
    rm -rf "$BENCH_DIR/stale-$i"
done

echo "Done. Deleted stale-1 through stale-$STALE_DELETE."
echo ""

# Query that matches stale + live paths
echo -e "${GREEN}=== Query with stale paths: 'stale' ===${NC}"
echo -e "${DIM}tp prunes dead paths during query; zoxide returns them as-is${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/stale.json" \
    "zoxide query stale" \
    "$TP stale"
echo ""

# Query a specific live path (stale-150 still exists)
echo -e "${GREEN}=== Query specific live path among stale: 'stale-150' ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/stale-specific.json" \
    "zoxide query stale-150" \
    "$TP stale-150"
echo ""

# ================================================================
# PART 4: Scale test
# ================================================================
SCALE_ENTRIES="${BENCH_SCALE:-5000}"
echo -e "${CYAN}━━━ Part 4: Scale test ($SCALE_ENTRIES entries) ━━━${NC}"
echo -e "${DIM}Larger database — tests whether the speed gap holds at scale${NC}"
echo ""

echo -e "${YELLOW}Seeding $SCALE_ENTRIES directories...${NC}"
for i in $(seq 1 "$SCALE_ENTRIES"); do
    mkdir -p "$BENCH_DIR/scale-$i/src"
done

for i in $(seq 1 "$SCALE_ENTRIES"); do
    zoxide add "$BENCH_DIR/scale-$i/src"
done

for i in $(seq 1 "$SCALE_ENTRIES"); do
    "$TP" add "$BENCH_DIR/scale-$i/src" 2>/dev/null
done

echo "Done."
echo ""

echo -e "${GREEN}=== Scale exact query: 'scale-2500' ($SCALE_ENTRIES entries) ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/scale-exact.json" \
    "zoxide query scale-2500" \
    "$TP scale-2500"
echo ""

echo -e "${GREEN}=== Scale broad query: 'scale' ($SCALE_ENTRIES entries) ===${NC}"
hyperfine --warmup "$WARMUP" --min-runs "$RUNS" -N --ignore-failure \
    --export-json "$RESULTS_DIR/scale-broad.json" \
    "zoxide query scale" \
    "$TP scale"
echo ""

# ================================================================
# Summary
# ================================================================
echo -e "${GREEN}━━━ Summary ━━━${NC}"
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
