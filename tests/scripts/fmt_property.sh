#!/usr/bin/env bash
# fmt_property.sh — a wrapper. The suite lives in ZyQuality (../zyquality), the
# project's point of record for testing; this keeps the name, the flags and the
# exit codes so nothing calling it has to change.
#
# It audits `zymbol fmt` over the shared corpus. Only one engine has a
# formatter, so this is not differential -- but it is a language-quality
# property over the corpus, and the corpus is there.
#
# Exit status: 0 clean, 1 found something, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

echo "fmt_property.sh: delegating to ZyQuality at $(zyq_root)"
echo "  → bash fmt/fmt_property.sh"
echo

cd "$(zyq_root)"
exec bash fmt/fmt_property.sh "$@"
