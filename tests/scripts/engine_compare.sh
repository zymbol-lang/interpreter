#!/usr/bin/env bash
# engine_compare.sh — one program through every engine that exists.
#
# A wrapper over ZyQuality (../zyquality), the project's point of record for
# testing.  This script was written because vm_compare.sh sees two engines,
# web/tests/test_runner.mjs sees two, zyml/tests/parity.sh sees two, and no
# suite ever put all four answers side by side.  zyq is that idea grown up:
# equivalence classes over N engines, with the exclusion rules and the
# redactions declared next to the corpus instead of once per runner.
#
#   bash tests/scripts/engine_compare.sh                    # the whole corpus
#   bash tests/scripts/engine_compare.sh loops/labels       # a subtree
#   bash tests/scripts/engine_compare.sh FILE.zy            # one file, in detail
#   bash tests/scripts/engine_compare.sh DIR --engines tw,vm
#
# Engine names are the ids in zyquality/engines.toml: zytw, zyvm, zyjs, zyml.
# The old short forms (tw, vm, js, zyml) are still accepted and translated.
#
# `--matrix` is now `-v`: the matrix was a way of seeing agreement as well as
# divergence, which is what verbose does.
#
# Exit status: 0 the engines agree, 1 they do not, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

# The old names, kept working so nothing that calls this has to be rewritten.
translate_engines() {
    local out=() e
    IFS=',' read -ra parts <<< "$1"
    for e in "${parts[@]}"; do
        case "$e" in
            tw)   out+=(zytw) ;;
            vm)   out+=(zyvm) ;;
            js)   out+=(zyjs) ;;
            ml)   out+=(zyml) ;;
            *)    out+=("$e") ;;
        esac
    done
    (IFS=','; echo "${out[*]}")
}

ARGS=()
TARGET=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --matrix)  ARGS+=(-v); shift ;;
        --strict)  ARGS+=(--strict); shift ;;
        --engines) ARGS+=(--engines "$(translate_engines "$2")"); shift 2 ;;
        --timeout) ARGS+=(--timeout "$2"); shift 2 ;;
        -v|--verbose) ARGS+=(-v); shift ;;
        -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)         TARGET="$1"; shift ;;
    esac
done

echo "engine_compare.sh: delegating to ZyQuality at $(zyq_root)"

# A single file is a different question from a sweep: you already know it
# diverges and you want to read what each engine said.  That is `zyq show`.
if [[ -n "$TARGET" && -f "$TARGET" ]]; then
    echo "  → zyq show $TARGET"
    echo
    zyq show "$(cd "$(dirname "$TARGET")" && pwd)/$(basename "$TARGET")" "${ARGS[@]}"
    exit 0
fi

if [[ -n "$TARGET" ]]; then
    # Accept both a corpus-relative path and an old tests/-relative one.
    ARGS+=(--filter "${TARGET#tests/}")
fi

echo "  → zyq consensus"
echo
zyq consensus "${ARGS[@]}"
