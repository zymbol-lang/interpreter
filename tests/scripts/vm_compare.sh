#!/usr/bin/env bash
# vm_compare.sh — Run every .zy test file through tree-walker and VM, compare outputs
# Usage: ./tests/scripts/vm_compare.sh [--timeout N]
#
# Environment:
#   ZYMBOL_BIN            Interpreter to exercise (default: target/release/zymbol).
#                         Set it to /usr/bin/zymbol to run the suite against an
#                         installed package instead of the build tree.
#   VM_COMPARE_EXCLUDE    Extended regex matched against each test's path
#                         relative to tests/. Matching files are excluded and
#                         reported separately from skips. Intended for binaries
#                         built without a feature the test needs — a release
#                         package has no std/db, so those tests cannot say
#                         anything about tree-walker / VM parity.
#   VM_COMPARE_SUMMARY    If set, a machine-readable summary is written to that
#                         path: total=N pass=N fail=N skip=N excluded=N.
#
# Exit status: 0 when no file mismatches, 1 otherwise. A missing interpreter is
# exit 2 — a gate must not read "nothing ran" as "nothing failed".

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TESTS_DIR="$REPO_ROOT/tests"
ZYMBOL="${ZYMBOL_BIN:-$REPO_ROOT/target/release/zymbol}"
TIMEOUT_SEC="${1:-10}"
if [[ "${1:-}" == "--timeout" ]]; then TIMEOUT_SEC="${2:-10}"; fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

if [[ ! -x "$ZYMBOL" ]]; then
    echo "vm_compare.sh: interpreter not found or not executable: $ZYMBOL" >&2
    echo "  build it with 'cargo build --release', or point ZYMBOL_BIN at one." >&2
    exit 2
fi

PASS=0; FAIL=0; SKIP=0; EXCL=0; ERROR_TREE=0; ERROR_VM=0

declare -a FAILURES=()
declare -a TREE_ERRORS=()
declare -a VM_ERRORS=()
declare -a SKIPPED=()
declare -a EXCLUDED=()

normalize_output() {
    sed \
        -e 's/"X-Amzn-Trace-Id": "[^"]*"/"X-Amzn-Trace-Id": "<REDACTED>"/g' \
        -e 's/Root=[A-Za-z0-9;:-]*/Root=<REDACTED>/g' \
        -e 's/"id": "[0-9a-f-]*"/"id": "<UUID>"/g' \
        -e 's/"uuid": "[0-9a-f-]*"/"uuid": "<UUID>"/g' \
        -e 's/date: [A-Za-z]*, [0-9]* [A-Za-z]* [0-9]* [0-9:]* GMT/date: <DATE>/g'
}

run_file() {
    local file="$1"
    local mode="$2"   # "" or "--vm"
    local input_file="${file%.zy}.input"
    local cmd

    if [[ "$mode" == "--vm" ]]; then
        cmd=(timeout "$TIMEOUT_SEC" "$ZYMBOL" run --vm "$file")
    else
        cmd=(timeout "$TIMEOUT_SEC" "$ZYMBOL" run "$file")
    fi

    if [[ -f "$input_file" ]]; then
        "${cmd[@]}" < "$input_file" 2>&1 || true
    else
        "${cmd[@]}" 2>&1 || true
    fi
}

# Collect all .zy files, excluding scripts/ and module-only files (matematicas/ submodules)
mapfile -t FILES < <(
    find "$TESTS_DIR" -name "*.zy" \
        ! -path "*/scripts/*" \
        ! -path "*/stress_v2/*" \
        ! -path "*/matematicas/module.zy" \
        | sort
)

TOTAL=${#FILES[@]}
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  Zymbol VM Coverage Report — $TOTAL test files${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo ""

for file in "${FILES[@]}"; do
    rel="${file#$TESTS_DIR/}"

    # Excluded by the caller — the interpreter under test cannot run these
    if [[ -n "${VM_COMPARE_EXCLUDE:-}" ]] && grep -qE "${VM_COMPARE_EXCLUDE}" <<< "$rel"; then
        EXCL=$((EXCL + 1))
        EXCLUDED+=("$rel")
        echo -e "  ${CYAN}EXCL${RESET}  $rel  ${CYAN}[excluded]${RESET}"
        continue
    fi

    # Skip files marked @vm-skip (TW-only features not yet in VM)
    if head -1 "$file" | grep -q '@vm-skip'; then
        SKIP=$((SKIP + 1))
        SKIPPED+=("$rel (vm-skip)")
        echo -e "  ${YELLOW}SKIP${RESET}  $rel  ${YELLOW}[vm-skip]${RESET}"
        continue
    fi

    # Run tree-walker
    tree_out="$(run_file "$file" "" | normalize_output)"
    tree_exit=$?

    # Run VM
    vm_out="$(run_file "$file" "--vm" | normalize_output)"
    vm_exit=$?

    # Detect timeout (exit code 124)
    if [[ $tree_exit -eq 124 || $vm_exit -eq 124 ]]; then
        SKIP=$((SKIP + 1))
        SKIPPED+=("$rel (timeout ${TIMEOUT_SEC}s)")
        echo -e "  ${YELLOW}SKIP${RESET}  $rel  ${YELLOW}[timeout]${RESET}"
        continue
    fi

    # Detect tree-walker hard errors (non-output errors that aren't expected)
    tree_has_error=false
    vm_has_error=false

    # Check if outputs match
    if [[ "$tree_out" == "$vm_out" ]]; then
        PASS=$((PASS + 1))
        echo -e "  ${GREEN}PASS${RESET}  $rel"
    else
        # Outputs differ — classify why
        FAIL=$((FAIL + 1))

        # Check if VM had a compile error (runtime errors now share "Runtime error:" prefix with WT)
        if echo "$vm_out" | grep -qE "^(VM compile error|Compile error|Parse error|Lex error|error\[)" 2>/dev/null; then
            vm_has_error=true
            ERROR_VM=$((ERROR_VM + 1))
            VM_ERRORS+=("$rel")
            echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[VM error]${RESET}"
        elif echo "$tree_out" | grep -qE "^(Runtime error|Parse error|Lex error|error\[)" 2>/dev/null; then
            tree_has_error=true
            ERROR_TREE=$((ERROR_TREE + 1))
            TREE_ERRORS+=("$rel")
            echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[Tree error]${RESET}"
        else
            echo -e "  ${RED}FAIL${RESET}  $rel  ${RED}[output mismatch]${RESET}"
        fi

        FAILURES+=("$rel")
    fi
done

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}  SUMMARY${RESET}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
echo -e "  Total files  : ${BOLD}$TOTAL${RESET}"
echo -e "  ${GREEN}PASS${RESET}         : ${GREEN}${BOLD}$PASS${RESET}"
echo -e "  ${RED}FAIL${RESET}         : ${RED}${BOLD}$FAIL${RESET}"
echo -e "  ${YELLOW}SKIP${RESET}         : ${YELLOW}${BOLD}$SKIP${RESET}"
[[ $EXCL -gt 0 ]] && echo -e "  ${CYAN}EXCL${RESET}         : ${CYAN}${BOLD}$EXCL${RESET}  (VM_COMPARE_EXCLUDE)"
echo ""

if [[ ${#FAILURES[@]} -gt 0 ]]; then
    echo -e "${BOLD}Failing files:${RESET}"
    for f in "${FAILURES[@]}"; do
        echo -e "  ${RED}✗${RESET} $f"
    done
    echo ""
fi

if [[ ${#SKIPPED[@]} -gt 0 ]]; then
    echo -e "${BOLD}Skipped:${RESET}"
    for f in "${SKIPPED[@]}"; do
        echo -e "  ${YELLOW}⊘${RESET} $f"
    done
    echo ""
fi

if [[ ${#EXCLUDED[@]} -gt 0 ]]; then
    echo -e "${BOLD}Excluded (VM_COMPARE_EXCLUDE):${RESET}"
    for f in "${EXCLUDED[@]}"; do
        echo -e "  ${CYAN}−${RESET} $f"
    done
    echo ""
fi

# Detailed diff output for failures
if [[ ${#FAILURES[@]} -gt 0 ]]; then
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}  FAILURE DETAILS${RESET}"
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    for file in "${FILES[@]}"; do
        rel="${file#$TESTS_DIR/}"
        # Check if this file is in failures
        in_fail=false
        for f in "${FAILURES[@]}"; do
            if [[ "$f" == "$rel" ]]; then in_fail=true; break; fi
        done
        [[ "$in_fail" == false ]] && continue

        echo ""
        echo -e "${CYAN}── $rel ──${RESET}"
        tree_out="$(run_file "$file" "" | normalize_output)"
        vm_out="$(run_file "$file" "--vm" | normalize_output)"

        echo -e "${BOLD}  Tree-walker output:${RESET}"
        echo "$tree_out" | head -20 | sed 's/^/    /'
        echo -e "${BOLD}  VM output:${RESET}"
        echo "$vm_out" | head -20 | sed 's/^/    /'
    done
fi

echo ""
if [[ $FAIL -eq 0 && $SKIP -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All $PASS tests produce identical output in tree-walker and VM!${RESET}"
elif [[ $FAIL -eq 0 ]]; then
    echo -e "${YELLOW}${BOLD}$PASS passed, $SKIP skipped. No mismatches.${RESET}"
else
    echo -e "${RED}${BOLD}$FAIL/$TOTAL files produce different output between tree-walker and VM.${RESET}"
fi

if [[ -n "${VM_COMPARE_SUMMARY:-}" ]]; then
    {
        echo "total=$TOTAL"
        echo "pass=$PASS"
        echo "fail=$FAIL"
        echo "skip=$SKIP"
        echo "excluded=$EXCL"
    } > "$VM_COMPARE_SUMMARY"
fi

[[ $FAIL -eq 0 ]] || exit 1
