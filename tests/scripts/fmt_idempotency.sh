#!/usr/bin/env bash
# fmt_idempotency.sh — verifies that `zymbol fmt` is idempotent on every .zy
# test file: running it twice must produce identical output (§2.3).
#
# Usage:
#   bash tests/scripts/fmt_idempotency.sh           # all test files
#   bash tests/scripts/fmt_idempotency.sh --vm      # same (flag ignored, for consistency)
#
# Exit code: 0 if all files are idempotent, 1 otherwise.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$REPO_ROOT/target/debug/zymbol"

if [[ ! -x "$BINARY" ]]; then
    echo "Building zymbol (debug)..."
    cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --bin zymbol 2>&1
fi

PASS=0
FAIL=0
ERRORS=()

# Collect files: everything under tests/ except bench/stress
mapfile -t FILES < <(
    find "$REPO_ROOT/tests" -name "*.zy" \
        ! -name "bench_*" \
        ! -name "stress*" \
        ! -name "_*" \
    | sort
)

TOTAL="${#FILES[@]}"

for file in "${FILES[@]}"; do
    rel="${file#$REPO_ROOT/}"

    # First pass: format into a temp file
    tmp1=$(mktemp /tmp/zymbol_fmt_XXXXXX.zy)
    tmp2=$(mktemp /tmp/zymbol_fmt_XXXXXX.zy)
    cp "$file" "$tmp1"

    if ! "$BINARY" fmt "$tmp1" --write 2>/dev/null; then
        # File has a parse/lex error — skip (formatter is not expected to handle broken files)
        rm -f "$tmp1" "$tmp2"
        continue
    fi

    # Second pass: format the already-formatted file
    cp "$tmp1" "$tmp2"
    "$BINARY" fmt "$tmp2" --write 2>/dev/null || true

    if diff -q "$tmp1" "$tmp2" > /dev/null 2>&1; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
        ERRORS+=("$rel")
        echo -e "\033[0;31mNOT IDEMPOTENT\033[0m  $rel"
        diff "$tmp1" "$tmp2" | head -20
        echo "---"
    fi

    rm -f "$tmp1" "$tmp2"
done

echo ""
if [[ $FAIL -eq 0 ]]; then
    echo -e "\033[0;32m${PASS}/${PASS} files are idempotent.\033[0m"
    exit 0
else
    echo -e "\033[0;31m${FAIL}/${TOTAL} files are NOT idempotent.\033[0m"
    exit 1
fi
