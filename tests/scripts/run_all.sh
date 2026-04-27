#!/usr/bin/env bash
# =============================================================================
# Zymbol-Lang — Full Benchmark Suite Runner
#
# Usage:
#   ./tests/scripts/run_all.sh                    # build + run once
#   ./tests/scripts/run_all.sh --runs 10          # run 10 times, show stats
#   ./tests/scripts/run_all.sh --no-build         # skip cargo build
#   ./tests/scripts/run_all.sh --vm               # also run VM benchmarks
#   ./tests/scripts/run_all.sh --runs 10 --vm --no-build
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/release/zymbol"

NO_BUILD=0
RUN_VM=0
RUNS=1

for arg in "$@"; do
    case "$arg" in
        --no-build) NO_BUILD=1 ;;
        --vm)       RUN_VM=1 ;;
        --runs)     ;;   # handled below via paired parsing
    esac
done

# Parse --runs N
args=("$@")
for (( i=0; i<${#args[@]}; i++ )); do
    if [[ "${args[$i]}" == "--runs" && $((i+1)) -lt ${#args[@]} ]]; then
        RUNS="${args[$((i+1))]}"
    fi
done

# -----------------------------------------------------------------------------
# Build
# -----------------------------------------------------------------------------
if [[ $NO_BUILD -eq 0 ]]; then
    echo ">>> Building Zymbol-Lang (release)..."
    cd "$PROJECT_ROOT"
    cargo build --release 2>&1 | tail -3
    echo ""
fi

if [[ ! -x "$BINARY" ]]; then
    echo "ERROR: binary not found at $BINARY" >&2
    exit 1
fi

# -----------------------------------------------------------------------------
# Helper: parse "real\t0m1.234s" → milliseconds (integer)
# -----------------------------------------------------------------------------
parse_ms() {
    # Input: e.g. "0m1.234s" or "1m23.456s"
    echo "$1" | awk -F'[ms]' '{
        split($0, a, "m")
        mins = a[1] + 0
        secs = substr(a[2], 1, length(a[2])-1) + 0
        printf "%d", int((mins * 60 + secs) * 1000 + 0.5)
    }'
}

# -----------------------------------------------------------------------------
# Helper: run a command N times, collect real-time ms, print stats
# -----------------------------------------------------------------------------
run_n_times() {
    local label="$1"
    local cmd="$2"      # full command string (will be eval'd)
    local n="$RUNS"

    echo "========================================"
    echo "  $label  (n=$n)"
    echo "========================================"

    local times=()

    for (( run=1; run<=n; run++ )); do
        # Capture output + timing; suppress program stdout on runs > 1
        local timing
        if [[ $run -eq 1 ]]; then
            timing=$( { time eval "$cmd" > /dev/null 2>&1; } 2>&1 )
            # Run once more to show output on first iteration
            eval "$cmd" 2>/dev/null || true
        fi
        timing=$( { time eval "$cmd" > /dev/null 2>/dev/null; } 2>&1 )
        local real_line
        real_line=$(echo "$timing" | grep '^real')
        local raw
        raw=$(echo "$real_line" | awk '{print $2}')
        local ms
        ms=$(parse_ms "$raw")
        times+=("$ms")
        printf "  run %2d: %s  (%dms)\n" "$run" "$raw" "$ms"
    done

    # Stats via awk
    printf '%s\n' "${times[@]}" | awk -v n="$n" '
    BEGIN { min=999999999; max=0; sum=0 }
    { sum += $1; if ($1 < min) min=$1; if ($1 > max) max=$1 }
    END {
        avg = sum / n
        printf "  ----------------------------------------\n"
        printf "  min: %dms  avg: %.0fms  max: %dms\n", min, avg, max
    }'
    echo ""
}

# -----------------------------------------------------------------------------
# Zymbol benchmarks
# -----------------------------------------------------------------------------
TOTAL_START=$(date +%s%N)

run_n_times "STRESS TEST (core)"        "\"$BINARY\" run \"$SCRIPT_DIR/stress.zy\""
run_n_times "BENCHMARK: Pattern Match"  "\"$BINARY\" run \"$SCRIPT_DIR/bench_match.zy\""
run_n_times "BENCHMARK: Recursion"      "\"$BINARY\" run \"$SCRIPT_DIR/bench_recursion.zy\""
run_n_times "BENCHMARK: Collections"    "\"$BINARY\" run \"$SCRIPT_DIR/bench_collections.zy\""
run_n_times "BENCHMARK: Strings"        "\"$BINARY\" run \"$SCRIPT_DIR/bench_strings.zy\""
run_n_times "BENCHMARK: Strings Stress"  "\"$BINARY\" run \"$SCRIPT_DIR/bench_strings_stress.zy\""
run_n_times "BENCHMARK: Strings Modify" "\"$BINARY\" run \"$SCRIPT_DIR/bench_strings_modify.zy\""

# -----------------------------------------------------------------------------
# Optional: VM benchmarks
# -----------------------------------------------------------------------------
if [[ $RUN_VM -eq 1 ]]; then
    run_n_times "VM: Stress (core)"        "\"$BINARY\" run --vm \"$SCRIPT_DIR/stress.zy\""
    run_n_times "VM: Pattern Match"        "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_match.zy\""
    run_n_times "VM: Recursion"            "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_recursion.zy\""
    run_n_times "VM: Collections"          "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_collections.zy\""
    run_n_times "VM: Strings"              "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_strings.zy\""
    run_n_times "VM: Strings Stress"       "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_strings_stress.zy\""
    run_n_times "VM: Strings Modify"       "\"$BINARY\" run --vm \"$SCRIPT_DIR/bench_strings_modify.zy\""
fi


TOTAL_END=$(date +%s%N)
TOTAL_MS=$(( (TOTAL_END - TOTAL_START) / 1000000 ))
echo "========================================"
echo "  Total wall time: ${TOTAL_MS}ms  (${RUNS} runs each)"
echo "========================================"
