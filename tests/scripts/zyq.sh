#!/usr/bin/env bash
# Find ZyQuality — the project's point of record for testing — or say why not.
#
# Every test script in this repository delegates to `zyq`.  This file is the one
# place that knows how to find it, so a moved checkout is one edit rather than
# five.
#
# Sourced, not executed:
#
#   source "$(dirname "${BASH_SOURCE[0]}")/zyq.sh"
#   zyq_require                        # exits 2 with instructions if absent
#   zyq consensus --engines zytw,zyvm
#
# Search order:
#   $ZYQ_ROOT        an explicit checkout — if set, it is the only candidate,
#                    because silently ignoring it and testing something else is
#                    worse than failing
#   ../zyquality     the sibling layout this workspace uses
#
# A candidate counts only if it holds both `zyq` and `engines.toml`: a binary
# without its configuration cannot run the corpus, and finding one without the
# other is how a wrapper ends up reporting on a checkout nobody meant to test.
#
# Why exit 2 and not 0 when it is missing: a gate must not read "nothing ran"
# as "nothing failed".  Every script here uses the same contract as zyq —
# 0 clean, 1 found something, 2 could not run.

# Resolved once, on first use, and remembered.
_ZYQ_ROOT_CACHE=""

_zyq_valid() { [[ -x "$1/zyq" && -f "$1/engines.toml" ]]; }

zyq_root() {
    [[ -n "$_ZYQ_ROOT_CACHE" ]] && { echo "$_ZYQ_ROOT_CACHE"; return 0; }

    local here sibling
    here="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

    if [[ -n "${ZYQ_ROOT:-}" ]]; then
        # An explicit setting is never second-guessed.  If it is wrong the
        # caller has to know, not be quietly served a different corpus.
        if _zyq_valid "$ZYQ_ROOT"; then
            _ZYQ_ROOT_CACHE="$(cd "$ZYQ_ROOT" && pwd -P)"
            echo "$_ZYQ_ROOT_CACHE"; return 0
        fi
        return 1
    fi

    sibling="$here/../zyquality"
    if _zyq_valid "$sibling"; then
        _ZYQ_ROOT_CACHE="$(cd "$sibling" && pwd -P)"
        echo "$_ZYQ_ROOT_CACHE"; return 0
    fi
    return 1
}

zyq_require() {
    zyq_root >/dev/null && return 0
    local who
    who="$(basename "${BASH_SOURCE[1]:-$0}")"
    if [[ -n "${ZYQ_ROOT:-}" ]]; then
        cat >&2 <<EOF
$who: ZYQ_ROOT is set to '$ZYQ_ROOT', which is not a ZyQuality checkout.
  Expected both '$ZYQ_ROOT/zyq' and '$ZYQ_ROOT/engines.toml'.
  Build it with: make -C '$ZYQ_ROOT'
EOF
    else
        cat >&2 <<EOF
$who: ZyQuality not found — QA for this project lives there.

  The corpus, the golden files and the engine comparison are in the zyquality
  repository, so that all four engines are graded against the same files.  This
  script is a thin wrapper over it.

  Get it:
      git clone https://github.com/zymbol-lang/zyquality.git ../zyquality
      make -C ../zyquality

  Or point at an existing checkout:
      ZYQ_ROOT=/path/to/zyquality $who
EOF
    fi
    exit 2
}

# `--root` is passed explicitly rather than relying on zyq's own guess: the
# binary infers its root from where it sits, which is right when you run it by
# hand and wrong the day someone copies just the binary onto PATH.
zyq() {
    local root
    root="$(zyq_root)" || zyq_require
    "$root/zyq" --root "$root" "$@"
}
