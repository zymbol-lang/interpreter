#!/usr/bin/env bash
# STRESS V2 runner — compares Zymbol VM vs Python (optimal code for each).
# Usage: bash tests/stress_v2/run.sh [--runs N]
#
# Both sides use idiomatic, optimal code for their respective language.

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
    local name="$1" lang="$2"
    local file_zy="$REPO_ROOT/tests/stress_v2/bench_${name}.zy"
    local file_py="$REPO_ROOT/tests/stress_v2/bench_${name}.py"
    local min_ms=999999

    echo ""
    echo "========================================"
    if [[ "$lang" == "zy" ]]; then
        echo "  VM: ${name}  (n=${RUNS})"
        for ((i=1; i<=RUNS; i++)); do
            ms=$(TIMEFORMAT='%R'; { time "$ZYMBOL" run "$file_zy" --vm > /dev/null; } 2>&1)
            ms=$(echo "$ms * 1000 / 1" | bc 2>/dev/null || echo "?")
            echo "  run $i: ${ms}ms"
            [[ "$ms" != "?" && "$ms" -lt "$min_ms" ]] && min_ms=$ms
        done
        echo "  min: ${min_ms}ms"
    else
        echo "  PYTHON: ${name}  (n=${RUNS})"
        for ((i=1; i<=RUNS; i++)); do
            ms=$(TIMEFORMAT='%R'; { time python3 "$file_py" > /dev/null; } 2>&1)
            ms=$(echo "$ms * 1000 / 1" | bc 2>/dev/null || echo "?")
            echo "  run $i: ${ms}ms"
            [[ "$ms" != "?" && "$ms" -lt "$min_ms" ]] && min_ms=$ms
        done
        echo "  min: ${min_ms}ms"
    fi
}

echo "========================================"
echo "  STRESS V2 — Optimal code, both sides"
echo "  Runs per benchmark: ${RUNS}"
echo "========================================"

for bench in "${BENCHMARKS[@]}"; do
    run_bench "$bench" zy
done

echo ""
echo "========================================"
echo "  Python comparison"
echo "========================================"

for bench in "${BENCHMARKS[@]}"; do
    run_bench "$bench" py
done

echo ""
echo "========================================"
echo "  Per-operation detail (single run)"
echo "========================================"

for bench in "${BENCHMARKS[@]}"; do
    echo ""
    echo "--- VM: ${bench} ---"
    "$ZYMBOL" run "$REPO_ROOT/tests/stress_v2/bench_${bench}.zy" --vm
    echo ""
    echo "--- Python: ${bench} ---"
    python3 "$REPO_ROOT/tests/stress_v2/bench_${bench}.py"
done
