#!/usr/bin/env bash
# vm_compare.sh — tree-walker against register VM over the whole corpus.
#
# This is now a wrapper.  The comparison, the corpus and the exclusion rules
# live in ZyQuality (../zyquality), which is the project's point of record for
# testing; this script keeps the name, the flags and the exit codes it always
# had so that nothing calling it has to change.
#
#   bash tests/scripts/vm_compare.sh
#   bash tests/scripts/vm_compare.sh --timeout 20
#
# Environment, unchanged in meaning:
#
#   ZYMBOL_BIN            Interpreter to exercise.  Set it to /usr/bin/zymbol to
#                         run the suite against an installed package instead of
#                         the build tree.  Passed straight through: engines.toml
#                         reads ${ZYMBOL_BIN:-zymbol}.
#   VM_COMPARE_SUMMARY    If set, a machine-readable summary is written there.
#   ZYQ_ROOT              Where the zyquality checkout is, if not ../zyquality.
#
# One environment variable changed shape.  VM_COMPARE_EXCLUDE took an extended
# regex over test paths, which every other runner had to re-invent for itself;
# exclusions are now declared once in zyquality/corpus.toml and grouped by tag:
#
#   was:  VM_COMPARE_EXCLUDE='stdlib/stdlib_db'
#   now:  bash tests/scripts/vm_compare.sh --without STD_DB
#
# Why the corpus moved: it existed twice, here and in zyquality/, and the two
# had already drifted 28 files apart — `arity/` and `loops/labels/` were tested
# by this script and invisible to every other engine's suite.  One copy, four
# engines, one set of rules.
#
# Exit status: 0 no mismatches, 1 a divergence, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

ARGS=()
TIMEOUT=10
while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout) TIMEOUT="$2"; shift 2 ;;
        --without) ARGS+=(--without "$2"); shift 2 ;;
        -v|--verbose) ARGS+=(-v); shift ;;
        -h|--help) sed -n '2,36p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) ARGS+=("$1"); shift ;;
    esac
done

if [[ -n "${VM_COMPARE_EXCLUDE:-}" ]]; then
    echo "vm_compare.sh: VM_COMPARE_EXCLUDE is no longer read." >&2
    echo "  Exclusions are declared in zyquality/corpus.toml and selected by tag:" >&2
    echo "    bash tests/scripts/vm_compare.sh --without STD_DB" >&2
    echo "  Run 'zyq audit' to see the tags this corpus defines." >&2
    exit 2
fi

echo "vm_compare.sh: delegating to ZyQuality at $(zyq_root)"
echo "  → zyq consensus --engines zytw,zyvm"
echo

if [[ -z "${VM_COMPARE_SUMMARY:-}" ]]; then
    set +e
    zyq consensus --engines zytw,zyvm --timeout "$TIMEOUT" "${ARGS[@]}"
    exit $?
fi

# The release workflow reads a summary file.  Produced from the JSON of the
# same run rather than a second one — sweeping the corpus twice would double a
# ten-minute gate — and from JSON rather than by parsing the human report,
# which would break the first time a column or a colour moved.
JSON="$(mktemp)"
trap 'rm -f "$JSON"' EXIT

set +e
zyq consensus --engines zytw,zyvm --timeout "$TIMEOUT" "${ARGS[@]}" --json > "$JSON"
STATUS=$?
set -e

# The old file's keys, so whatever reads it keeps working.  "fail" is a
# divergence; "skip" is a file no two engines could run, which is not a pass.
#
# Two ways of reading the same four numbers, because of where this runs.  The
# release gate exercises the INSTALLED binary inside `debian:12`, and that image
# ships no python3 — installing one to read four integers would mean the gate
# stops verifying a pristine system, which is the whole point of the container.
# So python3 is used when it is there, and `sed` when it is not.
summary_from_json() {
    if command -v python3 >/dev/null 2>&1; then
        python3 - "$1" "$2" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    s = json.load(f)["summary"]
with open(sys.argv[2], "w") as f:
    f.write("total=%d pass=%d fail=%d skip=%d excluded=0\n"
            % (s["total"], s["agree"], s["diverge"], s["too_few"]))
PY
        return $?
    fi

    # `zyq --json` writes the summary object last, on one line, with four
    # integer fields and no nesting inside it.  Anything else and this writes
    # nothing, which the caller reports as "no summary" rather than as a pass.
    local field
    for field in total agree diverge too_few; do
        local value
        value="$(sed -n "s/.*\"summary\":{[^}]*\"${field}\":\([0-9]\+\).*/\1/p" "$1")"
        [[ -n "$value" ]] || {
            echo "vm_compare.sh: cannot read '${field}' out of the JSON summary" >&2
            return 1
        }
        printf -v "json_${field}" '%s' "$value"
    done
    printf 'total=%d pass=%d fail=%d skip=%d excluded=0\n' \
        "$json_total" "$json_agree" "$json_diverge" "$json_too_few" > "$2"
}

# `if !` and not a bare call: under `set -e` a failure here would abort before
# the consensus verdict below could be propagated, and the caller would be told
# the summary is missing without ever learning what the corpus said.
if ! summary_from_json "$JSON" "$VM_COMPARE_SUMMARY"; then
    echo "vm_compare.sh: no summary written to ${VM_COMPARE_SUMMARY}" >&2
fi

# Print the summary too: a wrapper that goes silent because it was asked for a
# file is a wrapper nobody can debug.
if [[ -f "$VM_COMPARE_SUMMARY" ]]; then cat "$VM_COMPARE_SUMMARY"; fi
exit $STATUS
