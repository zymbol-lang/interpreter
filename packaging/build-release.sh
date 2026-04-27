#!/usr/bin/env bash
# build-release.sh — Linux-only release build
# Produces canonically-named Linux packages for the web install page.
#
# Windows (exe/msi), macOS (aarch64/x86_64), and aarch64 static musl are
# built automatically by GitHub Actions on release publish:
#   .github/workflows/release-windows.yml
#   .github/workflows/release-macos.yml
#   .github/workflows/release-aarch64-static.yml
#
# Usage: bash packaging/build-release.sh [--skip-static]
# Run from interpreter/ (workspace root).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/packaging/build-packages.sh"
OUT_DIR="${REPO_ROOT}/packaging/dist"
SKIP_STATIC=false

RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info()    { echo -e "${CYAN}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-static) SKIP_STATIC=true; shift ;;
        *) error "Unknown option: $1" ;;
    esac
done

VERSION="$(grep '^version' "${REPO_ROOT}/Cargo.toml" | head -1 | cut -d'"' -f2)"
[[ -n "${VERSION}" ]] || error "Could not parse version from Cargo.toml"

echo ""
echo "┌──────────────────────────────────────────────────────────────┐"
echo "│       Zymbol Linux Release Build — v${VERSION}                    │"
echo "└──────────────────────────────────────────────────────────────┘"
echo ""
info "Output directory: ${OUT_DIR}"
info "Version: ${VERSION}"
info "Windows / macOS / aarch64-musl → built by GitHub Actions on release"
echo ""

# ---------------------------------------------------------------------------
# Linux x86_64 — deb, rpm, pkg.tar.zst (reuse existing release binary)
# ---------------------------------------------------------------------------
BINARY="${REPO_ROOT}/target/release/zymbol"
[[ -f "${BINARY}" ]] || error "Release binary not found. Run: cargo build --release"

info "=== Linux x86_64 packages (deb, rpm, pkg.tar.zst) ==="
bash "${SCRIPT}" --no-timestamp --no-build --formats deb,rpm,arch --arch x86_64

# ---------------------------------------------------------------------------
# Linux x86_64 — static musl binary
# ---------------------------------------------------------------------------
if [[ "${SKIP_STATIC}" == false ]]; then
    info "=== Linux x86_64 static musl binary ==="
    bash "${SCRIPT}" --no-timestamp --formats static --arch x86_64
else
    warn "Skipping static x86_64 musl build (--skip-static)"
fi

# ---------------------------------------------------------------------------
# Linux aarch64 — deb, rpm, pkg.tar.zst (cross-compile with aarch64-linux-gnu-gcc)
# aarch64 static musl is handled by GitHub Actions (release-aarch64-static.yml)
# ---------------------------------------------------------------------------
info "=== Linux aarch64 packages (deb, rpm, pkg.tar.zst — cross-compile) ==="
bash "${SCRIPT}" --no-timestamp --formats deb,rpm,arch --arch aarch64

# ---------------------------------------------------------------------------
# Generate SHA256SUMS for all v{VERSION} Linux packages in dist/
# ---------------------------------------------------------------------------
info "=== Computing SHA256SUMS for v${VERSION} Linux packages ==="
cd "${OUT_DIR}"

mapfile -t PKG_FILES < <(find . -maxdepth 1 \( \
    -name "zymbol_lang_v${VERSION}_*.deb"          \
    -o -name "zymbol_lang_v${VERSION}_*.rpm"       \
    -o -name "zymbol_lang_v${VERSION}_*.pkg.tar.zst" \
    -o -name "zymbol_lang_v${VERSION}_*_linux"     \
\) -printf "%f\n" | sort)

if [[ ${#PKG_FILES[@]} -eq 0 ]]; then
    warn "No v${VERSION} Linux packages found in ${OUT_DIR}"
else
    sha256sum "${PKG_FILES[@]}" | tee "SHA256SUMS_v${VERSION}_linux"
    success "Hashes → ${OUT_DIR}/SHA256SUMS_v${VERSION}_linux"
fi

cd "${REPO_ROOT}"

# ---------------------------------------------------------------------------
# Print hash table for install.html
# ---------------------------------------------------------------------------
echo ""
echo "────────────────────────────────────────────────────────────────"
echo "  SHA256 hashes for install.html (v${VERSION} — Linux only):"
echo "────────────────────────────────────────────────────────────────"
declare -A HASH_MAP
while IFS='  ' read -r hash file; do
    HASH_MAP["${file}"]="${hash}"
done < "${OUT_DIR}/SHA256SUMS_v${VERSION}_linux"

print_hash() {
    local file="$1" label="$2"
    local hash="${HASH_MAP[${file}]:-MISSING}"
    local short="${hash:0:12}"
    printf "  %-55s  %s\n" "${label}" "${short}"
    printf "  %-55s  %s\n" "" "${hash}"
    echo ""
}

print_hash "zymbol_lang_v${VERSION}_x86_64.deb"           "Linux x86_64 .deb"
print_hash "zymbol_lang_v${VERSION}_x86_64.rpm"           "Linux x86_64 .rpm"
print_hash "zymbol_lang_v${VERSION}_x86_64.pkg.tar.zst"  "Linux x86_64 .pkg.tar.zst"
print_hash "zymbol_lang_v${VERSION}_x86_64_linux"         "Linux x86_64 static musl"
print_hash "zymbol_lang_v${VERSION}_aarch64.deb"          "Linux aarch64 .deb"
print_hash "zymbol_lang_v${VERSION}_aarch64.rpm"          "Linux aarch64 .rpm"
print_hash "zymbol_lang_v${VERSION}_aarch64.pkg.tar.zst" "Linux aarch64 .pkg.tar.zst"

echo "────────────────────────────────────────────────────────────────"
echo "  Windows + macOS hashes → available in GitHub release assets"
echo "  after triggering: release-windows.yml / release-macos.yml"
echo "────────────────────────────────────────────────────────────────"
echo ""
success "Linux release build complete! Packages in: ${OUT_DIR}"
ls -lh "${OUT_DIR}/zymbol_lang_v${VERSION}_"* 2>/dev/null || true
echo ""
