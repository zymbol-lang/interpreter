#!/usr/bin/env bash
# semantic_compare.sh — semantic diagnostics (E001–E013) against their goldens.
#
# A wrapper over ZyQuality (../zyquality), the project's point of record for
# testing.  These goldens are recorded by *checking* the program rather than
# running it: the files under errors/semantic/ are supposed to fail, so running
# them proves nothing and analysing them proves everything.
#
#   bash tests/scripts/semantic_compare.sh          # all of errors/semantic/
#   bash tests/scripts/semantic_compare.sh E002     # only paths matching
#   bash tests/scripts/semantic_compare.sh --regen  # re-record them
#
# That split used to be knowledge held between two scripts and written down in
# neither: this one looked only at errors/semantic/, expected_compare.sh looked
# at everything else.  A file that fell between them had a golden nobody ever
# compared.  It is now declared in zyquality/corpus.toml as a `[[golden]]` rule,
# and `zyq audit` reports a golden no engine can produce.
#
# Exit status: 0 all match, 1 a golden is stale, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

ARGS=(--via check --filter errors/semantic)
REGEN=0
for a in "$@"; do
    case "$a" in
        --regen) REGEN=1 ;;
        -*)      ARGS+=("$a") ;;
        *)       ARGS+=(--filter "$a") ;;
    esac
done

echo "semantic_compare.sh: delegating to ZyQuality at $(zyq_root)"

if [[ $REGEN -eq 1 ]]; then
    echo "  → zyq expect --regen --engines zytw --via check"
    echo
    zyq expect --regen --engines zytw "${ARGS[@]}"
else
    echo "  → zyq expect --via check"
    echo
    zyq expect "${ARGS[@]}"
fi
