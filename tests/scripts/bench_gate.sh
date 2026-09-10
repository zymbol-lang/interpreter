#!/usr/bin/env bash
# bench_gate.sh — a wrapper. The suite lives in ZyQuality (../zyquality), the
# project's point of record for testing; this keeps the name, the flags and the
# exit codes so nothing calling it has to change.
#
# The benchmark programs and this gate moved together. The baseline is wall
# time on one machine, so record it on the machine that will enforce it;
# BENCH_BASELINE points at a different file when you need more than one.
#
# Exit status: 0 clean, 1 found something, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

echo "bench_gate.sh: delegating to ZyQuality at $(zyq_root)"
echo "  → bash bench/bench_gate.sh"
echo

cd "$(zyq_root)"
exec bash bench/bench_gate.sh "$@"
