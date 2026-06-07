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
#   ./tests/scripts/expected_compare.sh --regen      # regenerate all .expected
#   ./tests/scripts/expected_compare.sh strings --regen
#   ./tests/scripts/expected_compare.sh --regen --smart   # regen with auto-wildcards
#
# Wildcards in .expected files:
#   ****         — any sequence of characters (glob, always available)
#   ***int***    — any integer: -?[0-9]+
#   ***float***  — any float: -?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?
#   ***num***    — any number (int or float)
#   ***time***   — execution timing: [0-9]+\.[0-9]+[mu]?s  e.g. 0.167s, 12ms
#   ***date***   — ISO date: [0-9]{4}-[0-9]{2}-[0-9]{2}   e.g. 2026-05-26
#   ***path***   — file path (non-whitespace): \S+
#
# Typed wildcards require python3. Without it, typed wildcards fall back to ****(glob).
#
# --smart mode (requires python3):
#   During --regen, automatically annotates known dynamic patterns in the output:
#   - Timing values (0.167s, 12ms) → ***time***
#   - ISO dates (2026-05-26)       → ***date***
#   Use it once per test directory to fix timing-sensitive benchmark tests.
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
SMART=0
FILTER=""
for arg in "$@"; do
    case "$arg" in
        --regen) REGEN=1 ;;
        --smart) SMART=1 ;;
        *)       FILTER="$arg" ;;
    esac
done

# ── Binary check ─────────────────────────────────────────────────────────────
if [[ ! -x "$ZYMBOL" ]]; then
    echo -e "${RED}error: binary not found at $ZYMBOL${RESET}"
    echo -e "${YELLOW}hint:  run 'cargo build --release' first${RESET}"
    exit 1
fi

# ── Python3 availability (needed for typed wildcards and --smart) ─────────────
_ZY_HAS_PYTHON=0
if command -v python3 &>/dev/null; then
    _ZY_HAS_PYTHON=1
fi

if [[ $SMART -eq 1 && $_ZY_HAS_PYTHON -eq 0 ]]; then
    echo -e "${RED}error: --smart requires python3${RESET}"
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
    local input_file="${file%.zy}.input"
    if [[ -f "$input_file" ]]; then
        timeout 10 "$ZYMBOL" run "$file" < "$input_file" 2>&1 | strip_warnings || true
    else
        timeout 10 "$ZYMBOL" run "$file" 2>&1 | strip_warnings || true
    fi
}

# ── Smart annotation: replace known dynamic patterns with typed wildcards ─────
# Takes the output text as argument $1, prints annotated version to stdout.
# Uses env var to avoid stdin conflicts with the Python heredoc.
smart_annotate() {
    ZYTEST_RAW="$1" python3 <<'PYEOF'
import re, os, sys

text = os.environ.get('ZYTEST_RAW', '')

# Timing values: 0.167s, 3.14ms, 0.003µs — number followed immediately by s/ms/µs/us
text = re.sub(r'\b[0-9]+\.[0-9]+[mu\xb5]?s\b', '***time***', text)
text = re.sub(r'\b[0-9]+ms\b', '***time***', text)

# ISO 8601 date: 2026-05-26
text = re.sub(r'\b[0-9]{4}-[0-9]{2}-[0-9]{2}\b', '***date***', text)

sys.stdout.write(text)
PYEOF
}

# ── Wildcard matching (Python3 path) ─────────────────────────────────────────
# Called when golden content contains *** markers.
# Passes actual/golden via environment to avoid quoting issues with special chars.
_matches_golden_python() {
    local actual="$1"
    local golden="$2"
    ZYTEST_ACTUAL="$actual" ZYTEST_GOLDEN="$golden" python3 <<'PYEOF'
import re, os, sys

actual_text = os.environ.get('ZYTEST_ACTUAL', '')
golden_text = os.environ.get('ZYTEST_GOLDEN', '')

# Wildcard token → regex pattern mapping (longest token first to avoid partial matches)
TOKENS = {
    '***float***': r'-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?',
    '***time***':  r'[0-9]+(\.[0-9]+)?[mu\xb5]?s',
    '***date***':  r'[0-9]{4}-[0-9]{2}-[0-9]{2}',
    '***path***':  r'\S+',
    '***int***':   r'-?[0-9]+',
    '***num***':   r'-?[0-9]+(\.[0-9]+)?',
    '****':        r'.*',
}

TOK_PAT = re.compile(
    '(' + '|'.join(re.escape(k) for k in TOKENS) + ')'
)

def line_to_pattern(line):
    """Convert a golden line with *** tokens to a full-line regex."""
    parts = TOK_PAT.split(line)
    return ''.join(TOKENS.get(p, re.escape(p)) for p in parts)

actual_lines = actual_text.split('\n')
golden_lines = golden_text.split('\n')

if len(actual_lines) != len(golden_lines):
    sys.exit(1)

for actual_line, golden_line in zip(actual_lines, golden_lines):
    if '***' in golden_line:
        pattern = line_to_pattern(golden_line)
        if not re.fullmatch(pattern, actual_line):
            sys.exit(1)
    elif actual_line != golden_line:
        sys.exit(1)

sys.exit(0)
PYEOF
}

# ── Wildcard matching (bash glob fallback — **** only) ───────────────────────
_matches_golden_glob() {
    local actual="$1"
    local golden="$2"

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

# ── Dispatch: typed wildcards (Python3) or glob fallback ─────────────────────
matches_golden() {
    local actual="$1"
    local golden="$2"

    # Fast path: no wildcards at all
    if [[ "$golden" != *"***"* ]]; then
        [[ "$actual" == "$golden" ]]
        return
    fi

    if [[ $_ZY_HAS_PYTHON -eq 1 ]]; then
        _matches_golden_python "$actual" "$golden"
    else
        _matches_golden_glob "$actual" "$golden"
    fi
}

# ── Collect .zy files ─────────────────────────────────────────────────────────
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
        ! -path "*/errors/semantic/*" \
        | sort
)

# ── Regen mode ───────────────────────────────────────────────────────────────
if [[ $REGEN -eq 1 ]]; then
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}  Regenerating .expected files${RESET}"
    if [[ -n "$FILTER" ]]; then
        echo -e "${BOLD}  Scope: $FILTER${RESET}"
    else
        echo -e "${BOLD}  Scope: all tests${RESET}"
    fi
    if [[ $SMART -eq 1 ]]; then
        echo -e "${BOLD}  Mode:  smart (timing and date wildcards auto-detected)${RESET}"
    fi
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo ""
    COUNT=0
    for file in "${ZY_FILES[@]}"; do
        expected="${file%.zy}.expected"
        out="$(run_file "$file")"
        if [[ $SMART -eq 1 ]]; then
            out="$(smart_annotate "$out")"
        fi
        echo "$out" > "$expected"
        rel="${file#$TESTS_DIR/}"
        if [[ $SMART -eq 1 ]]; then
            echo -e "  ${CYAN}REGEN${RESET}  $rel"
        else
            echo -e "  ${GREEN}REGEN${RESET}  $rel"
        fi
        COUNT=$((COUNT + 1))
    done
    echo ""
    if [[ $SMART -eq 1 ]]; then
        echo -e "${CYAN}${BOLD}Regenerated $COUNT .expected files (smart mode).${RESET}"
        echo -e "${YELLOW}hint: review annotated files — ***time*** and ***date*** wildcards were inserted.${RESET}"
    else
        echo -e "${GREEN}${BOLD}Regenerated $COUNT .expected files.${RESET}"
    fi
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
if [[ $_ZY_HAS_PYTHON -eq 1 ]]; then
    echo -e "${BOLD}  Match: typed wildcards (***int*** ***float*** ***time*** etc.)${RESET}"
else
    echo -e "${YELLOW}  Match: glob only (python3 not found — typed wildcards unavailable)${RESET}"
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
    echo -e "${YELLOW}hint: for timing/date variance, run with --regen --smart to auto-annotate wildcards.${RESET}"
    exit 1
fi
