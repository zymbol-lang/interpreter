#!/usr/bin/env bash
# STRESS V2 runner — Zymbol VM benchmarks.
# Usage: bash tests/stress_v2/run.sh [--runs N]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ZYMBOL="$REPO_ROOT/target/release/zymbol"
RUNS=3

while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs) RUNS="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -x "$ZYMBOL" ]]; then
    echo "ERROR: release binary not found. Run: cargo build --release"
    exit 1
fi

BENCHMARKS=(hof pipeline numeric text)

run_bench() {
    local name="$1"
    local file_zy="$REPO_ROOT/tests/stress_v2/bench_${name}.zy"
    local min_ms=999999

    echo ""
    echo "========================================"
    echo "  VM: ${name}  (n=${RUNS})"
    for ((i=1; i<=RUNS; i++)); do
        ms=$(TIMEFORMAT='%R'; { time "$ZYMBOL" run "$file_zy" --vm > /dev/null; } 2>&1)
        ms=$(echo "$ms * 1000 / 1" | bc 2>/dev/null || echo "?")
        echo "  run $i: ${ms}ms"
        [[ "$ms" != "?" && "$ms" -lt "$min_ms" ]] && min_ms=$ms
    done
    echo "  min: ${min_ms}ms"
}

echo "========================================"
echo "  STRESS V2 — VM benchmarks"
echo "  Runs per benchmark: ${RUNS}"
echo "========================================"

for bench in "${BENCHMARKS[@]}"; do
    run_bench "$bench"
done

echo ""
echo "========================================"
echo "  Per-operation detail (single run)"
echo "========================================"

for bench in "${BENCHMARKS[@]}"; do
    echo ""
    echo "--- VM: ${bench} ---"
    "$ZYMBOL" run "$REPO_ROOT/tests/stress_v2/bench_${bench}.zy" --vm
done
