#!/usr/bin/env bash
# run_all.sh — a wrapper. The suite lives in ZyQuality (../zyquality), the
# project's point of record for testing; this keeps the name, the flags and the
# exit codes so nothing calling it has to change.
#
# The benchmark programs are in ../zyquality/bench/ -- they print elapsed wall
# time, so they are not tests and never were.
#
# Exit status: 0 clean, 1 found something, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

echo "run_all.sh: delegating to ZyQuality at $(zyq_root)"
echo "  → bash bench/run_all.sh"
echo

cd "$(zyq_root)"
exec bash bench/run_all.sh "$@"
