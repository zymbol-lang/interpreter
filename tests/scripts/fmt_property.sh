#!/usr/bin/env bash
# fmt_property.sh — Property-based audit of `zymbol fmt` over the .zy corpus.
#
# Verifies four properties on every test/example file:
#   P1 reparse      — formatted output must still pass `zymbol check`
#   P2 idempotence  — fmt(fmt(x)) == fmt(x)
#   P3 semantics    — formatted program produces identical runtime output
#   P4 comments     — `//` and `/*` token counts are preserved
#
# Files are formatted via an in-place swap (original backed up and always
# restored) so module-name checks and relative imports stay valid.
#
# Usage:
#   ./tests/scripts/fmt_property.sh                       # report all failures
#   ./tests/scripts/fmt_property.sh --baseline FILE      # exit 0 iff no failure
#                                                         # absent from FILE (regressions only)
#   ./tests/scripts/fmt_property.sh --update-baseline FILE  # rewrite FILE with current failures

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TESTS_DIR="$REPO_ROOT/tests"
EXAMPLES_DIR="$REPO_ROOT/examples"
ZYMBOL="$REPO_ROOT/target/release/zymbol"
TIMEOUT_SEC=10

BASELINE=""
UPDATE_BASELINE=""
if [[ "${1:-}" == "--baseline" ]]; then BASELINE="${2:?--baseline needs a file}"; fi
if [[ "${1:-}" == "--update-baseline" ]]; then UPDATE_BASELINE="${2:?--update-baseline needs a file}"; fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

PASS=0; SKIP=0
declare -a FAILURES=()   # entries: "P1 rel", "P2 rel", "P3 rel", "P4 rel"
declare -a SKIPPED=()

WORK="$(mktemp -d)"
CURF=""
restore() { if [[ -n "$CURF" && -f "$WORK/cur" ]]; then cp "$WORK/cur" "$CURF"; fi; rm -rf "$WORK"; }
trap restore EXIT

# Diagnostics legitimately shift line/col after formatting; normalize spans.
normalize_spans() { sed -E 's/:[0-9]+:[0-9]+/:L:C/g'; }

run_file() { # $1 = .zy file (its own .input is used as stdin when present)
    local input_file="${1%.zy}.input"
    if [[ -f "$input_file" ]]; then
        timeout "$TIMEOUT_SEC" "$ZYMBOL" run "$1" < "$input_file" 2>&1 | normalize_spans
    else
        timeout "$TIMEOUT_SEC" "$ZYMBOL" run "$1" < /dev/null 2>&1 | normalize_spans
    fi
    return 0
}

mapfile -t FILES < <(
    find "$TESTS_DIR" "$EXAMPLES_DIR" -name "*.zy" \
        ! -path "*/scripts/*" \
        ! -path "*/stress_v2/*" \
        2>/dev/null | sort
)

TOTAL=${#FILES[@]}
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  Zymbol Formatter Property Report — $TOTAL files${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo ""

for file in "${FILES[@]}"; do
    rel="${file#$REPO_ROOT/}"

    # Only files whose ORIGINAL passes `check` are meaningful for the
    # formatter contract (negative tests expect lexer/parser/semantic errors).
    if ! "$ZYMBOL" check "$file" >/dev/null 2>&1; then
        SKIP=$((SKIP + 1)); SKIPPED+=("$rel (original fails check)")
        continue
    fi

    # fmt output goes straight to a file: command substitution would drop
    # NUL bytes (TUI sources legitimately contain them in char literals)
    if ! "$ZYMBOL" fmt "$file" > "$WORK/fmt1" 2>"$WORK/err"; then
        # Formatter refused a valid file — count as a P1-class failure.
        FAILURES+=("P1 $rel")
        echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[fmt error: $(head -c 120 "$WORK/err" | tr -d '\n')]${RESET}"
        continue
    fi

    file_failed=false

    # P2 — idempotence (via stdin, no swap needed)
    "$ZYMBOL" fmt - < "$WORK/fmt1" > "$WORK/fmt2" 2>/dev/null
    if ! cmp -s "$WORK/fmt1" "$WORK/fmt2"; then
        FAILURES+=("P2 $rel"); file_failed=true
        echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[P2 not idempotent]${RESET}"
    fi

    # P4 — comment token counts
    c1=$(grep -ao '//' "$file" | wc -l);  c2=$(grep -ao '//' "$WORK/fmt1" | wc -l)
    b1=$(grep -ao '/\*' "$file" | wc -l); b2=$(grep -ao '/\*' "$WORK/fmt1" | wc -l)
    if [[ "$c1" -ne "$c2" || "$b1" -ne "$b2" ]]; then
        FAILURES+=("P4 $rel"); file_failed=true
        echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[P4 comments //:$c1->$c2 /*:$b1->$b2]${RESET}"
    fi

    # P1 + P3 — swap formatted content in place, test, restore
    has_expected=false
    [[ -f "${file%.zy}.expected" ]] && has_expected=true
    o1=""
    if $has_expected; then o1="$(run_file "$file")"; fi

    CURF="$file"
    cp "$file" "$WORK/cur"
    cp "$WORK/fmt1" "$file"

    if ! "$ZYMBOL" check "$file" >/dev/null 2>&1; then
        FAILURES+=("P1 $rel"); file_failed=true
        echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[P1 reparse]${RESET}"
    elif $has_expected; then
        o2="$(run_file "$file")"
        if [[ "$o1" != "$o2" ]]; then
            # Nondeterminism guard: rerun the original before blaming fmt.
            cp "$WORK/cur" "$file"
            o1b="$(run_file "$file")"
            cp "$WORK/fmt1" "$file"
            if [[ "$o1" != "$o1b" ]]; then
                SKIP=$((SKIP + 1)); SKIPPED+=("$rel (nondeterministic output)")
            else
                FAILURES+=("P3 $rel"); file_failed=true
                echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[P3 semantics]${RESET}"
            fi
        fi
    fi

    cp "$WORK/cur" "$file"
    CURF=""

    if ! $file_failed; then
        PASS=$((PASS + 1))
    fi
done

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  SUMMARY${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "  Total files  : ${BOLD}$TOTAL${RESET}"
echo -e "  ${GREEN}PASS${RESET}         : ${GREEN}${BOLD}$PASS${RESET}"
echo -e "  ${RED}FAIL${RESET}         : ${RED}${BOLD}${#FAILURES[@]}${RESET}"
echo -e "  ${YELLOW}SKIP${RESET}         : ${YELLOW}${BOLD}$SKIP${RESET}"
for p in P1 P2 P3 P4; do
    n=$(printf '%s\n' "${FAILURES[@]:-}" | grep -c "^$p " || true)
    echo -e "    $p failures : $n"
done
echo ""

if [[ -n "$UPDATE_BASELINE" ]]; then
    printf '%s\n' "${FAILURES[@]:-}" | grep -v '^$' | sort > "$UPDATE_BASELINE"
    echo -e "${CYAN}Baseline written to $UPDATE_BASELINE ($(wc -l < "$UPDATE_BASELINE") entries)${RESET}"
    exit 0
fi

if [[ -n "$BASELINE" ]]; then
    new_failures="$(printf '%s\n' "${FAILURES[@]:-}" | grep -v '^$' | sort | comm -23 - <(sort "$BASELINE"))"
    if [[ -n "$new_failures" ]]; then
        echo -e "${RED}${BOLD}NEW failures not in baseline:${RESET}"
        echo "$new_failures" | sed 's/^/  ✗ /'
        exit 1
    fi
    fixed="$(printf '%s\n' "${FAILURES[@]:-}" | grep -v '^$' | sort | comm -13 - <(sort "$BASELINE"))"
    if [[ -n "$fixed" ]]; then
        echo -e "${GREEN}Fixed since baseline (consider --update-baseline):${RESET}"
        echo "$fixed" | sed 's/^/  ✓ /'
    fi
    echo -e "${GREEN}${BOLD}No regressions vs baseline.${RESET}"
    exit 0
fi

if [[ ${#FAILURES[@]} -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All $PASS files satisfy P1–P4!${RESET}"
    exit 0
else
    echo -e "${RED}${BOLD}${#FAILURES[@]} property failures.${RESET}"
    printf '%s\n' "${FAILURES[@]}" | sed 's/^/  ✗ /'
    exit 1
fi
