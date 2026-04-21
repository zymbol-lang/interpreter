#!/usr/bin/env bash
# =============================================================================
# semantic_compare.sh — Verify semantic errors (E001–E013) via zymbol check
#
# Runs `zymbol check` on each .zy file that has a matching .expected file,
# strips ANSI escape codes, then compares against the golden output.
# Lines in .expected may use **** as a wildcard for path-dependent parts.
#
# Usage:
#   ./tests/scripts/semantic_compare.sh              # all tests/errors/semantic/
#   ./tests/scripts/semantic_compare.sh E002         # filter by name
#   ./tests/scripts/semantic_compare.sh --regen      # regenerate all .expected
#   ./tests/scripts/semantic_compare.sh E004 --regen
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TESTS_DIR="$REPO_ROOT/tests/errors/semantic"
ZYMBOL="$REPO_ROOT/target/release/zymbol"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

REGEN=0
FILTER=""
for arg in "$@"; do
    case "$arg" in
        --regen) REGEN=1 ;;
        *)       FILTER="$arg" ;;
    esac
done

if [[ ! -x "$ZYMBOL" ]]; then
    echo -e "${RED}error: binary not found at $ZYMBOL${RESET}"
    echo -e "${YELLOW}hint:  run 'cargo build --release' first${RESET}"
    exit 1
fi

if [[ ! -d "$TESTS_DIR" ]]; then
    echo -e "${RED}error: semantic test directory not found: $TESTS_DIR${RESET}"
    exit 1
fi

# Strip ANSI escape codes from output
strip_ansi() {
    sed 's/\x1b\[[0-9;]*m//g'
}

# Run zymbol check on a file, strip ANSI, return output (stdout+stderr merged)
run_check() {
    local file="$1"
    timeout 10 "$ZYMBOL" check "$file" 2>&1 | strip_ansi || true
}

# Collect .zy files that have a matching .expected (optionally filtered by name)
if [[ -n "$FILTER" ]]; then
    mapfile -t ZY_FILES < <(
        find "$TESTS_DIR" -name "*.zy" \
            ! -path "*/scripts/*" \
            | grep "$FILTER" \
            | sort
    )
else
    mapfile -t ZY_FILES < <(
        find "$TESTS_DIR" -name "*.zy" \
            ! -path "*/scripts/*" \
            | sort
    )
fi

# Regen mode: generate .expected for every .zy file in scope
if [[ $REGEN -eq 1 ]]; then
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}  Regenerating semantic .expected files${RESET}"
    [[ -n "$FILTER" ]] && echo -e "${BOLD}  Filter: $FILTER${RESET}"
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo ""
    COUNT=0
    for file in "${ZY_FILES[@]}"; do
        expected="${file%.zy}.expected"
        out="$(run_check "$file")"
        echo "$out" > "$expected"
        rel="${file#$TESTS_DIR/}"
        echo -e "  ${GREEN}REGEN${RESET}  $rel"
        COUNT=$((COUNT + 1))
    done
    echo ""
    echo -e "${GREEN}${BOLD}Regenerated $COUNT semantic .expected files.${RESET}"
    exit 0
fi

# Wildcard match: **** in .expected matches any sequence of characters
matches_golden() {
    local actual="$1"
    local golden="$2"

    if [[ "$golden" != *"****"* ]]; then
        [[ "$actual" == "$golden" ]]
        return
    fi

    local actual_lines golden_lines
    mapfile -t actual_lines <<< "$actual"
    mapfile -t golden_lines <<< "$golden"

    [[ ${#actual_lines[@]} -eq ${#golden_lines[@]} ]] || return 1

    local i
    for (( i=0; i<${#golden_lines[@]}; i++ )); do
        local golden_line="${golden_lines[$i]}"
        local actual_line="${actual_lines[$i]}"
        if [[ "$golden_line" == *"****"* ]]; then
            local glob_pat="${golden_line//\*\*\*\*/*}"
            # shellcheck disable=SC2254
            case "$actual_line" in
                $glob_pat) ;;
                *) return 1 ;;
            esac
        else
            [[ "$actual_line" == "$golden_line" ]] || return 1
        fi
    done
    return 0
}

# Normal mode: compare against .expected
mapfile -t PAIRS < <(
    for file in "${ZY_FILES[@]}"; do
        expected="${file%.zy}.expected"
        [[ -f "$expected" ]] && echo "$file"
    done
)

TOTAL=${#PAIRS[@]}
PASS=0; FAIL=0
declare -a FAILURES=()

echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  Zymbol Semantic Error Tests — $TOTAL test pairs${RESET}"
[[ -n "$FILTER" ]] && echo -e "${BOLD}  Filter: $FILTER${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo ""

if [[ $TOTAL -eq 0 ]]; then
    echo -e "${YELLOW}No .zy + .expected pairs found in $TESTS_DIR${RESET}"
    echo -e "${YELLOW}hint: run with --regen to generate .expected files first.${RESET}"
    exit 0
fi

for file in "${PAIRS[@]}"; do
    expected="${file%.zy}.expected"
    rel="${file#$TESTS_DIR/}"

    actual="$(run_check "$file")"
    golden="$(cat "$expected")"

    if matches_golden "$actual" "$golden"; then
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
        actual="$(run_check "$file")"
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
    echo -e "${GREEN}${BOLD}All $PASS semantic error tests pass!${RESET}"
else
    echo -e "${RED}${BOLD}$FAIL/$TOTAL semantic error tests failed.${RESET}"
    echo -e "${YELLOW}hint: if the output is correct, run with --regen to update .expected files.${RESET}"
    exit 1
fi
