#!/usr/bin/env bash
# =============================================================================
# expected_compare.sh — Verify .zy output against .expected golden files
#
# For each .zy file that has a matching .expected file, runs the tree-walker
# and compares stdout+stderr (warnings stripped) against the expected output.
#
# Usage:
#   ./tests/scripts/expected_compare.sh              # all tests
#   ./tests/scripts/expected_compare.sh strings      # only tests/strings/
#   ./tests/scripts/expected_compare.sh index_nav    # only tests/index_nav/
#   ./tests/scripts/expected_compare.sh --regen      # regenerate all .expected
#   ./tests/scripts/expected_compare.sh strings --regen
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TESTS_DIR="$REPO_ROOT/tests"
ZYMBOL="$REPO_ROOT/target/release/zymbol"

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

# ── Args ─────────────────────────────────────────────────────────────────────
REGEN=0
FILTER=""
for arg in "$@"; do
    case "$arg" in
        --regen) REGEN=1 ;;
        *)       FILTER="$arg" ;;
    esac
done

# ── Binary check ─────────────────────────────────────────────────────────────
if [[ ! -x "$ZYMBOL" ]]; then
    echo -e "${RED}error: binary not found at $ZYMBOL${RESET}"
    echo -e "${YELLOW}hint:  run 'cargo build --release' first${RESET}"
    exit 1
fi

# ── Strip warnings from output (only keep program output) ────────────────────
strip_warnings() {
    grep -v "^warning:" \
    | grep -v "^  -->" \
    | grep -v "^   " \
    | grep -v "^  =" \
    | grep -v "^$" || true
}

# ── Run one file, return stdout only (no warnings) ───────────────────────────
run_file() {
    local file="$1"
    local dir
    dir="$(dirname "$file")"
    # Capture both stdout+stderr, strip warnings, then return only program lines
    timeout 10 "$ZYMBOL" run "$file" 2>&1 | strip_warnings || true
}

# ── Collect .zy files that have a matching .expected ─────────────────────────
if [[ -n "$FILTER" ]]; then
    SEARCH_DIR="$TESTS_DIR/$FILTER"
    if [[ ! -d "$SEARCH_DIR" ]]; then
        echo -e "${RED}error: directory not found: $SEARCH_DIR${RESET}"
        exit 1
    fi
else
    SEARCH_DIR="$TESTS_DIR"
fi

mapfile -t ZY_FILES < <(
    find "$SEARCH_DIR" -name "*.zy" \
        ! -path "*/scripts/*" \
        ! -path "*/matematicas/module.zy" \
        | sort
)

# In regen mode: generate .expected for every .zy file in scope
if [[ $REGEN -eq 1 ]]; then
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}  Regenerating .expected files${RESET}"
    if [[ -n "$FILTER" ]]; then
        echo -e "${BOLD}  Scope: $FILTER${RESET}"
    else
        echo -e "${BOLD}  Scope: all tests${RESET}"
    fi
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo ""
    COUNT=0
    for file in "${ZY_FILES[@]}"; do
        expected="${file%.zy}.expected"
        out="$(run_file "$file")"
        echo "$out" > "$expected"
        rel="${file#$TESTS_DIR/}"
        echo -e "  ${GREEN}REGEN${RESET}  $rel"
        COUNT=$((COUNT + 1))
    done
    echo ""
    echo -e "${GREEN}${BOLD}Regenerated $COUNT .expected files.${RESET}"
    exit 0
fi

# ── Normal mode: compare output against existing .expected ───────────────────
PASS=0; FAIL=0; SKIP=0
declare -a FAILURES=()

mapfile -t PAIRS < <(
    for file in "${ZY_FILES[@]}"; do
        expected="${file%.zy}.expected"
        if [[ -f "$expected" ]]; then
            echo "$file"
        fi
    done
)

TOTAL=${#PAIRS[@]}

echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  Zymbol Expected-Output Tests — $TOTAL test pairs${RESET}"
if [[ -n "$FILTER" ]]; then
    echo -e "${BOLD}  Scope: $FILTER${RESET}"
fi
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo ""

if [[ $TOTAL -eq 0 ]]; then
    echo -e "${YELLOW}No .zy + .expected pairs found.${RESET}"
    echo -e "${YELLOW}hint: run with --regen to generate .expected files first.${RESET}"
    exit 0
fi

for file in "${PAIRS[@]}"; do
    expected="${file%.zy}.expected"
    rel="${file#$TESTS_DIR/}"

    actual="$(run_file "$file")"
    golden="$(cat "$expected")"

    if [[ "$actual" == "$golden" ]]; then
        PASS=$((PASS + 1))
        echo -e "  ${GREEN}PASS${RESET}  $rel"
    else
        FAIL=$((FAIL + 1))
        FAILURES+=("$file")
        echo -e "  ${RED}FAIL${RESET}  $rel"
    fi
done

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  SUMMARY${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "  Total pairs : ${BOLD}$TOTAL${RESET}"
echo -e "  ${GREEN}PASS${RESET}        : ${GREEN}${BOLD}$PASS${RESET}"
echo -e "  ${RED}FAIL${RESET}        : ${RED}${BOLD}$FAIL${RESET}"
echo ""

if [[ ${#FAILURES[@]} -gt 0 ]]; then
    echo -e "${BOLD}Failure details:${RESET}"
    for file in "${FAILURES[@]}"; do
        rel="${file#$TESTS_DIR/}"
        expected="${file%.zy}.expected"
        echo ""
        echo -e "${CYAN}── $rel ──${RESET}"
        actual="$(run_file "$file")"
        golden="$(cat "$expected")"
        echo -e "${BOLD}  Expected:${RESET}"
        echo "$golden" | head -20 | sed 's/^/    /'
        echo -e "${BOLD}  Got:${RESET}"
        echo "$actual" | head -20 | sed 's/^/    /'
        echo -e "${BOLD}  Diff:${RESET}"
        diff <(echo "$golden") <(echo "$actual") | head -30 | sed 's/^/    /' || true
    done
    echo ""
fi

if [[ $FAIL -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All $PASS expected-output tests pass!${RESET}"
else
    echo -e "${RED}${BOLD}$FAIL/$TOTAL expected-output tests failed.${RESET}"
    echo -e "${YELLOW}hint: if the output is correct, run with --regen to update .expected files.${RESET}"
    exit 1
fi
