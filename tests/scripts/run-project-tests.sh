#!/usr/bin/env bash
# run-project-tests.sh — a wrapper. The suite lives in ZyQuality (../zyquality), the
# project's point of record for testing; this keeps the name, the flags and the
# exit codes so nothing calling it has to change.
#
# It runs the real programs written in Zymbol -- a go engine, two TUI games, a
# neural-network library, a code auditor, the playground -- which is a
# different question from the corpus and belongs with the rest of QA.
#
# Exit status: 0 clean, 1 found something, 2 could not run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./zyq.sh
source "$SCRIPT_DIR/zyq.sh"
zyq_require

echo "run-project-tests.sh: delegating to ZyQuality at $(zyq_root)"
echo "  → bash project/run-project-tests.sh"
echo

cd "$(zyq_root)"
exec bash project/run-project-tests.sh "$@"
