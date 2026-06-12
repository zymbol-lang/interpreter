#!/usr/bin/env bash
# =============================================================================
# Zymbol-Lang — Benchmark Regression Gate
#
# Runs each benchmark N times (both engines), takes the MEDIAN wall time and
# compares it against a recorded baseline. Fails when a benchmark regresses
# beyond the tolerance, making performance regressions visible in CI instead
# of silent.
#
# Usage:
#   ./tests/scripts/bench_gate.sh                 # gate against baseline
#   ./tests/scripts/bench_gate.sh --record        # (re)write the baseline
#   ./tests/scripts/bench_gate.sh --runs 7        # median of 7 runs (default 5)
#   ./tests/scripts/bench_gate.sh --tolerance 40  # allow +40% (default 30)
#
# Notes:
#   - Baselines are MACHINE-SPECIFIC (wall time). Record the baseline on the
#     same machine/CI runner that will enforce the gate.
#   - A regression must exceed BOTH the relative tolerance and an absolute
#     floor (+${ABS_FLOOR_MS}ms) — short benchmarks are dominated by process
#     startup noise and the floor keeps them from flapping.
#   - Benchmarks present in the run but missing from the baseline are
#     reported as NEW (not a failure); rerun with --record to adopt them.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/release/zymbol"
BASELINE_FILE="$SCRIPT_DIR/bench_baseline.txt"

RUNS=5
TOLERANCE=30        # percent
ABS_FLOOR_MS=20     # regression must also exceed baseline + this many ms
RECORD=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --record)    RECORD=1; shift ;;
        --runs)      RUNS="$2"; shift 2 ;;
        --tolerance) TOLERANCE="$2"; shift 2 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ ! -x "$BINARY" ]]; then
    echo "ERROR: release binary not found at $BINARY (run: cargo build --release)" >&2
    exit 1
fi

BOLD=$'\e[1m'; RED=$'\e[0;31m'; GREEN=$'\e[0;32m'; YELLOW=$'\e[1;33m'; RESET=$'\e[0m'

BENCHES=(stress bench_match bench_recursion bench_collections bench_strings bench_strings_stress bench_strings_modify)

now_ms() { echo $(( $(date +%s%N) / 1000000 )); }

# Median wall time (ms) of $RUNS executions of "$@".
median_ms() {
    local times=()
    for (( i=0; i<RUNS; i++ )); do
        local t0 t1
        t0=$(now_ms)
        "$@" > /dev/null 2>&1
        t1=$(now_ms)
        times+=( $(( t1 - t0 )) )
    done
    printf '%s\n' "${times[@]}" | sort -n | awk -v n="$RUNS" '
        { a[NR] = $1 }
        END {
            if (n % 2) print a[(n + 1) / 2]
            else print int((a[n/2] + a[n/2 + 1]) / 2)
        }'
}

declare -A BASELINE=()
if [[ -f "$BASELINE_FILE" ]]; then
    while read -r name ms _; do
        [[ -z "$name" || "$name" == \#* ]] && continue
        BASELINE["$name"]=$ms
    done < "$BASELINE_FILE"
fi

echo "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
if [[ $RECORD -eq 1 ]]; then
    echo "${BOLD}  Benchmark Gate — RECORDING baseline (median of $RUNS runs)${RESET}"
else
    echo "${BOLD}  Benchmark Gate — median of $RUNS runs, tolerance +${TOLERANCE}% (min +${ABS_FLOOR_MS}ms)${RESET}"
fi
echo "${BOLD}═══════════════════════════════════════════════════════════${RESET}"

PASS=0; FAIL=0; NEW=0
declare -a FAILURES=()
declare -a RESULTS=()

for bench in "${BENCHES[@]}"; do
    file="$SCRIPT_DIR/$bench.zy"
    for engine in tw vm; do
        name="$engine/$bench"
        if [[ "$engine" == "vm" ]]; then
            ms=$(median_ms "$BINARY" run --vm "$file")
        else
            ms=$(median_ms "$BINARY" run "$file")
        fi
        RESULTS+=("$name $ms")

        if [[ $RECORD -eq 1 ]]; then
            printf "  %-28s %6sms  recorded\n" "$name" "$ms"
            continue
        fi

        base="${BASELINE[$name]:-}"
        if [[ -z "$base" ]]; then
            NEW=$((NEW + 1))
            printf "  ${YELLOW}NEW ${RESET} %-28s %6sms  (no baseline — run --record)\n" "$name" "$ms"
            continue
        fi

        rel_limit=$(( base * (100 + TOLERANCE) / 100 ))
        abs_limit=$(( base + ABS_FLOOR_MS ))
        limit=$(( rel_limit > abs_limit ? rel_limit : abs_limit ))
        delta=$(( (ms - base) * 100 / (base > 0 ? base : 1) ))

        if (( ms > limit )); then
            FAIL=$((FAIL + 1))
            FAILURES+=("$name: ${ms}ms vs baseline ${base}ms (+${delta}%, limit ${limit}ms)")
            printf "  ${RED}FAIL${RESET} %-28s %6sms  vs %sms baseline (%+d%%)\n" "$name" "$ms" "$base" "$delta"
        else
            PASS=$((PASS + 1))
            printf "  ${GREEN}PASS${RESET} %-28s %6sms  vs %sms baseline (%+d%%)\n" "$name" "$ms" "$base" "$delta"
        fi
    done
done

echo "${BOLD}═══════════════════════════════════════════════════════════${RESET}"

if [[ $RECORD -eq 1 ]]; then
    {
        echo "# Benchmark baseline — median wall-time ms (machine-specific)."
        echo "# Recorded: $(date -u +%Y-%m-%dT%H:%M:%SZ) host=$(hostname) runs=$RUNS"
        printf '%s\n' "${RESULTS[@]}"
    } > "$BASELINE_FILE"
    echo "${GREEN}${BOLD}Baseline written to ${BASELINE_FILE#$PROJECT_ROOT/}${RESET}"
    exit 0
fi

echo "  PASS: $PASS   FAIL: $FAIL   NEW: $NEW"
if (( FAIL > 0 )); then
    echo ""
    echo "${RED}${BOLD}Performance regressions detected:${RESET}"
    printf '  %s\n' "${FAILURES[@]}"
    exit 1
fi
echo "${GREEN}${BOLD}No performance regressions.${RESET}"
