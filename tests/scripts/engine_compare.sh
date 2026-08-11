#!/usr/bin/env bash
# Differential test across every Zymbol engine that exists.
#
#   bash tests/scripts/engine_compare.sh FILE.zy            # one program
#   bash tests/scripts/engine_compare.sh DIR                # every .zy under DIR
#   bash tests/scripts/engine_compare.sh DIR --matrix       # print the full matrix, not just diffs
#   bash tests/scripts/engine_compare.sh DIR --engines tw,vm # restrict the set
#
# Why this exists alongside vm_compare.sh
# ---------------------------------------
# `vm_compare.sh` compares the tree-walker against the register VM, `web/tests/
# test_runner.mjs` compares the CLI against the browser engine, and `zyml/tests/
# parity.sh` compares the CLI against zyml. Each pair is covered; the four
# together never are. A construct on which all four engines disagree — such as a
# `@:label!` naming a label that was never declared — passes every one of those
# suites, because no suite ever puts all four answers side by side.
#
# Engines
# -------
#   tw    interpreter/target/release/zymbol run           (canonical)
#   vm    interpreter/target/release/zymbol run --vm
#   js    web/tests/run_one.mjs                           (browser engine)
#   zyml  zyml/zyml run                                   (OCaml closure compiler)
#
# A missing engine is reported once and skipped, so this runs in a checkout that
# has only the interpreter.
#
# Output normalisation
# --------------------
# Engines word their diagnostics differently and always will; what has to agree
# is *whether* a program is rejected and *what it printed before* being
# rejected. So each engine's result is reduced to:
#
#   stdout, verbatim   +   a verdict: OK | ERROR
#
# Compare with --strict to require the error text to match too (it will not,
# today, and that is a separate piece of work).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTERPRETER_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
WORKSPACE_DIR="$(cd "$INTERPRETER_DIR/.." && pwd)"

RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'
BLUE=$'\033[34m'; BOLD=$'\033[1m'; RESET=$'\033[0m'

TARGET=""
SHOW_MATRIX=0
STRICT=0
ENGINE_FILTER=""
TIMEOUT_SEC=15

while [[ $# -gt 0 ]]; do
    case "$1" in
        --matrix)  SHOW_MATRIX=1; shift ;;
        --strict)  STRICT=1; shift ;;
        --engines) ENGINE_FILTER="$2"; shift 2 ;;
        --timeout) TIMEOUT_SEC="$2"; shift 2 ;;
        -h|--help) sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)         TARGET="$1"; shift ;;
    esac
done

if [[ -z "$TARGET" ]]; then
    echo "usage: engine_compare.sh FILE.zy|DIR [--matrix] [--strict] [--engines tw,vm,js,zyml]" >&2
    exit 2
fi

# ─── Locate engines ───────────────────────────────────────────────────────────

ZYMBOL="$INTERPRETER_DIR/target/release/zymbol"
[[ -x "$ZYMBOL" ]] || ZYMBOL="$INTERPRETER_DIR/target/debug/zymbol"
[[ -x "$ZYMBOL" ]] || ZYMBOL="$(command -v zymbol || true)"

JS_DRIVER="$WORKSPACE_DIR/web/tests/run_one.mjs"
ZYML_BIN="$WORKSPACE_DIR/zyml/zyml"

declare -a ENGINES=()
declare -a MISSING=()

want() { [[ -z "$ENGINE_FILTER" ]] || [[ ",$ENGINE_FILTER," == *",$1,"* ]]; }

if want tw   && [[ -n "$ZYMBOL" && -x "$ZYMBOL" ]]; then ENGINES+=(tw);   elif want tw;   then MISSING+=("tw (no zymbol binary — run: cargo build --release)"); fi
if want vm   && [[ -n "$ZYMBOL" && -x "$ZYMBOL" ]]; then ENGINES+=(vm);   elif want vm;   then MISSING+=("vm (no zymbol binary)"); fi
if want js   && [[ -f "$JS_DRIVER" ]] && command -v node >/dev/null 2>&1; then ENGINES+=(js); elif want js; then MISSING+=("js (need node and web/tests/run_one.mjs)"); fi
if want zyml && [[ -x "$ZYML_BIN" ]]; then ENGINES+=(zyml); elif want zyml; then MISSING+=("zyml (no zyml/zyml binary — build it with: make -C ../zyml)"); fi

if [[ ${#ENGINES[@]} -lt 2 ]]; then
    echo "${RED}Need at least two engines to compare; found: ${ENGINES[*]:-none}${RESET}" >&2
    for m in "${MISSING[@]}"; do echo "  missing: $m" >&2; done
    exit 2
fi

# ─── Run one file through one engine ──────────────────────────────────────────
# Sets: RUN_STDOUT, RUN_STDERR, RUN_VERDICT (OK | ERROR | TIMEOUT)

run_engine() {
    local engine="$1" file="$2"
    local input_file="${file%.zy}.input"
    local out_f err_f rc
    out_f=$(mktemp); err_f=$(mktemp)

    case "$engine" in
        tw)   if [[ -f "$input_file" ]]; then timeout "$TIMEOUT_SEC" "$ZYMBOL" run "$file" <"$input_file" >"$out_f" 2>"$err_f"
              else timeout "$TIMEOUT_SEC" "$ZYMBOL" run "$file" </dev/null >"$out_f" 2>"$err_f"; fi ;;
        vm)   if [[ -f "$input_file" ]]; then timeout "$TIMEOUT_SEC" "$ZYMBOL" run --vm "$file" <"$input_file" >"$out_f" 2>"$err_f"
              else timeout "$TIMEOUT_SEC" "$ZYMBOL" run --vm "$file" </dev/null >"$out_f" 2>"$err_f"; fi ;;
        js)   if [[ -f "$input_file" ]]; then timeout "$TIMEOUT_SEC" node "$JS_DRIVER" "$file" --input "$input_file" >"$out_f" 2>"$err_f"
              else timeout "$TIMEOUT_SEC" node "$JS_DRIVER" "$file" >"$out_f" 2>"$err_f"; fi ;;
        zyml) if [[ -f "$input_file" ]]; then timeout "$TIMEOUT_SEC" "$ZYML_BIN" run "$file" <"$input_file" >"$out_f" 2>"$err_f"
              else timeout "$TIMEOUT_SEC" "$ZYML_BIN" run "$file" </dev/null >"$out_f" 2>"$err_f"; fi ;;
    esac
    rc=$?

    RUN_STDOUT=$(cat "$out_f")
    RUN_STDERR=$(cat "$err_f")
    rm -f "$out_f" "$err_f"

    if [[ $rc -eq 124 ]]; then
        RUN_VERDICT=TIMEOUT
    elif [[ $rc -ne 0 ]]; then
        RUN_VERDICT=ERROR
    # A zero exit with a diagnostic on stderr still means the engine rejected
    # something; the tree-walker prints warnings there too, so only lines that
    # look like errors count.
    elif grep -qE '^(error|Runtime error|Parse error|Lex error|VM compile error|Compile error|error\[)' <<<"$RUN_STDERR" 2>/dev/null; then
        RUN_VERDICT=ERROR
    else
        RUN_VERDICT=OK
    fi
}

# ─── Collect files ────────────────────────────────────────────────────────────

declare -a FILES=()
if [[ -d "$TARGET" ]]; then
    while IFS= read -r f; do FILES+=("$f"); done < <(find "$TARGET" -name '*.zy' | sort)
elif [[ -f "$TARGET" ]]; then
    FILES=("$TARGET")
else
    echo "${RED}not found: $TARGET${RESET}" >&2; exit 2
fi

# ─── Compare ──────────────────────────────────────────────────────────────────

echo "${BOLD}Engines:${RESET} ${ENGINES[*]}"
for m in "${MISSING[@]}"; do echo "  ${YELLOW}skipped${RESET} $m"; done
echo

AGREE=0; DISAGREE=0
declare -a DIVERGENT=()

for file in "${FILES[@]}"; do
    rel="${file#"$WORKSPACE_DIR"/}"
    declare -a v_out=() v_verdict=() v_err=()

    for e in "${ENGINES[@]}"; do
        run_engine "$e" "$file"
        v_out+=("$RUN_STDOUT")
        v_verdict+=("$RUN_VERDICT")
        v_err+=("$RUN_STDERR")
    done

    same=1
    for i in "${!ENGINES[@]}"; do
        [[ "${v_out[$i]}" == "${v_out[0]}" ]] || same=0
        [[ "${v_verdict[$i]}" == "${v_verdict[0]}" ]] || same=0
        if [[ $STRICT -eq 1 ]]; then
            [[ "${v_err[$i]}" == "${v_err[0]}" ]] || same=0
        fi
    done

    if [[ $same -eq 1 ]]; then
        AGREE=$((AGREE + 1))
        [[ $SHOW_MATRIX -eq 1 ]] && printf "  ${GREEN}AGREE${RESET}  %s  [%s]\n" "$rel" "${v_verdict[0]}"
    else
        DISAGREE=$((DISAGREE + 1))
        DIVERGENT+=("$rel")
        printf "  ${RED}DIFFER${RESET} %s\n" "$rel"
        for i in "${!ENGINES[@]}"; do
            printf "      ${BLUE}%-5s${RESET} %-8s stdout=%s\n" \
                "${ENGINES[$i]}" "${v_verdict[$i]}" "$(printf '%s' "${v_out[$i]}" | tr '\n' '\\' | sed 's/\\/\\n/g')"
            if [[ -n "${v_err[$i]}" ]]; then
                printf "            %s\n" "$(printf '%s' "${v_err[$i]}" | head -1)"
            fi
        done
    fi
done

echo
echo "${BOLD}────────────────────────────────────────${RESET}"
printf "  ${GREEN}AGREE${RESET}   : ${BOLD}%d${RESET}\n" "$AGREE"
printf "  ${RED}DIFFER${RESET}  : ${BOLD}%d${RESET}\n" "$DISAGREE"

if [[ $DISAGREE -eq 0 ]]; then
    echo "${GREEN}${BOLD}All engines agree.${RESET}"
    exit 0
fi
exit 1
