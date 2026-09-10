#!/usr/bin/env bash
# expected_compare.sh — the corpus against its .expected golden files.
#
# This is now a wrapper over ZyQuality (../zyquality), the project's point of
# record for testing.  The golden files, the typed wildcards and the output
# filters live there; this script keeps its name and flags.
#
#   bash tests/scripts/expected_compare.sh              # all goldens
#   bash tests/scripts/expected_compare.sh strings      # only paths matching
#   bash tests/scripts/expected_compare.sh --regen      # re-record them
#
# Three things changed, and all three were defects this script could not see:
#
#   * The typed wildcards (***int***, ***time***) were Python regexes, and
#     without python3 the script fell back to a plain `****` glob — silently
#     turning "an integer goes here" into "anything at all goes here".  zyq has
#     no fallback because it has no dependency.
#
#   * 47 goldens carried the path the corpus had when they were recorded
#     (`tests/arity/x.zy:8:1`, or an absolute /home/…).  They passed in exactly
#     one checkout, on one machine, run from one directory.  zyq strips the
#     corpus root before comparing, so a golden now says the same thing
#     everywhere, and those goldens were re-recorded once.
#
#   * `--regen --smart` inserted ***time*** and ***date*** markers by guessing
#     at the output.  It is gone: a wildcard is a claim about what may vary and
#     that is a decision, not something to infer from one sample.  Write the
#     marker into the golden by hand.
#
# Exit status: 0 all match, 1 a golden is stale, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

ARGS=()
REGEN=0
for a in "$@"; do
    case "$a" in
        --regen)  REGEN=1 ;;
        --smart)
            echo "expected_compare.sh: --smart is gone." >&2
            echo "  It guessed which parts of one sample were allowed to vary." >&2
            echo "  Write ***time*** / ***date*** into the golden yourself; the" >&2
            echo "  marker table is in zyquality/corpus.toml." >&2
            exit 2 ;;
        -*)       ARGS+=("$a") ;;
        *)        ARGS+=(--filter "$a") ;;
    esac
done

echo "expected_compare.sh: delegating to ZyQuality at $(zyq_root)"

if [[ $REGEN -eq 1 ]]; then
    echo "  → zyq expect --regen --engines zytw"
    echo
    zyq expect --regen --engines zytw "${ARGS[@]}"
else
    echo "  → zyq expect"
    echo
    zyq expect "${ARGS[@]}"
fi
