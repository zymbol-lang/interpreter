#!/usr/bin/env bash
# run-project-tests.sh — every real-program test in the workspace.
#
# interpreter/tests proves the language works on test cases. This proves it
# works on the programs actually written in it: a go engine, two TUI games, a
# neural-network library, a code auditor, the web playground.
#
# Usage: bash tests/scripts/run-project-tests.sh [OPTIONS]
#
#   --engine PATH   Interpreter under test (default: zymbol on PATH)
#   --vm            Also run what supports it through the register VM
#   --go LEVEL      AI-vs-AI go tournaments: none | quick | full   (default: none)
#                     quick = 1 game per board — does a game complete at all
#                     full  = 100x 9x9, 50x 13x13, 20x 19x19 — how well it plays
#   --only NAMES    Comma-separated subset: go,serpiente,klingon,zofia,zyaudit,web
#   -h, --help
#
# The project runners all invoke `zymbol` from PATH, so --engine works by
# putting a symlink of that name first on PATH — no runner needs changing, and
# the same command can therefore test a packaged binary:
#
#   bash run-project-tests.sh --engine /usr/bin/zymbol
#
# Two kinds of suite, and the difference matters
# ----------------------------------------------
# Most suites here judge the *interpreter*: point them at a binary and a failure
# means that binary is wrong. The `web` suites do not. They judge
# web/src/zymbol/zymbol.js, the hand-written JavaScript mirror that runs the
# playground — code that ships in no package at all. `test_runner.mjs` does
# invoke the CLI, but only as the reference to compare the mirror against, and
# it currently carries five known mirror gaps (IMPL_V008.md § E.3).
#
# So `--engine` selects the interpreter suites only, and skips `web` with a note:
# validating a .deb should never go red over browser debt. `--only web` still
# runs them explicitly when that is what you want.
#
# Exit 0 only when every selected suite passed.

set -uo pipefail

# The projects are sibling repositories of interpreter/, not part of it: GO,
# serpiente, klingon_galaxy, Zofia and ZyAudit each have their own .git and their
# own remote. This script lives here because it belongs with the other suites and
# should be versioned, but it can only run from a full workspace checkout — CI
# for interpreter/ alone will not find the projects. Override with ZY_WORKSPACE.
ROOT="${ZY_WORKSPACE:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"

if [[ ! -d "${ROOT}/GO" && ! -d "${ROOT}/serpiente" ]]; then
    echo "No project checkouts found under ${ROOT}" >&2
    echo "  This needs the multi-repo workspace (GO/, serpiente/, ...) alongside" >&2
    echo "  interpreter/. Point ZY_WORKSPACE at it if it lives elsewhere." >&2
    exit 2
fi
ENGINE=""
RUN_VM=false
GO_LEVEL="none"
ONLY=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --engine) ENGINE="$2"; shift 2 ;;
        --vm)     RUN_VM=true; shift ;;
        --go)     GO_LEVEL="$2"; shift 2 ;;
        --only)   ONLY="$2"; shift 2 ;;
        -h|--help) sed -n '2,27p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

case "${GO_LEVEL}" in
    none|quick|full) ;;
    *) echo "--go must be none, quick or full" >&2; exit 2 ;;
esac

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'
SUITES=0; FAILED=0
declare -a FAILURES=()

section() { echo ""; echo -e "${BOLD}════ $* ════${NC}"; }
pass()    { SUITES=$((SUITES+1)); echo -e "  ${GREEN}PASS${NC}  $*"; }
fail()    { SUITES=$((SUITES+1)); FAILED=$((FAILED+1)); FAILURES+=("$*"); echo -e "  ${RED}FAIL${NC}  $*"; }
note()    { echo -e "  ${YELLOW}note${NC}  $*"; }

wanted() {
    [[ -z "${ONLY}" ]] && return 0
    [[ ",${ONLY}," == *",$1,"* ]]
}

# --- engine selection -------------------------------------------------------
SHIM=""
if [[ -n "${ENGINE}" ]]; then
    [[ -x "${ENGINE}" ]] || { echo "Not executable: ${ENGINE}" >&2; exit 2; }
    ENGINE="$(cd "$(dirname "${ENGINE}")" && pwd)/$(basename "${ENGINE}")"
    SHIM="$(mktemp -d)"
    ln -s "${ENGINE}" "${SHIM}/zymbol"
    export PATH="${SHIM}:${PATH}"
    trap 'rm -rf "${SHIM}"' EXIT
fi

ZYMBOL="$(command -v zymbol || true)"
[[ -n "${ZYMBOL}" ]] || { echo "No zymbol on PATH — pass --engine PATH" >&2; exit 2; }

echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  Zymbol — real-program test run${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo "  engine : ${ZYMBOL}  ($(${ZYMBOL} --version 2>&1))"
echo "  go     : ${GO_LEVEL}"
[[ "${RUN_VM}" == true ]] && echo "  vm     : also running VM passes where supported"

# run_script <label> <dir> <command...>
run_script() {
    local label="$1" dir="$2"; shift 2
    local out
    if out="$(cd "${ROOT}/${dir}" && timeout 1800 "$@" 2>&1)"; then
        pass "${label}"
    else
        fail "${label}"
        echo "${out}" | tail -15 | sed 's/^/          /'
    fi
}

# --- GO ---------------------------------------------------------------------
if wanted go; then
    section "GO — 囲碁 engine"
    run_script "GO test suites (試験/全試験.sh)" GO bash 試験/全試験.sh
fi

# --- serpiente --------------------------------------------------------------
if wanted serpiente; then
    section "serpiente"
    run_script "serpiente suites (pruebas/todas.sh)" serpiente bash pruebas/todas.sh
fi

# --- klingon galaxy ---------------------------------------------------------
if wanted klingon; then
    section "klingon_galaxy — Hov veS"
    run_script "klingon suites (mIw/Hoch.sh)" klingon_galaxy bash mIw/Hoch.sh
fi

# --- Zofia ------------------------------------------------------------------
# No runner of its own: each test has a sibling .expected.
if wanted zofia; then
    section "Zofia — neural network library"
    zofia_fail=0
    for t in "${ROOT}"/Zofia/tests/*.zy; do
        exp="${t%.zy}.expected"
        [[ -f "${exp}" ]] || continue
        for mode in "" "--vm"; do
            [[ "${mode}" == "--vm" && "${RUN_VM}" != true ]] && continue
            label="$(basename "${t}") ${mode:-tree-walker}"
            out="$(cd "${ROOT}/Zofia" && timeout 120 zymbol run ${mode} "${t}" 2>&1)"
            if [[ "${out}" == "$(cat "${exp}")" ]]; then
                pass "${label}"
            else
                fail "${label}"
                diff <(echo "${out}") "${exp}" 2>/dev/null | head -8 | sed 's/^/          /'
                zofia_fail=$((zofia_fail+1))
            fi
        done
    done
fi

# --- ZyAudit ----------------------------------------------------------------
# These print reports rather than PASS/FAIL, so this is a smoke run: a suite
# that errors out is a failure, a suite that produces output is not judged.
if wanted zyaudit; then
    section "ZyAudit — code auditor (smoke only)"
    note "these tests print reports, not verdicts — only crashes are caught"
    for t in "${ROOT}"/ZyAudit/测试/*.zy; do
        label="$(basename "${t}")"
        out="$(cd "${ROOT}/ZyAudit" && timeout 120 zymbol run "${t}" 2>&1)"
        if grep -qE "Runtime error|Parse error|error\[|VM compile error" <<< "${out}"; then
            fail "${label}"
            grep -E "Runtime error|Parse error|error\[|VM compile error" <<< "${out}" | head -3 | sed 's/^/          /'
        else
            pass "${label}"
        fi
    done
fi

# --- web --------------------------------------------------------------------
# These judge the JS mirror, not the interpreter — see the header. When a
# specific binary is under test they are off-topic, so they only run if asked
# for by name.
if wanted web && { [[ -z "${ENGINE}" ]] || [[ "${ONLY}" == *web* ]]; }; then
    section "web — playground engine (JS mirror, not the packaged binary)"
    if command -v node >/dev/null; then
        run_script "zyp reader + module resolver" web node tests/test_zyp.mjs

        # The JS mirror has known, documented gaps (IMPL_V008.md § E.3), so a
        # bare pass/fail would be red forever and tell nobody anything. Compare
        # against the recorded set instead: unchanged is a pass, anything new or
        # unexpectedly fixed is worth a look. Same idea as
        # tests/scripts/fmt_property.sh --baseline.
        KNOWN_WEB_FAILURES="bugs/bug_mm11_iterator_leftover.zy
bugs/bug_mm4_module_const_guard.zy
bugs/bug_mm9_const_call_depth.zy
errors/parser/parent_path_alias.zy
modules_scope/interp_global_const.zy"

        web_out="$(cd "${ROOT}/web" && timeout 1800 node tests/test_runner.mjs 2>&1)"
        actual="$(grep -oP '^\s+✗\s+\K\S+' <<< "${web_out}" | sort)"
        expected="$(sort <<< "${KNOWN_WEB_FAILURES}")"

        if [[ "${actual}" == "${expected}" ]]; then
            pass "CLI vs JS parity — 5 known gaps, unchanged (IMPL_V008.md § E.3)"
        else
            newly_broken="$(comm -13 <(echo "${expected}") <(echo "${actual}"))"
            newly_fixed="$(comm -23 <(echo "${expected}") <(echo "${actual}"))"
            if [[ -n "${newly_broken}" ]]; then
                fail "CLI vs JS parity — new divergences beyond the known set"
                echo "${newly_broken}" | sed 's/^/          new: /'
            else
                pass "CLI vs JS parity — no new divergences"
            fi
            [[ -n "${newly_fixed}" ]] && \
                note "no longer failing (update § E.3): $(tr '\n' ' ' <<< "${newly_fixed}")"
        fi

        [[ -f "${ROOT}/web/tests/test_catalog.mjs" ]] \
            && run_script "example catalog" web node tests/test_catalog.mjs --check
    else
        note "node not installed — skipping web suites"
    fi
elif wanted web && [[ -n "${ENGINE}" ]]; then
    section "web — skipped"
    note "these judge web/src/zymbol/zymbol.js, which is in no package;"
    note "--engine is about a binary, so they say nothing here. Use --only web."
fi

# --- GO tournaments ---------------------------------------------------------
# The real workout: full games, thousands of engine moves each, scoring and
# territory counted at the end.
#
# Measured per game on this workspace — the engine is the reason to run these
# through the VM, and the reason `full` is a coffee break rather than a weekend:
#
#            tree-walker      VM     ratio
#   9x9           23s          1s      23x
#   13x13        193s          7s      28x
#   19x19        825s         38s      22x
#
# So `--go full --vm` is ~20 minutes and `--go full` on the tree-walker is ~8
# hours. The engine flag follows --vm.
#
# That ratio is worth noticing on its own: ARCHITECTURE.md quotes the VM at ~4x,
# measured on synthetic benchmarks. On a real program — deep recursion, a board
# copied per candidate move — it is five times that.
if wanted go && [[ "${GO_LEVEL}" != "none" ]]; then
    section "GO — AI vs AI tournaments (${GO_LEVEL})"

    if [[ "${GO_LEVEL}" == "quick" ]]; then
        BOARDS=("9:1" "13:1" "19:1")
        # One 19x19 game costs 825s in the tree-walker and 38s in the VM, and it
        # is the same game either way. At `quick` the tree-walker proves itself
        # on the small boards — 3.5 minutes for both — and the VM covers 19x19.
        # `full` is where every board runs under whatever engines are selected.
        TW_MAX_BOARD=13
    else
        BOARDS=("9:100" "13:50" "19:20")
        TW_MAX_BOARD=19
    fi

    # Default engine is the tree-walker, the one `zymbol run` gives you. --vm
    # adds a VM pass: the point is that a game completes under both engines, not
    # which finishes sooner.
    ENGINES=("")
    [[ "${RUN_VM}" == true ]] && ENGINES+=("--vm")

    for engine_flag in "${ENGINES[@]}"; do
        for spec in "${BOARDS[@]}"; do
            board="${spec%%:*}"; games="${spec##*:}"

            if [[ -z "${engine_flag}" && "${board}" -gt "${TW_MAX_BOARD}" ]]; then
                note "skipping ${board}x${board} on the tree-walker at --go quick (${games} game would take ~$(( board == 19 ? 14 : 3 )) min; the VM pass covers this board)"
                continue
            fi

            label="${games} game(s) on ${board}x${board} (${engine_flag:-tree-walker})"
            echo -e "  ${CYAN}····${NC}  ${label} — running"
            start=$(date +%s)
            out="$(cd "${ROOT}/GO" && timeout 36000 zymbol run ${engine_flag} 棋戦.zy "${games}" "${board}" 静 en 2>&1)"
            rc=$?
            elapsed=$(( $(date +%s) - start ))

            if [[ ${rc} -ne 0 ]]; then
                fail "${label} — exit ${rc} after ${elapsed}s"
                echo "${out}" | tail -10 | sed 's/^/          /'
            elif grep -qE "Runtime error|Parse error|error\[" <<< "${out}"; then
                fail "${label} — engine error after ${elapsed}s"
                grep -E "Runtime error|Parse error|error\[" <<< "${out}" | head -3 | sed 's/^/          /'
            elif ! grep -q "records written" <<< "${out}"; then
                # No record means no game was played to the end.
                fail "${label} — finished in ${elapsed}s without recording a game"
                echo "${out}" | tail -8 | sed 's/^/          /'
            else
                pass "${label} — ${elapsed}s"
                grep -E "records written" <<< "${out}" | sed 's/^/          /'
            fi
        done
    done
fi

# --- summary ----------------------------------------------------------------
echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
if [[ ${FAILED} -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}  PASS — ${SUITES}/${SUITES} suites${NC}"
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
    exit 0
fi
echo -e "${RED}${BOLD}  FAIL — ${FAILED} of ${SUITES} suites failed${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
for f in "${FAILURES[@]}"; do echo -e "  ${RED}✗${NC} ${f}"; done
exit 1
