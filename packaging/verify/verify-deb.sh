#!/usr/bin/env bash
# verify-deb.sh — Prove that a built .deb is actually installable and usable.
#
# Runs as root inside a pristine Debian container (the CI gate uses debian:12).
# Everything here answers one question: if a user runs `dpkg -i` on this file,
# do they end up with a working zymbol?
#
# Usage:
#   bash packaging/verify/verify-deb.sh --deb PATH [OPTIONS]
#
# Options:
#   --deb      PATH   The .deb to verify (required)
#   --repo     DIR    Source checkout, for the E2E suite (default: this repo)
#   --version  X.Y.Z  Expected version (default: read from <repo>/Cargo.toml)
#   --arch     ARCH   Expected Debian architecture (default: amd64)
#   --scope    SCOPE  smoke | full   (default: full)
#                       smoke = metadata, install, linkage, CLI commands
#                       full  = smoke + tests/scripts/vm_compare.sh against the
#                               INSTALLED binary
#   --force-host      Allow running outside a container (see the guard below)
#   -h, --help
#
# Exit status: 0 when every check passes, 1 otherwise. Checks do not abort on
# first failure — one run reports every problem the package has.
#
# Local equivalent of the CI gate (from interpreter/):
#   docker run --rm -v "$PWD:/workspace" -w /workspace debian:12 \
#     bash packaging/verify/verify-deb.sh --deb packaging/dist/zymbol_lang_v0.0.8_x86_64.deb

set -uo pipefail

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------
DEB=""
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXPECTED_VERSION=""
EXPECTED_ARCH="amd64"
SCOPE="full"
FORCE_HOST=false

usage() { sed -n '2,31p' "${BASH_SOURCE[0]}"; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --deb)        DEB="$2";              shift 2 ;;
        --repo)       REPO_ROOT="$2";        shift 2 ;;
        --version)    EXPECTED_VERSION="$2"; shift 2 ;;
        --arch)       EXPECTED_ARCH="$2";    shift 2 ;;
        --scope)      SCOPE="$2";            shift 2 ;;
        --force-host) FORCE_HOST=true;       shift ;;
        -h|--help)    usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

# This script installs and then REMOVES zymbol-lang system-wide. That is fine in
# a throwaway container and destructive on a workstation, so refuse the latter
# unless the caller says otherwise.
if [[ "${FORCE_HOST}" != true ]]; then
    if [[ ! -e /.dockerenv && ! -e /run/.containerenv && "${CI:-}" != "true" ]]; then
        echo "verify-deb.sh installs and removes zymbol-lang system-wide." >&2
        echo "No container detected — refusing to touch this machine." >&2
        echo "  Run it in one:" >&2
        echo "    docker run --rm -v \"\$PWD:/workspace\" -w /workspace debian:12 \\" >&2
        echo "      bash packaging/verify/verify-deb.sh --deb <file.deb>" >&2
        echo "  Or pass --force-host if you really mean this machine." >&2
        exit 2
    fi
fi

[[ -n "${DEB}" ]]  || { echo "--deb is required" >&2; exit 2; }
[[ -f "${DEB}" ]]  || { echo "no such file: ${DEB}" >&2; exit 2; }
DEB="$(cd "$(dirname "${DEB}")" && pwd)/$(basename "${DEB}")"

case "${SCOPE}" in
    smoke|full) ;;
    *) echo "--scope must be smoke or full" >&2; exit 2 ;;
esac

if [[ -z "${EXPECTED_VERSION}" ]]; then
    if [[ -f "${REPO_ROOT}/Cargo.toml" ]]; then
        EXPECTED_VERSION=$(grep '^version' "${REPO_ROOT}/Cargo.toml" | head -1 | cut -d'"' -f2)
    fi
    [[ -n "${EXPECTED_VERSION}" ]] \
        || { echo "cannot determine expected version — pass --version" >&2; exit 2; }
fi

# The E2E suite needs the test corpus from the source tree.
if [[ "${SCOPE}" == "full" && ! -d "${REPO_ROOT}/tests" ]]; then
    echo "--scope full needs the source checkout — pass --repo DIR" >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BOLD='\033[1m'; NC='\033[0m'
CHECKS=0; FAILED=0
declare -a FAILURES=()

section() { echo ""; echo -e "${BOLD}── $* ──${NC}"; }
ok()      { CHECKS=$((CHECKS+1)); echo -e "  ${GREEN}ok${NC}    $*"; }
bad()     { CHECKS=$((CHECKS+1)); FAILED=$((FAILED+1)); FAILURES+=("$*")
            echo -e "  ${RED}FAIL${NC}  $*"; }
note()    { echo -e "  ${YELLOW}note${NC}  $*"; }

# check <description> <command...> — passes when the command exits 0.
check() {
    local desc="$1"; shift
    local out
    if out="$("$@" 2>&1)"; then
        ok "${desc}"
    else
        bad "${desc}"
        [[ -n "${out}" ]] && echo "${out}" | sed 's/^/          /' | head -15
    fi
}

# check_contains <description> <haystack> <needle>
check_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if grep -qF -- "${needle}" <<< "${haystack}"; then
        ok "${desc}"
    else
        bad "${desc} — expected to find: ${needle}"
    fi
}

echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  Zymbol .deb verification${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo "  package : ${DEB}"
echo "  version : ${EXPECTED_VERSION}"
echo "  arch    : ${EXPECTED_ARCH}"
echo "  scope   : ${SCOPE}"

# ---------------------------------------------------------------------------
# 1. Metadata — before touching the system
# ---------------------------------------------------------------------------
section "1. Package metadata"

if ! CONTROL="$(dpkg-deb --field "${DEB}" 2>&1)"; then
    bad "dpkg-deb can read the archive"
    echo "${CONTROL}" | sed 's/^/          /'
    echo -e "\n${RED}${BOLD}Archive is unreadable — nothing else can be checked.${NC}"
    exit 1
fi
ok "dpkg-deb can read the archive"

field() { grep -m1 "^$1:" <<< "${CONTROL}" | cut -d' ' -f2-; }

[[ "$(field Package)"      == "zymbol-lang"          ]] && ok "Package: zymbol-lang"            || bad "Package is '$(field Package)', expected zymbol-lang"
[[ "$(field Version)"      == "${EXPECTED_VERSION}"  ]] && ok "Version: ${EXPECTED_VERSION}"    || bad "Version is '$(field Version)', expected ${EXPECTED_VERSION}"
[[ "$(field Architecture)" == "${EXPECTED_ARCH}"     ]] && ok "Architecture: ${EXPECTED_ARCH}"  || bad "Architecture is '$(field Architecture)', expected ${EXPECTED_ARCH}"
[[ -n "$(field Maintainer)"  ]] && ok "Maintainer is set"  || bad "Maintainer is empty"
[[ -n "$(field Description)" ]] && ok "Description is set" || bad "Description is empty"
check_contains "Depends declares libc6" "$(field Depends)" "libc6"

# The filename is what users download; a mismatch with the metadata is a bug.
EXPECTED_BASENAME_ARCH="x86_64"
[[ "${EXPECTED_ARCH}" == "arm64" ]] && EXPECTED_BASENAME_ARCH="aarch64"
EXPECTED_BASENAME="zymbol_lang_v${EXPECTED_VERSION}_${EXPECTED_BASENAME_ARCH}.deb"
[[ "$(basename "${DEB}")" == "${EXPECTED_BASENAME}" ]] \
    && ok "Filename matches release convention: ${EXPECTED_BASENAME}" \
    || bad "Filename is '$(basename "${DEB}")', release convention is '${EXPECTED_BASENAME}'"

# ---------------------------------------------------------------------------
# 2. Payload — what lands on disk
# ---------------------------------------------------------------------------
section "2. Package contents"

CONTENTS="$(dpkg-deb --contents "${DEB}" 2>&1)" || bad "dpkg-deb --contents failed"
for path in \
    "./usr/bin/zymbol" \
    "./usr/share/applications/zymbol.desktop" \
    "./usr/share/doc/zymbol-lang/copyright" \
    "./usr/share/pixmaps/zymbol.png"
do
    check_contains "ships ${path#.}" "${CONTENTS}" " ${path}"
done

BIN_LINE="$(grep -E ' \./usr/bin/zymbol$' <<< "${CONTENTS}" || true)"
if [[ "${BIN_LINE}" == -rwxr-xr-x* ]]; then
    ok "/usr/bin/zymbol is mode 755"
else
    bad "/usr/bin/zymbol has wrong mode: ${BIN_LINE:-<absent>}"
fi

if grep -qE ' \./usr/(local|opt)/' <<< "${CONTENTS}"; then
    bad "package writes outside the FHS locations it declares (/usr/local or /opt)"
else
    ok "no files under /usr/local or /opt"
fi

# ---------------------------------------------------------------------------
# 3. Installation on a pristine system
# ---------------------------------------------------------------------------
section "3. Installation"

if ! command -v dpkg >/dev/null; then
    note "dpkg unavailable — skipping installation checks (run this in a Debian container)"
else
    INSTALL_OUT="$(dpkg -i "${DEB}" 2>&1)"
    INSTALL_RC=$?
    if [[ ${INSTALL_RC} -eq 0 ]]; then
        ok "dpkg -i succeeds with no unmet dependencies"
    else
        bad "dpkg -i failed (rc=${INSTALL_RC})"
        echo "${INSTALL_OUT}" | sed 's/^/          /' | head -20
    fi

    STATUS="$(dpkg-query -W -f='${Status}' zymbol-lang 2>/dev/null || echo absent)"
    [[ "${STATUS}" == "install ok installed" ]] \
        && ok "dpkg status: install ok installed" \
        || bad "dpkg status is '${STATUS}'"

    AUDIT="$(dpkg --audit 2>&1)"
    [[ -z "${AUDIT}" ]] \
        && ok "dpkg --audit is clean" \
        || { bad "dpkg --audit reports problems"; echo "${AUDIT}" | sed 's/^/          /' | head -10; }
fi

ZYMBOL_BIN=/usr/bin/zymbol
if [[ -x "${ZYMBOL_BIN}" ]]; then
    ok "${ZYMBOL_BIN} exists and is executable"
else
    bad "${ZYMBOL_BIN} missing after install — remaining checks cannot run"
    echo -e "\n${RED}${BOLD}FAILED: ${FAILED}/${CHECKS} checks${NC}"
    exit 1
fi

RESOLVED="$(command -v zymbol || true)"
[[ "${RESOLVED}" == "${ZYMBOL_BIN}" ]] \
    && ok "'zymbol' resolves on PATH to ${ZYMBOL_BIN}" \
    || bad "'zymbol' resolves to '${RESOLVED:-nothing}'"

VERSION_OUT="$("${ZYMBOL_BIN}" --version 2>&1)"
check_contains "--version reports ${EXPECTED_VERSION}" "${VERSION_OUT}" "${EXPECTED_VERSION}"

# ---------------------------------------------------------------------------
# 4. Dynamic linkage — every .so must be covered by Depends
# ---------------------------------------------------------------------------
section "4. Dynamic linkage"

# Release packages are built with --no-default-features precisely so the binary
# needs nothing beyond libc: std/db links libodbc, which Depends does not declare
# and a pristine Debian does not ship. Anything outside this list means the
# package was built with the wrong feature set, or Depends is now a lie.
ALLOWED_LIBS="linux-vdso|libgcc_s|libm|libc|libdl|libpthread|librt|ld-linux"

if ! command -v ldd >/dev/null; then
    note "ldd unavailable — skipping linkage check"
else
    LDD_OUT="$(ldd "${ZYMBOL_BIN}" 2>&1)"
    if grep -q "not a dynamic executable\|statically linked" <<< "${LDD_OUT}"; then
        ok "binary is statically linked (no shared library requirements)"
    else
        UNEXPECTED=""
        while read -r lib _; do
            [[ -z "${lib}" ]] && continue
            [[ "${lib}" == /* ]] && continue          # the loader line
            grep -qE "^(${ALLOWED_LIBS})\." <<< "${lib}" || UNEXPECTED+="${lib} "
        done < <(awk '{print $1}' <<< "${LDD_OUT}")

        if [[ -z "${UNEXPECTED}" ]]; then
            ok "links only against libc and friends"
        else
            bad "links against undeclared libraries: ${UNEXPECTED}"
            note "release packages must be built with --no-default-features (see release-linux.yml)"
        fi

        grep -q "not found" <<< "${LDD_OUT}" \
            && bad "unresolved shared libraries on a pristine system" \
            || ok "every shared library resolves"
    fi
fi

# Does Depends actually cover the glibc this binary needs? The checks above only
# catch it when the container's glibc happens to be too old: build on a newer
# distro than the one you verify on and they fire, build on an older one and they
# stay silent while `Depends: libc6 (>= 2.17)` is still a lie to every user on an
# older release. Comparing the binary's own symbol versions against the declared
# floor catches it everywhere, including in CI.
#
# The versioned symbol names live in .dynstr as plain strings, so `grep -a` finds
# them without binutils — debian:12 ships no objdump and this must not need to
# apt-get anything into a container whose cleanliness is the point.
NEEDED_GLIBC="$(grep -ao 'GLIBC_[0-9]\+\.[0-9]\+' "${ZYMBOL_BIN}" 2>/dev/null \
    | sed 's/GLIBC_//' | sort -V -u | tail -1)"
DECLARED_GLIBC="$(field Depends | grep -o 'libc6 *(>= *[0-9]\+\.[0-9]\+' | grep -o '[0-9]\+\.[0-9]\+' | tail -1)"

if [[ -z "${NEEDED_GLIBC}" ]]; then
    note "no versioned glibc symbols found — static binary, or an unreadable format"
elif [[ -z "${DECLARED_GLIBC}" ]]; then
    bad "binary needs glibc ${NEEDED_GLIBC} but Depends states no libc6 minimum"
elif [[ "$(printf '%s\n%s\n' "${DECLARED_GLIBC}" "${NEEDED_GLIBC}" | sort -V | tail -1)" == "${DECLARED_GLIBC}" ]]; then
    ok "Depends libc6 (>= ${DECLARED_GLIBC}) covers the required glibc ${NEEDED_GLIBC}"
else
    bad "binary needs glibc ${NEEDED_GLIBC} but Depends only asks for libc6 (>= ${DECLARED_GLIBC})"
    note "dpkg would install this on a system too old to run it: it satisfies the"
    note "declared dependency and then fails at exec with a GLIBC_* version error."
    note "Either compute Depends from the binary, or build on the oldest supported"
    note "distro — see packaging/verify/README.md."
fi

# ---------------------------------------------------------------------------
# 5. The installed binary actually works
# ---------------------------------------------------------------------------
section "5. CLI smoke tests"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT
# src/ holds only what gets packaged into the .zyp — a stray .zy there makes
# `zymbol package` warn about unreachable files. Everything else lives in aux/.
mkdir -p "${WORK}/src" "${WORK}/aux"

cat > "${WORK}/src/main.zy" <<'ZY'
>> "hola" ¶
@ i:1..3 { >> i " " }
>> ¶
suma = (a, b) -> a + b
>> suma(20, 22) ¶
pares = [1, 2, 3, 4]$| (x -> x % 2 == 0)
@ n:pares { >> n " " }
>> ¶
ZY
EXPECTED_RUN=$'hola\n1 2 3 \n42\n2 4 '

# Tracks whether the binary can run at all. Section 6 keys off this: a binary
# that fails identically under both engines scores a perfect parity run, so the
# E2E suite must not be trusted — or even started — unless this is true.
BINARY_RUNS=true

RUN_TW="$("${ZYMBOL_BIN}" run "${WORK}/src/main.zy" 2>&1)"
if [[ "${RUN_TW}" == "${EXPECTED_RUN}" ]]; then
    ok "run (tree-walker) produces the expected output"
else
    bad "run (tree-walker) output mismatch"
    BINARY_RUNS=false
    echo "${RUN_TW}" | sed 's/^/          /' | head -10
fi

RUN_VM="$("${ZYMBOL_BIN}" run --vm "${WORK}/src/main.zy" 2>&1)"
if [[ "${RUN_VM}" == "${EXPECTED_RUN}" ]]; then
    ok "run --vm produces the expected output"
else
    bad "run --vm output mismatch"
    BINARY_RUNS=false
    echo "${RUN_VM}" | sed 's/^/          /' | head -10
fi

check "check accepts a valid program" "${ZYMBOL_BIN}" check "${WORK}/src/main.zy"

# Assert on content, not just on "something came out": a dynamic-loader error on
# stderr is also non-empty output, and would otherwise pass this.
FMT_OUT="$("${ZYMBOL_BIN}" fmt "${WORK}/src/main.zy" 2>&1)"
check_contains "fmt emits formatted source" "${FMT_OUT}" 'suma = (a, b) -> a + b'

# Unicode: the language is symbol-first, so a broken locale in the package is fatal.
printf '>> "áéí 日本 देवनागरी" ¶\n' > "${WORK}/aux/unicode.zy"
UNI_OUT="$("${ZYMBOL_BIN}" run "${WORK}/aux/unicode.zy" 2>&1)"
check_contains "handles non-ASCII source and output" "${UNI_OUT}" "देवनागरी"

# Error path: a bad program must fail loudly, not silently succeed. Check the
# diagnostic, not just the exit status — a binary that cannot start also exits
# non-zero, and would otherwise "pass" this for the wrong reason.
printf '>> undefined_name ¶\n' > "${WORK}/aux/broken.zy"
BROKEN_OUT="$("${ZYMBOL_BIN}" check "${WORK}/aux/broken.zy" 2>&1)"; BROKEN_RC=$?
if [[ ${BROKEN_RC} -eq 0 ]]; then
    bad "check exits 0 on a program with an undefined name"
else
    check_contains "check rejects an undefined name with a diagnostic" \
        "${BROKEN_OUT}" "undefined_name"
fi

# REPL over a pipe (no TTY) — this is how scripts drive it.
REPL_OUT="$(printf '>> 6*7 ¶\n' | "${ZYMBOL_BIN}" repl 2>&1)"
check_contains "repl evaluates over piped stdin" "${REPL_OUT}" "42"

# .zyp packaging round-trip.
if "${ZYMBOL_BIN}" package "${WORK}/src" -o "${WORK}/out.zyp" --script main.zy >/dev/null 2>&1; then
    ok "package builds a .zyp"
    ZYP_OUT="$("${ZYMBOL_BIN}" run "${WORK}/out.zyp" 2>&1)"
    [[ "${ZYP_OUT}" == "${EXPECTED_RUN}" ]] \
        && ok "run of the .zyp matches the source output" \
        || { bad ".zyp run output mismatch"; echo "${ZYP_OUT}" | sed 's/^/          /' | head -10; }
else
    bad "package failed to build a .zyp"
fi

# ---------------------------------------------------------------------------
# 6. E2E parity suite against the INSTALLED binary
# ---------------------------------------------------------------------------
if [[ "${SCOPE}" == "full" && "${BINARY_RUNS}" != true ]]; then
    section "6. E2E parity suite — NOT RUN"
    bad "E2E suite skipped: the binary cannot execute a trivial program"
    note "vm_compare.sh compares the two engines against each other, so a binary"
    note "that fails identically in both would report a flawless 544/544. Fix the"
    note "failures in section 5 before this number means anything."

elif [[ "${SCOPE}" == "full" ]]; then
    section "6. E2E parity suite (tests/scripts/vm_compare.sh)"

    # The whole corpus runs, including the std/db tests: a package built
    # --no-default-features has no std/db, and both engines must then report the
    # identical "module not found: std/db". They did not until the VM stopped
    # appending .zy to stdlib module names — that divergence surfaced here first,
    # so this suite stays unfiltered to keep catching its like.
    SUMMARY="${WORK}/vm_compare.summary"
    if ZYMBOL_BIN="${ZYMBOL_BIN}" VM_COMPARE_SUMMARY="${SUMMARY}" \
        bash "${REPO_ROOT}/tests/scripts/vm_compare.sh" > "${WORK}/vm_compare.log" 2>&1
    then
        ok "vm_compare.sh: no tree-walker / VM mismatches"
    else
        bad "vm_compare.sh reported mismatches (full log below)"
        sed -n '/SUMMARY/,$p' "${WORK}/vm_compare.log" | head -60 | sed 's/^/          /'
    fi

    if [[ -f "${SUMMARY}" ]]; then
        # shellcheck disable=SC1090
        source "${SUMMARY}"
        echo "          total=${total} pass=${pass} fail=${fail} skip=${skip} excluded=${excluded}"
        # A suite that silently stops collecting files would otherwise "pass".
        [[ "${total:-0}" -ge 500 ]] \
            && ok "suite exercised ${total} test files" \
            || bad "suite only found ${total:-0} test files — expected 500+"
        # Every file the suite collected must be accounted for, and none may
        # mismatch. Deliberately not a hard-coded corpus size: the previous
        # `pass >= 540` was calibrated on a working tree that also held
        # gitignored files (tests/output/ — .gitignore:23), so it counted 544
        # where a clean checkout collects 536 and no CI run could ever satisfy
        # it. The invariant below says what the gate actually means and does not
        # need recalibrating every time a test is added or removed.
        [[ "${fail:-1}" -eq 0 \
           && $(( ${pass:-0} + ${skip:-0} + ${excluded:-0} )) -eq "${total:-0}" ]] \
            && ok "${pass} files byte-identical under both engines (of ${total} collected)" \
            || bad "unaccounted results: total=${total} pass=${pass} fail=${fail} skip=${skip} excluded=${excluded}"
        [[ "${skip:-0}" -eq 0 ]] \
            && ok "no tests skipped" \
            || note "${skip} test(s) skipped (timeout or @vm-skip) — not a gate failure"
    else
        bad "vm_compare.sh wrote no summary — did it run?"
    fi
fi

# ---------------------------------------------------------------------------
# 7. Reinstall and removal
# ---------------------------------------------------------------------------
section "7. Reinstall and removal"

if command -v dpkg >/dev/null; then
    check "reinstalling over itself succeeds" dpkg -i "${DEB}"
    [[ -x "${ZYMBOL_BIN}" ]] && ok "binary survives reinstall" || bad "binary missing after reinstall"

    if dpkg -r zymbol-lang >/dev/null 2>&1; then
        ok "dpkg -r removes the package"
        [[ ! -e "${ZYMBOL_BIN}" ]] \
            && ok "removal deletes /usr/bin/zymbol" \
            || bad "/usr/bin/zymbol survives removal"
        [[ ! -e /usr/share/applications/zymbol.desktop ]] \
            && ok "removal deletes the desktop entry" \
            || bad "desktop entry survives removal"
    else
        bad "dpkg -r failed"
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
if [[ ${FAILED} -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}  PASS — ${CHECKS}/${CHECKS} checks${NC}"
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
    exit 0
fi

echo -e "${RED}${BOLD}  FAIL — ${FAILED} of ${CHECKS} checks failed${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
for f in "${FAILURES[@]}"; do echo -e "  ${RED}✗${NC} ${f}"; done
exit 1
